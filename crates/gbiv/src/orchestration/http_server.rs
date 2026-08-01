//! HTTP Server: the `gbiv start` daemon's request-handling logic.
//!
//! See `docs/llds/http-server.md` and `docs/specs/http-server.md`. This module
//! holds the pure, dependency-injected request logic (color validation, the
//! prompt-response guard, query parsing, response shaping, error-status
//! mapping, and the three endpoint handlers) so it is unit-testable without a
//! real tiny_http server or a real tmux session — the same pattern
//! `pane_locator` and `tmux_driver` use for their own testability. The actual
//! `tiny_http` wiring, port file lifecycle, and worker threads live in
//! `orchestration::daemon`.

use gbiv_core::palette::Palette;
use gbiv_core::tmux::TmuxError;

use super::pane_locator::{LocatorError, Resolution};
use super::tmux_driver::{Capture, CaptureRange, DEFAULT_CAP_BYTES};

/// `GET /sessions` default/max `lines` (HTTP-SRV-021).
pub const SESSIONS_DEFAULT_LINES: usize = 35;
pub const SESSIONS_MAX_LINES: usize = 1000;

/// `GET /session/:color` default/max `lines` (HTTP-SRV-028).
pub const SESSION_DEFAULT_LINES: usize = 200;
pub const SESSION_MAX_LINES: usize = 5000;

/// Worker thread count for the daemon's accept loop (HTTP-SRV-014).
pub const WORKER_THREADS: usize = 16;

// ---- Color normalization & validation (HTTP-SRV-017, 018, 019) -----------

/// Lowercase and strip a trailing `/` from a raw `:color` path segment.
// @spec HTTP-SRV-017
pub fn normalize_color(raw: &str) -> String {
    raw.trim_end_matches('/').to_lowercase()
}

/// Outcome of validating a raw `:color` path segment against the active palette.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ColorValidation {
    /// Normalized, palette-valid color name.
    Valid(String),
    /// Not a member of the active palette after normalization.
    Invalid,
}

/// Normalize then validate a `:color` path segment against the active palette
/// (base ROYGBIV plus any configured extras) loaded at daemon startup.
// @spec HTTP-SRV-018, HTTP-SRV-019
pub fn validate_color(raw: &str, palette: &Palette) -> ColorValidation {
    let normalized = normalize_color(raw);
    if palette.contains(&normalized) {
        ColorValidation::Valid(normalized)
    } else {
        ColorValidation::Invalid
    }
}

// ---- Prompt-response guard (HTTP-SRV-040 through HTTP-SRV-046) -----------

/// A rejected send, carrying the machine-readable rule name that matched.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GuardRejection {
    pub reason: &'static str,
}

/// Whether every byte of `s` is ASCII digits (used for the numeric-choice rule).
fn is_ascii_digits(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit())
}

/// Test trimmed `text` against the prompt-response guard's rule set. Returns
/// `None` when `text` passes through untouched (including multi-word
/// natural-language text). Callers must trim `text` and reject empty/
/// all-whitespace input as `400` *before* calling this (HTTP-SRV-039) — this
/// function assumes a non-empty trimmed string.
// @spec HTTP-SRV-041, HTTP-SRV-042, HTTP-SRV-043, HTTP-SRV-044, HTTP-SRV-046
pub fn guard_check(trimmed: &str) -> Option<GuardRejection> {
    let lower = trimmed.to_ascii_lowercase();
    if trimmed.len() == 1 && (lower == "y" || lower == "n") {
        return Some(GuardRejection {
            reason: "single-letter yes/no",
        });
    }
    if lower == "yes" || lower == "no" {
        return Some(GuardRejection {
            reason: "yes/no word",
        });
    }
    if trimmed.chars().count() <= 3 && is_ascii_digits(trimmed) {
        return Some(GuardRejection {
            reason: "numeric choice",
        });
    }
    let mut chars = trimmed.chars();
    if let (Some(c), None) = (chars.next(), chars.next()) {
        if !c.is_alphanumeric() {
            return Some(GuardRejection {
                reason: "bare punctuation",
            });
        }
    }
    None
}

// ---- Query parsing (HTTP-SRV-021, 022, 028 through 032) -------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinesParseError {
    NotNumeric,
}

/// Parse a `lines` query value, defaulting when absent and clamping to `max`.
// @spec HTTP-SRV-021, HTTP-SRV-022, HTTP-SRV-028
pub fn parse_lines(raw: Option<&str>, default: usize, max: usize) -> Result<usize, LinesParseError> {
    match raw {
        None => Ok(default.min(max)),
        Some(s) => s
            .parse::<usize>()
            .map(|n| n.min(max))
            .map_err(|_| LinesParseError::NotNumeric),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RangeParseError {
    /// `lines` supplied together with `start_line`/`end_line` (HTTP-SRV-031).
    MixedParams,
    /// Only one of `start_line`/`end_line` supplied (HTTP-SRV-030).
    IncompleteRangePair,
    /// A numeric param failed to parse (HTTP-SRV-032).
    NotNumeric,
}

/// The capture request resolved from `GET /session/:color`'s query params,
/// prior to being handed to the tmux Driver as a `CaptureRange`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequestedRange {
    Tail(usize),
    Window { start: i32, end: i32 },
}

/// Resolve `GET /session/:color`'s mutually-exclusive `lines` vs.
/// `start_line`+`end_line` query groups (HTTP-SRV-028 through HTTP-SRV-032).
/// `start_line == "top"` maps to `i32::MIN` (top of history).
pub fn parse_range(
    lines: Option<&str>,
    start_line: Option<&str>,
    end_line: Option<&str>,
) -> Result<RequestedRange, RangeParseError> {
    let has_range_param = start_line.is_some() || end_line.is_some();
    if lines.is_some() && has_range_param {
        return Err(RangeParseError::MixedParams);
    }
    if has_range_param {
        let (Some(s), Some(e)) = (start_line, end_line) else {
            return Err(RangeParseError::IncompleteRangePair);
        };
        let start = if s == "top" {
            i32::MIN
        } else {
            s.parse::<i32>().map_err(|_| RangeParseError::NotNumeric)?
        };
        let end = e.parse::<i32>().map_err(|_| RangeParseError::NotNumeric)?;
        return Ok(RequestedRange::Window { start, end });
    }
    let n = parse_lines(lines, SESSION_DEFAULT_LINES, SESSION_MAX_LINES)
        .map_err(|_| RangeParseError::NotNumeric)?;
    Ok(RequestedRange::Tail(n))
}

pub fn to_capture_range(r: RequestedRange) -> CaptureRange {
    match r {
        RequestedRange::Tail(lines) => CaptureRange::Tail { lines },
        RequestedRange::Window { start, end } => CaptureRange::Window { start, end },
    }
}

// ---- Error-status mapping (HTTP-SRV-053 through HTTP-SRV-056) ------------

/// The HTTP status a typed error maps to, independent of any particular
/// endpoint's extra business-logic statuses (404-for-invalid-color, etc.).
// @spec HTTP-SRV-053, HTTP-SRV-054, HTTP-SRV-055, HTTP-SRV-056
pub fn map_tmux_error(err: &TmuxError) -> u16 {
    match err {
        TmuxError::SessionNotFound(_) => 503,
        TmuxError::PaneNotFound(_) => 404,
        TmuxError::SendKeysIncomplete(_) => 502,
        TmuxError::NotInstalled | TmuxError::Other(_) => 500,
    }
}

/// Unwraps to the inner `TmuxError` mapping (HTTP-SRV-053's `LocatorError` case).
pub fn map_locator_error(err: &LocatorError) -> u16 {
    match err {
        LocatorError::TmuxSession(inner) => map_tmux_error(inner),
    }
}

// ---- Response bodies (serde) ----------------------------------------------

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct SessionEntry {
    pub color: String,
    pub tmux_window: Option<String>,
    pub claude_pane: Option<String>,
    pub pane_status: &'static str,
    pub output: Option<String>,
    pub captured_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub other_claude_panes: Option<Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct RangeReturned {
    pub start_line: i32,
    pub end_line: i32,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct SessionDetail {
    pub color: String,
    pub claude_pane: Option<String>,
    pub pane_status: &'static str,
    pub captured_at: Option<String>,
    pub output: Option<String>,
    pub output_truncated: bool,
    pub output_original_bytes: usize,
    pub output_returned_bytes: usize,
    pub range_returned: Option<RangeReturned>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub other_claude_panes: Option<Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct SendSuccess {
    pub ok: bool,
    pub sent_to_pane: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub other_claude_panes: Option<Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct SendGuardError {
    pub ok: bool,
    pub error: &'static str,
    pub reason: &'static str,
    pub color: String,
    pub explanation: String,
    pub docs: &'static str,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct SendConflictError {
    pub ok: bool,
    pub error: &'static str,
    pub color: String,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct ErrorBody {
    pub error: String,
}

fn error_response(status: u16, message: impl Into<String>) -> HttpResponse {
    HttpResponse::json(status, &ErrorBody { error: message.into() })
}

/// The standard `404` for a `:color` that fails active-palette validation
/// (HTTP-SRV-019). Public so `daemon::handle_request` can reject an invalid
/// color for `POST /session/:color/send` *before* reading the request body
/// (HTTP-SRV-037) while still producing a byte-identical body to the one
/// `handle_session_get`/`handle_session_send` return internally.
pub fn invalid_color_response(color_raw: &str) -> HttpResponse {
    error_response(404, format!("unknown color: {color_raw}"))
}

#[derive(Debug, serde::Deserialize)]
struct SendRequest {
    text: Option<String>,
}

/// The explanation text sent back on a guard rejection (HTTP-SRV-045). Verbose
/// on purpose: the caller is typically an LLM, and a terse rejection invites
/// creative retries (see http-server LLD § "Prompt-Response Guard").
pub fn guard_explanation(color: &str, reason: &str) -> String {
    format!(
        "gbiv refused this send because the trimmed text matches the shape of a \
         response to a Claude Code permission prompt or AskUserQuestion choice \
         (rule: {reason}). gbiv never answers prompts on the user's behalf in v1 \
         — a misread of pane state could approve actions the user has not seen. \
         Do NOT retry by paraphrasing, padding with filler words, or otherwise \
         reshaping the same intent to slip past the guard; the guard is \
         shape-based but the rule's purpose is intent-based, and bypassing it \
         violates the user's trust. Correct action: tell the user that {color} \
         appears to be waiting on a prompt and ask them to answer it themselves \
         in {color}'s tmux window. If the user genuinely wants to send \
         substantive natural-language guidance (not a prompt answer), send that \
         instead."
    )
}

// ---- Request/response plumbing shared by handlers -------------------------

/// A handler's outcome, independent of any particular HTTP library.
#[derive(Debug, Clone, PartialEq)]
pub struct HttpResponse {
    pub status: u16,
    pub body: String,
}

impl HttpResponse {
    fn json(status: u16, body: &impl serde::Serialize) -> HttpResponse {
        HttpResponse {
            status,
            body: serde_json::to_string(body).expect("response bodies are always serializable"),
        }
    }
}

/// Current UTC time as RFC 3339, for `captured_at` fields. Kept as a single
/// injectable seam so tests can assert on a fixed clock without touching the
/// real system clock (`Utc::now`/`SystemTime::now` are otherwise untestable
/// without freezing time globally).
pub trait Clock {
    fn now_rfc3339(&self) -> String;
}

// ---- GET /sessions (HTTP-SRV-020 through HTTP-SRV-027) --------------------

/// `locate_panes`-shaped injection: batch-resolve every color against one
/// shared tmux/process snapshot (see pane_locator::locate_panes, PANE-LOC-024).
pub type LocatePanesFn<'a> =
    dyn Fn(&str, &[&str]) -> Result<Vec<(String, Result<Resolution, LocatorError>)>, LocatorError> + 'a;

/// `capture_pane`-shaped injection (see tmux_driver::capture_pane).
pub type CapturePaneFn<'a> = dyn Fn(&str, CaptureRange, usize) -> Result<Capture, TmuxError> + 'a;

/// `locate_pane`-shaped injection: single-color resolution (see
/// pane_locator::locate_pane).
pub type LocatePaneFn<'a> = dyn Fn(&str, &str) -> Result<Resolution, LocatorError> + 'a;

/// `send_keys`-shaped injection (see tmux_driver::send_keys).
pub type SendKeysFn<'a> = dyn Fn(&str, &str) -> Result<(), TmuxError> + 'a;

// @spec HTTP-SRV-020, HTTP-SRV-021, HTTP-SRV-022, HTTP-SRV-023, HTTP-SRV-024,
// HTTP-SRV-025, HTTP-SRV-026, HTTP-SRV-027
pub fn handle_sessions(
    palette: &Palette,
    session: &str,
    lines_param: Option<&str>,
    locate_panes: &LocatePanesFn,
    capture_pane: &CapturePaneFn,
    clock: &dyn Clock,
) -> HttpResponse {
    let lines = match parse_lines(lines_param, SESSIONS_DEFAULT_LINES, SESSIONS_MAX_LINES) {
        Ok(n) => n,
        Err(LinesParseError::NotNumeric) => return error_response(400, "lines must be a non-negative integer"),
    };

    let colors: Vec<&str> = palette.names().iter().map(String::as_str).collect();
    let resolutions = match locate_panes(session, &colors) {
        Ok(r) => r,
        Err(e) => return error_response(map_locator_error(&e), e.to_string()),
    };

    let mut entries = Vec::with_capacity(resolutions.len());
    for (color, resolution) in resolutions {
        let entry = match resolution {
            Ok(Resolution::Ok { pane_id, other_pane_ids }) => {
                match capture_pane(&pane_id, CaptureRange::Tail { lines }, DEFAULT_CAP_BYTES) {
                    Ok(capture) => SessionEntry {
                        color: color.clone(),
                        tmux_window: Some(color),
                        claude_pane: Some(pane_id),
                        pane_status: "ok",
                        output: Some(capture.text),
                        captured_at: Some(clock.now_rfc3339()),
                        other_claude_panes: (!other_pane_ids.is_empty()).then_some(other_pane_ids),
                    },
                    // A capture failure (e.g. pane vanished between resolution and
                    // capture) degrades this one color rather than failing the survey.
                    Err(_) => SessionEntry {
                        color: color.clone(),
                        tmux_window: Some(color),
                        claude_pane: Some(pane_id),
                        pane_status: "no_claude_pane",
                        output: None,
                        captured_at: None,
                        other_claude_panes: None,
                    },
                }
            }
            Ok(Resolution::NoWindow) => SessionEntry {
                color: color.clone(),
                tmux_window: None,
                claude_pane: None,
                pane_status: "no_window",
                output: None,
                captured_at: None,
                other_claude_panes: None,
            },
            Ok(Resolution::NoClaudePane) => SessionEntry {
                color: color.clone(),
                tmux_window: Some(color),
                claude_pane: None,
                pane_status: "no_claude_pane",
                output: None,
                captured_at: None,
                other_claude_panes: None,
            },
            // A per-color resolution failure (e.g. that window's panes vanished
            // between the shared window list and this color's lookup) degrades to
            // the closest existing status rather than failing the whole survey.
            Err(_) => SessionEntry {
                color: color.clone(),
                tmux_window: None,
                claude_pane: None,
                pane_status: "no_window",
                output: None,
                captured_at: None,
                other_claude_panes: None,
            },
        };
        entries.push(entry);
    }

    HttpResponse::json(200, &entries)
}

// ---- GET /session/:color (HTTP-SRV-028 through HTTP-SRV-036) --------------

// @spec HTTP-SRV-028, HTTP-SRV-029, HTTP-SRV-030, HTTP-SRV-031, HTTP-SRV-032,
// HTTP-SRV-033, HTTP-SRV-034, HTTP-SRV-035, HTTP-SRV-036
#[allow(clippy::too_many_arguments)]
pub fn handle_session_get(
    palette: &Palette,
    session: &str,
    color_raw: &str,
    lines: Option<&str>,
    start_line: Option<&str>,
    end_line: Option<&str>,
    locate_pane: &LocatePaneFn,
    capture_pane: &CapturePaneFn,
    clock: &dyn Clock,
) -> HttpResponse {
    let color = match validate_color(color_raw, palette) {
        ColorValidation::Valid(c) => c,
        ColorValidation::Invalid => return invalid_color_response(color_raw),
    };

    let range = match parse_range(lines, start_line, end_line) {
        Ok(r) => r,
        Err(RangeParseError::MixedParams) => {
            return error_response(400, "lines is mutually exclusive with start_line/end_line")
        }
        Err(RangeParseError::IncompleteRangePair) => {
            return error_response(400, "start_line and end_line must both be supplied")
        }
        Err(RangeParseError::NotNumeric) => return error_response(400, "lines/start_line/end_line must be integers"),
    };

    let resolution = match locate_pane(session, &color) {
        Ok(r) => r,
        Err(e) => return error_response(map_locator_error(&e), e.to_string()),
    };

    match resolution {
        Resolution::NoWindow => error_response(404, format!("no tmux window for color: {color}")),
        Resolution::NoClaudePane => HttpResponse::json(
            200,
            &SessionDetail {
                color,
                claude_pane: None,
                pane_status: "no_claude_pane",
                captured_at: None,
                output: None,
                output_truncated: false,
                output_original_bytes: 0,
                output_returned_bytes: 0,
                range_returned: None,
                other_claude_panes: None,
            },
        ),
        Resolution::Ok { pane_id, other_pane_ids } => {
            match capture_pane(&pane_id, to_capture_range(range), DEFAULT_CAP_BYTES) {
                Ok(capture) => HttpResponse::json(
                    200,
                    &SessionDetail {
                        color,
                        claude_pane: Some(pane_id),
                        pane_status: "ok",
                        captured_at: Some(clock.now_rfc3339()),
                        output: Some(capture.text),
                        output_truncated: capture.truncated,
                        output_original_bytes: capture.original_bytes,
                        output_returned_bytes: capture.returned_bytes,
                        range_returned: Some(RangeReturned {
                            start_line: capture.range_returned.0,
                            end_line: capture.range_returned.1,
                        }),
                        other_claude_panes: (!other_pane_ids.is_empty()).then_some(other_pane_ids),
                    },
                ),
                Err(e) => error_response(map_tmux_error(&e), e.to_string()),
            }
        }
    }
}

// ---- POST /session/:color/send (HTTP-SRV-037 through HTTP-SRV-052) -------

// @spec HTTP-SRV-037, HTTP-SRV-038, HTTP-SRV-039, HTTP-SRV-040, HTTP-SRV-041,
// HTTP-SRV-042, HTTP-SRV-043, HTTP-SRV-044, HTTP-SRV-045, HTTP-SRV-046,
// HTTP-SRV-047, HTTP-SRV-048, HTTP-SRV-049, HTTP-SRV-050, HTTP-SRV-051,
// HTTP-SRV-052
pub fn handle_session_send(
    palette: &Palette,
    session: &str,
    color_raw: &str,
    raw_body: &str,
    locate_pane: &LocatePaneFn,
    send_keys: &SendKeysFn,
) -> HttpResponse {
    // HTTP-SRV-037: route (:color) is validated before the body is even parsed.
    let color = match validate_color(color_raw, palette) {
        ColorValidation::Valid(c) => c,
        ColorValidation::Invalid => return invalid_color_response(color_raw),
    };

    let text = match serde_json::from_str::<SendRequest>(raw_body) {
        Ok(SendRequest { text: Some(t) }) => t,
        Ok(SendRequest { text: None }) => return error_response(400, "text field is required"),
        Err(_) => return error_response(400, "request body must be valid JSON"),
    };
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return error_response(400, "text must not be empty");
    }

    if let Some(rejection) = guard_check(trimmed) {
        return HttpResponse::json(
            409,
            &SendGuardError {
                ok: false,
                error: "looks_like_prompt_response",
                reason: rejection.reason,
                color: color.clone(),
                explanation: guard_explanation(&color, rejection.reason),
                docs: "docs/high-level-design.md#reject-prompt-shaped-input-on-send",
            },
        );
    }

    let resolution = match locate_pane(session, &color) {
        Ok(r) => r,
        Err(e) => return error_response(map_locator_error(&e), e.to_string()),
    };

    match resolution {
        Resolution::NoWindow => error_response(404, format!("no tmux window for color: {color}")),
        Resolution::NoClaudePane => HttpResponse::json(
            409,
            &SendConflictError {
                ok: false,
                error: "no_claude_pane",
                color,
            },
        ),
        Resolution::Ok { pane_id, other_pane_ids } => match send_keys(&pane_id, trimmed) {
            Ok(()) => HttpResponse::json(
                200,
                &SendSuccess {
                    ok: true,
                    sent_to_pane: pane_id,
                    other_claude_panes: (!other_pane_ids.is_empty()).then_some(other_pane_ids),
                },
            ),
            Err(e) => error_response(map_tmux_error(&e), e.to_string()),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn palette() -> Palette {
        Palette::default()
    }

    // ---- normalize_color / validate_color (HTTP-SRV-017, 018, 019) -------

    // @spec HTTP-SRV-017
    #[test]
    fn normalize_color_lowercases_and_strips_trailing_slash() {
        assert_eq!(normalize_color("RED/"), "red");
        assert_eq!(normalize_color("Red"), "red");
        assert_eq!(normalize_color("red"), "red");
    }

    // @spec HTTP-SRV-018
    #[test]
    fn validate_color_accepts_base_color() {
        assert_eq!(
            validate_color("RED/", &palette()),
            ColorValidation::Valid("red".to_string())
        );
    }

    // @spec HTTP-SRV-018
    #[test]
    fn validate_color_accepts_configured_extra() {
        let p = Palette::from_extras(vec!["amber".to_string()]);
        assert_eq!(
            validate_color("Amber", &p),
            ColorValidation::Valid("amber".to_string())
        );
    }

    // @spec HTTP-SRV-019
    #[test]
    fn validate_color_rejects_unknown_name() {
        assert_eq!(validate_color("purple", &palette()), ColorValidation::Invalid);
    }

    // ---- guard_check (HTTP-SRV-041 through HTTP-SRV-046) ------------------

    // @spec HTTP-SRV-041
    #[test]
    fn guard_rejects_single_letter_yes_no_case_insensitive() {
        for s in ["y", "n", "Y", "N"] {
            assert_eq!(
                guard_check(s),
                Some(GuardRejection {
                    reason: "single-letter yes/no"
                }),
                "expected rejection for {s:?}"
            );
        }
    }

    // @spec HTTP-SRV-042
    #[test]
    fn guard_rejects_yes_no_word_case_insensitive() {
        for s in ["yes", "no", "YES", "No"] {
            assert_eq!(
                guard_check(s),
                Some(GuardRejection { reason: "yes/no word" }),
                "expected rejection for {s:?}"
            );
        }
    }

    // @spec HTTP-SRV-043
    #[test]
    fn guard_rejects_numeric_choice_up_to_three_digits() {
        for s in ["1", "12", "123"] {
            assert_eq!(
                guard_check(s),
                Some(GuardRejection { reason: "numeric choice" }),
                "expected rejection for {s:?}"
            );
        }
        // Four digits is not a "choice" shape (LLD: `^\d{1,3}$`).
        assert_eq!(guard_check("1234"), None);
    }

    // @spec HTTP-SRV-044
    #[test]
    fn guard_rejects_bare_punctuation() {
        for s in ["?", "!", "."] {
            assert_eq!(
                guard_check(s),
                Some(GuardRejection {
                    reason: "bare punctuation"
                }),
                "expected rejection for {s:?}"
            );
        }
    }

    // @spec HTTP-SRV-046
    #[test]
    fn guard_passes_multiword_natural_language() {
        assert_eq!(guard_check("yes please run that"), None);
        assert_eq!(guard_check("please run the tests"), None);
    }

    // @spec HTTP-SRV-043
    #[test]
    fn guard_does_not_reject_multi_char_non_digit_non_yesno() {
        // A two-letter word is not a single-letter y/n, not "yes"/"no", not
        // numeric, and not a single punctuation char.
        assert_eq!(guard_check("ok"), None);
    }

    // ---- parse_lines (HTTP-SRV-021, 022, 028) -----------------------------

    // @spec HTTP-SRV-021
    #[test]
    fn parse_lines_defaults_when_absent() {
        assert_eq!(parse_lines(None, 35, 1000), Ok(35));
    }

    // @spec HTTP-SRV-021
    #[test]
    fn parse_lines_clamps_to_max() {
        assert_eq!(parse_lines(Some("5000"), 35, 1000), Ok(1000));
    }

    // @spec HTTP-SRV-022
    #[test]
    fn parse_lines_rejects_non_numeric() {
        assert_eq!(parse_lines(Some("abc"), 35, 1000), Err(LinesParseError::NotNumeric));
    }

    // ---- parse_range (HTTP-SRV-028 through HTTP-SRV-032) ------------------

    // @spec HTTP-SRV-028
    #[test]
    fn parse_range_defaults_to_tail_when_nothing_supplied() {
        assert_eq!(
            parse_range(None, None, None),
            Ok(RequestedRange::Tail(SESSION_DEFAULT_LINES))
        );
    }

    // @spec HTTP-SRV-028
    #[test]
    fn parse_range_tail_mode_from_lines() {
        assert_eq!(parse_range(Some("50"), None, None), Ok(RequestedRange::Tail(50)));
    }

    // @spec HTTP-SRV-029
    #[test]
    fn parse_range_window_mode() {
        assert_eq!(
            parse_range(None, Some("-700"), Some("-313")),
            Ok(RequestedRange::Window {
                start: -700,
                end: -313
            })
        );
    }

    // @spec HTTP-SRV-029
    #[test]
    fn parse_range_top_maps_to_i32_min() {
        assert_eq!(
            parse_range(None, Some("top"), Some("-701")),
            Ok(RequestedRange::Window {
                start: i32::MIN,
                end: -701
            })
        );
    }

    // @spec HTTP-SRV-030
    #[test]
    fn parse_range_incomplete_pair_is_error() {
        assert_eq!(
            parse_range(None, Some("-700"), None),
            Err(RangeParseError::IncompleteRangePair)
        );
        assert_eq!(
            parse_range(None, None, Some("-1")),
            Err(RangeParseError::IncompleteRangePair)
        );
    }

    // @spec HTTP-SRV-031
    #[test]
    fn parse_range_mixed_params_is_error() {
        assert_eq!(
            parse_range(Some("50"), Some("-700"), Some("-313")),
            Err(RangeParseError::MixedParams)
        );
    }

    // @spec HTTP-SRV-032
    #[test]
    fn parse_range_non_numeric_is_error() {
        assert_eq!(
            parse_range(None, Some("abc"), Some("-1")),
            Err(RangeParseError::NotNumeric)
        );
        assert_eq!(parse_range(Some("abc"), None, None), Err(RangeParseError::NotNumeric));
    }

    // ---- to_capture_range --------------------------------------------------

    #[test]
    fn to_capture_range_maps_variants() {
        assert_eq!(
            to_capture_range(RequestedRange::Tail(10)),
            CaptureRange::Tail { lines: 10 }
        );
        assert_eq!(
            to_capture_range(RequestedRange::Window { start: -5, end: 0 }),
            CaptureRange::Window { start: -5, end: 0 }
        );
    }

    // ---- error-status mapping (HTTP-SRV-053 through HTTP-SRV-056) --------

    // @spec HTTP-SRV-053
    #[test]
    fn map_tmux_error_session_not_found_is_503() {
        assert_eq!(map_tmux_error(&TmuxError::SessionNotFound("s".into())), 503);
    }

    // @spec HTTP-SRV-054
    #[test]
    fn map_tmux_error_pane_not_found_is_404() {
        assert_eq!(map_tmux_error(&TmuxError::PaneNotFound("p".into())), 404);
    }

    // @spec HTTP-SRV-055
    #[test]
    fn map_tmux_error_send_keys_incomplete_is_502() {
        assert_eq!(map_tmux_error(&TmuxError::SendKeysIncomplete("p".into())), 502);
    }

    // @spec HTTP-SRV-056
    #[test]
    fn map_tmux_error_other_and_not_installed_are_500() {
        assert_eq!(map_tmux_error(&TmuxError::Other("boom".into())), 500);
        assert_eq!(map_tmux_error(&TmuxError::NotInstalled), 500);
    }

    // @spec HTTP-SRV-053
    #[test]
    fn map_locator_error_unwraps_to_inner_tmux_mapping() {
        let err = LocatorError::TmuxSession(TmuxError::SessionNotFound("s".into()));
        assert_eq!(map_locator_error(&err), 503);
    }

    // ---- HttpResponse::json --------------------------------------------

    #[test]
    fn http_response_json_serializes_body() {
        let body = SendSuccess {
            ok: true,
            sent_to_pane: "%12".to_string(),
            other_claude_panes: None,
        };
        let resp = HttpResponse::json(200, &body);
        assert_eq!(resp.status, 200);
        assert_eq!(resp.body, r#"{"ok":true,"sent_to_pane":"%12"}"#);
    }

    // @spec HTTP-SRV-049
    #[test]
    fn send_success_includes_other_claude_panes_when_present() {
        let body = SendSuccess {
            ok: true,
            sent_to_pane: "%1".to_string(),
            other_claude_panes: Some(vec!["%2".to_string()]),
        };
        let json = serde_json::to_string(&body).unwrap();
        assert!(json.contains(r#""other_claude_panes":["%2"]"#), "got: {json}");
    }

    // @spec HTTP-SRV-025
    #[test]
    fn session_entry_omits_other_claude_panes_when_absent() {
        let entry = SessionEntry {
            color: "red".into(),
            tmux_window: Some("red".into()),
            claude_pane: None,
            pane_status: "no_claude_pane",
            output: None,
            captured_at: None,
            other_claude_panes: None,
        };
        let json = serde_json::to_string(&entry).unwrap();
        assert!(!json.contains("other_claude_panes"), "got: {json}");
    }

    // @spec HTTP-SRV-045
    #[test]
    fn guard_explanation_names_color_and_reason() {
        let text = guard_explanation("red", "yes/no word");
        assert!(text.contains("red"));
        assert!(text.contains("yes/no word"));
        assert!(text.contains("Do NOT retry"));
    }

    // @spec HTTP-SRV-048
    #[test]
    fn send_conflict_error_serializes_no_claude_pane_shape() {
        let body = SendConflictError {
            ok: false,
            error: "no_claude_pane",
            color: "red".to_string(),
        };
        let json = serde_json::to_string(&body).unwrap();
        assert_eq!(json, r#"{"ok":false,"error":"no_claude_pane","color":"red"}"#);
    }

    // ---- handle_sessions / handle_session_get / handle_session_send -------
    // These exercise the full endpoint logic (HTTP-SRV-020..052) against
    // injected fake locator/driver closures. They are written against the
    // handlers' intended contract per docs/specs/http-server.md and currently
    // fail via `todo!()` — Phase 6 implements the bodies these pin down.

    struct FixedClock;
    impl Clock for FixedClock {
        fn now_rfc3339(&self) -> String {
            "2026-08-01T00:00:00Z".to_string()
        }
    }

    fn ok_resolution(pane: &str) -> Result<Resolution, LocatorError> {
        Ok(Resolution::Ok {
            pane_id: pane.to_string(),
            other_pane_ids: vec![],
        })
    }

    // @spec HTTP-SRV-023, HTTP-SRV-026
    #[test]
    fn sessions_ok_color_includes_output_and_status_ok() {
        let locate = |_session: &str, colors: &[&str]| {
            Ok(colors
                .iter()
                .map(|c| (c.to_string(), ok_resolution("%1")))
                .collect())
        };
        let capture = |_pane: &str, _range: CaptureRange, _max: usize| {
            Ok(Capture {
                text: "hello".to_string(),
                truncated: false,
                original_bytes: 5,
                returned_bytes: 5,
                range_requested: CaptureRange::Tail { lines: 35 },
                range_returned: (-35, 0),
            })
        };
        let resp = handle_sessions(&palette(), "sess", None, &locate, &capture, &FixedClock);
        assert_eq!(resp.status, 200);
        let parsed: serde_json::Value = serde_json::from_str(&resp.body).unwrap();
        let red = parsed
            .as_array()
            .unwrap()
            .iter()
            .find(|e| e["color"] == "red")
            .unwrap();
        assert_eq!(red["pane_status"], "ok");
        assert_eq!(red["output"], "hello");
    }

    // @spec HTTP-SRV-024
    #[test]
    fn sessions_no_window_color_has_null_fields() {
        let locate = |_s: &str, colors: &[&str]| {
            Ok(colors
                .iter()
                .map(|c| (c.to_string(), Ok(Resolution::NoWindow)))
                .collect())
        };
        let capture = |_p: &str, _r: CaptureRange, _m: usize| unreachable!("no pane to capture");
        let resp = handle_sessions(&palette(), "sess", None, &locate, &capture, &FixedClock);
        let parsed: serde_json::Value = serde_json::from_str(&resp.body).unwrap();
        let red = parsed
            .as_array()
            .unwrap()
            .iter()
            .find(|e| e["color"] == "red")
            .unwrap();
        assert_eq!(red["pane_status"], "no_window");
        assert!(red["tmux_window"].is_null());
        assert!(red["claude_pane"].is_null());
        assert!(red["output"].is_null());
        assert!(red["captured_at"].is_null());
    }

    // @spec HTTP-SRV-027
    #[test]
    fn sessions_session_not_found_is_503_for_whole_request() {
        let locate = |_s: &str, _c: &[&str]| {
            Err(LocatorError::TmuxSession(TmuxError::SessionNotFound(
                "sess".into(),
            )))
        };
        let capture = |_p: &str, _r: CaptureRange, _m: usize| unreachable!();
        let resp = handle_sessions(&palette(), "sess", None, &locate, &capture, &FixedClock);
        assert_eq!(resp.status, 503);
    }

    // @spec HTTP-SRV-022
    #[test]
    fn sessions_non_numeric_lines_is_400() {
        let locate = |_s: &str, _c: &[&str]| unreachable!("must not resolve panes on bad input");
        let capture = |_p: &str, _r: CaptureRange, _m: usize| unreachable!();
        let resp = handle_sessions(&palette(), "sess", Some("abc"), &locate, &capture, &FixedClock);
        assert_eq!(resp.status, 400);
    }

    // @spec HTTP-SRV-035
    #[test]
    fn session_get_invalid_color_is_404_without_locator_call() {
        let locate = |_s: &str, _c: &str| unreachable!("must not resolve panes for invalid color");
        let capture = |_p: &str, _r: CaptureRange, _m: usize| unreachable!();
        let resp = handle_session_get(
            &palette(),
            "sess",
            "purple",
            None,
            None,
            None,
            &locate,
            &capture,
            &FixedClock,
        );
        assert_eq!(resp.status, 404);
    }

    // @spec HTTP-SRV-036
    #[test]
    fn session_get_session_not_found_is_503() {
        let locate =
            |_s: &str, _c: &str| Err(LocatorError::TmuxSession(TmuxError::SessionNotFound("s".into())));
        let capture = |_p: &str, _r: CaptureRange, _m: usize| unreachable!();
        let resp = handle_session_get(
            &palette(),
            "sess",
            "red",
            None,
            None,
            None,
            &locate,
            &capture,
            &FixedClock,
        );
        assert_eq!(resp.status, 503);
    }

    // @spec HTTP-SRV-030
    #[test]
    fn session_get_incomplete_range_pair_is_400() {
        let locate = |_s: &str, _c: &str| unreachable!("must not resolve panes on bad query params");
        let capture = |_p: &str, _r: CaptureRange, _m: usize| unreachable!();
        let resp = handle_session_get(
            &palette(),
            "sess",
            "red",
            None,
            Some("-700"),
            None,
            &locate,
            &capture,
            &FixedClock,
        );
        assert_eq!(resp.status, 400);
    }

    // @spec HTTP-SRV-033, HTTP-SRV-034
    #[test]
    fn session_get_ok_returns_full_body_and_200() {
        let locate = |_s: &str, _c: &str| ok_resolution("%12");
        let capture = |pane: &str, range: CaptureRange, _max: usize| {
            assert_eq!(pane, "%12");
            assert_eq!(range, CaptureRange::Tail { lines: 200 });
            Ok(Capture {
                text: "recent output".to_string(),
                truncated: true,
                original_bytes: 5000,
                returned_bytes: 4096,
                range_requested: range,
                range_returned: (-312, 0),
            })
        };
        let resp = handle_session_get(
            &palette(),
            "sess",
            "red",
            None,
            None,
            None,
            &locate,
            &capture,
            &FixedClock,
        );
        assert_eq!(resp.status, 200);
        let body: serde_json::Value = serde_json::from_str(&resp.body).unwrap();
        assert_eq!(body["color"], "red");
        assert_eq!(body["claude_pane"], "%12");
        assert_eq!(body["pane_status"], "ok");
        assert_eq!(body["output"], "recent output");
        assert_eq!(body["output_truncated"], true);
        assert_eq!(body["output_original_bytes"], 5000);
        assert_eq!(body["output_returned_bytes"], 4096);
        assert_eq!(body["range_returned"]["start_line"], -312);
        assert_eq!(body["range_returned"]["end_line"], 0);
    }

    // @spec HTTP-SRV-034
    #[test]
    fn session_get_no_claude_pane_is_200_with_null_output() {
        let locate = |_s: &str, _c: &str| Ok(Resolution::NoClaudePane);
        let capture = |_p: &str, _r: CaptureRange, _m: usize| unreachable!("no pane to capture");
        let resp = handle_session_get(
            &palette(),
            "sess",
            "red",
            None,
            None,
            None,
            &locate,
            &capture,
            &FixedClock,
        );
        assert_eq!(resp.status, 200);
        let body: serde_json::Value = serde_json::from_str(&resp.body).unwrap();
        assert_eq!(body["pane_status"], "no_claude_pane");
        assert!(body["claude_pane"].is_null());
        assert!(body["output"].is_null());
    }

    // @spec HTTP-SRV-029
    #[test]
    fn session_get_window_mode_maps_to_capture_range_window() {
        let locate = |_s: &str, _c: &str| ok_resolution("%1");
        let capture = |_pane: &str, range: CaptureRange, _max: usize| {
            assert_eq!(range, CaptureRange::Window { start: -700, end: -313 });
            Ok(Capture {
                text: "chunk".to_string(),
                truncated: false,
                original_bytes: 5,
                returned_bytes: 5,
                range_requested: range,
                range_returned: (-700, -313),
            })
        };
        let resp = handle_session_get(
            &palette(),
            "sess",
            "red",
            None,
            Some("-700"),
            Some("-313"),
            &locate,
            &capture,
            &FixedClock,
        );
        assert_eq!(resp.status, 200);
    }

    // @spec HTTP-SRV-037
    #[test]
    fn send_invalid_color_is_404_before_body_parsed() {
        let locate = |_s: &str, _c: &str| unreachable!("must not resolve panes for invalid color");
        let send = |_p: &str, _t: &str| unreachable!("must not send for invalid color");
        // Malformed JSON body — proves color validation runs first (HTTP-SRV-037),
        // since a 400 (body) must not win over the 404 (route) here.
        let resp = handle_session_send(&palette(), "sess", "purple", "not json", &locate, &send);
        assert_eq!(resp.status, 404);
    }

    // @spec HTTP-SRV-038
    #[test]
    fn send_invalid_json_body_is_400() {
        let locate = |_s: &str, _c: &str| ok_resolution("%1");
        let send = |_p: &str, _t: &str| unreachable!("must not send for malformed body");
        let resp = handle_session_send(&palette(), "sess", "red", "not json", &locate, &send);
        assert_eq!(resp.status, 400);
    }

    // @spec HTTP-SRV-039
    #[test]
    fn send_empty_text_is_400_and_skips_guard() {
        let locate = |_s: &str, _c: &str| ok_resolution("%1");
        let send = |_p: &str, _t: &str| unreachable!("must not send for empty text");
        let resp = handle_session_send(&palette(), "sess", "red", r#"{"text": "   "}"#, &locate, &send);
        assert_eq!(resp.status, 400);
    }

    // @spec HTTP-SRV-040, HTTP-SRV-045
    #[test]
    fn send_guard_rejects_before_pane_resolution() {
        let locate = |_s: &str, _c: &str| unreachable!("guard must reject before pane resolution");
        let send = |_p: &str, _t: &str| unreachable!("guard must reject before send");
        let resp = handle_session_send(&palette(), "sess", "red", r#"{"text": "yes"}"#, &locate, &send);
        assert_eq!(resp.status, 409);
        let body: serde_json::Value = serde_json::from_str(&resp.body).unwrap();
        assert_eq!(body["error"], "looks_like_prompt_response");
        assert_eq!(body["reason"], "yes/no word");
    }

    // @spec HTTP-SRV-047
    #[test]
    fn send_no_window_after_guard_passes_is_404() {
        let locate = |_s: &str, _c: &str| Ok(Resolution::NoWindow);
        let send = |_p: &str, _t: &str| unreachable!();
        let resp = handle_session_send(
            &palette(),
            "sess",
            "red",
            r#"{"text": "please run the tests"}"#,
            &locate,
            &send,
        );
        assert_eq!(resp.status, 404);
    }

    // @spec HTTP-SRV-048
    #[test]
    fn send_no_claude_pane_is_409_conflict_shape() {
        let locate = |_s: &str, _c: &str| Ok(Resolution::NoClaudePane);
        let send = |_p: &str, _t: &str| unreachable!();
        let resp = handle_session_send(
            &palette(),
            "sess",
            "red",
            r#"{"text": "please run the tests"}"#,
            &locate,
            &send,
        );
        assert_eq!(resp.status, 409);
        let body: serde_json::Value = serde_json::from_str(&resp.body).unwrap();
        assert_eq!(body["error"], "no_claude_pane");
        assert!(body.get("explanation").is_none());
    }

    // @spec HTTP-SRV-049
    #[test]
    fn send_multiple_claude_panes_is_200_with_other_panes_listed() {
        let locate = |_s: &str, _c: &str| {
            Ok(Resolution::Ok {
                pane_id: "%1".to_string(),
                other_pane_ids: vec!["%2".to_string()],
            })
        };
        let send = |pane: &str, _t: &str| {
            assert_eq!(pane, "%1");
            Ok(())
        };
        let resp = handle_session_send(
            &palette(),
            "sess",
            "red",
            r#"{"text": "please run the tests"}"#,
            &locate,
            &send,
        );
        assert_eq!(resp.status, 200);
        let body: serde_json::Value = serde_json::from_str(&resp.body).unwrap();
        assert_eq!(body["other_claude_panes"], serde_json::json!(["%2"]));
    }

    // @spec HTTP-SRV-050
    #[test]
    fn send_success_shape() {
        let locate = |_s: &str, _c: &str| ok_resolution("%12");
        let send = |pane: &str, text: &str| {
            assert_eq!(pane, "%12");
            assert_eq!(text, "please run the tests");
            Ok(())
        };
        let resp = handle_session_send(
            &palette(),
            "sess",
            "red",
            r#"{"text": "please run the tests"}"#,
            &locate,
            &send,
        );
        assert_eq!(resp.status, 200);
        assert_eq!(resp.body, r#"{"ok":true,"sent_to_pane":"%12"}"#);
    }

    // @spec HTTP-SRV-051
    #[test]
    fn send_keys_incomplete_is_502() {
        let locate = |_s: &str, _c: &str| ok_resolution("%12");
        let send = |_p: &str, _t: &str| Err(TmuxError::SendKeysIncomplete("%12".into()));
        let resp = handle_session_send(
            &palette(),
            "sess",
            "red",
            r#"{"text": "please run the tests"}"#,
            &locate,
            &send,
        );
        assert_eq!(resp.status, 502);
    }

    // @spec HTTP-SRV-052
    #[test]
    fn send_session_not_found_is_503() {
        let locate =
            |_s: &str, _c: &str| Err(LocatorError::TmuxSession(TmuxError::SessionNotFound("s".into())));
        let send = |_p: &str, _t: &str| unreachable!();
        let resp = handle_session_send(
            &palette(),
            "sess",
            "red",
            r#"{"text": "please run the tests"}"#,
            &locate,
            &send,
        );
        assert_eq!(resp.status, 503);
    }
}

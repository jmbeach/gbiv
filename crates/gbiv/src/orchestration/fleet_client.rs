//! `gbiv fleet status|get|send`: pure request/response logic for the fleet
//! orchestration client subcommands.
//!
//! See `docs/llds/orchestrate-cli.md` and `docs/specs/orchestrate-cli.md`.
//! This module holds the pure, dependency-free logic (port-file content
//! parsing, URL building, response-to-exit-code mapping, and the local
//! color/text/guard pre-checks for `send`) so it is unit-testable without a
//! real HTTP call or tmux session — the same split `http_server` uses for the
//! daemon side. The actual `ureq` HTTP call, port-file path resolution via
//! `core::find_gbiv_root`, and stdout/stderr/logging glue live in `fleet_cli`.

use gbiv_core::palette::Palette;

use super::http_server::{guard_check, guard_explanation, normalize_color};

// ---- Exit codes (docs/llds/orchestrate-cli.md § exit code tables) ---------

pub const EXIT_OK: i32 = 0;
pub const EXIT_OTHER: i32 = 1;
pub const EXIT_DAEMON_NOT_RUNNING: i32 = 2;
pub const EXIT_INVALID_COLOR: i32 = 3;
pub const EXIT_NO_CLAUDE_PANE: i32 = 4;
pub const EXIT_SEND_INCOMPLETE: i32 = 5;
pub const EXIT_GUARD_REJECTED: i32 = 6;

/// The result of handling one HTTP response (or a local pre-check
/// rejection): what to print where, and what the process should exit with.
/// `fleet_cli` is the only thing that ever prints or calls `process::exit` —
/// every function in this module returns an `Outcome` (or a `Result`
/// carrying one) so tests can assert on the value directly instead of
/// capturing stdout/stderr.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Outcome {
    pub exit_code: i32,
    pub stdout: Option<String>,
    pub stderr: Option<String>,
}

impl Outcome {
    pub(crate) fn ok_stdout(body: impl Into<String>) -> Outcome {
        Outcome {
            exit_code: EXIT_OK,
            stdout: Some(body.into()),
            stderr: None,
        }
    }

    pub(crate) fn err(exit_code: i32, message: impl Into<String>) -> Outcome {
        Outcome {
            exit_code,
            stdout: None,
            stderr: Some(message.into()),
        }
    }
}

// ---- Port file content (FLEET-CLI-011, FLEET-CLI-012) ---------------------

/// Parse a port file's already-read content. Path resolution (walking to the
/// gbiv root, locating the repo, checking existence) is `fleet_cli`'s job
/// (FLEET-CLI-010, FLEET-CLI-011's "does not exist" case) since it touches
/// the filesystem; this function is the pure "is this content a valid port"
/// check (FLEET-CLI-012).
// @spec FLEET-CLI-012
pub fn parse_port_file_content(content: &str) -> Result<u16, Outcome> {
    content
        .trim()
        .parse::<u16>()
        .map_err(|_| Outcome::err(EXIT_DAEMON_NOT_RUNNING, "port file is corrupt"))
}

// ---- URL building (FLEET-CLI-002 through FLEET-CLI-004, FLEET-CLI-042) ----

/// Build the `GET /sessions` URL, forwarding `--lines` verbatim if present
/// (FLEET-CLI-002).
// @spec FLEET-CLI-002
pub fn sessions_url(port: u16, lines: Option<&str>) -> String {
    match lines {
        Some(n) => format!("http://127.0.0.1:{port}/sessions?lines={n}"),
        None => format!("http://127.0.0.1:{port}/sessions"),
    }
}

/// Build the `GET /session/:color` URL. `lines` is tail mode; `start_line`/
/// `end_line` is window mode (FLEET-CLI-003, FLEET-CLI-004). Callers are
/// responsible for not supplying both (clap's job, FLEET-CLI-005/006) — this
/// function just forwards whatever it's given.
// @spec FLEET-CLI-003, FLEET-CLI-004
pub fn session_url(
    port: u16,
    color: &str,
    lines: Option<&str>,
    start_line: Option<&str>,
    end_line: Option<&str>,
) -> String {
    let base = format!("http://127.0.0.1:{port}/session/{color}");
    match (lines, start_line, end_line) {
        (Some(n), _, _) => format!("{base}?lines={n}"),
        (None, Some(start), Some(end)) => format!("{base}?start_line={start}&end_line={end}"),
        (None, _, _) => base,
    }
}

/// Build the `POST /session/:color/send` URL (FLEET-CLI-042). `color` is
/// expected to already be normalized (FLEET-CLI-038).
// @spec FLEET-CLI-042
pub fn send_url(port: u16, normalized_color: &str) -> String {
    format!("http://127.0.0.1:{port}/session/{normalized_color}/send")
}

// ---- Response handling: gbiv fleet status (FLEET-CLI-020, FLEET-CLI-021) --

/// Map a `GET /sessions` HTTP response to an `Outcome`.
// @spec FLEET-CLI-020, FLEET-CLI-021
pub fn handle_status_response(status: u16, body: &str) -> Outcome {
    match status {
        200 => Outcome::ok_stdout(body),
        _ => Outcome::err(EXIT_OTHER, body),
    }
}

// ---- Response handling: gbiv fleet get (FLEET-CLI-030 through -034) -------

/// Map a `GET /session/:color` HTTP response to an `Outcome`.
// @spec FLEET-CLI-030, FLEET-CLI-031, FLEET-CLI-032, FLEET-CLI-033, FLEET-CLI-034
pub fn handle_get_response(status: u16, body: &str) -> Outcome {
    match status {
        200 if body_pane_status(body).as_deref() == Some("no_claude_pane") => Outcome {
            exit_code: EXIT_NO_CLAUDE_PANE,
            stdout: Some(body.to_string()),
            stderr: None,
        },
        200 => Outcome::ok_stdout(body),
        404 => Outcome::err(EXIT_INVALID_COLOR, body),
        _ => Outcome::err(EXIT_OTHER, body),
    }
}

/// Extract the top-level `pane_status` string field from a response body, if
/// present and well-formed. Shared by `handle_get_response` (FLEET-CLI-031).
fn body_pane_status(body: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(body).ok()?;
    value.get("pane_status")?.as_str().map(str::to_string)
}

// ---- Response handling: gbiv fleet send (FLEET-CLI-044 through -049) ------

/// Map a `POST /session/:color/send` HTTP response to an `Outcome`. The two
/// `409` shapes (`no_claude_pane` vs `looks_like_prompt_response`) are
/// distinguished by the response body's `error` field (see
/// `http_server::SendConflictError`/`SendGuardError`).
// @spec FLEET-CLI-044, FLEET-CLI-045, FLEET-CLI-046, FLEET-CLI-047, FLEET-CLI-048, FLEET-CLI-049
pub fn handle_send_response(status: u16, body: &str) -> Outcome {
    match status {
        200 => Outcome::ok_stdout(body),
        404 => Outcome::err(EXIT_INVALID_COLOR, body),
        409 => handle_send_conflict(body),
        502 => Outcome::err(EXIT_SEND_INCOMPLETE, body),
        _ => Outcome::err(EXIT_OTHER, body),
    }
}

/// A `409` from `POST /session/:color/send` is one of two shapes
/// (`http_server::SendConflictError` or `SendGuardError`), distinguished by
/// the `error` field. FLEET-CLI-047: a guard rejection surfaces only the
/// `explanation` field to stderr, not the whole JSON body.
fn handle_send_conflict(body: &str) -> Outcome {
    let value: serde_json::Value = match serde_json::from_str(body) {
        Ok(v) => v,
        Err(_) => return Outcome::err(EXIT_OTHER, format!("unexpected response body: {body}")),
    };
    match value.get("error").and_then(|e| e.as_str()) {
        Some("looks_like_prompt_response") => {
            let explanation = value
                .get("explanation")
                .and_then(|e| e.as_str())
                .unwrap_or(body);
            Outcome::err(EXIT_GUARD_REJECTED, explanation)
        }
        _ => Outcome::err(EXIT_NO_CLAUDE_PANE, body),
    }
}

// ---- Local pre-checks for gbiv fleet send (FLEET-CLI-038 through -041) ----

/// FLEET-CLI-038: validate + normalize `raw_color` against the active
/// palette before any other local check.
// @spec FLEET-CLI-038
pub fn validate_color_locally(raw_color: &str, palette: &Palette) -> Result<String, Outcome> {
    let normalized = normalize_color(raw_color);
    if palette.contains(&normalized) {
        Ok(normalized)
    } else {
        Err(Outcome::err(
            EXIT_INVALID_COLOR,
            format!("unknown color: {raw_color}"),
        ))
    }
}

/// FLEET-CLI-039: trim `raw_text`; empty after trimming is a local error.
// @spec FLEET-CLI-039
pub fn validate_text_locally(raw_text: &str) -> Result<String, Outcome> {
    let trimmed = raw_text.trim();
    if trimmed.is_empty() {
        Err(Outcome::err(EXIT_OTHER, "text must not be empty"))
    } else {
        Ok(trimmed.to_string())
    }
}

/// FLEET-CLI-040, FLEET-CLI-041: evaluate the shared guard against already
/// -trimmed text; on rejection, build the explanation with the *normalized*
/// color (FLEET-CLI-041) via the same `guard_explanation` the server calls.
// @spec FLEET-CLI-040, FLEET-CLI-041
pub fn check_guard_locally(normalized_color: &str, trimmed_text: &str) -> Option<Outcome> {
    guard_check(trimmed_text).map(|rejection| {
        Outcome::err(
            EXIT_GUARD_REJECTED,
            guard_explanation(normalized_color, rejection.reason.as_str()),
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use gbiv_core::palette::Palette;

    // ---- parse_port_file_content (FLEET-CLI-012) ---------------------------

    #[test]
    // @spec FLEET-CLI-012
    fn parse_port_file_content_parses_trimmed_decimal() {
        assert_eq!(parse_port_file_content("54321\n"), Ok(54321));
    }

    #[test]
    // @spec FLEET-CLI-012
    fn parse_port_file_content_rejects_malformed_content() {
        let outcome = parse_port_file_content("not-a-port\n").unwrap_err();
        assert_eq!(outcome.exit_code, EXIT_DAEMON_NOT_RUNNING);
        assert!(outcome.stdout.is_none());
        assert!(
            outcome
                .stderr
                .as_deref()
                .unwrap_or_default()
                .contains("corrupt"),
            "got: {outcome:?}"
        );
    }

    #[test]
    // @spec FLEET-CLI-012
    fn parse_port_file_content_rejects_empty_content() {
        assert!(parse_port_file_content("").is_err());
    }

    #[test]
    // @spec FLEET-CLI-012
    fn parse_port_file_content_rejects_out_of_range_value() {
        // u16::MAX + 1
        assert!(parse_port_file_content("65536").is_err());
    }

    // ---- sessions_url (FLEET-CLI-002) --------------------------------------

    #[test]
    // @spec FLEET-CLI-002
    fn sessions_url_without_lines() {
        assert_eq!(sessions_url(54321, None), "http://127.0.0.1:54321/sessions");
    }

    #[test]
    // @spec FLEET-CLI-002
    fn sessions_url_forwards_lines_verbatim() {
        assert_eq!(
            sessions_url(54321, Some("50")),
            "http://127.0.0.1:54321/sessions?lines=50"
        );
    }

    // ---- session_url (FLEET-CLI-003, FLEET-CLI-004) ------------------------

    #[test]
    // @spec FLEET-CLI-003
    fn session_url_tail_mode() {
        assert_eq!(
            session_url(54321, "red", Some("200"), None, None),
            "http://127.0.0.1:54321/session/red?lines=200"
        );
    }

    #[test]
    // @spec FLEET-CLI-003
    fn session_url_no_params() {
        assert_eq!(
            session_url(54321, "red", None, None, None),
            "http://127.0.0.1:54321/session/red"
        );
    }

    #[test]
    // @spec FLEET-CLI-004
    fn session_url_window_mode() {
        assert_eq!(
            session_url(54321, "red", None, Some("10"), Some("20")),
            "http://127.0.0.1:54321/session/red?start_line=10&end_line=20"
        );
    }

    #[test]
    // @spec FLEET-CLI-004
    fn session_url_window_mode_forwards_top_literal() {
        assert_eq!(
            session_url(54321, "red", None, Some("top"), Some("20")),
            "http://127.0.0.1:54321/session/red?start_line=top&end_line=20"
        );
    }

    // ---- send_url -----------------------------------------------------------

    #[test]
    // @spec FLEET-CLI-042
    fn send_url_builds_path() {
        assert_eq!(
            send_url(54321, "red"),
            "http://127.0.0.1:54321/session/red/send"
        );
    }

    // ---- handle_status_response (FLEET-CLI-020, FLEET-CLI-021) -------------

    #[test]
    // @spec FLEET-CLI-020
    fn handle_status_response_200_passthrough_regardless_of_pane_status() {
        let body = r#"[{"color":"red","pane_status":"error","error":"boom"}]"#;
        let outcome = handle_status_response(200, body);
        assert_eq!(outcome.exit_code, EXIT_OK);
        assert_eq!(outcome.stdout.as_deref(), Some(body));
        assert!(outcome.stderr.is_none());
    }

    #[test]
    // @spec FLEET-CLI-021
    fn handle_status_response_503_session_not_found() {
        let outcome = handle_status_response(503, r#"{"error":"tmux session not found"}"#);
        assert_eq!(outcome.exit_code, EXIT_OTHER);
        assert!(outcome.stdout.is_none());
    }

    // ---- handle_get_response (FLEET-CLI-030 through -034) ------------------

    #[test]
    // @spec FLEET-CLI-030
    fn handle_get_response_200_ok_pane_status() {
        let body = r#"{"color":"red","pane_status":"ok"}"#;
        let outcome = handle_get_response(200, body);
        assert_eq!(outcome.exit_code, EXIT_OK);
        assert_eq!(outcome.stdout.as_deref(), Some(body));
    }

    #[test]
    // @spec FLEET-CLI-030
    fn handle_get_response_200_ok_pane_status_with_multiple_panes_is_still_exit_0() {
        let body = r#"{"color":"red","pane_status":"ok","other_claude_panes":["%3"]}"#;
        let outcome = handle_get_response(200, body);
        assert_eq!(outcome.exit_code, EXIT_OK);
    }

    #[test]
    // @spec FLEET-CLI-031
    fn handle_get_response_200_no_claude_pane_is_exit_4() {
        let body = r#"{"color":"red","pane_status":"no_claude_pane"}"#;
        let outcome = handle_get_response(200, body);
        assert_eq!(outcome.exit_code, EXIT_NO_CLAUDE_PANE);
        assert_eq!(outcome.stdout.as_deref(), Some(body));
    }

    #[test]
    // @spec FLEET-CLI-032
    fn handle_get_response_404_is_exit_3() {
        let outcome = handle_get_response(404, r#"{"error":"unknown color: purple"}"#);
        assert_eq!(outcome.exit_code, EXIT_INVALID_COLOR);
        assert!(outcome.stdout.is_none());
    }

    #[test]
    // @spec FLEET-CLI-033
    fn handle_get_response_400_is_exit_1() {
        let outcome = handle_get_response(400, r#"{"error":"start_line must not be after end_line"}"#);
        assert_eq!(outcome.exit_code, EXIT_OTHER);
    }

    #[test]
    // @spec FLEET-CLI-034
    fn handle_get_response_500_and_503_are_exit_1() {
        assert_eq!(
            handle_get_response(500, r#"{"error":"internal"}"#).exit_code,
            EXIT_OTHER
        );
        assert_eq!(
            handle_get_response(503, r#"{"error":"tmux session not found"}"#).exit_code,
            EXIT_OTHER
        );
    }

    // ---- handle_send_response (FLEET-CLI-044 through -049) -----------------

    #[test]
    // @spec FLEET-CLI-044
    fn handle_send_response_200_is_exit_0() {
        let body = r#"{"ok":true,"sent_to_pane":"%3"}"#;
        let outcome = handle_send_response(200, body);
        assert_eq!(outcome.exit_code, EXIT_OK);
        assert_eq!(outcome.stdout.as_deref(), Some(body));
    }

    #[test]
    // @spec FLEET-CLI-044
    fn handle_send_response_200_with_multiple_panes_is_still_exit_0() {
        let body = r#"{"ok":true,"sent_to_pane":"%3","other_claude_panes":["%4"]}"#;
        assert_eq!(handle_send_response(200, body).exit_code, EXIT_OK);
    }

    #[test]
    // @spec FLEET-CLI-045
    fn handle_send_response_404_is_exit_3() {
        let outcome = handle_send_response(404, r#"{"error":"unknown color: purple"}"#);
        assert_eq!(outcome.exit_code, EXIT_INVALID_COLOR);
    }

    #[test]
    // @spec FLEET-CLI-046
    fn handle_send_response_409_no_claude_pane_is_exit_4() {
        let body = r#"{"ok":false,"error":"no_claude_pane","color":"red"}"#;
        let outcome = handle_send_response(409, body);
        assert_eq!(outcome.exit_code, EXIT_NO_CLAUDE_PANE);
        assert_eq!(outcome.stderr.as_deref(), Some(body));
    }

    #[test]
    // @spec FLEET-CLI-047
    fn handle_send_response_409_guard_rejection_is_exit_6_with_explanation_only() {
        let body = r#"{"ok":false,"error":"looks_like_prompt_response","reason":"yes/no word","color":"red","explanation":"gbiv refused this send...","docs":"docs/high-level-design.md#x"}"#;
        let outcome = handle_send_response(409, body);
        assert_eq!(outcome.exit_code, EXIT_GUARD_REJECTED);
        assert_eq!(
            outcome.stderr.as_deref(),
            Some("gbiv refused this send...")
        );
    }

    #[test]
    // @spec FLEET-CLI-048
    fn handle_send_response_502_is_exit_5() {
        let outcome = handle_send_response(502, r#"{"error":"send incomplete"}"#);
        assert_eq!(outcome.exit_code, EXIT_SEND_INCOMPLETE);
    }

    #[test]
    // @spec FLEET-CLI-049
    fn handle_send_response_500_and_503_are_exit_1() {
        assert_eq!(
            handle_send_response(500, r#"{"error":"internal"}"#).exit_code,
            EXIT_OTHER
        );
        assert_eq!(
            handle_send_response(503, r#"{"error":"tmux session not found"}"#).exit_code,
            EXIT_OTHER
        );
    }

    // ---- validate_color_locally (FLEET-CLI-038) -----------------------------

    #[test]
    // @spec FLEET-CLI-038
    fn validate_color_locally_accepts_base_color() {
        assert_eq!(
            validate_color_locally("red", &Palette::default()),
            Ok("red".to_string())
        );
    }

    #[test]
    // @spec FLEET-CLI-038
    fn validate_color_locally_normalizes_case_and_trailing_slash() {
        assert_eq!(
            validate_color_locally("RED/", &Palette::default()),
            Ok("red".to_string())
        );
    }

    #[test]
    // @spec FLEET-CLI-038
    fn validate_color_locally_rejects_unknown_color() {
        let outcome = validate_color_locally("purple", &Palette::default()).unwrap_err();
        assert_eq!(outcome.exit_code, EXIT_INVALID_COLOR);
        assert!(outcome.stdout.is_none());
    }

    // ---- validate_text_locally (FLEET-CLI-039) ------------------------------

    #[test]
    // @spec FLEET-CLI-039
    fn validate_text_locally_trims_and_accepts_nonempty() {
        assert_eq!(
            validate_text_locally("  hello there  "),
            Ok("hello there".to_string())
        );
    }

    #[test]
    // @spec FLEET-CLI-039
    fn validate_text_locally_rejects_empty_after_trim() {
        let outcome = validate_text_locally("   ").unwrap_err();
        assert_eq!(outcome.exit_code, EXIT_OTHER);
    }

    #[test]
    // @spec FLEET-CLI-039
    fn validate_text_locally_rejects_fully_empty() {
        assert!(validate_text_locally("").is_err());
    }

    // ---- check_guard_locally (FLEET-CLI-040, FLEET-CLI-041) -----------------

    #[test]
    // @spec FLEET-CLI-040
    fn check_guard_locally_passes_multiword_text() {
        assert_eq!(check_guard_locally("red", "please run the tests"), None);
    }

    #[test]
    // @spec FLEET-CLI-040
    fn check_guard_locally_rejects_yes_no_word() {
        let outcome = check_guard_locally("red", "yes").unwrap();
        assert_eq!(outcome.exit_code, EXIT_GUARD_REJECTED);
        assert!(outcome.stdout.is_none());
        assert!(
            outcome.stderr.as_deref().unwrap_or_default().contains("red"),
            "explanation should mention the (normalized) color: {outcome:?}"
        );
    }

    #[test]
    // @spec FLEET-CLI-041
    fn check_guard_locally_uses_normalized_color_in_explanation() {
        // FLEET-CLI-041: explanation text uses the already-normalized color,
        // so a caller must normalize before calling this (this function does
        // not re-normalize) -- verifies the explanation echoes exactly what
        // it was given, not some other casing.
        let outcome = check_guard_locally("red", "y").unwrap();
        let text = outcome.stderr.unwrap();
        assert!(text.contains("red"));
        assert!(!text.contains("RED"));
    }

    #[test]
    // @spec FLEET-CLI-040
    fn check_guard_locally_matches_every_rule_from_http_server_guard_check() {
        // Sanity: this is a thin wrapper over http_server::guard_check, not a
        // reimplementation (docs/llds/orchestrate-cli.md decision). Exercise
        // one example per rule to confirm the delegation, not the full rule
        // set (which is exhaustively tested in http_server.rs already).
        for text in ["y", "n", "yes", "1", "?"] {
            assert!(
                check_guard_locally("red", text).is_some(),
                "expected guard rejection for {text:?}"
            );
        }
    }
}

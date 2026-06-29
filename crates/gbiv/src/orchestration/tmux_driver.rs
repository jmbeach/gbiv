//! Orchestration-only tmux operations: `list_panes`, `capture_pane`, `send_keys`.
//!
//! See `docs/llds/tmux-driver.md`. The shared primitives (`tmux_available`,
//! `has_session`, `list_windows`, `session_name_for_root`) live in
//! `gbiv_core::tmux`; this module adds the pane-level read/write used only by the
//! fleet daemon. It reuses `gbiv_core::tmux::TmuxError` so every caller maps the
//! same variants.

use std::io::ErrorKind;
use std::process::Command;

pub use gbiv_core::tmux::TmuxError;

/// Default byte cap on a capture (~16k tokens). Callers pass this unless they
/// deliberately want more.
pub const DEFAULT_CAP_BYTES: usize = 64 * 1024;

/// Hard ceiling on a capture; the driver clamps any larger `max_bytes` to this.
pub const HARD_MAX_BYTES: usize = 256 * 1024;

// @spec TMX-DRV-023
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaneInfo {
    pub id: String,
    pub pid: u32,
    pub current_command: String,
    pub current_path: String,
}

// @spec TMX-DRV-015, TMX-DRV-016
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureRange {
    /// Tail of the buffer — `lines` rows up to the bottom of the visible pane.
    Tail { lines: usize },
    /// Explicit row window in tmux offset semantics. `start: i32::MIN` means the
    /// top of history (the literal `-` argument to tmux `-S`).
    Window { start: i32, end: i32 },
}

// @spec TMX-DRV-015
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Capture {
    pub text: String,
    pub truncated: bool,
    pub original_bytes: usize,
    pub returned_bytes: usize,
    pub range_requested: CaptureRange,
    pub range_returned: (i32, i32),
}

/// Captured subprocess result, decoupled from `std::process::Output` so the
/// driver's logic is testable with an injected runner.
struct CmdOut {
    success: bool,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}

/// Run `tmux` with the given args, mapping a missing binary to `NotInstalled`.
fn run_tmux(args: &[String]) -> Result<CmdOut, TmuxError> {
    let out = Command::new("tmux").args(args).output().map_err(|e| {
        if e.kind() == ErrorKind::NotFound {
            TmuxError::NotInstalled
        } else {
            TmuxError::Other(e.to_string())
        }
    })?;
    Ok(CmdOut {
        success: out.status.success(),
        stdout: out.stdout,
        stderr: out.stderr,
    })
}

// ---- list_panes -----------------------------------------------------------

fn list_panes_args(window_target: &str) -> Vec<String> {
    vec![
        "list-panes".into(),
        "-t".into(),
        window_target.into(),
        "-F".into(),
        "#{pane_id}\t#{pane_pid}\t#{pane_current_command}\t#{pane_current_path}".into(),
    ]
}

fn parse_panes_output(stdout: &[u8]) -> Result<Vec<PaneInfo>, TmuxError> {
    String::from_utf8_lossy(stdout)
        .lines()
        .filter(|l| !l.is_empty())
        .map(|line| {
            let parts: Vec<&str> = line.splitn(4, '\t').collect();
            if parts.len() < 4 {
                return Err(TmuxError::Other(format!("malformed pane line: {:?}", line)));
            }
            let pid = parts[1]
                .parse::<u32>()
                .map_err(|_| TmuxError::Other(format!("malformed pane line (pid): {:?}", line)))?;
            Ok(PaneInfo {
                id: parts[0].to_string(),
                pid,
                current_command: parts[2].to_string(),
                current_path: parts[3].to_string(),
            })
        })
        .collect()
}

fn list_panes_with<R>(window_target: &str, run: R) -> Result<Vec<PaneInfo>, TmuxError>
where
    R: Fn(&[String]) -> Result<CmdOut, TmuxError>,
{
    let out = run(&list_panes_args(window_target))?;
    if !out.success {
        return Err(TmuxError::PaneNotFound(window_target.to_string()));
    }
    parse_panes_output(&out.stdout)
}

// @spec TMX-DRV-013, TMX-DRV-014, TMX-DRV-024
pub fn list_panes(window_target: &str) -> Result<Vec<PaneInfo>, TmuxError> {
    list_panes_with(window_target, run_tmux)
}

// ---- capture_pane ---------------------------------------------------------

fn capture_args(pane_id: &str, range: CaptureRange) -> Result<Vec<String>, TmuxError> {
    let mut args = vec![
        "capture-pane".into(),
        "-t".into(),
        pane_id.into(),
        "-p".into(),
    ];
    match range {
        CaptureRange::Tail { lines } => {
            args.push("-S".into());
            args.push(format!("-{}", lines));
        }
        CaptureRange::Window { start, end } => {
            if start > end {
                return Err(TmuxError::Other("invalid range".into()));
            }
            args.push("-S".into());
            args.push(if start == i32::MIN {
                "-".into()
            } else {
                start.to_string()
            });
            args.push("-E".into());
            args.push(end.to_string());
        }
    }
    args.push("-J".into());
    Ok(args)
}

struct CapResult {
    text: String,
    truncated: bool,
    original_bytes: usize,
    returned_bytes: usize,
}

/// Stable prefix lets the skill/CLI pattern-match on a truncated capture.
fn truncation_marker(dropped: usize, total: usize, kept: usize) -> String {
    format!(
        "[…truncated {} of {} bytes from the head; showing the most recent {}. \
         To page earlier history, re-call with CaptureRange::Window {{ start, end }} bounding the dropped range.]\n",
        dropped, total, kept
    )
}

fn apply_cap(raw: &str, max_bytes: usize) -> CapResult {
    let cap = max_bytes.min(HARD_MAX_BYTES);
    let original_bytes = raw.len();
    if original_bytes <= cap {
        return CapResult {
            text: raw.to_string(),
            truncated: false,
            original_bytes,
            returned_bytes: original_bytes,
        };
    }
    // Keep the tail; advance the cut forward to the next UTF-8 boundary so the
    // returned text is always valid UTF-8.
    let mut cut = original_bytes - cap;
    while cut < original_bytes && !raw.is_char_boundary(cut) {
        cut += 1;
    }
    let kept = &raw[cut..];
    let marker = truncation_marker(cut, original_bytes, kept.len());
    let text = format!("{}{}", marker, kept);
    let returned_bytes = text.len();
    CapResult {
        text,
        truncated: true,
        original_bytes,
        returned_bytes,
    }
}

fn capture_pane_with<R>(
    pane_id: &str,
    range: CaptureRange,
    max_bytes: usize,
    run: R,
) -> Result<Capture, TmuxError>
where
    R: Fn(&[String]) -> Result<CmdOut, TmuxError>,
{
    // capture_args rejects an invalid range before any tmux invocation.
    let args = capture_args(pane_id, range)?;
    let out = run(&args)?;
    if !out.success {
        let stderr = String::from_utf8_lossy(&out.stderr);
        if stderr.contains("can't find") || stderr.contains("no such") {
            return Err(TmuxError::PaneNotFound(pane_id.to_string()));
        }
        return Err(TmuxError::Other(format!("capture-pane failed for {pane_id}: {stderr}")));
    }
    let raw = String::from_utf8_lossy(&out.stdout);
    let capped = apply_cap(&raw, max_bytes);
    // range_returned reflects the requested row window; the byte-cap-dropped head
    // is reported via original_bytes/returned_bytes/truncated (see tmux-driver LLD
    // § Technical Debt — precise row-shift for pagination is deferred).
    // @spec TMX-DRV-015 — clamp to i32::MIN rather than overflow/panic for huge tails.
    let range_returned = match range {
        CaptureRange::Tail { lines } => {
            let start = i32::try_from(lines).map(|n| -n).unwrap_or(i32::MIN);
            (start, 0)
        }
        CaptureRange::Window { start, end } => (start, end),
    };
    Ok(Capture {
        text: capped.text,
        truncated: capped.truncated,
        original_bytes: capped.original_bytes,
        returned_bytes: capped.returned_bytes,
        range_requested: range,
        range_returned,
    })
}

// @spec TMX-DRV-015, TMX-DRV-016, TMX-DRV-017, TMX-DRV-018, TMX-DRV-019, TMX-DRV-020
pub fn capture_pane(
    pane_id: &str,
    range: CaptureRange,
    max_bytes: usize,
) -> Result<Capture, TmuxError> {
    capture_pane_with(pane_id, range, max_bytes, run_tmux)
}

// ---- send_keys ------------------------------------------------------------

fn send_text_args(pane_id: &str, text: &str) -> Vec<String> {
    vec![
        "send-keys".into(),
        "-t".into(),
        pane_id.into(),
        "-l".into(),
        "--".into(),
        text.into(),
    ]
}

fn send_enter_args(pane_id: &str) -> Vec<String> {
    vec![
        "send-keys".into(),
        "-t".into(),
        pane_id.into(),
        "Enter".into(),
    ]
}

fn send_keys_with<R>(pane_id: &str, text: &str, run: R) -> Result<(), TmuxError>
where
    R: Fn(&[String]) -> Result<(), TmuxError>,
{
    // The text error propagates verbatim; only an Enter failure (text already
    // delivered) becomes SendKeysIncomplete.
    run(&send_text_args(pane_id, text))?;
    run(&send_enter_args(pane_id)).map_err(|_| TmuxError::SendKeysIncomplete(pane_id.to_string()))
}

// @spec TMX-DRV-021, TMX-DRV-022
pub fn send_keys(pane_id: &str, text: &str) -> Result<(), TmuxError> {
    send_keys_with(pane_id, text, |args| {
        let out = run_tmux(args)?;
        if out.success {
            Ok(())
        } else {
            let stderr = String::from_utf8_lossy(&out.stderr);
            if stderr.contains("can't find") || stderr.contains("no such") {
                Err(TmuxError::PaneNotFound(pane_id.to_string()))
            } else {
                Err(TmuxError::Other(format!("send-keys failed for {pane_id}: {stderr}")))
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- parse_panes_output ----

    // @spec TMX-DRV-013, TMX-DRV-023
    #[test]
    fn parse_panes_output_valid() {
        let out = parse_panes_output(b"%1\t1234\tnode\t/home/u\n%2\t5678\tzsh\t/tmp\n").unwrap();
        assert_eq!(
            out,
            vec![
                PaneInfo {
                    id: "%1".into(),
                    pid: 1234,
                    current_command: "node".into(),
                    current_path: "/home/u".into(),
                },
                PaneInfo {
                    id: "%2".into(),
                    pid: 5678,
                    current_command: "zsh".into(),
                    current_path: "/tmp".into(),
                },
            ]
        );
    }

    // @spec TMX-DRV-024
    #[test]
    fn parse_panes_output_too_few_fields() {
        let err = parse_panes_output(b"%1\t1234\tnode\n").unwrap_err();
        match err {
            TmuxError::Other(msg) => {
                assert!(msg.contains("malformed pane line"), "got: {msg:?}");
                assert!(msg.contains("node"), "should include the offending line: {msg:?}");
            }
            e => panic!("expected Other, got {e:?}"),
        }
    }

    // @spec TMX-DRV-024
    #[test]
    fn parse_panes_output_non_numeric_pid() {
        let err = parse_panes_output(b"%1\tnotapid\tnode\t/home/u\n").unwrap_err();
        match err {
            TmuxError::Other(msg) => assert!(msg.contains("notapid")),
            e => panic!("expected Other, got {e:?}"),
        }
    }

    // ---- capture_args ----

    // @spec TMX-DRV-015
    #[test]
    fn capture_args_tail() {
        let args = capture_args("%3", CaptureRange::Tail { lines: 50 }).unwrap();
        assert_eq!(args, vec!["capture-pane", "-t", "%3", "-p", "-S", "-50", "-J"]);
    }

    // @spec TMX-DRV-016
    #[test]
    fn capture_args_window() {
        let args = capture_args("%3", CaptureRange::Window { start: -100, end: -1 }).unwrap();
        assert_eq!(
            args,
            vec!["capture-pane", "-t", "%3", "-p", "-S", "-100", "-E", "-1", "-J"]
        );
    }

    // @spec TMX-DRV-016
    #[test]
    fn capture_args_window_top_of_history() {
        let args = capture_args("%3", CaptureRange::Window { start: i32::MIN, end: 0 }).unwrap();
        // start == i32::MIN maps to the literal "-".
        let s_idx = args.iter().position(|a| a == "-S").unwrap();
        assert_eq!(args[s_idx + 1], "-");
    }

    // @spec TMX-DRV-017
    #[test]
    fn capture_args_invalid_range() {
        let err = capture_args("%3", CaptureRange::Window { start: 5, end: 1 }).unwrap_err();
        assert_eq!(err, TmuxError::Other("invalid range".into()));
    }

    // ---- apply_cap ----

    // @spec TMX-DRV-018
    #[test]
    fn apply_cap_under_cap_is_untouched() {
        let r = apply_cap("hello", 64);
        assert!(!r.truncated);
        assert_eq!(r.text, "hello");
        assert_eq!(r.original_bytes, 5);
        assert_eq!(r.returned_bytes, 5);
    }

    // @spec TMX-DRV-018
    #[test]
    fn apply_cap_over_cap_keeps_tail_with_marker_at_utf8_boundary() {
        // 'é' is two bytes; build a string whose cut point lands mid-codepoint.
        let raw = format!("{}é-TAIL", "x".repeat(100));
        let cap = 8; // forces truncation; cut would split 'é' without boundary care
        let r = apply_cap(&raw, cap);
        assert!(r.truncated);
        assert!(r.text.starts_with("[…truncated "), "marker missing: {:?}", r.text);
        assert_eq!(r.original_bytes, raw.len());
        assert_eq!(r.returned_bytes, r.text.len());
        // Tail is preserved and the result is valid UTF-8 (String guarantees it).
        assert!(r.text.ends_with("-TAIL"));
    }

    // @spec TMX-DRV-019
    #[test]
    fn apply_cap_clamps_to_hard_max() {
        let raw = "a".repeat(HARD_MAX_BYTES + 4096);
        // Requesting more than the hard max behaves identically to the hard max.
        let huge = apply_cap(&raw, usize::MAX);
        let clamped = apply_cap(&raw, HARD_MAX_BYTES);
        assert!(huge.truncated);
        assert_eq!(huge.returned_bytes, clamped.returned_bytes);
    }

    // ---- send args ----

    // @spec TMX-DRV-021
    #[test]
    fn send_args_text_then_enter() {
        assert_eq!(
            send_text_args("%5", "hi there"),
            vec!["send-keys", "-t", "%5", "-l", "--", "hi there"]
        );
        assert_eq!(send_enter_args("%5"), vec!["send-keys", "-t", "%5", "Enter"]);
    }

    // @spec TMX-DRV-022
    #[test]
    fn send_keys_enter_failure_is_incomplete() {
        // Fake runner: the text call succeeds, the Enter call fails.
        let run = |args: &[String]| {
            if args.iter().any(|a| a == "Enter") {
                Err(TmuxError::Other("enter boom".into()))
            } else {
                Ok(())
            }
        };
        let err = send_keys_with("%5", "hi", run).unwrap_err();
        assert_eq!(err, TmuxError::SendKeysIncomplete("%5".into()));
    }

    // @spec TMX-DRV-022
    #[test]
    fn send_keys_text_failure_propagates_original_error() {
        // Fake runner: the text call itself fails — propagate that error verbatim.
        let run = |args: &[String]| {
            if args.iter().any(|a| a == "Enter") {
                Ok(())
            } else {
                Err(TmuxError::PaneNotFound("%5".into()))
            }
        };
        let err = send_keys_with("%5", "hi", run).unwrap_err();
        assert_eq!(err, TmuxError::PaneNotFound("%5".into()));
    }

    // ---- list_panes_with ----

    // @spec TMX-DRV-013
    #[test]
    fn list_panes_with_success_parses() {
        let run = |_args: &[String]| {
            Ok(CmdOut {
                success: true,
                stdout: b"%1\t1234\tnode\t/home/u\n".to_vec(),
                stderr: vec![],
            })
        };
        let panes = list_panes_with("sess:red", run).unwrap();
        assert_eq!(panes.len(), 1);
        assert_eq!(panes[0].id, "%1");
    }

    // @spec TMX-DRV-014
    #[test]
    fn list_panes_with_missing_window_is_pane_not_found() {
        let run = |_args: &[String]| {
            Ok(CmdOut {
                success: false,
                stdout: vec![],
                stderr: b"can't find window".to_vec(),
            })
        };
        let err = list_panes_with("sess:nope", run).unwrap_err();
        assert_eq!(err, TmuxError::PaneNotFound("sess:nope".into()));
    }

    // ---- capture_pane_with ----

    // @spec TMX-DRV-020
    #[test]
    fn capture_pane_with_missing_pane_is_pane_not_found() {
        let run = |_args: &[String]| {
            Ok(CmdOut {
                success: false,
                stdout: vec![],
                stderr: b"can't find pane".to_vec(),
            })
        };
        let err =
            capture_pane_with("%99", CaptureRange::Tail { lines: 10 }, DEFAULT_CAP_BYTES, run)
                .unwrap_err();
        assert_eq!(err, TmuxError::PaneNotFound("%99".into()));
    }

    // @spec TMX-DRV-017
    #[test]
    fn capture_pane_with_invalid_range_does_not_invoke_tmux() {
        let run = |_args: &[String]| -> Result<CmdOut, TmuxError> {
            panic!("tmux must not be invoked for an invalid range");
        };
        let err = capture_pane_with(
            "%3",
            CaptureRange::Window { start: 5, end: 1 },
            DEFAULT_CAP_BYTES,
            run,
        )
        .unwrap_err();
        assert_eq!(err, TmuxError::Other("invalid range".into()));
    }

    // @spec TMX-DRV-015
    #[test]
    fn capture_pane_with_success_returns_capture() {
        let run = |_args: &[String]| {
            Ok(CmdOut {
                success: true,
                stdout: b"recent output\n".to_vec(),
                stderr: vec![],
            })
        };
        let cap =
            capture_pane_with("%3", CaptureRange::Tail { lines: 35 }, DEFAULT_CAP_BYTES, run)
                .unwrap();
        assert!(!cap.truncated);
        assert_eq!(cap.text, "recent output\n");
        assert_eq!(cap.range_requested, CaptureRange::Tail { lines: 35 });
        assert_eq!(cap.range_returned, (-35, 0));
    }

    // @spec TMX-DRV-018, TMX-DRV-019
    #[test]
    fn capture_pane_with_truncates_large_output() {
        let big = "x".repeat(DEFAULT_CAP_BYTES + 1024);
        let run = {
            let big = big.clone();
            move |_args: &[String]| {
                Ok(CmdOut {
                    success: true,
                    stdout: big.as_bytes().to_vec(),
                    stderr: vec![],
                })
            }
        };
        let cap = capture_pane_with(
            "%3",
            CaptureRange::Tail { lines: 200 },
            DEFAULT_CAP_BYTES,
            run,
        )
        .unwrap();
        assert!(cap.truncated);
        assert_eq!(cap.original_bytes, big.len());
        assert!(cap.returned_bytes < cap.original_bytes);
        assert!(cap.text.starts_with("[…truncated "));
    }

    // @spec TMX-DRV-020
    #[test]
    fn capture_pane_with_non_pane_error_returns_other() {
        let run = |_args: &[String]| {
            Ok(CmdOut {
                success: false,
                stdout: vec![],
                stderr: b"server not running".to_vec(),
            })
        };
        let err =
            capture_pane_with("%3", CaptureRange::Tail { lines: 10 }, DEFAULT_CAP_BYTES, run)
                .unwrap_err();
        match err {
            TmuxError::Other(msg) => assert!(msg.contains("server not running")),
            e => panic!("expected Other, got {e:?}"),
        }
    }
}

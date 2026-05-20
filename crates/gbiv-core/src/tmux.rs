//! Shared tmux primitives for the gbiv binary and the roy daemon.
//!
//! See `docs/gbiv-core/llds/tmux-primitives.md` and the TMX-CORE-* specs in
//! `docs/gbiv-core/specs/tmux-primitives.md` for the contract.

use std::io::ErrorKind;
use std::process::{Command, ExitStatus};

/// Window metadata returned by [`list_windows`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowInfo {
    pub id: String,
    pub name: String,
}

/// Shared tmux error type. Roy populates the pane variants; gbiv-core never does.
#[derive(Debug, thiserror::Error)]
pub enum TmuxError {
    #[error("tmux binary not on PATH")]
    NotInstalled,
    #[error("tmux session not found: {0}")]
    SessionNotFound(String),
    #[error("tmux pane not found: {0}")]
    PaneNotFound(String),
    #[error("send-keys completed for text but Enter failed for pane {0}")]
    SendKeysIncomplete(String),
    #[error("tmux: {0}")]
    Other(String),
}

/// @spec TMX-CORE-010, TMX-CORE-013, TMX-CORE-014, TMX-CORE-015, TMX-CORE-016
pub fn tmux_available() -> Result<(), TmuxError> {
    match Command::new("tmux").arg("-V").output() {
        Ok(output) => {
            if output.status.success() {
                Ok(())
            } else {
                let stderr = String::from_utf8_lossy(&output.stderr);
                Err(TmuxError::Other(build_other_message(&stderr, output.status)))
            }
        }
        Err(e) if e.kind() == ErrorKind::NotFound => Err(TmuxError::NotInstalled),
        Err(e) => Err(TmuxError::Other(format!("failed to exec tmux: {e}"))),
    }
}

/// @spec TMX-CORE-020, TMX-CORE-021, TMX-CORE-022, TMX-CORE-023, TMX-CORE-024
pub fn has_session(name: &str) -> Result<bool, TmuxError> {
    match Command::new("tmux")
        .args(["has-session", "-t", name])
        .output()
    {
        Ok(output) => {
            if output.status.success() {
                Ok(true)
            } else {
                let stderr = String::from_utf8_lossy(&output.stderr);
                if stderr.to_lowercase().contains("can't find session") {
                    Ok(false)
                } else {
                    Err(TmuxError::Other(build_other_message(&stderr, output.status)))
                }
            }
        }
        Err(e) if e.kind() == ErrorKind::NotFound => Err(TmuxError::NotInstalled),
        Err(e) => Err(TmuxError::Other(format!("failed to exec tmux: {e}"))),
    }
}

/// @spec TMX-CORE-030, TMX-CORE-031, TMX-CORE-032, TMX-CORE-033, TMX-CORE-034, TMX-CORE-035, TMX-CORE-036
pub fn list_windows(session: &str) -> Result<Vec<WindowInfo>, TmuxError> {
    match Command::new("tmux")
        .args([
            "list-windows",
            "-t",
            session,
            "-F",
            "#{window_id}\t#{window_name}",
        ])
        .output()
    {
        Ok(output) => {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                parse_list_windows_output(&stdout)
            } else {
                let stderr = String::from_utf8_lossy(&output.stderr);
                if stderr.to_lowercase().contains("can't find session") {
                    Err(TmuxError::SessionNotFound(session.to_string()))
                } else {
                    Err(TmuxError::Other(build_other_message(&stderr, output.status)))
                }
            }
        }
        Err(e) if e.kind() == ErrorKind::NotFound => Err(TmuxError::NotInstalled),
        Err(e) => Err(TmuxError::Other(format!("failed to exec tmux: {e}"))),
    }
}

/// @spec TMX-CORE-040, TMX-CORE-041, TMX-CORE-042
pub fn session_name_for_root(folder_name: &str) -> String {
    folder_name.to_string()
}

/// Pure parser for `tmux list-windows -F '#{window_id}\t#{window_name}'` stdout.
/// Exposed for unit testing without a tmux subprocess.
///
/// @spec TMX-CORE-031, TMX-CORE-032, TMX-CORE-036
pub(crate) fn parse_list_windows_output(stdout: &str) -> Result<Vec<WindowInfo>, TmuxError> {
    let mut result = Vec::new();
    for line in stdout.lines() {
        if line.is_empty() {
            continue;
        }
        let parts: Vec<&str> = line.split('\t').collect();
        if parts.len() != 2 {
            return Err(TmuxError::Other(format!(
                "malformed list-windows line: {line}"
            )));
        }
        result.push(WindowInfo {
            id: parts[0].to_string(),
            name: parts[1].to_string(),
        });
    }
    Ok(result)
}

/// Constructs the message payload for [`TmuxError::Other`] from a failed tmux invocation.
/// Exposed for unit testing.
///
/// @spec TMX-CORE-060, TMX-CORE-061
pub(crate) fn build_other_message(stderr_lossy: &str, status: ExitStatus) -> String {
    let trimmed = stderr_lossy.trim();
    if !trimmed.is_empty() {
        return trimmed.to_string();
    }
    match status.code() {
        Some(code) => format!("exit status: {code}"),
        None => "exit status: signal".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::process::ExitStatusExt;

    /// @spec TMX-CORE-031
    #[test]
    fn parse_list_windows_parses_well_formed_lines_in_order() {
        let stdout = "@1\tmain\n@2\tred\n@3\tindigo\n";
        let got = parse_list_windows_output(stdout).expect("parse should succeed");
        assert_eq!(
            got,
            vec![
                WindowInfo { id: "@1".into(), name: "main".into() },
                WindowInfo { id: "@2".into(), name: "red".into() },
                WindowInfo { id: "@3".into(), name: "indigo".into() },
            ]
        );
    }

    /// @spec TMX-CORE-036
    #[test]
    fn parse_list_windows_empty_stdout_is_empty_vec() {
        let got = parse_list_windows_output("").expect("empty input is Ok(vec![])");
        assert!(got.is_empty());
    }

    /// @spec TMX-CORE-031
    #[test]
    fn parse_list_windows_tolerates_missing_trailing_newline() {
        let stdout = "@7\torange";
        let got = parse_list_windows_output(stdout).expect("trailing newline optional");
        assert_eq!(got, vec![WindowInfo { id: "@7".into(), name: "orange".into() }]);
    }

    /// @spec TMX-CORE-032
    #[test]
    fn parse_list_windows_rejects_malformed_line_all_or_nothing() {
        let stdout = "@1\tmain\n@2_no_tab_here\n@3\tred\n";
        let err = parse_list_windows_output(stdout).expect_err("malformed line aborts the call");
        match err {
            TmuxError::Other(msg) => assert!(
                msg.contains("@2_no_tab_here"),
                "Other message must include the offending raw line; got: {msg}"
            ),
            other => panic!("expected TmuxError::Other, got {other:?}"),
        }
    }

    /// @spec TMX-CORE-032
    #[test]
    fn parse_list_windows_rejects_line_with_extra_tab() {
        let stdout = "@1\tname\twith\ttab\n";
        let err = parse_list_windows_output(stdout).expect_err("extra fields abort the call");
        assert!(matches!(err, TmuxError::Other(_)));
    }

    /// @spec TMX-CORE-060
    #[test]
    fn build_other_message_uses_trimmed_stderr_when_nonempty() {
        let status = ExitStatus::from_raw(1 << 8);
        let msg = build_other_message("  can't find session: red\n", status);
        assert_eq!(msg, "can't find session: red");
    }

    /// @spec TMX-CORE-061
    #[test]
    fn build_other_message_falls_back_to_exit_status_when_stderr_empty() {
        let status = ExitStatus::from_raw(2 << 8);
        assert_eq!(build_other_message("", status), "exit status: 2");
        assert_eq!(build_other_message("   \n\t  ", status), "exit status: 2");
    }

    /// @spec TMX-CORE-061
    #[test]
    fn build_other_message_signal_fallback() {
        let status = ExitStatus::from_raw(9);
        let msg = build_other_message("", status);
        assert_eq!(msg, "exit status: signal");
    }

    /// @spec TMX-CORE-040, TMX-CORE-041
    #[test]
    fn session_name_for_root_returns_folder_name_unchanged() {
        assert_eq!(session_name_for_root("my-project"), "my-project");
        assert_eq!(session_name_for_root(""), "");
        assert_eq!(session_name_for_root("weird:name"), "weird:name");
    }
}

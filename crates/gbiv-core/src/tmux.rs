use std::process::Command as ProcessCommand;
use thiserror::Error;

// @spec TMX-DRV-001
#[derive(Debug, Error, PartialEq, Eq)]
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

// @spec TMX-DRV-002
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowInfo {
    pub id: String,
    pub name: String,
}

// @spec TMX-DRV-003, TMX-DRV-004
pub fn tmux_available() -> Result<(), TmuxError> {
    match ProcessCommand::new("tmux").arg("-V").output() {
        Ok(out) if out.status.success() => Ok(()),
        // Non-zero exit means tmux is present but broken, not simply absent.
        Ok(out) => Err(TmuxError::Other(format!(
            "tmux -V exited with status {:?}",
            out.status.code()
        ))),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Err(TmuxError::NotInstalled),
        Err(e) => Err(TmuxError::Other(e.to_string())),
    }
}

// @spec TMX-DRV-005, TMX-DRV-006, TMX-DRV-007
pub fn has_session(session: &str) -> Result<bool, TmuxError> {
    match ProcessCommand::new("tmux")
        .args(["has-session", "-t", session])
        .output()
    {
        Ok(out) => Ok(out.status.success()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Err(TmuxError::NotInstalled),
        Err(e) => Err(TmuxError::Other(e.to_string())),
    }
}

fn parse_windows_output(stdout: &[u8]) -> Result<Vec<WindowInfo>, TmuxError> {
    String::from_utf8_lossy(stdout)
        .lines()
        .filter(|l| !l.is_empty())
        .map(|line| {
            let mut parts = line.splitn(2, '\t');
            let id = parts.next().unwrap_or("").to_string();
            let name = parts.next().unwrap_or("").to_string();
            if id.is_empty() || name.is_empty() {
                return Err(TmuxError::Other(format!(
                    "malformed window line: {:?}",
                    line
                )));
            }
            Ok(WindowInfo { id, name })
        })
        .collect()
}

// @spec TMX-DRV-008, TMX-DRV-009, TMX-DRV-010, TMX-DRV-011
pub fn list_windows(session: &str) -> Result<Vec<WindowInfo>, TmuxError> {
    let out = ProcessCommand::new("tmux")
        .args([
            "list-windows",
            "-t",
            session,
            "-F",
            "#{window_id}\t#{window_name}",
        ])
        .output()
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                TmuxError::NotInstalled
            } else {
                TmuxError::Other(e.to_string())
            }
        })?;

    if !out.status.success() {
        // Use exit status rather than locale/version-dependent stderr strings.
        // Callers that need `has_session` already checked it; a non-zero exit
        // here is the session-not-found (or disappeared) case.
        return Err(TmuxError::SessionNotFound(session.to_string()));
    }

    parse_windows_output(&out.stdout)
}

// @spec TMX-DRV-012
pub fn session_name_for_root(folder_name: &str) -> String {
    folder_name.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    // @spec TMX-DRV-003
    #[test]
    #[serial]
    fn test_tmux_available_not_installed() {
        let tmpdir = std::env::temp_dir().join("gbiv_core_tmux_empty_path");
        std::fs::create_dir_all(&tmpdir).unwrap();
        let original = std::env::var("PATH").unwrap_or_default();
        unsafe { std::env::set_var("PATH", &tmpdir) };

        let result = tmux_available();

        unsafe { std::env::set_var("PATH", &original) };

        assert_eq!(result, Err(TmuxError::NotInstalled));
    }

    // @spec TMX-DRV-007
    #[test]
    #[serial]
    fn test_has_session_not_installed() {
        let tmpdir = std::env::temp_dir().join("gbiv_core_tmux_empty_path2");
        std::fs::create_dir_all(&tmpdir).unwrap();
        let original = std::env::var("PATH").unwrap_or_default();
        unsafe { std::env::set_var("PATH", &tmpdir) };

        let result = has_session("any");

        unsafe { std::env::set_var("PATH", &original) };

        assert_eq!(result, Err(TmuxError::NotInstalled));
    }

    // @spec TMX-DRV-010
    #[test]
    #[serial]
    fn test_list_windows_not_installed() {
        let tmpdir = std::env::temp_dir().join("gbiv_core_tmux_empty_path3");
        std::fs::create_dir_all(&tmpdir).unwrap();
        let original = std::env::var("PATH").unwrap_or_default();
        unsafe { std::env::set_var("PATH", &tmpdir) };

        let result = list_windows("any");

        unsafe { std::env::set_var("PATH", &original) };

        assert_eq!(result, Err(TmuxError::NotInstalled));
    }

    // @spec TMX-DRV-012
    #[test]
    fn test_session_name_for_root_returns_folder_name() {
        assert_eq!(session_name_for_root("myproject"), "myproject");
        assert_eq!(session_name_for_root("foo-bar"), "foo-bar");
    }

    // @spec TMX-DRV-011
    #[test]
    fn test_parse_windows_output_malformed_line() {
        let result = parse_windows_output(b"@1\twindow-one\nnoid-or-name-missing\n");
        assert!(result.is_err());
        match result.unwrap_err() {
            TmuxError::Other(msg) => assert!(msg.contains("malformed window line")),
            e => panic!("expected TmuxError::Other, got {:?}", e),
        }
    }

    #[test]
    fn test_parse_windows_output_valid() {
        let result = parse_windows_output(b"@1\twindow-one\n@2\twindow-two\n");
        assert_eq!(
            result,
            Ok(vec![
                WindowInfo {
                    id: "@1".to_string(),
                    name: "window-one".to_string()
                },
                WindowInfo {
                    id: "@2".to_string(),
                    name: "window-two".to_string()
                },
            ])
        );
    }
}

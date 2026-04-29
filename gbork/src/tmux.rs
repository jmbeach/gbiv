use std::fmt;
use std::io;
use std::process::Command;

#[derive(Debug)]
pub enum TmuxError {
    NotInstalled,
    SessionNotFound,
    PaneNotFound,
    SendKeysIncomplete,
    Other(String),
}

impl fmt::Display for TmuxError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TmuxError::NotInstalled => write!(f, "tmux is not installed or not on PATH"),
            TmuxError::SessionNotFound => write!(f, "tmux session not found"),
            TmuxError::PaneNotFound => write!(f, "tmux pane not found"),
            TmuxError::SendKeysIncomplete => {
                write!(f, "tmux send-keys: text sent but Enter failed")
            }
            TmuxError::Other(msg) => write!(f, "tmux error: {msg}"),
        }
    }
}

impl std::error::Error for TmuxError {}

#[derive(Debug, Clone)]
pub struct WindowInfo {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone)]
pub struct PaneInfo {
    pub id: String,
    pub pid: u32,
    pub current_command: String,
    pub current_path: String,
}

fn run_tmux(args: &[&str]) -> Result<String, TmuxError> {
    let output = match Command::new("tmux").args(args).output() {
        Ok(o) => o,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Err(TmuxError::NotInstalled),
        Err(e) => return Err(TmuxError::Other(e.to_string())),
    };
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        Err(classify(&stderr))
    }
}

fn classify(stderr: &str) -> TmuxError {
    let s = stderr.to_lowercase();
    if s.contains("can't find session") || s.contains("no server running") {
        TmuxError::SessionNotFound
    } else if s.contains("can't find pane") || s.contains("can't find window") {
        TmuxError::PaneNotFound
    } else {
        TmuxError::Other(stderr.to_string())
    }
}

pub fn check_installed() -> Result<(), TmuxError> {
    run_tmux(&["-V"]).map(|_| ())
}

pub fn session_exists(session: &str) -> Result<bool, TmuxError> {
    match run_tmux(&["has-session", "-t", session]) {
        Ok(_) => Ok(true),
        Err(TmuxError::SessionNotFound) => Ok(false),
        Err(e) => Err(e),
    }
}

pub fn list_windows(session: &str) -> Result<Vec<WindowInfo>, TmuxError> {
    let raw = run_tmux(&[
        "list-windows",
        "-t",
        session,
        "-F",
        "#{window_id}\t#{window_name}",
    ])?;
    let mut out = Vec::new();
    for line in raw.lines() {
        if line.is_empty() {
            continue;
        }
        let mut parts = line.splitn(2, '\t');
        let id = parts
            .next()
            .ok_or_else(|| TmuxError::Other(format!("malformed list-windows line: {line}")))?;
        let name = parts
            .next()
            .ok_or_else(|| TmuxError::Other(format!("malformed list-windows line: {line}")))?;
        out.push(WindowInfo {
            id: id.to_string(),
            name: name.to_string(),
        });
    }
    Ok(out)
}

pub fn list_panes(window_target: &str) -> Result<Vec<PaneInfo>, TmuxError> {
    let raw = run_tmux(&[
        "list-panes",
        "-t",
        window_target,
        "-F",
        "#{pane_id}\t#{pane_pid}\t#{pane_current_command}\t#{pane_current_path}",
    ])?;
    let mut out = Vec::new();
    for line in raw.lines() {
        if line.is_empty() {
            continue;
        }
        let parts: Vec<&str> = line.splitn(4, '\t').collect();
        if parts.len() != 4 {
            return Err(TmuxError::Other(format!("malformed list-panes line: {line}")));
        }
        let pid: u32 = parts[1]
            .parse()
            .map_err(|_| TmuxError::Other(format!("invalid pid in list-panes: {}", parts[1])))?;
        out.push(PaneInfo {
            id: parts[0].to_string(),
            pid,
            current_command: parts[2].to_string(),
            current_path: parts[3].to_string(),
        });
    }
    Ok(out)
}

pub fn capture_pane(pane_id: &str, lines: usize) -> Result<String, TmuxError> {
    let s_arg = format!("-{lines}");
    run_tmux(&["capture-pane", "-t", pane_id, "-p", "-S", &s_arg, "-J"])
}

pub fn send_keys(pane_id: &str, text: &str) -> Result<(), TmuxError> {
    run_tmux(&["send-keys", "-t", pane_id, "-l", "--", text])?;
    run_tmux(&["send-keys", "-t", pane_id, "Enter"]).map_err(|e| match e {
        TmuxError::Other(_) | TmuxError::PaneNotFound => TmuxError::SendKeysIncomplete,
        other => other,
    })?;
    Ok(())
}

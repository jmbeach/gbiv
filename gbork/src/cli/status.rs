use crate::api::{PaneStatus, SessionEntry};
use crate::cli::require_daemon_url;

pub fn run(lines: u32, json: bool) -> u8 {
    let base = match require_daemon_url() {
        Ok(u) => u,
        Err(code) => return code,
    };
    let url = format!("{base}/sessions?lines={lines}");
    let resp = match ureq::get(&url).timeout(std::time::Duration::from_secs(30)).call() {
        Ok(r) => r,
        Err(ureq::Error::Status(code, r)) => {
            eprintln!(
                "gbork: status: HTTP {code}: {}",
                r.into_string().unwrap_or_default()
            );
            return 1;
        }
        Err(e) => {
            eprintln!("gbork: status: {e}");
            eprintln!("(daemon may have stopped; restart with `gbork start`)");
            return 2;
        }
    };
    let body = match resp.into_string() {
        Ok(b) => b,
        Err(e) => {
            eprintln!("gbork: status: failed to read response: {e}");
            return 1;
        }
    };
    if json {
        println!("{body}");
        return 0;
    }
    let entries: Vec<SessionEntry> = match serde_json::from_str(&body) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("gbork: status: invalid JSON from server: {e}");
            return 1;
        }
    };
    for e in entries {
        let preview = preview_line(e.output.as_deref());
        let pane = e
            .claude_pane
            .as_deref()
            .or_else(|| {
                e.claude_panes
                    .as_ref()
                    .and_then(|v| v.first().map(|s| s.as_str()))
            })
            .unwrap_or("");
        let status = format!("{:?}", e.pane_status).to_lowercase();
        println!(
            "{:<8} {:<22} {:<6}  ↳ {}",
            e.color,
            status,
            pane,
            preview.unwrap_or_else(|| match e.pane_status {
                PaneStatus::NoWindow => "(no tmux window)".to_string(),
                PaneStatus::NoClaudePane => "(no claude pane in this window)".to_string(),
                PaneStatus::MultipleClaudePanes => "(multiple claude panes)".to_string(),
                PaneStatus::Ok => "(no recent output)".to_string(),
            })
        );
    }
    0
}

fn preview_line(output: Option<&str>) -> Option<String> {
    let s = output?;
    let mut last: Option<String> = None;
    for line in s.lines() {
        let trimmed = line.trim_end();
        if trimmed.is_empty() {
            continue;
        }
        last = Some(trimmed.to_string());
    }
    last.map(|s| {
        if s.len() > 100 {
            format!("{}…", &s[..100])
        } else {
            s
        }
    })
}

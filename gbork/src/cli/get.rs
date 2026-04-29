use crate::api::{PaneStatus, SessionDetail};
use crate::cli::require_daemon_url;

pub fn run(color: &str, lines: u32, json: bool) -> u8 {
    let base = match require_daemon_url() {
        Ok(u) => u,
        Err(code) => return code,
    };
    let url = format!("{base}/session/{}?lines={lines}", color.to_lowercase());
    let resp = match ureq::get(&url)
        .timeout(std::time::Duration::from_secs(30))
        .call()
    {
        Ok(r) => r,
        Err(ureq::Error::Status(404, r)) => {
            if json {
                println!("{}", r.into_string().unwrap_or_default());
            } else {
                eprintln!("gbork: get: {color} has no tmux window");
            }
            return 3;
        }
        Err(ureq::Error::Status(code, r)) => {
            eprintln!(
                "gbork: get: HTTP {code}: {}",
                r.into_string().unwrap_or_default()
            );
            return 1;
        }
        Err(e) => {
            eprintln!("gbork: get: {e}");
            return 2;
        }
    };
    let body = match resp.into_string() {
        Ok(b) => b,
        Err(e) => {
            eprintln!("gbork: get: failed to read response: {e}");
            return 1;
        }
    };
    if json {
        println!("{body}");
        return match serde_json::from_str::<SessionDetail>(&body)
            .map(|d| d.pane_status)
            .unwrap_or(PaneStatus::Ok)
        {
            PaneStatus::Ok => 0,
            PaneStatus::NoWindow => 3,
            PaneStatus::NoClaudePane | PaneStatus::MultipleClaudePanes => 4,
        };
    }
    let detail: SessionDetail = match serde_json::from_str(&body) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("gbork: get: invalid JSON: {e}");
            return 1;
        }
    };
    match detail.pane_status {
        PaneStatus::Ok => {
            println!(
                "[{} pane={} captured_at={}]",
                detail.color,
                detail.claude_pane.unwrap_or_default(),
                detail.captured_at.unwrap_or_default()
            );
            print!("{}", detail.output.unwrap_or_default());
            0
        }
        PaneStatus::NoClaudePane => {
            eprintln!("gbork: get: {color}: window exists but no claude pane");
            4
        }
        PaneStatus::MultipleClaudePanes => {
            eprintln!(
                "gbork: get: {color}: multiple claude panes: {:?}",
                detail.claude_panes.unwrap_or_default()
            );
            4
        }
        PaneStatus::NoWindow => {
            eprintln!("gbork: get: {color} has no tmux window");
            3
        }
    }
}

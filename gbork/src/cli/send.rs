use serde_json::json;

use crate::api::SendResponse;
use crate::cli::require_daemon_url;

pub fn run(color: &str, text: &str) -> u8 {
    if text.is_empty() {
        eprintln!("gbork: send: text must not be empty");
        return 1;
    }
    let base = match require_daemon_url() {
        Ok(u) => u,
        Err(code) => return code,
    };
    let url = format!("{base}/session/{}/send", color.to_lowercase());
    let resp = match ureq::post(&url)
        .timeout(std::time::Duration::from_secs(30))
        .send_json(json!({ "text": text }))
    {
        Ok(r) => r,
        Err(ureq::Error::Status(404, r)) => {
            eprintln!("gbork: send: {color}: {}", r.into_string().unwrap_or_default());
            return 3;
        }
        Err(ureq::Error::Status(409, r)) => {
            let body = r.into_string().unwrap_or_default();
            let msg: SendResponse = serde_json::from_str(&body).unwrap_or(SendResponse {
                ok: false,
                sent_to_pane: None,
                error: Some("conflict".to_string()),
                color: Some(color.to_string()),
            });
            eprintln!(
                "gbork: send: {color}: {}",
                msg.error.as_deref().unwrap_or("conflict")
            );
            return 4;
        }
        Err(ureq::Error::Status(502, r)) => {
            eprintln!(
                "gbork: send: text sent but Enter failed: {}",
                r.into_string().unwrap_or_default()
            );
            return 5;
        }
        Err(ureq::Error::Status(code, r)) => {
            eprintln!(
                "gbork: send: HTTP {code}: {}",
                r.into_string().unwrap_or_default()
            );
            return 1;
        }
        Err(e) => {
            eprintln!("gbork: send: {e}");
            return 2;
        }
    };
    let body = resp.into_string().unwrap_or_default();
    let parsed: SendResponse = serde_json::from_str(&body).unwrap_or(SendResponse {
        ok: true,
        sent_to_pane: None,
        error: None,
        color: None,
    });
    if parsed.ok {
        println!(
            "sent to {} pane {}",
            color,
            parsed.sent_to_pane.unwrap_or_default()
        );
        0
    } else {
        eprintln!(
            "gbork: send: {color}: {}",
            parsed.error.as_deref().unwrap_or("unknown error")
        );
        1
    }
}

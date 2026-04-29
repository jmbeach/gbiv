use std::collections::HashMap;
use std::io::Read;
use std::sync::Arc;
use std::thread;

use chrono::Utc;
use tiny_http::{Header, Method, Request, Response, Server};

use crate::api::{
    ErrorBody, PaneStatus, SendRequest, SendResponse, SessionDetail, SessionEntry,
};
use crate::locator::{self, Resolution};
use crate::tmux::{self, TmuxError};
use crate::COLORS;

pub struct Config {
    pub session_name: String,
}

pub struct ServerHandle {
    pub port: u16,
    pub join: thread::JoinHandle<()>,
}

pub fn serve(cfg: Config) -> Result<ServerHandle, String> {
    let server = Server::http("127.0.0.1:0").map_err(|e| format!("bind failed: {e}"))?;
    let port = server.server_addr().to_ip().map(|a| a.port()).unwrap_or(0);
    let cfg = Arc::new(cfg);
    let server = Arc::new(server);
    let join = thread::spawn(move || {
        for req in server.incoming_requests() {
            let cfg = cfg.clone();
            thread::spawn(move || {
                handle(req, &cfg);
            });
        }
    });
    Ok(ServerHandle { port, join })
}

fn handle(mut req: Request, cfg: &Config) {
    let url = req.url().to_string();
    let method = req.method().clone();
    let (path, query) = split_path_query(&url);
    let response_bytes_status = match (method, path.as_str()) {
        (Method::Get, "/sessions") => sessions(cfg, &query),
        (Method::Get, p) if p.starts_with("/session/") && !p.contains("/send") => {
            let color = strip_color(p, "/session/");
            session_detail(cfg, &color, &query)
        }
        (Method::Post, p) if p.starts_with("/session/") && p.ends_with("/send") => {
            let color = strip_color(
                p.trim_end_matches("/send").trim_end_matches('/'),
                "/session/",
            );
            let mut body = String::new();
            let _ = req.as_reader().read_to_string(&mut body);
            send(cfg, &color, &body)
        }
        _ => {
            let body = serde_json::to_vec(&ErrorBody {
                error: format!("not found: {url}"),
            })
            .unwrap();
            (404, body)
        }
    };
    let (status, body) = response_bytes_status;
    let _ = req.respond(json_response(status, body));
}

fn json_response(status: u16, body: Vec<u8>) -> Response<std::io::Cursor<Vec<u8>>> {
    Response::from_data(body)
        .with_status_code(status)
        .with_header(
            Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..]).unwrap(),
        )
}

fn split_path_query(url: &str) -> (String, HashMap<String, String>) {
    let mut q = HashMap::new();
    let (path, qstr) = match url.find('?') {
        Some(i) => (url[..i].to_string(), &url[i + 1..]),
        None => (url.to_string(), ""),
    };
    for pair in qstr.split('&') {
        if pair.is_empty() {
            continue;
        }
        let mut it = pair.splitn(2, '=');
        let k = it.next().unwrap_or("").to_string();
        let v = it.next().unwrap_or("").to_string();
        q.insert(k, v);
    }
    (path, q)
}

fn strip_color(path: &str, prefix: &str) -> String {
    path.trim_end_matches('/')
        .strip_prefix(prefix)
        .unwrap_or("")
        .trim_matches('/')
        .to_lowercase()
}

fn parse_lines(query: &HashMap<String, String>, default: u32, max: u32) -> Result<u32, String> {
    match query.get("lines") {
        Some(v) => {
            let n: u32 = v.parse().map_err(|_| format!("invalid lines: {v}"))?;
            Ok(n.min(max))
        }
        None => Ok(default),
    }
}

fn sessions(cfg: &Config, query: &HashMap<String, String>) -> (u16, Vec<u8>) {
    let lines = match parse_lines(query, 50, 1000) {
        Ok(n) => n,
        Err(e) => return (400, json_err(&e)),
    };
    let mut entries = Vec::with_capacity(7);
    for color in COLORS {
        entries.push(build_entry(cfg, color, lines));
    }
    (200, serde_json::to_vec(&entries).unwrap())
}

fn build_entry(cfg: &Config, color: &str, lines: u32) -> SessionEntry {
    let res = match locator::locate(&cfg.session_name, color) {
        Ok(r) => r,
        Err(TmuxError::SessionNotFound) => {
            return SessionEntry {
                color: color.to_string(),
                tmux_window: None,
                claude_pane: None,
                claude_panes: None,
                pane_status: PaneStatus::NoWindow,
                output: None,
                captured_at: None,
            };
        }
        Err(_) => {
            return SessionEntry {
                color: color.to_string(),
                tmux_window: None,
                claude_pane: None,
                claude_panes: None,
                pane_status: PaneStatus::NoWindow,
                output: None,
                captured_at: None,
            };
        }
    };
    match res {
        Resolution::Ok { pane_id } => {
            let output = tmux::capture_pane(&pane_id, lines as usize).ok();
            SessionEntry {
                color: color.to_string(),
                tmux_window: Some(color.to_string()),
                claude_pane: Some(pane_id),
                claude_panes: None,
                pane_status: PaneStatus::Ok,
                captured_at: Some(now_iso()),
                output,
            }
        }
        Resolution::NoWindow => SessionEntry {
            color: color.to_string(),
            tmux_window: None,
            claude_pane: None,
            claude_panes: None,
            pane_status: PaneStatus::NoWindow,
            output: None,
            captured_at: None,
        },
        Resolution::NoClaudePane => SessionEntry {
            color: color.to_string(),
            tmux_window: Some(color.to_string()),
            claude_pane: None,
            claude_panes: None,
            pane_status: PaneStatus::NoClaudePane,
            output: None,
            captured_at: None,
        },
        Resolution::MultipleClaudePanes { pane_ids } => SessionEntry {
            color: color.to_string(),
            tmux_window: Some(color.to_string()),
            claude_pane: None,
            claude_panes: Some(pane_ids),
            pane_status: PaneStatus::MultipleClaudePanes,
            output: None,
            captured_at: None,
        },
    }
}

fn session_detail(cfg: &Config, color: &str, query: &HashMap<String, String>) -> (u16, Vec<u8>) {
    if !COLORS.contains(&color) {
        return (404, json_err(&format!("unknown color: {color}")));
    }
    let lines = match parse_lines(query, 200, 5000) {
        Ok(n) => n,
        Err(e) => return (400, json_err(&e)),
    };
    let res = match locator::locate(&cfg.session_name, color) {
        Ok(r) => r,
        Err(TmuxError::SessionNotFound) => {
            return (
                503,
                json_err(&format!("tmux session not found: {}", cfg.session_name)),
            )
        }
        Err(e) => return (500, json_err(&e.to_string())),
    };
    match res {
        Resolution::NoWindow => (404, json_err(&format!("no tmux window for color: {color}"))),
        Resolution::Ok { pane_id } => {
            let output = match tmux::capture_pane(&pane_id, lines as usize) {
                Ok(s) => s,
                Err(TmuxError::PaneNotFound) => {
                    return (
                        404,
                        json_err(&format!("pane disappeared during capture: {pane_id}")),
                    )
                }
                Err(e) => return (500, json_err(&e.to_string())),
            };
            let body = SessionDetail {
                color: color.to_string(),
                claude_pane: Some(pane_id),
                claude_panes: None,
                pane_status: PaneStatus::Ok,
                captured_at: Some(now_iso()),
                output: Some(output),
            };
            (200, serde_json::to_vec(&body).unwrap())
        }
        Resolution::NoClaudePane => {
            let body = SessionDetail {
                color: color.to_string(),
                claude_pane: None,
                claude_panes: None,
                pane_status: PaneStatus::NoClaudePane,
                captured_at: None,
                output: None,
            };
            (200, serde_json::to_vec(&body).unwrap())
        }
        Resolution::MultipleClaudePanes { pane_ids } => {
            let body = SessionDetail {
                color: color.to_string(),
                claude_pane: None,
                claude_panes: Some(pane_ids),
                pane_status: PaneStatus::MultipleClaudePanes,
                captured_at: None,
                output: None,
            };
            (200, serde_json::to_vec(&body).unwrap())
        }
    }
}

fn send(cfg: &Config, color: &str, raw_body: &str) -> (u16, Vec<u8>) {
    if !COLORS.contains(&color) {
        return (404, json_err(&format!("unknown color: {color}")));
    }
    let req: SendRequest = match serde_json::from_str(raw_body) {
        Ok(r) => r,
        Err(e) => return (400, json_err(&format!("invalid JSON: {e}"))),
    };
    if req.text.is_empty() {
        return (400, json_err("text must not be empty"));
    }
    let res = match locator::locate(&cfg.session_name, color) {
        Ok(r) => r,
        Err(TmuxError::SessionNotFound) => {
            return (
                503,
                json_err(&format!("tmux session not found: {}", cfg.session_name)),
            )
        }
        Err(e) => return (500, json_err(&e.to_string())),
    };
    let pane_id = match res {
        Resolution::Ok { pane_id } => pane_id,
        Resolution::NoWindow => {
            return (404, json_err(&format!("no tmux window for color: {color}")))
        }
        Resolution::NoClaudePane => {
            return (
                409,
                serde_json::to_vec(&SendResponse {
                    ok: false,
                    sent_to_pane: None,
                    error: Some("no_claude_pane".to_string()),
                    color: Some(color.to_string()),
                })
                .unwrap(),
            )
        }
        Resolution::MultipleClaudePanes { .. } => {
            return (
                409,
                serde_json::to_vec(&SendResponse {
                    ok: false,
                    sent_to_pane: None,
                    error: Some("multiple_claude_panes".to_string()),
                    color: Some(color.to_string()),
                })
                .unwrap(),
            )
        }
    };
    match tmux::send_keys(&pane_id, &req.text) {
        Ok(()) => {
            let body = SendResponse {
                ok: true,
                sent_to_pane: Some(pane_id),
                error: None,
                color: None,
            };
            (200, serde_json::to_vec(&body).unwrap())
        }
        Err(TmuxError::SendKeysIncomplete) => (
            502,
            serde_json::to_vec(&SendResponse {
                ok: false,
                sent_to_pane: Some(pane_id),
                error: Some("send_keys_incomplete".to_string()),
                color: Some(color.to_string()),
            })
            .unwrap(),
        ),
        Err(e) => (500, json_err(&e.to_string())),
    }
}

fn json_err(msg: &str) -> Vec<u8> {
    serde_json::to_vec(&ErrorBody {
        error: msg.to_string(),
    })
    .unwrap()
}

fn now_iso() -> String {
    Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

//! `gbiv start`: daemon lifecycle — root/session discovery, palette load, TCP
//! bind, port file, worker threads, request routing, and shutdown cleanup.
//!
//! See `docs/llds/http-server.md` and `docs/specs/http-server.md`. The
//! request-handling logic itself lives in `http_server` (pure, dependency-
//! injected, unit-tested); this module is the thin `tiny_http`/filesystem/
//! signal-handling glue that wires the real Pane Locator and tmux Driver in.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::thread;

use gbiv_core::gitignore::ensure_gitignore_entry;
use gbiv_core::palette::Palette;
use gbiv_core::root::{find_gbiv_root, find_repo_in_worktree};
use gbiv_core::tmux::{session_name_for_root, tmux_available};

use super::clock::RealClock;
use super::http_server::{self, WORKER_THREADS};
use super::pane_locator::{locate_pane, locate_panes};
use super::tmux_driver::{capture_pane, send_keys};

/// Options for `gbiv start`, mirroring the CLI flags (HTTP-SRV-057, HTTP-SRV-058).
pub struct StartOptions {
    /// `--session-name` override; falls back to `session_name_for_root` (HTTP-SRV-003).
    pub session_name: Option<String>,
    /// `--bind` is parsed but ignored in v1 (HTTP-SRV-058, HTTP-SRV-015).
    pub bind: Option<String>,
}

/// Run the `gbiv start` daemon in the foreground until Ctrl+C/SIGTERM.
// @spec HTTP-SRV-001, HTTP-SRV-002, HTTP-SRV-003, HTTP-SRV-004, HTTP-SRV-005,
// HTTP-SRV-006, HTTP-SRV-007, HTTP-SRV-008, HTTP-SRV-009, HTTP-SRV-010,
// HTTP-SRV-011, HTTP-SRV-015, HTTP-SRV-016, HTTP-SRV-057, HTTP-SRV-058,
// HTTP-SRV-059
pub fn run(options: StartOptions) -> anyhow::Result<()> {
    let _ = options.bind; // reserved, intentionally ignored (HTTP-SRV-058)

    let cwd = std::env::current_dir()?;
    let gbiv_root = find_gbiv_root(&cwd)
        .ok_or_else(|| anyhow::anyhow!("not inside a gbiv project"))?;
    let repo = find_repo_in_worktree(&gbiv_root.root.join("main"))
        .ok_or_else(|| anyhow::anyhow!("could not find a git repo under main/"))?;

    let session = resolve_session_name(options.session_name, &gbiv_root.folder_name);

    let palette = Palette::load(&gbiv_root.root)?;

    tmux_available().map_err(|e| anyhow::anyhow!("tmux is not available: {e}"))?;

    let server = bind_server()?;
    let port = server_port(&server)?;

    let gbiv_dir = repo.join(".gbiv");
    let port_file = write_port_file(&gbiv_dir, port)?;
    ensure_gitignore_entry(&repo.join(".git"), ".gbiv/")?;

    println!("gbiv listening on http://127.0.0.1:{port}");
    tracing::info!(port, session = %session, root = %gbiv_root.root.display(), "gbiv start");

    register_shutdown_handler(port_file.clone())?;

    let server = Arc::new(server);
    let palette = Arc::new(palette);
    let session = Arc::new(session);

    let handles: Vec<_> = (0..WORKER_THREADS)
        .map(|_| {
            let server = Arc::clone(&server);
            let palette = Arc::clone(&palette);
            let session = Arc::clone(&session);
            thread::spawn(move || worker_loop(&server, &palette, &session))
        })
        .collect();
    for handle in handles {
        let _ = handle.join();
    }
    Ok(())
}

// @spec HTTP-SRV-003
/// Resolve the tmux session name: an explicit `--session-name` override wins,
/// otherwise it's derived from the gbiv root's folder name.
fn resolve_session_name(override_name: Option<String>, folder_name: &str) -> String {
    override_name.unwrap_or_else(|| session_name_for_root(folder_name))
}

// @spec HTTP-SRV-006, HTTP-SRV-015
fn bind_server() -> anyhow::Result<tiny_http::Server> {
    tiny_http::Server::http("127.0.0.1:0")
        .map_err(|e| anyhow::anyhow!("failed to bind 127.0.0.1: {e} (another gbiv daemon may already be running)"))
}

fn server_port(server: &tiny_http::Server) -> anyhow::Result<u16> {
    match server.server_addr() {
        tiny_http::ListenAddr::IP(addr) => Ok(addr.port()),
        other => anyhow::bail!("unexpected listen address: {other}"),
    }
}

// @spec HTTP-SRV-007, HTTP-SRV-008
/// Create `<repo>/.gbiv/` if missing and write the bound port as ASCII decimal
/// plus a trailing newline, returning the port file's path.
fn write_port_file(gbiv_dir: &Path, port: u16) -> anyhow::Result<PathBuf> {
    fs::create_dir_all(gbiv_dir)?;
    let port_file = gbiv_dir.join("port");
    fs::write(&port_file, format!("{port}\n"))?;
    Ok(port_file)
}

// @spec HTTP-SRV-012, HTTP-SRV-013
fn remove_port_file_best_effort(port_file: &Path) {
    if let Err(e) = fs::remove_file(port_file) {
        tracing::warn!(error = %e, path = %port_file.display(), "failed to remove port file on shutdown");
    }
}

// @spec HTTP-SRV-012, HTTP-SRV-013
// The `termination` feature (crates/gbiv/Cargo.toml) is required for `ctrlc` to
// catch SIGTERM in addition to SIGINT — without it, only Ctrl+C is handled and
// `kill`/process-manager shutdowns leave the port file stale.
fn register_shutdown_handler(port_file: PathBuf) -> anyhow::Result<()> {
    ctrlc::set_handler(move || {
        remove_port_file_best_effort(&port_file);
        std::process::exit(0);
    })
    .map_err(|e| anyhow::anyhow!("failed to install shutdown handler: {e}"))
}

// @spec HTTP-SRV-014
fn worker_loop(server: &tiny_http::Server, palette: &Palette, session: &str) {
    loop {
        match server.recv() {
            Ok(request) => handle_request(request, palette, session),
            Err(e) => {
                tracing::error!(error = %e, "worker thread exiting: recv failed");
                break;
            }
        }
    }
}

/// Split a `tiny_http` request URL (`/session/red?lines=50`) into its path and
/// an already-parsed query-parameter lookup.
fn split_query(url: &str) -> (&str, Vec<(&str, &str)>) {
    match url.split_once('?') {
        None => (url, Vec::new()),
        Some((path, query)) => {
            let params = query
                .split('&')
                .filter(|p| !p.is_empty())
                .filter_map(|p| p.split_once('='))
                .collect();
            (path, params)
        }
    }
}

fn query_get<'a>(params: &[(&'a str, &'a str)], key: &str) -> Option<&'a str> {
    params.iter().find(|(k, _)| *k == key).map(|(_, v)| *v)
}

fn read_body(request: &mut tiny_http::Request) -> String {
    let mut body = String::new();
    let _ = request.as_reader().read_to_string(&mut body);
    body
}

fn method_not_allowed() -> http_server::HttpResponse {
    http_server::HttpResponse {
        status: 404,
        body: r#"{"error":"not found"}"#.to_string(),
    }
}

fn handle_request(mut request: tiny_http::Request, palette: &Palette, session: &str) {
    let method = request.method().clone();
    let url = request.url().to_string();
    let (path, query) = split_query(&url);
    let segments: Vec<&str> = path.trim_matches('/').split('/').collect();

    let response = match (&method, segments.as_slice()) {
        (tiny_http::Method::Get, ["sessions"]) => {
            let lines = query_get(&query, "lines");
            http_server::handle_sessions(
                palette,
                session,
                lines,
                &|s, colors| locate_panes(s, colors),
                &|pane, range, max| capture_pane(pane, range, max),
                &RealClock,
            )
        }
        (tiny_http::Method::Get, ["session", color]) => {
            let lines = query_get(&query, "lines");
            let start_line = query_get(&query, "start_line");
            let end_line = query_get(&query, "end_line");
            http_server::handle_session_get(
                palette,
                session,
                color,
                lines,
                start_line,
                end_line,
                &|s, c| locate_pane(s, c),
                &|pane, range, max| capture_pane(pane, range, max),
                &RealClock,
            )
        }
        (tiny_http::Method::Post, ["session", color, "send"]) => {
            let body = read_body(&mut request);
            http_server::handle_session_send(
                palette,
                session,
                color,
                &body,
                &|s, c| locate_pane(s, c),
                &|pane, text| send_keys(pane, text),
            )
        }
        _ => method_not_allowed(),
    };

    let tiny_response = tiny_http::Response::from_string(response.body)
        .with_status_code(response.status)
        .with_header(
            tiny_http::Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..])
                .expect("static header is always valid"),
        );
    if let Err(e) = request.respond(tiny_response) {
        tracing::error!(error = %e, "failed to write HTTP response");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- split_query / query_get -----------------------------------------

    #[test]
    fn split_query_no_query_string() {
        let (path, params) = split_query("/sessions");
        assert_eq!(path, "/sessions");
        assert!(params.is_empty());
    }

    #[test]
    fn split_query_parses_params() {
        let (path, params) = split_query("/session/red?lines=50&start_line=-1");
        assert_eq!(path, "/session/red");
        assert_eq!(query_get(&params, "lines"), Some("50"));
        assert_eq!(query_get(&params, "start_line"), Some("-1"));
        assert_eq!(query_get(&params, "missing"), None);
    }

    #[test]
    fn split_query_ignores_malformed_pairs() {
        let (_, params) = split_query("/sessions?&lonely&lines=5");
        assert_eq!(query_get(&params, "lines"), Some("5"));
        assert_eq!(params.len(), 1);
    }

    // ---- resolve_session_name (HTTP-SRV-003) ------------------------------

    // @spec HTTP-SRV-003
    #[test]
    fn resolve_session_name_uses_override_when_present() {
        assert_eq!(
            resolve_session_name(Some("custom".to_string()), "myproject"),
            "custom"
        );
    }

    // @spec HTTP-SRV-003
    #[test]
    fn resolve_session_name_falls_back_to_folder_name() {
        assert_eq!(resolve_session_name(None, "myproject"), "myproject");
    }

    // ---- write_port_file / remove_port_file_best_effort (HTTP-SRV-007, 008, 012, 013) ----

    // @spec HTTP-SRV-007, HTTP-SRV-008
    #[test]
    fn write_port_file_creates_dir_and_writes_ascii_decimal() {
        let tmp = tempfile::TempDir::new().unwrap();
        let gbiv_dir = tmp.path().join(".gbiv");
        let port_file = write_port_file(&gbiv_dir, 54321).unwrap();
        assert_eq!(port_file, gbiv_dir.join("port"));
        assert_eq!(fs::read_to_string(&port_file).unwrap(), "54321\n");
    }

    // @spec HTTP-SRV-008
    #[test]
    fn write_port_file_overwrites_existing_content() {
        let tmp = tempfile::TempDir::new().unwrap();
        let gbiv_dir = tmp.path().join(".gbiv");
        write_port_file(&gbiv_dir, 1).unwrap();
        let port_file = write_port_file(&gbiv_dir, 2).unwrap();
        assert_eq!(fs::read_to_string(&port_file).unwrap(), "2\n");
    }

    // @spec HTTP-SRV-012
    #[test]
    fn remove_port_file_best_effort_deletes_file() {
        let tmp = tempfile::TempDir::new().unwrap();
        let port_file = tmp.path().join("port");
        fs::write(&port_file, "1\n").unwrap();
        remove_port_file_best_effort(&port_file);
        assert!(!port_file.exists());
    }

    // @spec HTTP-SRV-013
    #[test]
    fn remove_port_file_best_effort_does_not_panic_when_missing() {
        let tmp = tempfile::TempDir::new().unwrap();
        let port_file = tmp.path().join("does-not-exist");
        remove_port_file_best_effort(&port_file); // must not panic
    }

    // ---- bind_server / server_port (HTTP-SRV-006, HTTP-SRV-015) -----------

    // @spec HTTP-SRV-006, HTTP-SRV-015
    #[test]
    fn bind_server_binds_loopback_with_kernel_assigned_port() {
        let server = bind_server().unwrap();
        let port = server_port(&server).unwrap();
        assert_ne!(port, 0, "kernel should have assigned a concrete port");
        match server.server_addr() {
            tiny_http::ListenAddr::IP(addr) => assert!(addr.ip().is_loopback()),
            other => panic!("expected an IP listen address, got {other:?}"),
        }
    }

    // ---- WORKER_THREADS (HTTP-SRV-014) ------------------------------------

    // @spec HTTP-SRV-014
    #[test]
    fn worker_thread_count_is_sixteen() {
        assert_eq!(WORKER_THREADS, 16);
    }
}

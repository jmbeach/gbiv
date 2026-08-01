//! `gbiv start`: daemon lifecycle — root/session discovery, palette load, TCP
//! bind, port file, worker threads, request routing, and shutdown cleanup.
//!
//! See `docs/llds/http-server.md` and `docs/specs/http-server.md`. The
//! request-handling logic itself lives in `http_server` (pure, dependency-
//! injected, unit-tested); this module is the thin `tiny_http`/filesystem/
//! signal-handling glue that wires the real Pane Locator and tmux Driver in.

use std::fs;
use std::io::Read;
use std::net::{SocketAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

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

    let gbiv_dir = repo.join(".gbiv");
    let port_file = gbiv_dir.join("port");
    reject_if_daemon_already_running(&port_file)?;

    let server = bind_server()?;
    let port = server_port(&server)?;
    write_port_file(&gbiv_dir, port)?;
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

// Binding always asks the kernel for a fresh ephemeral port (`127.0.0.1:0`),
// so an OS-level bind failure here is never "another gbiv daemon is already
// running" (that case is caught earlier by `reject_if_daemon_already_running`,
// via a liveness probe against the *previous* port file) — it's a genuine
// resource/permission problem.
// @spec HTTP-SRV-006, HTTP-SRV-015
fn bind_server() -> anyhow::Result<tiny_http::Server> {
    tiny_http::Server::http("127.0.0.1:0")
        .map_err(|e| anyhow::anyhow!("failed to bind an ephemeral port on 127.0.0.1: {e}"))
}

/// Read the port recorded in an existing `.gbiv/port` file, if any.
fn read_existing_port(port_file: &Path) -> Option<u16> {
    fs::read_to_string(port_file).ok()?.trim().parse().ok()
}

/// Whether a daemon is still listening on `port` (a short-timeout loopback
/// connect — this is a liveness probe, not a claim about *which* process).
fn daemon_is_alive(port: u16) -> bool {
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    TcpStream::connect_timeout(&addr, Duration::from_millis(200)).is_ok()
}

// @spec HTTP-SRV-060, HTTP-SRV-061
/// If `port_file` names a port that's still accepting connections, another
/// daemon owns this workspace's port file — refuse to start a second one
/// rather than silently overwriting it and orphaning the first (the previous
/// `bind_server` error message described this guarantee without actually
/// providing it, since binding `127.0.0.1:0` can't fail on a port conflict).
/// A stale port file (daemon no longer listening) is not an error — `run`
/// proceeds to bind fresh and overwrite it.
fn reject_if_daemon_already_running(port_file: &Path) -> anyhow::Result<()> {
    let Some(port) = read_existing_port(port_file) else {
        return Ok(());
    };
    if daemon_is_alive(port) {
        anyhow::bail!(
            "another gbiv daemon is already running for this workspace on port {port} (see {})",
            port_file.display()
        );
    }
    tracing::info!(port, "stale port file found (daemon not responding); starting fresh");
    Ok(())
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

/// Cap on a `POST /session/:color/send` body. Generous for a `{"text": "..."}`
/// payload while bounding how much an oversized/endless local client can make
/// one worker thread buffer in memory (loopback-only per HTTP-SRV-016, so this
/// is defense-in-depth rather than a hardening measure against a hostile network).
const MAX_SEND_BODY_BYTES: usize = 64 * 1024;

/// Read a request body capped at `max_bytes`. A declared `Content-Length`
/// over the cap is rejected without reading anything off the socket; an
/// undeclared/chunked body that turns out to exceed the cap is also rejected
/// (by reading one byte past the limit) rather than silently truncated and
/// parsed as if it were complete.
fn read_body_capped(request: &mut tiny_http::Request, max_bytes: usize) -> Result<String, ()> {
    if let Some(len) = request.body_length() {
        if len > max_bytes {
            return Err(());
        }
    }
    let mut buf = Vec::new();
    request
        .as_reader()
        .take(max_bytes as u64 + 1)
        .read_to_end(&mut buf)
        .map_err(|_| ())?;
    if buf.len() > max_bytes {
        return Err(());
    }
    String::from_utf8(buf).map_err(|_| ())
}

fn method_not_allowed() -> http_server::HttpResponse {
    http_server::HttpResponse {
        status: 404,
        body: r#"{"error":"not found"}"#.to_string(),
    }
}

fn body_too_large_response() -> http_server::HttpResponse {
    http_server::HttpResponse {
        status: 400,
        body: r#"{"error":"request body too large"}"#.to_string(),
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
        // HTTP-SRV-037: an invalid color must 404 without reading the body at
        // all — checked here, before `read_body_capped` ever touches the
        // socket, since `handle_session_send`'s own internal color check runs
        // too late to avoid the read.
        (tiny_http::Method::Post, ["session", color, "send"]) => {
            match http_server::validate_color(color, palette) {
                http_server::ColorValidation::Invalid => http_server::invalid_color_response(color),
                http_server::ColorValidation::Valid(_) => {
                    match read_body_capped(&mut request, MAX_SEND_BODY_BYTES) {
                        Ok(body) => http_server::handle_session_send(
                            palette,
                            session,
                            color,
                            &body,
                            &|s, c| locate_pane(s, c),
                            &|pane, text| send_keys(pane, text),
                        ),
                        Err(()) => body_too_large_response(),
                    }
                }
            }
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

    // ---- read_existing_port / daemon_is_alive / reject_if_daemon_already_running
    // ---- (HTTP-SRV-060, HTTP-SRV-061) --------------------------------------

    #[test]
    fn read_existing_port_parses_trimmed_content() {
        let tmp = tempfile::TempDir::new().unwrap();
        let port_file = tmp.path().join("port");
        fs::write(&port_file, "54321\n").unwrap();
        assert_eq!(read_existing_port(&port_file), Some(54321));
    }

    #[test]
    fn read_existing_port_missing_file_is_none() {
        let tmp = tempfile::TempDir::new().unwrap();
        assert_eq!(read_existing_port(&tmp.path().join("nope")), None);
    }

    #[test]
    fn read_existing_port_malformed_content_is_none() {
        let tmp = tempfile::TempDir::new().unwrap();
        let port_file = tmp.path().join("port");
        fs::write(&port_file, "not-a-port\n").unwrap();
        assert_eq!(read_existing_port(&port_file), None);
    }

    #[test]
    fn daemon_is_alive_true_for_a_listening_port() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        assert!(daemon_is_alive(port));
    }

    #[test]
    fn daemon_is_alive_false_once_the_listener_is_dropped() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        drop(listener);
        assert!(!daemon_is_alive(port));
    }

    // @spec HTTP-SRV-061
    #[test]
    fn reject_if_daemon_already_running_allows_missing_port_file() {
        let tmp = tempfile::TempDir::new().unwrap();
        reject_if_daemon_already_running(&tmp.path().join("port")).unwrap();
    }

    // @spec HTTP-SRV-061
    #[test]
    fn reject_if_daemon_already_running_allows_stale_port_file() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        drop(listener);
        let tmp = tempfile::TempDir::new().unwrap();
        let port_file = tmp.path().join("port");
        fs::write(&port_file, format!("{port}\n")).unwrap();
        reject_if_daemon_already_running(&port_file).unwrap();
    }

    // @spec HTTP-SRV-060
    #[test]
    fn reject_if_daemon_already_running_errors_for_a_live_port() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let tmp = tempfile::TempDir::new().unwrap();
        let port_file = tmp.path().join("port");
        fs::write(&port_file, format!("{port}\n")).unwrap();
        let err = reject_if_daemon_already_running(&port_file).unwrap_err();
        assert!(err.to_string().contains("already running"), "got: {err}");
    }

    // ---- read_body_capped (HTTP-SRV-062) -----------------------------------
    // `tiny_http::Request` can't be constructed directly, so these spin a real
    // server on a loopback ephemeral port and drive it with a hand-written raw
    // HTTP request over an actual `TcpStream` — the same shape as the manual
    // end-to-end smoke test, just scoped to one function.

    fn request_from_raw_http(raw_request: &[u8]) -> tiny_http::Request {
        use std::io::Write;
        let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
        let addr = match server.server_addr() {
            tiny_http::ListenAddr::IP(a) => a,
            other => panic!("expected an IP listen address, got {other:?}"),
        };
        let mut stream = TcpStream::connect(addr).unwrap();
        stream.write_all(raw_request).unwrap();
        server.recv().unwrap()
    }

    // @spec HTTP-SRV-062
    #[test]
    fn read_body_capped_reads_body_under_cap() {
        let raw = b"POST /session/red/send HTTP/1.1\r\nHost: x\r\nContent-Length: 5\r\n\r\nhello";
        let mut request = request_from_raw_http(raw);
        assert_eq!(read_body_capped(&mut request, 1024), Ok("hello".to_string()));
    }

    // @spec HTTP-SRV-062
    #[test]
    fn read_body_capped_rejects_declared_content_length_over_cap() {
        // The full declared body is sent (tiny_http's `recv()` waits for it to
        // arrive before returning the `Request`) — `read_body_capped` must
        // reject based on the `Content-Length` header alone, without reading
        // any of that body back off the reader.
        let body = "a".repeat(1000);
        let raw = format!("POST /session/red/send HTTP/1.1\r\nHost: x\r\nContent-Length: 1000\r\n\r\n{body}");
        let mut request = request_from_raw_http(raw.as_bytes());
        assert_eq!(read_body_capped(&mut request, 10), Err(()));
    }
}

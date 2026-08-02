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

/// The bound-but-not-yet-serving result of [`bootstrap`]: everything through
/// HTTP-SRV-009 (port file written, `.gitignore` entry ensured), before any
/// process-wide side effect (stdout/log lines, the shutdown handler, worker
/// threads) that would make bootstrapping itself hard to test in isolation.
struct Bootstrap {
    server: tiny_http::Server,
    port: u16,
    port_file: PathBuf,
    root: PathBuf,
    session: String,
    palette: Palette,
}

// @spec HTTP-SRV-001, HTTP-SRV-002, HTTP-SRV-003, HTTP-SRV-004, HTTP-SRV-005,
// HTTP-SRV-006, HTTP-SRV-007, HTTP-SRV-008, HTTP-SRV-009, HTTP-SRV-060,
// HTTP-SRV-061
/// Discover the gbiv root from `cwd`, load the palette, verify tmux, guard
/// against an already-running daemon, bind, and write the port file +
/// gitignore entry. Factored out of `run` (which is otherwise one giant
/// function no test can drive without a real process cwd and full worker
/// spawn) so this sequencing — the part most likely to have an ordering bug —
/// is unit-testable against a temp directory.
fn bootstrap(cwd: &Path, options: StartOptions) -> anyhow::Result<Bootstrap> {
    let _ = options.bind; // reserved, intentionally ignored (HTTP-SRV-058)

    let gbiv_root =
        find_gbiv_root(cwd).ok_or_else(|| anyhow::anyhow!("not inside a gbiv project"))?;
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

    Ok(Bootstrap {
        server,
        port,
        port_file,
        root: gbiv_root.root,
        session,
        palette,
    })
}

/// Run the `gbiv start` daemon in the foreground until Ctrl+C/SIGTERM.
// @spec HTTP-SRV-010, HTTP-SRV-011, HTTP-SRV-012, HTTP-SRV-013, HTTP-SRV-014,
// HTTP-SRV-016, HTTP-SRV-057, HTTP-SRV-058, HTTP-SRV-059
pub fn run(options: StartOptions) -> anyhow::Result<()> {
    let cwd = std::env::current_dir()?;
    let Bootstrap {
        server,
        port,
        port_file,
        root,
        session,
        palette,
    } = bootstrap(&cwd, options)?;

    // HTTP-SRV-010 deliberately requires both: the stdout print is the
    // human-facing startup banner a developer sees typing `gbiv start` in a
    // terminal, independent of whatever RUST_LOG level is configured; the
    // info! line right after is the structured, stderr-routed log entry a
    // log collector or `-v` session would capture. They carry the same
    // information but serve different consumers.
    println!("gbiv listening on http://127.0.0.1:{port}");
    tracing::info!(port, session = %session, root = %root.display(), "gbiv start");

    register_shutdown_handler(port_file.clone())?;

    let server = Arc::new(server);
    let palette = Arc::new(palette);
    let session = Arc::new(session);

    let handles: Vec<_> = (0..WORKER_THREADS)
        .map(|worker_id| {
            let server = Arc::clone(&server);
            let palette = Arc::clone(&palette);
            let session = Arc::clone(&session);
            thread::spawn(move || worker_loop(worker_id, &server, &palette, &session))
        })
        .collect();
    for handle in handles {
        let _ = handle.join();
    }
    tracing::warn!("all worker threads have exited; daemon is no longer serving requests");
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
    // gbiv itself never creates the port file as a symlink; one being present
    // means some other local process planted it — refuse to write through it
    // rather than blindly following it to wherever it points (a plain
    // `fs::write` follows symlinks, so an attacker-controlled symlink here
    // could redirect this write to an arbitrary file the daemon's user can
    // write). Removing it first means the subsequent write always creates a
    // fresh regular file.
    if fs::symlink_metadata(&port_file)
        .map(|m| m.file_type().is_symlink())
        .unwrap_or(false)
    {
        fs::remove_file(&port_file)?;
    }
    fs::write(&port_file, format!("{port}\n"))?;
    Ok(port_file)
}

// @spec HTTP-SRV-012, HTTP-SRV-013
fn remove_port_file_best_effort(port_file: &Path) {
    match fs::remove_file(port_file) {
        Ok(()) => tracing::info!(path = %port_file.display(), "port file removed; shutdown complete"),
        Err(e) => tracing::warn!(error = %e, path = %port_file.display(), "failed to remove port file on shutdown"),
    }
}

// @spec HTTP-SRV-012, HTTP-SRV-013
// The `termination` feature (crates/gbiv/Cargo.toml) is required for `ctrlc` to
// catch SIGTERM in addition to SIGINT — without it, only Ctrl+C is handled and
// `kill`/process-manager shutdowns leave the port file stale.
fn register_shutdown_handler(port_file: PathBuf) -> anyhow::Result<()> {
    ctrlc::set_handler(move || {
        tracing::info!("received shutdown signal");
        remove_port_file_best_effort(&port_file);
        std::process::exit(0);
    })
    .map_err(|e| anyhow::anyhow!("failed to install shutdown handler: {e}"))
}

/// The real Pane Locator / tmux Driver wired up as an `http_server::Deps`.
/// Function items (`locate_panes` etc.) and a unit-struct `RealClock` are all
/// zero-sized and const-evaluable, so this is `'static` with no allocation —
/// safe to construct fresh per accepted request rather than threading a
/// shared instance through the worker pool.
fn production_deps() -> http_server::Deps<'static> {
    http_server::Deps {
        locate_panes: &locate_panes,
        locate_pane: &locate_pane,
        capture_pane: &capture_pane,
        send_keys: &send_keys,
        clock: &RealClock,
    }
}

// @spec HTTP-SRV-014
fn worker_loop(worker_id: usize, server: &tiny_http::Server, palette: &Palette, session: &str) {
    let deps = production_deps();
    loop {
        match server.recv() {
            Ok(request) => handle_request(request, palette, session, &deps),
            Err(e) => {
                tracing::error!(worker = worker_id, error = %e, "worker thread exiting: recv failed");
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

/// Routes one accepted request to the right `http_server` handler and writes
/// the response. `deps` is injected (rather than calling `pane_locator`/
/// `tmux_driver` directly) so the routing table itself — which method+path
/// maps to which handler, and the invalid-color-before-body-read ordering
/// below — is unit-testable against a real `tiny_http::Request` without a
/// live tmux session (see the `handle_request_routes_*` tests).
// @spec HTTP-SRV-063
fn handle_request(mut request: tiny_http::Request, palette: &Palette, session: &str, deps: &http_server::Deps) {
    let start = std::time::Instant::now();
    let method = request.method().clone();
    let url = request.url().to_string();
    let (path, query) = split_query(&url);
    let segments: Vec<&str> = path.trim_matches('/').split('/').collect();

    let response = match (&method, segments.as_slice()) {
        (tiny_http::Method::Get, ["sessions"]) => {
            let lines = query_get(&query, "lines");
            http_server::handle_sessions(palette, session, lines, deps)
        }
        (tiny_http::Method::Get, ["session", color]) => {
            let lines = query_get(&query, "lines");
            let start_line = query_get(&query, "start_line");
            let end_line = query_get(&query, "end_line");
            http_server::handle_session_get(palette, session, color, lines, start_line, end_line, deps)
        }
        // HTTP-SRV-037: an invalid color must 404 without reading the body at
        // all — checked here, before `read_body_capped` ever touches the
        // socket, since `handle_session_send`'s own internal color check runs
        // too late to avoid the read.
        (tiny_http::Method::Post, ["session", color, "send"]) => {
            match http_server::validate_color(color, palette) {
                http_server::ColorValidation::Invalid => http_server::invalid_color_response(color),
                http_server::ColorValidation::Valid(_) => match read_body_capped(&mut request, MAX_SEND_BODY_BYTES) {
                    Ok(body) => http_server::handle_session_send(palette, session, color, &body, deps),
                    Err(()) => body_too_large_response(),
                },
            }
        }
        _ => method_not_allowed(),
    };

    let status = response.status;
    let duration_ms = start.elapsed().as_millis();
    tracing::info!(method = %method, path = %path, status, duration_ms, "request");

    let tiny_response = tiny_http::Response::from_string(response.body)
        .with_status_code(status)
        .with_header(
            tiny_http::Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..])
                .expect("static header is always valid"),
        );
    if let Err(e) = request.respond(tiny_response) {
        tracing::error!(error = %e, method = %method, path = %path, status, "failed to write HTTP response");
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

    #[cfg(unix)]
    #[test]
    fn write_port_file_refuses_to_follow_a_symlink() {
        let tmp = tempfile::TempDir::new().unwrap();
        let gbiv_dir = tmp.path().join(".gbiv");
        fs::create_dir_all(&gbiv_dir).unwrap();
        let secret = tmp.path().join("secret").with_extension("txt");
        fs::write(&secret, "do not overwrite me").unwrap();
        std::os::unix::fs::symlink(&secret, gbiv_dir.join("port")).unwrap();

        let port_file = write_port_file(&gbiv_dir, 12345).unwrap();

        assert_eq!(fs::read_to_string(&port_file).unwrap(), "12345\n");
        assert_eq!(
            fs::read_to_string(&secret).unwrap(),
            "do not overwrite me",
            "the symlink target must be untouched"
        );
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

    // Accepted flakiness tradeoff: this assumes the OS won't reassign the
    // just-dropped ephemeral port to something else in the microseconds
    // before the connect attempt below. Vanishingly unlikely in practice; if
    // this test is ever seen to flake, this is the mechanism to suspect.
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

    // ---- handle_request routing (HTTP-SRV-037 and general dispatch) -------
    // Unlike `read_body_capped`'s tests above (which only need a `Request`),
    // these exercise `handle_request` end to end — real socket, real
    // response written back — with a fake `http_server::Deps` standing in for
    // the real Pane Locator/tmux Driver, so no live tmux session is required.

    fn unreachable_deps() -> http_server::Deps<'static> {
        http_server::Deps {
            locate_panes: &|_, _| unreachable!("locate_panes should not be called"),
            locate_pane: &|_, _| unreachable!("locate_pane should not be called"),
            capture_pane: &|_, _, _| unreachable!("capture_pane should not be called"),
            send_keys: &|_, _| unreachable!("send_keys should not be called"),
            clock: &RealClock,
        }
    }

    /// Like `request_from_raw_http`, but keeps the client `TcpStream` alive
    /// so the test can read `handle_request`'s written response back.
    fn request_and_stream(raw_request: &[u8]) -> (tiny_http::Request, TcpStream) {
        use std::io::Write;
        let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
        let addr = match server.server_addr() {
            tiny_http::ListenAddr::IP(a) => a,
            other => panic!("expected an IP listen address, got {other:?}"),
        };
        let mut stream = TcpStream::connect(addr).unwrap();
        stream.write_all(raw_request).unwrap();
        let request = server.recv().unwrap();
        (request, stream)
    }

    fn read_status_code(stream: &mut TcpStream) -> u16 {
        let mut buf = [0u8; 512];
        let n = stream.read(&mut buf).unwrap();
        let text = String::from_utf8_lossy(&buf[..n]);
        text.split_whitespace()
            .nth(1)
            .expect("response should have a status line")
            .parse()
            .expect("status code should be numeric")
    }

    // @spec HTTP-SRV-037
    #[test]
    fn handle_request_invalid_color_send_is_404_without_reading_body() {
        // The body is genuinely over MAX_SEND_BODY_BYTES and is fully sent
        // (tiny_http's `recv()` blocks until the declared Content-Length
        // arrives, so a body that's merely *declared* but never sent would
        // hang here rather than exercise the routing logic under test). If
        // routing read the body before checking the color, HTTP-SRV-062's
        // cap would trip and this would come back 400 "too large" instead of
        // the clean 404 asserted below.
        let body = "a".repeat(MAX_SEND_BODY_BYTES + 1);
        let raw = format!(
            "POST /session/purple/send HTTP/1.1\r\nHost: x\r\nContent-Length: {}\r\n\r\n{body}",
            body.len()
        );
        let (request, mut stream) = request_and_stream(raw.as_bytes());
        handle_request(request, &Palette::default(), "sess", &unreachable_deps());
        assert_eq!(read_status_code(&mut stream), 404);
    }

    #[test]
    fn handle_request_routes_get_sessions_to_locate_panes() {
        let deps = http_server::Deps {
            locate_panes: &|_, colors| {
                Ok(colors
                    .iter()
                    .map(|c| (c.to_string(), Ok(crate::orchestration::pane_locator::Resolution::NoWindow)))
                    .collect())
            },
            locate_pane: &|_, _| unreachable!("GET /sessions must use locate_panes, not locate_pane"),
            capture_pane: &|_, _, _| unreachable!("no pane resolved; capture must not be called"),
            send_keys: &|_, _| unreachable!("GET /sessions must never send keys"),
            clock: &RealClock,
        };
        let (request, mut stream) = request_and_stream(b"GET /sessions HTTP/1.1\r\nHost: x\r\n\r\n");
        handle_request(request, &Palette::default(), "sess", &deps);
        assert_eq!(read_status_code(&mut stream), 200);
    }

    #[test]
    fn handle_request_routes_get_session_color_to_locate_pane() {
        let deps = http_server::Deps {
            locate_panes: &|_, _| unreachable!("GET /session/:color must use locate_pane, not locate_panes"),
            locate_pane: &|_, _| Ok(crate::orchestration::pane_locator::Resolution::NoWindow),
            capture_pane: &|_, _, _| unreachable!("no pane resolved; capture must not be called"),
            send_keys: &|_, _| unreachable!("GET /session/:color must never send keys"),
            clock: &RealClock,
        };
        let (request, mut stream) = request_and_stream(b"GET /session/red HTTP/1.1\r\nHost: x\r\n\r\n");
        handle_request(request, &Palette::default(), "sess", &deps);
        // NoWindow maps to 404 (http_server::handle_session_get) — proves this
        // request reached that handler, not handle_sessions (which would 200).
        assert_eq!(read_status_code(&mut stream), 404);
    }

    #[test]
    fn handle_request_routes_post_session_send_to_locate_pane() {
        let deps = http_server::Deps {
            locate_panes: &|_, _| unreachable!("POST /send must use locate_pane, not locate_panes"),
            locate_pane: &|_, _| Ok(crate::orchestration::pane_locator::Resolution::NoWindow),
            capture_pane: &|_, _, _| unreachable!("POST /send must never capture"),
            send_keys: &|_, _| unreachable!("no pane resolved; send_keys must not be called"),
            clock: &RealClock,
        };
        let body = r#"{"text": "please run the tests"}"#;
        let raw = format!("POST /session/red/send HTTP/1.1\r\nHost: x\r\nContent-Length: {}\r\n\r\n{body}", body.len());
        let (request, mut stream) = request_and_stream(raw.as_bytes());
        handle_request(request, &Palette::default(), "sess", &deps);
        assert_eq!(read_status_code(&mut stream), 404);
    }

    #[test]
    fn handle_request_unknown_route_is_404() {
        let (request, mut stream) = request_and_stream(b"GET /nonexistent HTTP/1.1\r\nHost: x\r\n\r\n");
        handle_request(request, &Palette::default(), "sess", &unreachable_deps());
        assert_eq!(read_status_code(&mut stream), 404);
    }

    // ---- bootstrap (HTTP-SRV-001..009, 060, 061) ---------------------------

    fn init_git_repo(path: &Path) {
        std::process::Command::new("git")
            .args(["init", "-q"])
            .current_dir(path)
            .output()
            .unwrap();
    }

    // @spec HTTP-SRV-001, HTTP-SRV-002, HTTP-SRV-003, HTTP-SRV-004, HTTP-SRV-005,
    // HTTP-SRV-006, HTTP-SRV-007, HTTP-SRV-008, HTTP-SRV-009
    #[test]
    fn bootstrap_binds_writes_port_file_and_gitignore_entry() {
        let tmp = tempfile::TempDir::new().unwrap();
        let gbiv_root = tmp.path().join("proj");
        let main_repo = gbiv_root.join("main").join("proj");
        fs::create_dir_all(&main_repo).unwrap();
        init_git_repo(&main_repo);
        fs::create_dir_all(gbiv_root.join("red")).unwrap(); // base-color dir so find_gbiv_root succeeds

        let options = StartOptions {
            session_name: Some("test-session".to_string()),
            bind: None,
        };
        let result = bootstrap(&main_repo, options).unwrap();

        assert_eq!(result.session, "test-session");
        assert_eq!(result.root, gbiv_root);
        assert_ne!(result.port, 0);

        let port_contents = fs::read_to_string(&result.port_file).unwrap();
        assert_eq!(port_contents.trim().parse::<u16>().unwrap(), result.port);

        let exclude = fs::read_to_string(main_repo.join(".git/info/exclude")).unwrap();
        assert!(exclude.contains(".gbiv/"), "got: {exclude:?}");
    }

    // @spec HTTP-SRV-001
    #[test]
    fn bootstrap_fails_when_not_inside_a_gbiv_project() {
        let tmp = tempfile::TempDir::new().unwrap();
        let options = StartOptions {
            session_name: None,
            bind: None,
        };
        let err = match bootstrap(tmp.path(), options) {
            Err(e) => e,
            Ok(_) => panic!("expected bootstrap to fail"),
        };
        assert!(err.to_string().contains("not inside a gbiv project"), "got: {err}");
    }

    #[test]
    fn bootstrap_rejects_when_a_daemon_is_already_running() {
        let tmp = tempfile::TempDir::new().unwrap();
        let gbiv_root = tmp.path().join("proj");
        let main_repo = gbiv_root.join("main").join("proj");
        fs::create_dir_all(&main_repo).unwrap();
        init_git_repo(&main_repo);
        fs::create_dir_all(gbiv_root.join("red")).unwrap();

        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let live_port = listener.local_addr().unwrap().port();
        let gbiv_dir = main_repo.join(".gbiv");
        fs::create_dir_all(&gbiv_dir).unwrap();
        fs::write(gbiv_dir.join("port"), format!("{live_port}\n")).unwrap();

        let options = StartOptions {
            session_name: None,
            bind: None,
        };
        let err = match bootstrap(&main_repo, options) {
            Err(e) => e,
            Ok(_) => panic!("expected bootstrap to fail"),
        };
        assert!(err.to_string().contains("already running"), "got: {err}");
    }

    // @spec HTTP-SRV-061
    #[test]
    fn bootstrap_proceeds_past_a_stale_port_file() {
        let tmp = tempfile::TempDir::new().unwrap();
        let gbiv_root = tmp.path().join("proj");
        let main_repo = gbiv_root.join("main").join("proj");
        fs::create_dir_all(&main_repo).unwrap();
        init_git_repo(&main_repo);
        fs::create_dir_all(gbiv_root.join("red")).unwrap();

        // A port file naming a port nothing is listening on.
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let stale_port = listener.local_addr().unwrap().port();
        drop(listener);
        let gbiv_dir = main_repo.join(".gbiv");
        fs::create_dir_all(&gbiv_dir).unwrap();
        fs::write(gbiv_dir.join("port"), format!("{stale_port}\n")).unwrap();

        let options = StartOptions {
            session_name: None,
            bind: None,
        };
        let result = bootstrap(&main_repo, options).unwrap();

        // A fresh port was bound and written, overwriting the stale one.
        assert_ne!(result.port, stale_port);
        let port_contents = fs::read_to_string(&result.port_file).unwrap();
        assert_eq!(port_contents.trim().parse::<u16>().unwrap(), result.port);
    }
}

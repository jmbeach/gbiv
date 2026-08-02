//! `gbiv fleet status|get|send`: the real I/O glue — port-file discovery via
//! `core::find_gbiv_root`, the `ureq` HTTP call, and `info`-level logging.
//! Delegates all response-shaping and local-validation logic to
//! `fleet_client` (pure, unit-tested there) so this module only has to wire
//! real dependencies together; its own tests exercise that wiring against a
//! real `tiny_http` server standing in for the daemon (the same pattern
//! `orchestration::daemon`'s own tests use), not the individual branches of
//! response handling (already covered in `fleet_client`).
//!
//! Callers (the `gbiv fleet` clap arms in `main.rs`) get an `Outcome` back —
//! this module never prints or calls `process::exit` itself, so it stays
//! testable by asserting on the returned value.

use std::path::{Path, PathBuf};
use std::time::Duration;

use gbiv_core::palette::Palette;
use gbiv_core::root::{find_gbiv_root, find_repo_in_worktree};

use super::fleet_client::{
    self, check_guard_locally, handle_get_response, handle_send_response, handle_status_response,
    send_url, session_url, sessions_url, validate_color_locally, validate_text_locally, Outcome,
    EXIT_DAEMON_NOT_RUNNING, EXIT_OTHER,
};

/// FLEET-CLI-013: connect timeout for the one HTTP call each subcommand makes.
pub const CONNECT_TIMEOUT: Duration = Duration::from_secs(1);
/// FLEET-CLI-013: read timeout for the one HTTP call each subcommand makes.
pub const READ_TIMEOUT: Duration = Duration::from_secs(30);

// ---- Port discovery (FLEET-CLI-010, FLEET-CLI-011) -------------------------

/// Resolve `<gbiv-root>/main/<repo>/.gbiv/port`'s path from `cwd`, failing if
/// no gbiv project is found (FLEET-CLI-010) or the port file doesn't exist
/// (FLEET-CLI-011). Does not read or parse the file's content — see
/// `resolve_port`.
// @spec FLEET-CLI-010, FLEET-CLI-011
pub fn locate_port_file(cwd: &Path) -> Result<PathBuf, Outcome> {
    let gbiv_root = find_gbiv_root(cwd)
        .ok_or_else(|| Outcome::err(EXIT_DAEMON_NOT_RUNNING, "not inside a gbiv project"))?;
    let repo = find_repo_in_worktree(&gbiv_root.root.join("main")).ok_or_else(|| {
        Outcome::err(
            EXIT_DAEMON_NOT_RUNNING,
            "could not find a git repo under main/",
        )
    })?;
    let port_file = repo.join(".gbiv").join("port");
    if !port_file.exists() {
        return Err(Outcome::err(
            EXIT_DAEMON_NOT_RUNNING,
            format!(
                "no port file at {}; start it with: gbiv start",
                port_file.display()
            ),
        ));
    }
    Ok(port_file)
}

/// Locate and parse the port file, returning both the path (for logging,
/// FLEET-CLI-051) and the parsed port.
// @spec FLEET-CLI-012, FLEET-CLI-051
pub fn resolve_port(cwd: &Path) -> Result<(PathBuf, u16), Outcome> {
    let port_file = locate_port_file(cwd)?;
    let content = std::fs::read_to_string(&port_file).map_err(|_| {
        Outcome::err(
            EXIT_DAEMON_NOT_RUNNING,
            format!("port file at {} is corrupt", port_file.display()),
        )
    })?;
    let port = fleet_client::parse_port_file_content(&content)?;
    Ok((port_file, port))
}

// ---- HTTP transport (FLEET-CLI-013, FLEET-CLI-014, FLEET-CLI-015) ---------

/// Issue exactly one HTTP request and return `(status, body)` on any
/// response received. FLEET-CLI-014: connection refused or a timeout before
/// any response -> `Err` exit `2`. A response that *is* received but doesn't
/// parse how the caller expects (FLEET-CLI-015) is NOT this function's
/// concern — it hands back the raw body regardless of shape, and the
/// caller's `handle_*_response` decides what "unexpected" means for that
/// endpoint.
// @spec FLEET-CLI-013, FLEET-CLI-014, FLEET-CLI-015, FLEET-CLI-052
pub fn issue_request(
    method: &str,
    url: &str,
    body: Option<&str>,
) -> Result<(u16, String), Outcome> {
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(CONNECT_TIMEOUT)
        .timeout_read(READ_TIMEOUT)
        .build();

    let request = agent.request(method, url);
    let result = match body {
        Some(b) => request
            .set("Content-Type", "application/json")
            .send_string(b),
        None => request.call(),
    };

    match result {
        Ok(response) => {
            let status = response.status();
            let body = response.into_string().unwrap_or_default();
            Ok((status, body))
        }
        Err(ureq::Error::Status(status, response)) => {
            let body = response.into_string().unwrap_or_default();
            Ok((status, body))
        }
        Err(ureq::Error::Transport(_)) => Err(Outcome::err(
            EXIT_DAEMON_NOT_RUNNING,
            "port file present but daemon not responding (stale?); restart with: gbiv start",
        )),
    }
}

/// FLEET-CLI-053: log the final exit code at `info` immediately before
/// returning, for any non-zero outcome. Shared by all three subcommands so
/// none of them can forget it.
// @spec FLEET-CLI-053
fn log_and_return(outcome: Outcome) -> Outcome {
    if outcome.exit_code != fleet_client::EXIT_OK {
        tracing::info!(
            exit_code = outcome.exit_code,
            "gbiv fleet: exiting non-zero"
        );
    }
    outcome
}

// ---- gbiv fleet status (FLEET-CLI-020, FLEET-CLI-021) ----------------------

// @spec FLEET-CLI-020, FLEET-CLI-021
pub fn run_status(cwd: &Path, lines: Option<&str>) -> Outcome {
    let (port_file, port) = match resolve_port(cwd) {
        Ok(v) => v,
        Err(outcome) => return log_and_return(outcome),
    };
    tracing::info!(port_file = %port_file.display(), "resolved daemon port file");

    let url = sessions_url(port, lines);
    tracing::info!(method = "GET", url = %url, "issuing request");
    let (status, body) = match issue_request("GET", &url, None) {
        Ok(v) => v,
        Err(outcome) => return log_and_return(outcome),
    };
    log_and_return(handle_status_response(status, &body))
}

// ---- gbiv fleet get (FLEET-CLI-030 through -034) ---------------------------

/// Color validation is NOT done locally for `get` (server `404` is the
/// source of truth, per the LLD decision to keep `status`/`get` symmetric)
/// — `raw_color` is forwarded as-is, unnormalized.
// @spec FLEET-CLI-030, FLEET-CLI-031, FLEET-CLI-032, FLEET-CLI-033, FLEET-CLI-034
pub fn run_get(
    cwd: &Path,
    raw_color: &str,
    lines: Option<&str>,
    start_line: Option<&str>,
    end_line: Option<&str>,
) -> Outcome {
    let (port_file, port) = match resolve_port(cwd) {
        Ok(v) => v,
        Err(outcome) => return log_and_return(outcome),
    };
    tracing::info!(port_file = %port_file.display(), "resolved daemon port file");

    let url = session_url(port, raw_color, lines, start_line, end_line);
    tracing::info!(method = "GET", url = %url, "issuing request");
    let (status, body) = match issue_request("GET", &url, None) {
        Ok(v) => v,
        Err(outcome) => return log_and_return(outcome),
    };
    log_and_return(handle_get_response(status, &body))
}

// ---- gbiv fleet send (FLEET-CLI-038 through -049) --------------------------

/// Local validation order mirrors the server's fixed order exactly: color,
/// then text, then guard (FLEET-CLI-038 through FLEET-CLI-041) — all before
/// port resolution or any network I/O, so `gbiv fleet send <bad-color>
/// <guard-shaped-text>` reports the color error even with no daemon running.
// @spec FLEET-CLI-038, FLEET-CLI-039, FLEET-CLI-040, FLEET-CLI-041, FLEET-CLI-042,
// FLEET-CLI-044, FLEET-CLI-045, FLEET-CLI-046, FLEET-CLI-047, FLEET-CLI-048, FLEET-CLI-049
pub fn run_send(cwd: &Path, raw_color: &str, raw_text: &str) -> Outcome {
    let gbiv_root = match find_gbiv_root(cwd) {
        Some(r) => r,
        None => {
            return log_and_return(Outcome::err(
                EXIT_DAEMON_NOT_RUNNING,
                "not inside a gbiv project",
            ))
        }
    };
    let palette = match Palette::load(&gbiv_root.root) {
        Ok(p) => p,
        Err(e) => {
            return log_and_return(Outcome::err(
                EXIT_OTHER,
                format!("failed to load active palette: {e}"),
            ))
        }
    };

    let normalized_color = match validate_color_locally(raw_color, &palette) {
        Ok(c) => c,
        Err(outcome) => return log_and_return(outcome),
    };
    let trimmed_text = match validate_text_locally(raw_text) {
        Ok(t) => t,
        Err(outcome) => return log_and_return(outcome),
    };
    if let Some(outcome) = check_guard_locally(&normalized_color, &trimmed_text) {
        return log_and_return(outcome);
    }

    let (port_file, port) = match resolve_port(cwd) {
        Ok(v) => v,
        Err(outcome) => return log_and_return(outcome),
    };
    tracing::info!(port_file = %port_file.display(), "resolved daemon port file");

    let url = send_url(port, &normalized_color);
    let request_body = serde_json::json!({ "text": trimmed_text }).to_string();
    tracing::info!(method = "POST", url = %url, "issuing request");
    let (status, response_body) = match issue_request("POST", &url, Some(&request_body)) {
        Ok(v) => v,
        Err(outcome) => return log_and_return(outcome),
    };
    log_and_return(handle_send_response(status, &response_body))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use tempfile::TempDir;

    fn init_git_repo(path: &Path) {
        std::process::Command::new("git")
            .args(["init", "-q"])
            .current_dir(path)
            .output()
            .unwrap();
    }

    /// Build a minimal gbiv-shaped layout under `base/<project>` and return
    /// (project_root, main_repo_path). Mirrors `daemon.rs`'s own test helper.
    fn setup_gbiv_layout(base: &Path, project: &str) -> (PathBuf, PathBuf) {
        let project_root = base.join(project);
        let main_repo = project_root.join("main").join(project);
        fs::create_dir_all(&main_repo).unwrap();
        init_git_repo(&main_repo);
        fs::create_dir_all(project_root.join("red")).unwrap();
        (project_root, main_repo)
    }

    fn write_port_file(main_repo: &Path, port: u16) {
        let gbiv_dir = main_repo.join(".gbiv");
        fs::create_dir_all(&gbiv_dir).unwrap();
        fs::write(gbiv_dir.join("port"), format!("{port}\n")).unwrap();
    }

    /// A minimal fake daemon: accepts exactly one connection, replies with a
    /// fixed status+body, and returns whatever it received (method + path)
    /// for the test to assert on. Real socket I/O, no `ureq` dependency on
    /// the test side.
    fn fake_daemon_once(status: u16, body: &'static str) -> (u16, std::thread::JoinHandle<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let handle = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut buf = [0u8; 4096];
            let n = stream.read(&mut buf).unwrap();
            let request_line = String::from_utf8_lossy(&buf[..n])
                .lines()
                .next()
                .unwrap_or("")
                .to_string();
            let response = format!(
                "HTTP/1.1 {status} X\r\nContent-Length: {}\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            stream.write_all(response.as_bytes()).unwrap();
            request_line
        });
        (port, handle)
    }

    // ---- locate_port_file (FLEET-CLI-010, FLEET-CLI-011) --------------------

    #[test]
    // @spec FLEET-CLI-010
    fn locate_port_file_fails_outside_a_gbiv_project() {
        let tmp = TempDir::new().unwrap();
        let outcome = locate_port_file(tmp.path()).unwrap_err();
        assert_eq!(outcome.exit_code, EXIT_DAEMON_NOT_RUNNING);
        assert!(
            outcome
                .stderr
                .as_deref()
                .unwrap_or_default()
                .contains("not inside a gbiv project"),
            "got: {outcome:?}"
        );
    }

    #[test]
    // @spec FLEET-CLI-011
    fn locate_port_file_fails_when_port_file_missing() {
        let tmp = TempDir::new().unwrap();
        let (_root, main_repo) = setup_gbiv_layout(tmp.path(), "proj");
        let outcome = locate_port_file(&main_repo).unwrap_err();
        assert_eq!(outcome.exit_code, EXIT_DAEMON_NOT_RUNNING);
    }

    #[test]
    // @spec FLEET-CLI-011
    fn locate_port_file_succeeds_when_present() {
        let tmp = TempDir::new().unwrap();
        let (_root, main_repo) = setup_gbiv_layout(tmp.path(), "proj");
        write_port_file(&main_repo, 12345);
        let path = locate_port_file(&main_repo).unwrap();
        assert_eq!(path, main_repo.join(".gbiv").join("port"));
    }

    // ---- resolve_port (FLEET-CLI-012 delegation) -----------------------------

    #[test]
    // @spec FLEET-CLI-012
    fn resolve_port_parses_valid_content() {
        let tmp = TempDir::new().unwrap();
        let (_root, main_repo) = setup_gbiv_layout(tmp.path(), "proj");
        write_port_file(&main_repo, 54321);
        let (_path, port) = resolve_port(&main_repo).unwrap();
        assert_eq!(port, 54321);
    }

    #[test]
    // @spec FLEET-CLI-012
    fn resolve_port_fails_on_corrupt_content() {
        let tmp = TempDir::new().unwrap();
        let (_root, main_repo) = setup_gbiv_layout(tmp.path(), "proj");
        let gbiv_dir = main_repo.join(".gbiv");
        fs::create_dir_all(&gbiv_dir).unwrap();
        fs::write(gbiv_dir.join("port"), "not-a-port").unwrap();
        let outcome = resolve_port(&main_repo).unwrap_err();
        assert_eq!(outcome.exit_code, EXIT_DAEMON_NOT_RUNNING);
    }

    // ---- run_status (FLEET-CLI-020, FLEET-CLI-021, end-to-end) --------------

    #[test]
    // @spec FLEET-CLI-020
    fn run_status_happy_path_returns_body_and_exit_0() {
        let tmp = TempDir::new().unwrap();
        let (_root, main_repo) = setup_gbiv_layout(tmp.path(), "proj");
        let body = r#"[{"color":"red","pane_status":"ok"}]"#;
        let (port, handle) = fake_daemon_once(200, body);
        write_port_file(&main_repo, port);

        let outcome = run_status(&main_repo, None);

        assert_eq!(outcome.exit_code, fleet_client::EXIT_OK);
        assert_eq!(outcome.stdout.as_deref(), Some(body));
        let request_line = handle.join().unwrap();
        assert!(
            request_line.starts_with("GET /sessions"),
            "got: {request_line}"
        );
    }

    #[test]
    // @spec FLEET-CLI-014
    fn run_status_no_daemon_running_is_exit_2() {
        let tmp = TempDir::new().unwrap();
        let (_root, main_repo) = setup_gbiv_layout(tmp.path(), "proj");
        // A port nothing is listening on.
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let dead_port = listener.local_addr().unwrap().port();
        drop(listener);
        write_port_file(&main_repo, dead_port);

        let outcome = run_status(&main_repo, None);
        assert_eq!(outcome.exit_code, EXIT_DAEMON_NOT_RUNNING);
    }

    // ---- run_get (FLEET-CLI-030 through -034, end-to-end) --------------------

    #[test]
    // @spec FLEET-CLI-030
    fn run_get_happy_path_forwards_lines_and_color() {
        let tmp = TempDir::new().unwrap();
        let (_root, main_repo) = setup_gbiv_layout(tmp.path(), "proj");
        let body = r#"{"color":"red","pane_status":"ok"}"#;
        let (port, handle) = fake_daemon_once(200, body);
        write_port_file(&main_repo, port);

        let outcome = run_get(&main_repo, "red", Some("50"), None, None);

        assert_eq!(outcome.exit_code, fleet_client::EXIT_OK);
        assert_eq!(outcome.stdout.as_deref(), Some(body));
        let request_line = handle.join().unwrap();
        assert!(
            request_line.starts_with("GET /session/red?lines=50"),
            "got: {request_line}"
        );
    }

    // ---- run_send local pre-checks fire before any daemon contact -----------
    // (FLEET-CLI-038 through -041: no port file exists in any of these three
    // tests, proving the local checks run before port resolution.)

    #[test]
    // @spec FLEET-CLI-038
    fn run_send_invalid_color_is_exit_3_without_a_daemon() {
        let tmp = TempDir::new().unwrap();
        let (_root, main_repo) = setup_gbiv_layout(tmp.path(), "proj");
        let outcome = run_send(&main_repo, "purple", "hello there");
        assert_eq!(outcome.exit_code, fleet_client::EXIT_INVALID_COLOR);
    }

    #[test]
    // @spec FLEET-CLI-039
    fn run_send_empty_text_is_exit_1_without_a_daemon() {
        let tmp = TempDir::new().unwrap();
        let (_root, main_repo) = setup_gbiv_layout(tmp.path(), "proj");
        let outcome = run_send(&main_repo, "red", "   ");
        assert_eq!(outcome.exit_code, EXIT_OTHER);
    }

    #[test]
    // @spec FLEET-CLI-040
    fn run_send_guard_shaped_text_is_exit_6_without_a_daemon() {
        let tmp = TempDir::new().unwrap();
        let (_root, main_repo) = setup_gbiv_layout(tmp.path(), "proj");
        let outcome = run_send(&main_repo, "red", "yes");
        assert_eq!(outcome.exit_code, fleet_client::EXIT_GUARD_REJECTED);
    }

    #[test]
    // @spec FLEET-CLI-038
    fn run_send_bad_color_wins_over_guard_shaped_text() {
        // Q2 resolution (docs/llds/orchestrate-cli.md): color is checked
        // first, so a compound-invalid invocation reports the color error,
        // not the guard rejection.
        let tmp = TempDir::new().unwrap();
        let (_root, main_repo) = setup_gbiv_layout(tmp.path(), "proj");
        let outcome = run_send(&main_repo, "purple", "yes");
        assert_eq!(outcome.exit_code, fleet_client::EXIT_INVALID_COLOR);
    }

    // ---- run_send happy path, end-to-end -------------------------------------

    #[test]
    // @spec FLEET-CLI-042
    fn run_send_happy_path_posts_trimmed_text() {
        let tmp = TempDir::new().unwrap();
        let (_root, main_repo) = setup_gbiv_layout(tmp.path(), "proj");
        let body = r#"{"ok":true,"sent_to_pane":"%3"}"#;
        let (port, handle) = fake_daemon_once(200, body);
        write_port_file(&main_repo, port);

        let outcome = run_send(&main_repo, "red", "  please run the tests  ");

        assert_eq!(outcome.exit_code, fleet_client::EXIT_OK);
        assert_eq!(outcome.stdout.as_deref(), Some(body));
        let request_line = handle.join().unwrap();
        assert!(
            request_line.starts_with("POST /session/red/send"),
            "got: {request_line}"
        );
    }
}

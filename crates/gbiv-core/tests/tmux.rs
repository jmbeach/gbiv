//! Integration tests for `gbiv_core::tmux` public surface.
//!
//! Pure tests run unconditionally; tests that need a real tmux installation
//! skip with a printed note when `tmux -V` is not available, so the suite
//! stays green on dev machines and CI runners that lack tmux.

use gbiv_core::tmux::{
    has_session, list_windows, session_name_for_root, tmux_available, TmuxError, WindowInfo,
};
use std::process::Command;

fn tmux_on_path() -> bool {
    Command::new("tmux")
        .arg("-V")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn unique_session_name(suffix: &str) -> String {
    let pid = std::process::id();
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("gbiv-core-test-{pid}-{nanos}-{suffix}")
}

fn kill_session(name: &str) {
    let _ = Command::new("tmux")
        .args(["kill-session", "-t", name])
        .output();
}

/// @spec TMX-CORE-001, TMX-CORE-002, TMX-CORE-003
/// Compile-time check: the public surface matches the LLD.
#[test]
fn public_surface_compiles() {
    // Functions exist with the documented signatures.
    let _: fn() -> Result<(), TmuxError> = tmux_available;
    let _: fn(&str) -> Result<bool, TmuxError> = has_session;
    let _: fn(&str) -> Result<Vec<WindowInfo>, TmuxError> = list_windows;
    let _: fn(&str) -> String = session_name_for_root;

    // WindowInfo exposes id and name as pub fields.
    let w = WindowInfo { id: "@1".into(), name: "main".into() };
    assert_eq!(w.id, "@1");
    assert_eq!(w.name, "main");

    // All five TmuxError variants exist and Display per the LLD.
    let v: Vec<TmuxError> = vec![
        TmuxError::NotInstalled,
        TmuxError::SessionNotFound("s".into()),
        TmuxError::PaneNotFound("p".into()),
        TmuxError::SendKeysIncomplete("p".into()),
        TmuxError::Other("o".into()),
    ];
    assert_eq!(v.len(), 5);
}

/// @spec TMX-CORE-003
#[test]
fn tmux_error_display_messages_match_lld() {
    assert_eq!(
        format!("{}", TmuxError::NotInstalled),
        "tmux binary not on PATH"
    );
    assert_eq!(
        format!("{}", TmuxError::SessionNotFound("red".into())),
        "tmux session not found: red"
    );
    assert_eq!(
        format!("{}", TmuxError::PaneNotFound("%3".into())),
        "tmux pane not found: %3"
    );
    assert_eq!(
        format!("{}", TmuxError::SendKeysIncomplete("%3".into())),
        "send-keys completed for text but Enter failed for pane %3"
    );
    assert_eq!(
        format!("{}", TmuxError::Other("boom".into())),
        "tmux: boom"
    );
}

/// @spec TMX-CORE-040, TMX-CORE-041, TMX-CORE-042
#[test]
fn session_name_for_root_is_pure_identity() {
    assert_eq!(session_name_for_root("alpha"), "alpha");
    assert_eq!(session_name_for_root(""), "");
    // No validation: tmux-illegal characters pass through.
    assert_eq!(session_name_for_root("a:b.c"), "a:b.c");
}

/// @spec TMX-CORE-010, TMX-CORE-016
#[test]
fn tmux_available_returns_ok_when_tmux_installed() {
    if !tmux_on_path() {
        eprintln!("skipping: tmux not on PATH");
        return;
    }
    tmux_available().expect("tmux_available should return Ok when tmux is installed");
}

/// @spec TMX-CORE-013
/// Verifies a `NotInstalled` reaches the caller when tmux is genuinely missing.
/// Implemented by running the test binary with `PATH=""` is impractical from
/// within the suite; instead this test is informational and only runs on
/// machines where tmux is *not* installed.
#[test]
fn tmux_available_returns_not_installed_when_missing() {
    if tmux_on_path() {
        eprintln!("skipping: tmux is on PATH; cannot exercise NotInstalled path here");
        return;
    }
    match tmux_available() {
        Err(TmuxError::NotInstalled) => {}
        other => panic!("expected Err(NotInstalled), got {other:?}"),
    }
}

/// @spec TMX-CORE-020, TMX-CORE-021
#[test]
fn has_session_distinguishes_present_and_missing() {
    if !tmux_on_path() {
        eprintln!("skipping: tmux not on PATH");
        return;
    }

    let name = unique_session_name("has");
    // Missing → Ok(false)
    assert_eq!(
        has_session(&name).expect("has_session should succeed"),
        false,
        "expected Ok(false) for a session that does not exist"
    );

    // Create, recheck, then clean up.
    let create = Command::new("tmux")
        .args(["new-session", "-d", "-s", &name])
        .status()
        .expect("spawn tmux new-session");
    if !create.success() {
        eprintln!("skipping: could not create transient tmux session (tmux server unavailable?)");
        return;
    }

    let present = has_session(&name);
    kill_session(&name);
    assert_eq!(
        present.expect("has_session should succeed for existing session"),
        true,
        "expected Ok(true) for the session we just created"
    );
}

/// @spec TMX-CORE-030, TMX-CORE-031, TMX-CORE-033
#[test]
fn list_windows_against_real_session() {
    if !tmux_on_path() {
        eprintln!("skipping: tmux not on PATH");
        return;
    }

    let name = unique_session_name("list");

    // Missing session → SessionNotFound(name)
    match list_windows(&name) {
        Err(TmuxError::SessionNotFound(got)) => assert_eq!(got, name),
        other => panic!("expected SessionNotFound({name}), got {other:?}"),
    }

    // Create with two named windows.
    let created = Command::new("tmux")
        .args(["new-session", "-d", "-s", &name, "-n", "first"])
        .status()
        .expect("spawn tmux new-session");
    if !created.success() {
        eprintln!("skipping: could not create transient tmux session");
        return;
    }
    let _ = Command::new("tmux")
        .args(["new-window", "-t", &name, "-n", "second"])
        .status();

    let windows = list_windows(&name);
    kill_session(&name);

    let windows = windows.expect("list_windows should succeed for existing session");
    let names: Vec<&str> = windows.iter().map(|w| w.name.as_str()).collect();
    assert!(
        names.contains(&"first") && names.contains(&"second"),
        "expected windows 'first' and 'second', got {names:?}"
    );
    // Each id should look like @<digits>.
    for w in &windows {
        assert!(
            w.id.starts_with('@') && w.id.len() > 1,
            "window id should be in @<n> form, got {:?}",
            w.id
        );
    }
}

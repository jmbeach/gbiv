//! gbv-1ht tests (Phase 5).
//! Extension runtime — engine, discovery and loading, events, sandbox and the
//! `api_version` 1 host API, against `Runtime::load`, the four event methods
//! and `take_messages`.
//!
//! Not covered here (command-level, see the `gbiv start` / `gbiv done` tests):
//! - EXT-EVT-012 (`after_*` fire only after the success line is printed)
//! - EXT-LOAD-009 (only `start` / `done` load extensions)

use std::fs;
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;
use std::time::{Duration, Instant};

use super::test_support::{config, config_with, fast_limits, layout, manifest, Layout};
use super::{ExtensionError, ExtensionSettings, ExtensionsConfig, Limits, Runtime};

// ---- helpers ----------------------------------------------------------------

fn load_with(l: &Layout, cfg: &ExtensionsConfig, limits: Limits) -> Runtime {
    match Runtime::load(&l.paths(), cfg, limits) {
        Ok(rt) => rt,
        Err(e) => panic!("expected extensions to load, got: {e}"),
    }
}

fn load(l: &Layout, cfg: &ExtensionsConfig) -> Runtime {
    load_with(l, cfg, fast_limits())
}

fn load_err(l: &Layout, cfg: &ExtensionsConfig) -> ExtensionError {
    match Runtime::load(&l.paths(), cfg, fast_limits()) {
        Ok(_) => panic!("expected loading to fail"),
        Err(e) => e,
    }
}

/// One project extension `x` with no capabilities or grants, loaded.
fn single(init_lua: &str) -> (Layout, Runtime) {
    let l = layout();
    l.project_ext("x", &[], init_lua);
    let rt = load(&l, &config(&["x"]));
    (l, rt)
}

/// Assert the result is a veto by `name`; return its message.
fn veto_message<T: std::fmt::Debug>(r: Result<T, ExtensionError>, name: &str) -> String {
    match r {
        Err(ExtensionError::Veto { name: n, message }) => {
            assert_eq!(n, name, "veto should be attributed to {name}");
            message
        }
        other => panic!("expected a veto by {name}, got {other:?}"),
    }
}

/// Run `f` on a worker thread and fail (rather than hang) if it takes longer
/// than `limit`. A panic in `f` is re-raised here.
fn within<T: Send + 'static>(limit: Duration, f: impl FnOnce() -> T + Send + 'static) -> T {
    let (tx, rx) = std::sync::mpsc::channel();
    let handle = thread::spawn(move || {
        let _ = tx.send(f());
    });
    match rx.recv_timeout(limit) {
        Ok(v) => v,
        Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
            panic!("did not finish within {limit:?}")
        }
        Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => match handle.join() {
            Err(payload) => std::panic::resume_unwind(payload),
            Ok(()) => panic!("worker exited without a result"),
        },
    }
}

/// Lua helpers available at the top of every `run_body` extension.
const PRELUDE: &str = r#"
local function try(label, f, ...)
  local ok = pcall(f, ...)
  gbiv.log(label .. "=" .. tostring(ok))
end
"#;

/// Load extension `x` with `caps` and `settings`, whose `before_done` runs
/// `body` (with `try` from PRELUDE in scope); fire `before_done`.
fn run_body(
    l: &Layout,
    caps: &[&str],
    settings: ExtensionSettings,
    body: &str,
) -> (Result<(), ExtensionError>, Vec<String>) {
    let init = format!("{PRELUDE}\nreturn {{ before_done = function(ctx)\n{body}\nend }}\n");
    l.project_ext("x", caps, &init);
    let mut rt = load(l, &config_with("x", settings));
    let _ = rt.take_messages();
    let r = rt.before_done("red", "task");
    let msgs = rt.take_messages();
    (r, msgs)
}

/// `run_body` that must not veto; returns the messages.
fn run_ok(l: &Layout, caps: &[&str], settings: ExtensionSettings, body: &str) -> Vec<String> {
    let (r, msgs) = run_body(l, caps, settings, body);
    if let Err(e) = r {
        panic!("handler should not veto, got: {e}\nmessages: {msgs:?}");
    }
    msgs
}

/// A Lua string literal for `s`.
fn lua_str(s: &str) -> String {
    format!("{s:?}")
}

fn lua_path(p: &Path) -> String {
    lua_str(p.to_str().expect("utf-8 path"))
}

fn paths_grant(paths: Vec<PathBuf>) -> ExtensionSettings {
    ExtensionSettings {
        paths,
        ..Default::default()
    }
}

/// `<main_repo>/backlog`, created, and a settings value granting it.
fn backlog_grant(l: &Layout) -> (PathBuf, ExtensionSettings) {
    let backlog = l.main_repo.join("backlog");
    fs::create_dir_all(&backlog).unwrap();
    (backlog.clone(), paths_grant(vec![backlog]))
}

fn git(dir: &Path, args: &[&str]) -> std::process::Output {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .output()
        .expect("run git");
    out
}

fn git_ok(dir: &Path, args: &[&str]) {
    let out = git(dir, args);
    assert!(
        out.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

/// `git init` `dir` with a local identity and one commit.
fn init_repo(dir: &Path) {
    fs::create_dir_all(dir).unwrap();
    git_ok(dir, &["init", "-q"]);
    git_ok(dir, &["config", "user.name", "Test"]);
    git_ok(dir, &["config", "user.email", "test@example.com"]);
    fs::write(dir.join("README.md"), "hi\n").unwrap();
    git_ok(dir, &["add", "README.md"]);
    git_ok(dir, &["commit", "-q", "-m", "initial"]);
}

// ---- Engine -----------------------------------------------------------------

// @spec EXT-ENG-001
#[test]
fn extensions_run_on_lua_5_4() {
    let (_l, mut rt) = single(
        r#"return { before_start = function(ctx)
  gbiv.log(_VERSION)
  gbiv.log(math.type(7 // 2))
  gbiv.log(type(utf8.char))
end }"#,
    );
    assert_eq!(rt.before_start("fix it", "red").unwrap(), None);
    assert_eq!(rt.take_messages(), vec!["x: Lua 5.4", "x: integer", "x: function"]);
}

// @spec EXT-ENG-002
#[test]
fn each_extension_has_its_own_globals() {
    let l = layout();
    l.project_ext(
        "a",
        &[],
        r#"leaked = "from-a-init"
string.upper = nil
return { before_start = function(ctx)
  shared = "from-a-handler"
  gbiv.log = function() end
end }"#,
    );
    l.project_ext(
        "b",
        &[],
        r#"return { before_start = function(ctx)
  gbiv.log(tostring(leaked) .. "," .. tostring(shared) .. "," .. string.upper("ok"))
end }"#,
    );
    let mut rt = load(&l, &config(&["a", "b"]));
    assert_eq!(rt.before_start("t", "red").unwrap(), None);
    assert_eq!(rt.take_messages(), vec!["b: nil,nil,OK"]);
}

// @spec EXT-ENG-003
#[test]
fn state_lives_for_one_invocation_only() {
    let l = layout();
    l.project_ext(
        "x",
        &[],
        r#"local count = 0
return { after_start = function(ctx)
  count = count + 1
  calls = (calls or 0) + 1
  gbiv.log(count .. "/" .. calls)
end }"#,
    );
    let cfg = config(&["x"]);
    {
        let mut first = load(&l, &cfg);
        first.after_start("red", "t");
        first.after_start("red", "t");
        assert_eq!(first.take_messages(), vec!["x: 1/1", "x: 2/2"]);
    }
    let mut second = load(&l, &cfg);
    second.after_start("red", "t");
    assert_eq!(second.take_messages(), vec!["x: 1/1"]);
}

// ---- Discovery and loading --------------------------------------------------

const LOG_WHERE: &str = r#"return { before_start = function(ctx) gbiv.log(WHERE) end }"#;

fn where_init(label: &str) -> String {
    format!("WHERE = {}\n{LOG_WHERE}", lua_str(label))
}

// @spec EXT-LOAD-001
#[test]
fn project_directory_wins_over_user_directory() {
    let l = layout();
    l.write_ext(&l.project_ext, "x", &manifest("x", &[]), &where_init("project"));
    l.write_ext(&l.user_ext, "x", &manifest("x", &[]), &where_init("user"));
    let mut rt = load(&l, &config(&["x"]));
    assert_eq!(rt.names(), vec!["x".to_string()]);
    rt.before_start("t", "red").unwrap();
    assert_eq!(rt.take_messages(), vec!["x: project"]);
}

// @spec EXT-LOAD-001
#[test]
fn user_directory_is_used_when_project_has_none() {
    let l = layout();
    l.write_ext(&l.user_ext, "x", &manifest("x", &[]), &where_init("user"));
    let mut rt = load(&l, &config(&["x"]));
    rt.before_start("t", "red").unwrap();
    assert_eq!(rt.take_messages(), vec!["x: user"]);
}

// @spec EXT-LOAD-002
#[test]
fn directory_names_match_case_sensitively() {
    let l = layout();
    l.write_ext(&l.project_ext, "Vat", &manifest("Vat", &[]), "return {}");
    l.write_ext(&l.user_ext, "VAT", &manifest("VAT", &[]), "return {}");
    match load_err(&l, &config(&["vat"])) {
        ExtensionError::NotFound { name, .. } => assert_eq!(name, "vat"),
        other => panic!("expected NotFound, got {other:?}"),
    }
}

// @spec EXT-LOAD-003
#[test]
fn missing_extension_names_both_paths_and_the_install_command() {
    let l = layout();
    let err = load_err(&l, &config(&["ghost"]));
    let text = err.to_string();
    match &err {
        ExtensionError::NotFound { name, searched } => {
            assert_eq!(name, "ghost");
            assert_eq!(
                searched,
                &vec![l.project_ext.join("ghost"), l.user_ext.join("ghost")]
            );
        }
        other => panic!("expected NotFound, got {other:?}"),
    }
    assert!(text.contains("ghost"), "{text}");
    assert!(text.contains(l.project_ext.join("ghost").to_str().unwrap()), "{text}");
    assert!(text.contains(l.user_ext.join("ghost").to_str().unwrap()), "{text}");
    assert!(text.contains("gbiv install-extension ghost"), "{text}");
}

// @spec EXT-LOAD-004
#[test]
fn missing_manifest_in_project_copy_does_not_fall_back_to_user_copy() {
    let l = layout();
    let proj = l.project_ext.join("x");
    fs::create_dir_all(&proj).unwrap();
    fs::write(proj.join("init.lua"), "return {}").unwrap();
    l.write_ext(&l.user_ext, "x", &manifest("x", &[]), "return {}");
    let err = load_err(&l, &config(&["x"]));
    let text = err.to_string();
    match err {
        ExtensionError::BadManifest { name, path, .. } => {
            assert_eq!(name, "x");
            assert_eq!(path, proj.join("extension.toml"));
        }
        other => panic!("expected BadManifest, got {other:?}"),
    }
    assert!(text.contains(proj.join("extension.toml").to_str().unwrap()), "{text}");
}

// @spec EXT-LOAD-004
#[test]
fn unparseable_manifest_is_an_error_naming_the_file() {
    let l = layout();
    let proj = l.write_ext(&l.project_ext, "x", "name = \n[[[", "return {}");
    l.write_ext(&l.user_ext, "x", &manifest("x", &[]), "return {}");
    match load_err(&l, &config(&["x"])) {
        ExtensionError::BadManifest { path, .. } => assert_eq!(path, proj.join("extension.toml")),
        other => panic!("expected BadManifest, got {other:?}"),
    }
}

fn assert_bad_manifest(dir_name: &str, manifest_src: &str) {
    let l = layout();
    let ext = l.write_ext(&l.project_ext, dir_name, manifest_src, "return {}");
    match Runtime::load(&l.paths(), &config(&[dir_name]), fast_limits()) {
        Err(ExtensionError::BadManifest { path, .. }) => {
            assert_eq!(path, ext.join("extension.toml"), "manifest:\n{manifest_src}")
        }
        Err(other) => panic!("expected BadManifest for:\n{manifest_src}\ngot {other:?}"),
        Ok(_) => panic!("expected BadManifest for:\n{manifest_src}\nbut it loaded"),
    }
}

// @spec EXT-LOAD-005
#[test]
fn manifest_requires_name_version_and_integer_api_version() {
    // name differs from the directory
    assert_bad_manifest("x", "name = \"y\"\nversion = \"0.1.0\"\napi_version = 1\n");
    // name missing
    assert_bad_manifest("x", "version = \"0.1.0\"\napi_version = 1\n");
    // name outside [A-Za-z0-9_-]+
    assert_bad_manifest("bad.name", "name = \"bad.name\"\nversion = \"0.1.0\"\napi_version = 1\n");
    assert_bad_manifest("bad name", "name = \"bad name\"\nversion = \"0.1.0\"\napi_version = 1\n");
    // version missing
    assert_bad_manifest("x", "name = \"x\"\napi_version = 1\n");
    // api_version missing or not an integer
    assert_bad_manifest("x", "name = \"x\"\nversion = \"0.1.0\"\n");
    assert_bad_manifest("x", "name = \"x\"\nversion = \"0.1.0\"\napi_version = \"1\"\n");
    assert_bad_manifest("x", "name = \"x\"\nversion = \"0.1.0\"\napi_version = 1.5\n");
}

// @spec EXT-LOAD-005
#[test]
fn capabilities_must_be_a_subset_of_the_four_groups() {
    assert_bad_manifest(
        "x",
        "name = \"x\"\nversion = \"0.1.0\"\napi_version = 1\ncapabilities = [\"fs\", \"net\"]\n",
    );
    assert_bad_manifest(
        "x",
        "name = \"x\"\nversion = \"0.1.0\"\napi_version = 1\ncapabilities = \"fs\"\n",
    );
}

// @spec EXT-LOAD-005
#[test]
fn description_and_capabilities_are_optional() {
    let l = layout();
    l.write_ext(
        &l.project_ext,
        "min_ext-2",
        "name = \"min_ext-2\"\nversion = \"0.1.0\"\napi_version = 1\n",
        "return {}",
    );
    l.write_ext(
        &l.project_ext,
        "full",
        "name = \"full\"\nversion = \"2.0.0\"\napi_version = 1\n\
         description = \"everything\"\ncapabilities = [\"fs\", \"git\", \"http\", \"env\"]\n",
        "return {}",
    );
    let rt = load(&l, &config(&["min_ext-2", "full"]));
    assert_eq!(rt.names(), vec!["min_ext-2".to_string(), "full".to_string()]);
}

// @spec EXT-LOAD-006
#[test]
fn unsupported_api_version_fails_naming_extension_and_versions() {
    for wanted in [0_i64, 2, 99] {
        let l = layout();
        l.write_ext(
            &l.project_ext,
            "x",
            &format!("name = \"x\"\nversion = \"0.1.0\"\napi_version = {wanted}\n"),
            "return {}",
        );
        let err = load_err(&l, &config(&["x"]));
        let text = err.to_string();
        match err {
            ExtensionError::ApiVersion {
                name,
                wanted: w,
                supported,
            } => {
                assert_eq!(name, "x");
                assert_eq!(w, wanted);
                assert_eq!(supported, vec![1]);
            }
            other => panic!("expected ApiVersion, got {other:?}"),
        }
        assert!(text.contains("\"x\""), "{text}");
        assert!(text.contains(&wanted.to_string()), "{text}");
        assert!(text.contains("[1]"), "{text}");
    }
}

fn assert_load_error(init_lua: &str) -> String {
    let l = layout();
    l.project_ext("x", &[], init_lua);
    match load_err(&l, &config(&["x"])) {
        ExtensionError::Load { name, detail } => {
            assert_eq!(name, "x");
            detail
        }
        other => panic!("expected Load error for init.lua:\n{init_lua}\ngot {other:?}"),
    }
}

// @spec EXT-LOAD-007
#[test]
fn init_lua_must_return_a_table() {
    for init in [
        "return nil",
        "return 42",
        "return \"before_start\"",
        "return true",
        "return function() end",
        "local x = 1",
    ] {
        assert_load_error(init);
    }
}

// @spec EXT-LOAD-007
#[test]
fn hook_table_rejects_unknown_keys_and_non_functions() {
    let detail = assert_load_error("return { before_strat = function(ctx) end }");
    assert!(detail.contains("before_strat"), "detail should name the bad key: {detail}");
    assert_load_error("return { before_start = 5 }");
    assert_load_error("return { after_done = \"later\" }");
    assert_load_error("return { function() end }");
    assert_load_error("return { before_start = function() end, extra = function() end }");
}

// @spec EXT-LOAD-007
#[test]
fn init_lua_errors_and_syntax_errors_fail_loading() {
    assert_load_error("error(\"init exploded\")");
    assert_load_error("return { before_start = function(ctx) end");
}

// @spec EXT-LOAD-007
#[test]
fn any_subset_of_the_four_hooks_loads() {
    let l = layout();
    l.project_ext("none", &[], "return {}");
    l.project_ext(
        "all",
        &[],
        r#"return {
  before_start = function(ctx) end,
  after_start = function(ctx) end,
  before_done = function(ctx) end,
  after_done = function(ctx) end,
}"#,
    );
    l.project_ext("some", &[], "return { after_done = function(ctx) end }");
    let rt = load(&l, &config(&["none", "all", "some"]));
    assert_eq!(
        rt.names(),
        vec!["none".to_string(), "all".to_string(), "some".to_string()]
    );
}

// @spec EXT-LOAD-008
#[test]
fn a_later_load_failure_aborts_before_any_handler_can_run() {
    let l = layout();
    l.project_ext(
        "good",
        &[],
        "return { before_start = function(ctx) gbiv.log(\"ran\") end }",
    );
    l.project_ext("broken", &[], "return 7");
    match Runtime::load(&l.paths(), &config(&["good", "broken", "absent"]), fast_limits()) {
        Err(ExtensionError::Load { name, .. }) => assert_eq!(name, "broken"),
        Err(other) => panic!("expected the broken extension's Load error, got {other:?}"),
        Ok(_) => panic!("a load failure must abort the whole runtime"),
    }
}

// @spec EXT-LOAD-010
#[test]
fn require_loads_modules_from_the_extension_directory() {
    let l = layout();
    let ext = l.project_ext(
        "x",
        &[],
        r#"local util = require("lib.util")
local top = require("top")
return { before_start = function(ctx)
  gbiv.log(util.name .. "," .. top.name .. "," .. require("lib.util").name)
end }"#,
    );
    fs::create_dir_all(ext.join("lib")).unwrap();
    fs::write(ext.join("lib/util.lua"), "return { name = \"util\" }").unwrap();
    fs::write(ext.join("top.lua"), "return { name = \"top\" }").unwrap();
    let mut rt = load(&l, &config(&["x"]));
    rt.before_start("t", "red").unwrap();
    assert_eq!(rt.take_messages(), vec!["x: util,top,util"]);
}

// @spec EXT-LOAD-010
#[test]
fn require_refuses_missing_files_and_paths_outside_the_extension() {
    let l = layout();
    // Modules outside the extension directory that must never load.
    fs::write(l.root.join("outside.lua"), "return \"LEAKED\"").unwrap();
    fs::write(l.project_ext.join("sibling.lua"), "return \"LEAKED\"").unwrap();
    let outdir = l.root.join("outdir");
    fs::create_dir_all(&outdir).unwrap();
    fs::write(outdir.join("mod.lua"), "return \"LEAKED\"").unwrap();

    let abs = l.root.join("outside");
    let body = format!(
        r#"try("missing", require, "nope")
try("dotdot", require, "../sibling")
try("dotdot2", require, "..sibling")
try("slashdotdot", require, "lib/../../sibling")
try("absolute", require, {abs})
try("symlink_file", require, "evil")
try("symlink_dir", require, "sub.mod")
"#,
        abs = lua_path(&abs)
    );
    let init = format!("{PRELUDE}\nreturn {{ before_done = function(ctx)\n{body}\nend }}\n");
    let ext = l.project_ext("x", &[], &init);
    std::os::unix::fs::symlink(l.root.join("outside.lua"), ext.join("evil.lua")).unwrap();
    std::os::unix::fs::symlink(&outdir, ext.join("sub")).unwrap();

    let mut rt = load(&l, &config(&["x"]));
    rt.before_done("red", "t").unwrap();
    assert_eq!(
        rt.take_messages(),
        vec![
            "x: missing=false",
            "x: dotdot=false",
            "x: dotdot2=false",
            "x: slashdotdot=false",
            "x: absolute=false",
            "x: symlink_file=false",
            "x: symlink_dir=false",
        ]
    );
}

// @spec EXT-LOAD-010
#[test]
fn require_of_a_missing_module_in_init_fails_loading() {
    assert_load_error("local m = require(\"not_there\")\nreturn {}");
}

// ---- Configuration warnings -------------------------------------------------

// @spec EXT-CFG-008
#[test]
fn granting_an_undeclared_capability_warns_and_continues() {
    let l = layout();
    l.project_ext("x", &[], "return {}");
    let settings = ExtensionSettings {
        paths: vec![l.main_repo.clone()],
        http: true,
        env: vec!["HOME".to_string()],
        ..Default::default()
    };
    let mut rt = load(&l, &config_with("x", settings));
    let msgs = rt.take_messages();
    let warns: Vec<&String> = msgs.iter().filter(|m| m.starts_with("x: warning: ")).collect();
    assert!(
        warns.iter().any(|m| m.contains("http")),
        "expected an http grant warning: {msgs:?}"
    );
    assert!(
        warns.iter().any(|m| m.contains("env")),
        "expected an env grant warning: {msgs:?}"
    );
    assert!(
        warns.iter().any(|m| m.contains("paths") || m.contains("fs")),
        "expected a paths grant warning: {msgs:?}"
    );
}

// @spec EXT-CFG-008
#[test]
fn grants_matching_declared_capabilities_do_not_warn() {
    let l = layout();
    l.project_ext("x", &["fs", "git", "http", "env"], "return {}");
    let settings = ExtensionSettings {
        paths: vec![l.main_repo.clone()],
        http: true,
        env: vec!["HOME".to_string()],
        ..Default::default()
    };
    let mut rt = load(&l, &config_with("x", settings));
    assert_eq!(rt.take_messages(), Vec::<String>::new());
}

// ---- Events -------------------------------------------------------------------

/// An extension logging `<event>` from every hook except `skip`.
fn all_hooks_except(skip: &str) -> String {
    let mut s = String::from("local h = {}\n");
    for ev in ["before_start", "after_start", "before_done", "after_done"] {
        if ev != skip {
            s.push_str(&format!(
                "h.{ev} = function(ctx) gbiv.log(\"{ev}\") end\n"
            ));
        }
    }
    s.push_str("return h\n");
    s
}

// @spec EXT-EVT-001
#[test]
fn handlers_run_in_effective_list_order_skipping_missing_hooks() {
    let l = layout();
    l.project_ext("zeta", &[], &all_hooks_except("after_start"));
    l.project_ext("alpha", &[], &all_hooks_except("before_start"));
    l.project_ext("mid", &[], &all_hooks_except("before_done"));
    let mut rt = load(&l, &config(&["zeta", "alpha", "mid"]));
    assert_eq!(rt.names(), vec!["zeta", "alpha", "mid"]);

    assert_eq!(rt.before_start("t", "red").unwrap(), None);
    assert_eq!(rt.take_messages(), vec!["zeta: before_start", "mid: before_start"]);

    rt.after_start("red", "t");
    assert_eq!(rt.take_messages(), vec!["alpha: after_start", "mid: after_start"]);

    rt.before_done("red", "t").unwrap();
    assert_eq!(rt.take_messages(), vec!["zeta: before_done", "alpha: before_done"]);

    rt.after_done("red", "t");
    assert_eq!(
        rt.take_messages(),
        vec!["zeta: after_done", "alpha: after_done", "mid: after_done"]
    );
}

// @spec EXT-EVT-001
#[test]
fn no_extensions_means_events_are_no_ops() {
    let l = layout();
    let mut rt = load(&l, &ExtensionsConfig::default());
    assert!(rt.names().is_empty());
    assert_eq!(rt.before_start("t", "red").unwrap(), None);
    rt.after_start("red", "t");
    rt.before_done("red", "t").unwrap();
    rt.after_done("red", "t");
    assert!(rt.take_messages().is_empty());
}

// @spec EXT-EVT-002
#[test]
fn before_start_ctx_has_arg_color_text_and_options() {
    let l = layout();
    l.project_ext(
        "a",
        &[],
        r#"return { before_start = function(ctx)
  gbiv.log("arg=<" .. ctx.arg .. ">")
  gbiv.log("color=" .. ctx.color)
  gbiv.log("text=" .. tostring(ctx.text))
  gbiv.log("opt=" .. tostring(ctx.options.backlog))
  return { text = "[gbv-1] Named entry" }
end }"#,
    );
    l.project_ext(
        "b",
        &[],
        r#"return { before_start = function(ctx)
  gbiv.log("arg=<" .. ctx.arg .. ">")
  gbiv.log("text=" .. tostring(ctx.text))
  gbiv.log("options=" .. type(ctx.options))
end }"#,
    );
    let mut cfg = config(&["a", "b"]);
    cfg.settings.get_mut("a").unwrap().options =
        "backlog = \"my-backlog\"\n".parse::<toml::Table>().unwrap();
    let mut rt = load(&l, &cfg);
    let arg = "  Fix: the [weird]  bug ";
    assert_eq!(
        rt.before_start(arg, "green").unwrap(),
        Some("[gbv-1] Named entry".to_string())
    );
    assert_eq!(
        rt.take_messages(),
        vec![
            "a: arg=<  Fix: the [weird]  bug >",
            "a: color=green",
            "a: text=nil",
            "a: opt=my-backlog",
            "b: arg=<  Fix: the [weird]  bug >",
            "b: text=[gbv-1] Named entry",
            "b: options=table",
        ]
    );
}

// @spec EXT-CFG-005, EXT-EVT-002
#[test]
fn options_are_converted_from_toml_to_lua_values() {
    let l = layout();
    l.project_ext(
        "x",
        &[],
        r#"return { before_start = function(ctx)
  local o = ctx.options
  gbiv.log(o.s .. "|" .. math.type(o.i) .. "|" .. math.type(o.f) .. "|" .. tostring(o.b))
  gbiv.log(#o.list .. "|" .. o.list[1] .. "|" .. o.list[2])
  gbiv.log(o.nested.inner .. "|" .. tostring(o.nested.deep.n))
end }"#,
    );
    let options = "s = \"str\"\ni = 3\nf = 1.5\nb = true\nlist = [\"one\", \"two\"]\n\
                   [nested]\ninner = \"v\"\n[nested.deep]\nn = 9\n"
        .parse::<toml::Table>()
        .unwrap();
    let mut rt = load(
        &l,
        &config_with(
            "x",
            ExtensionSettings {
                options,
                ..Default::default()
            },
        ),
    );
    rt.before_start("t", "red").unwrap();
    assert_eq!(
        rt.take_messages(),
        vec!["x: str|integer|float|true", "x: 2|one|two", "x: v|9"]
    );
}

// @spec EXT-EVT-003
#[test]
fn other_events_get_color_text_and_options() {
    let l = layout();
    let log_ctx = |ev: &str| {
        format!(
            "{ev} = function(ctx) gbiv.log(\"{ev} \" .. ctx.color .. \" <\" .. ctx.text .. \"> \" .. tostring(ctx.options.k)) end,\n"
        )
    };
    let init = format!(
        "return {{\n{}{}{}}}\n",
        log_ctx("after_start"),
        log_ctx("before_done"),
        log_ctx("after_done")
    );
    l.project_ext("x", &[], &init);
    let mut cfg = config(&["x"]);
    cfg.settings.get_mut("x").unwrap().options = "k = \"v\"\n".parse::<toml::Table>().unwrap();
    let mut rt = load(&l, &cfg);

    rt.after_start("blue", "[gbv-1ht] [blocked-by:gbv-x2v] Title");
    rt.before_done("indigo", "Plain text").unwrap();
    rt.after_done("violet", "[gbv-9] Done thing");
    assert_eq!(
        rt.take_messages(),
        vec![
            "x: after_start blue <[gbv-1ht] [blocked-by:gbv-x2v] Title> v",
            "x: before_done indigo <Plain text> v",
            "x: after_done violet <[gbv-9] Done thing> v",
        ]
    );
}

/// Three extensions; `b`'s `<event>` handler runs `b_body`.
fn three_with_middle(event: &str, b_body: &str) -> (Layout, Runtime) {
    let l = layout();
    let logger = format!("return {{ {event} = function(ctx) gbiv.log(\"ran\") end }}");
    l.project_ext("a", &[], &logger);
    l.project_ext(
        "b",
        &[],
        &format!("return {{ {event} = function(ctx)\n{b_body}\nend }}"),
    );
    l.project_ext("c", &[], &logger);
    let rt = load(&l, &config(&["a", "b", "c"]));
    (l, rt)
}

// @spec EXT-EVT-004
#[test]
fn before_start_error_vetoes_and_stops_later_handlers() {
    let (_l, mut rt) = three_with_middle("before_start", "error(\"task is claimed by sam\")");
    let r = rt.before_start("t", "red");
    let err_text = match &r {
        Err(e) => e.to_string(),
        Ok(_) => String::new(),
    };
    assert_eq!(veto_message(r, "b"), "task is claimed by sam");
    assert_eq!(err_text, "b: task is claimed by sam");
    assert_eq!(rt.take_messages(), vec!["a: ran"]);
}

// @spec EXT-EVT-004
#[test]
fn before_done_error_vetoes_and_stops_later_handlers() {
    let (_l, mut rt) = three_with_middle("before_done", "error(\"not finished\")");
    assert_eq!(veto_message(rt.before_done("red", "t"), "b"), "not finished");
    assert_eq!(rt.take_messages(), vec!["a: ran"]);
}

// @spec EXT-EVT-004
#[test]
fn runtime_fault_in_a_before_handler_is_a_veto() {
    let (_l, mut rt) = three_with_middle("before_start", "local t = nil\nreturn t.field");
    let msg = veto_message(rt.before_start("t", "red"), "b");
    assert!(msg.contains("attempt to index"), "{msg}");
    assert_eq!(rt.take_messages(), vec!["a: ran"]);

    let (_l, mut rt) = three_with_middle("before_done", "return #nil_global_value");
    let msg = veto_message(rt.before_done("red", "t"), "b");
    assert!(msg.contains("attempt to get length"), "{msg}");
    assert_eq!(rt.take_messages(), vec!["a: ran"]);
}

fn veto_text_for(error_expr: &str) -> String {
    let l = layout();
    let ext = l.project_ext(
        "x",
        &[],
        &format!("local lib = require(\"lib\")\nreturn {{ before_done = function(ctx)\n{error_expr}\nend }}"),
    );
    fs::write(
        ext.join("lib.lua"),
        "local M = {}\nfunction M.fail()\n  error(\"deep failure\")\nend\nreturn M\n",
    )
    .unwrap();
    let mut rt = load(&l, &config(&["x"]));
    veto_message(rt.before_done("red", "t"), "x")
}

// @spec EXT-EVT-005
#[test]
fn veto_messages_strip_position_and_stringify_values() {
    assert_eq!(veto_text_for("error(\"boom\")"), "boom");
    assert_eq!(veto_text_for("error(\"boom\", 0)"), "boom");
    assert_eq!(veto_text_for("error(\"boom\", 2)"), "boom");
    assert_eq!(veto_text_for("lib.fail()"), "deep failure");
    assert_eq!(veto_text_for("error(42)"), "42");
    assert_eq!(veto_text_for("error(true)"), "true");
    assert_eq!(
        veto_text_for("error(setmetatable({}, { __tostring = function() return \"custom\" end }))"),
        "custom"
    );
    let table_msg = veto_text_for("error({ code = 1 })");
    assert!(table_msg.starts_with("table: "), "{table_msg}");
}

// @spec EXT-EVT-005
#[test]
fn nil_error_prints_no_message() {
    assert_eq!(veto_text_for("error(nil)"), "(no message)");
    assert_eq!(veto_text_for("error()"), "(no message)");
    let l = layout();
    l.project_ext("x", &[], "return { before_start = function(ctx) error() end }");
    let mut rt = load(&l, &config(&["x"]));
    let err = rt.before_start("t", "red").unwrap_err();
    assert_eq!(err.to_string(), "x: (no message)");
}

// @spec EXT-EVT-006
#[test]
fn a_veto_sends_nothing_back_to_handlers_that_already_ran() {
    let l = layout();
    l.project_ext(
        "claimer",
        &[],
        r#"return {
  before_start = function(ctx) gbiv.log("claimed") end,
  after_start = function(ctx) gbiv.log("after_start") end,
  after_done = function(ctx) gbiv.log("after_done") end,
}"#,
    );
    l.project_ext("vetoer", &[], "return { before_start = function(ctx) error(\"no\") end }");
    let mut rt = load(&l, &config(&["claimer", "vetoer"]));
    veto_message(rt.before_start("t", "red"), "vetoer");
    assert_eq!(rt.take_messages(), vec!["claimer: claimed"]);
    // And there is no hook name a rollback could be delivered to.
    assert_load_error("return { start_aborted = function(ctx) end }");
}

// @spec EXT-EVT-007
#[test]
fn nil_or_false_names_nothing_and_continues() {
    let l = layout();
    l.project_ext("a", &[], "return { before_start = function(ctx) gbiv.log(\"a\") return nil end }");
    l.project_ext("b", &[], "return { before_start = function(ctx) gbiv.log(\"b\") return false end }");
    l.project_ext("c", &[], "return { before_start = function(ctx) gbiv.log(\"c\") end }");
    let mut rt = load(&l, &config(&["a", "b", "c"]));
    assert_eq!(rt.before_start("t", "red").unwrap(), None);
    assert_eq!(rt.take_messages(), vec!["a: a", "b: b", "c: c"]);

    let l = layout();
    l.project_ext("a", &[], "return { before_start = function(ctx) return false end }");
    l.project_ext("b", &[], "return { before_start = function(ctx) return { text = \"B\" } end }");
    let mut rt = load(&l, &config(&["a", "b"]));
    assert_eq!(rt.before_start("t", "red").unwrap(), Some("B".to_string()));
}

// @spec EXT-EVT-008
#[test]
fn a_text_table_names_the_entry_trimmed_ignoring_other_fields() {
    let (_l, mut rt) = single(
        r#"return { before_start = function(ctx)
  return { text = "  [gbv-1ht] [blocked-by:gbv-x2v] Title \t ", id = "gbv-1ht", extra = {} }
end }"#,
    );
    assert_eq!(
        rt.before_start("gbv-1ht", "red").unwrap(),
        Some("[gbv-1ht] [blocked-by:gbv-x2v] Title".to_string())
    );
    assert!(rt.take_messages().is_empty());

    let (_l, mut rt) = single("return { before_start = function(ctx) return { text = \"Title\\n\" } end }");
    assert_eq!(rt.before_start("t", "red").unwrap(), Some("Title".to_string()));
}

// @spec EXT-EVT-009
#[test]
fn first_naming_wins_and_later_naming_warns() {
    let l = layout();
    l.project_ext("a", &[], "return { before_start = function(ctx) return { text = \"A\" } end }");
    l.project_ext(
        "b",
        &[],
        "return { before_start = function(ctx) gbiv.log(\"saw \" .. ctx.text) return { text = \"B\" } end }",
    );
    l.project_ext("c", &[], "return { before_start = function(ctx) gbiv.log(\"saw \" .. ctx.text) end }");
    let mut rt = load(&l, &config(&["a", "b", "c"]));
    assert_eq!(rt.before_start("t", "red").unwrap(), Some("A".to_string()));
    let msgs = rt.take_messages();
    assert_eq!(msgs.len(), 3, "{msgs:?}");
    assert_eq!(msgs[0], "b: saw A");
    assert!(msgs[1].starts_with("b: warning: "), "{msgs:?}");
    assert_eq!(msgs[2], "c: saw A");
}

// @spec EXT-EVT-010
#[test]
fn invalid_return_values_are_vetoes() {
    for ret in [
        "42",
        "\"just a string\"",
        "true",
        "function() end",
        "{}",
        "{ name = \"x\" }",
        "{ text = 5 }",
        "{ text = \"   \" }",
        "{ text = \"\" }",
        "{ text = \"line one\\nline two\" }",
        "{ text = \"[done] old entry\" }",
        "{ text = \"  [in-progress] old entry\" }",
    ] {
        let l = layout();
        l.project_ext(
            "a",
            &[],
            &format!("return {{ before_start = function(ctx) return {ret} end }}"),
        );
        l.project_ext("b", &[], "return { before_start = function(ctx) gbiv.log(\"ran\") end }");
        let mut rt = load(&l, &config(&["a", "b"]));
        let r = rt.before_start("t", "red");
        match r {
            Err(ExtensionError::Veto { name, .. }) => assert_eq!(name, "a", "return {ret}"),
            other => panic!("return {ret} should be a veto by a, got {other:?}"),
        }
        assert!(rt.take_messages().is_empty(), "return {ret}: b must not run");
    }
}

// @spec EXT-EVT-010
#[test]
fn an_invalid_return_after_naming_is_still_a_veto() {
    let l = layout();
    l.project_ext("a", &[], "return { before_start = function(ctx) return { text = \"A\" } end }");
    l.project_ext("b", &[], "return { before_start = function(ctx) return { text = \"[done] x\" } end }");
    let mut rt = load(&l, &config(&["a", "b"]));
    veto_message(rt.before_start("t", "red"), "b");
}

// @spec EXT-EVT-011
#[test]
fn after_handler_errors_become_warnings_and_later_handlers_run() {
    for event in ["after_start", "after_done"] {
        let l = layout();
        l.project_ext(
            "a",
            &[],
            &format!("return {{ {event} = function(ctx) error(\"late failure\") end }}"),
        );
        l.project_ext(
            "b",
            &[],
            &format!("return {{ {event} = function(ctx) local t = nil; return t.x end }}"),
        );
        l.project_ext(
            "c",
            &[],
            &format!("return {{ {event} = function(ctx) gbiv.log(\"ran\") end }}"),
        );
        let mut rt = load(&l, &config(&["a", "b", "c"]));
        if event == "after_start" {
            rt.after_start("red", "t");
        } else {
            rt.after_done("red", "t");
        }
        let msgs = rt.take_messages();
        assert_eq!(msgs.len(), 3, "{event}: {msgs:?}");
        assert!(
            msgs[0].starts_with("a: ") && msgs[0].ends_with("late failure"),
            "{event}: {msgs:?}"
        );
        assert!(
            msgs[1].starts_with("b: ") && msgs[1].contains("attempt to index"),
            "{event}: {msgs:?}"
        );
        assert_eq!(msgs[2], "c: ran", "{event}");
    }
}

// EXT-EVT-012 is command-level ordering (success line before after_*): covered
// by the `gbiv start` / `gbiv done` command tests, not testable on Runtime.

// ---- Sandbox ------------------------------------------------------------------

// @spec EXT-SBX-001
#[test]
fn only_the_safe_standard_libraries_are_open() {
    let (_l, mut rt) = single(
        r#"return { before_start = function(ctx)
  gbiv.log(type(string) .. "," .. type(table) .. "," .. type(math) .. "," .. type(utf8) .. "," .. type(coroutine))
  local keys = {}
  for k in pairs(os) do keys[#keys + 1] = k end
  table.sort(keys)
  gbiv.log("os=" .. table.concat(keys, ","))
  gbiv.log(math.type(os.time()) .. "," .. type(os.clock()) .. "," .. type(os.date("%Y")))
  gbiv.log(tostring(os.execute) .. "," .. tostring(os.getenv) .. "," .. tostring(os.remove) .. "," .. tostring(os.exit))
end }"#,
    );
    rt.before_start("t", "red").unwrap();
    assert_eq!(
        rt.take_messages(),
        vec![
            "x: table,table,table,table,table",
            "x: os=clock,date,time",
            "x: integer,number,string",
            "x: nil,nil,nil,nil",
        ]
    );
}

// @spec EXT-SBX-002
#[test]
fn dangerous_globals_are_absent_and_require_is_gbivs_loader() {
    let (_l, mut rt) = single(
        r#"return { before_start = function(ctx)
  for _, name in ipairs({ "io", "debug", "package", "load", "loadfile", "dofile", "collectgarbage" }) do
    gbiv.log(name .. "=" .. tostring(_G[name]))
  end
  gbiv.log("require=" .. type(require))
  gbiv.log("require_io=" .. tostring((pcall(require, "io"))))
  gbiv.log("require_os=" .. tostring((pcall(require, "os"))))
end }"#,
    );
    rt.before_start("t", "red").unwrap();
    assert_eq!(
        rt.take_messages(),
        vec![
            "x: io=nil",
            "x: debug=nil",
            "x: package=nil",
            "x: load=nil",
            "x: loadfile=nil",
            "x: dofile=nil",
            "x: collectgarbage=nil",
            "x: require=function",
            "x: require_io=false",
            "x: require_os=false",
        ]
    );
}

// @spec EXT-SBX-003, EXT-EVT-004
#[test]
fn exceeding_the_memory_limit_vetoes() {
    let l = layout();
    l.project_ext(
        "x",
        &[],
        r#"return { before_start = function(ctx)
  local t = {}
  for i = 1, 1000 do t[i] = string.rep("x", 1024 * 1024) .. i end
  gbiv.log("allocated " .. #t)
end }"#,
    );
    let mut rt = load_with(&l, &config(&["x"]), Limits::default());
    veto_message(rt.before_start("t", "red"), "x");
    assert!(rt.take_messages().is_empty());
}

// @spec EXT-SBX-003
#[test]
fn memory_limit_is_64_mib_and_raises_a_catchable_lua_error() {
    let (_l, mut rt) = {
        let l = layout();
        l.project_ext(
            "x",
            &[],
            r#"return { before_done = function(ctx)
  local t = {}
  for i = 1, 40 do t[i] = string.rep("x", 1024 * 1024) .. i end
  gbiv.log("40MiB=" .. #t)
  t = nil
  local ok = pcall(string.rep, "y", 80 * 1024 * 1024)
  gbiv.log("80MiB=" .. tostring(ok))
end }"#,
        );
        let rt = load_with(&l, &config(&["x"]), Limits::default());
        (l, rt)
    };
    rt.before_done("red", "t").unwrap();
    assert_eq!(rt.take_messages(), vec!["x: 40MiB=40", "x: 80MiB=false"]);
}

// @spec EXT-SBX-004, EXT-EVT-004
#[test]
fn a_runaway_handler_is_stopped_at_the_time_limit() {
    let (r, msgs, took) = within(Duration::from_secs(20), || {
        let (_l, mut rt) = single("return { before_start = function(ctx) while true do end end }");
        let started = Instant::now();
        let r = rt.before_start("t", "red");
        (r, rt.take_messages(), started.elapsed())
    });
    veto_message(r, "x");
    assert!(msgs.is_empty(), "{msgs:?}");
    assert!(took >= Duration::from_millis(1500), "stopped too early: {took:?}");
    assert!(took < Duration::from_secs(15), "not stopped in time: {took:?}");
}

// @spec EXT-SBX-004, EXT-EVT-011
#[test]
fn a_runaway_after_handler_becomes_a_warning() {
    let msgs = within(Duration::from_secs(20), || {
        let l = layout();
        l.project_ext("a", &[], "return { after_done = function(ctx) while true do end end }");
        l.project_ext("b", &[], "return { after_done = function(ctx) gbiv.log(\"ran\") end }");
        let mut rt = load(&l, &config(&["a", "b"]));
        rt.after_done("red", "t");
        rt.take_messages()
    });
    assert_eq!(msgs.len(), 2, "{msgs:?}");
    assert!(msgs[0].starts_with("a: "), "{msgs:?}");
    assert_eq!(msgs[1], "b: ran");
}

// @spec EXT-SBX-004
#[test]
fn time_in_host_functions_counts_toward_the_limit() {
    // 1.5 s asleep in the host plus 1 s of Lua exceeds the 2 s budget.
    let (r, msgs) = within(Duration::from_secs(20), || {
        let (_l, mut rt) = single(
            r#"return { before_start = function(ctx)
  gbiv.sleep(1.5)
  local t = os.clock()
  while os.clock() - t < 1.0 do end
  gbiv.log("finished")
end }"#,
        );
        let r = rt.before_start("t", "red");
        (r, rt.take_messages())
    });
    veto_message(r, "x");
    assert!(msgs.is_empty(), "{msgs:?}");
}

// @spec EXT-SBX-004
#[test]
fn the_time_limit_is_per_handler_call() {
    let (r, msgs) = within(Duration::from_secs(20), || {
        let l = layout();
        let slow = "return { before_start = function(ctx) gbiv.sleep(1.2) gbiv.log(\"done\") end }";
        l.project_ext("a", &[], slow);
        l.project_ext("b", &[], slow);
        let mut rt = load(&l, &config(&["a", "b"]));
        let r = rt.before_start("t", "red");
        (r, rt.take_messages())
    });
    assert_eq!(r.unwrap(), None);
    assert_eq!(msgs, vec!["a: done", "b: done"]);
}

// @spec EXT-SBX-005
#[test]
fn sleep_is_bounded_by_the_remaining_budget() {
    let (r, msgs, took) = within(Duration::from_secs(20), || {
        let (_l, mut rt) =
            single("return { before_start = function(ctx) gbiv.sleep(10) gbiv.log(\"woke\") end }");
        let started = Instant::now();
        let r = rt.before_start("t", "red");
        (r, rt.take_messages(), started.elapsed())
    });
    veto_message(r, "x");
    assert!(took < Duration::from_secs(6), "sleep outlived the 2 s budget: {took:?}");
    assert!(msgs.is_empty(), "{msgs:?}");
}

// @spec EXT-SBX-005
#[test]
fn a_hanging_git_child_is_killed_at_the_deadline() {
    // Accepts TCP connections (kernel backlog) but never answers.
    let silent = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://127.0.0.1:{}/repo.git", silent.local_addr().unwrap().port());
    let (r, msgs, took) = within(Duration::from_secs(30), move || {
        let l = layout();
        let (backlog, settings) = backlog_grant(&l);
        init_repo(&backlog);
        let body = format!(
            "gbiv.git({dir}, {{ \"ls-remote\", {url} }})\ngbiv.log(\"returned\")",
            dir = lua_path(&backlog),
            url = lua_str(&url)
        );
        let started = Instant::now();
        let (r, msgs) = run_body(&l, &["git"], settings, &body);
        (r, msgs, started.elapsed())
    });
    veto_message(r, "x");
    assert!(took < Duration::from_secs(10), "git was not stopped at the deadline: {took:?}");
    assert!(msgs.is_empty(), "{msgs:?}");
    drop(silent);
}

// @spec EXT-SBX-005
#[test]
fn a_hanging_http_request_is_bounded_by_the_budget() {
    let silent = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://127.0.0.1:{}/slow", silent.local_addr().unwrap().port());
    let (r, msgs, took) = within(Duration::from_secs(30), move || {
        let l = layout();
        let body = format!(
            "gbiv.http({{ method = \"GET\", url = {} }})\ngbiv.log(\"returned\")",
            lua_str(&url)
        );
        let started = Instant::now();
        let (r, msgs) = run_body(&l, &["http"], http_grant(), &body);
        (r, msgs, started.elapsed())
    });
    veto_message(r, "x");
    assert!(took < Duration::from_secs(10), "http was not bounded by the budget: {took:?}");
    assert!(msgs.is_empty(), "{msgs:?}");
    drop(silent);
}

// ---- Host API: always present ---------------------------------------------

// @spec EXT-API-001
#[test]
fn gbiv_table_has_the_always_present_members() {
    let (_l, mut rt) = single(
        r#"return { before_start = function(ctx)
  gbiv.log(type(gbiv) .. "," .. type(gbiv.api_version) .. "," .. tostring(gbiv.api_version) .. "," .. tostring(gbiv.api_version == 1))
  gbiv.log(type(gbiv.log) .. "," .. type(gbiv.warn) .. "," .. type(gbiv.sleep) .. "," .. type(gbiv.toml) .. "," .. type(gbiv.toml.decode))
end }"#,
    );
    rt.before_start("t", "red").unwrap();
    assert_eq!(
        rt.take_messages(),
        vec![
            "x: table,number,1,true",
            "x: function,function,function,table,function",
        ]
    );
}

// @spec EXT-API-002
#[test]
fn log_and_warn_are_prefixed_with_the_extension_name_in_order() {
    let l = layout();
    l.project_ext(
        "alpha",
        &[],
        "return { before_done = function(ctx) gbiv.log(\"hello\") gbiv.warn(\"careful\") gbiv.log(\"again\") end }",
    );
    l.project_ext("beta", &[], "return { before_done = function(ctx) gbiv.warn(\"second\") end }");
    let mut rt = load(&l, &config(&["alpha", "beta"]));
    rt.before_done("red", "t").unwrap();
    assert_eq!(
        rt.take_messages(),
        vec![
            "alpha: hello",
            "alpha: warning: careful",
            "alpha: again",
            "beta: warning: second",
        ]
    );
    assert!(rt.take_messages().is_empty(), "take_messages drains");
}

// @spec EXT-API-003
#[test]
fn sleep_sleeps_for_short_durations() {
    let (_l, mut rt) = single("return { before_start = function(ctx) gbiv.sleep(0.3) gbiv.log(\"woke\") end }");
    let started = Instant::now();
    rt.before_start("t", "red").unwrap();
    assert!(started.elapsed() >= Duration::from_millis(250));
    assert_eq!(rt.take_messages(), vec!["x: woke"]);
}

// @spec EXT-API-003
#[test]
fn sleep_is_capped_at_ten_seconds_per_call() {
    let (r, msgs, took) = within(Duration::from_secs(30), || {
        let l = layout();
        l.project_ext(
            "x",
            &[],
            "return { before_start = function(ctx) gbiv.sleep(1000) gbiv.log(\"woke\") end }",
        );
        let mut rt = load_with(&l, &config(&["x"]), Limits::default());
        let started = Instant::now();
        let r = rt.before_start("t", "red");
        (r, rt.take_messages(), started.elapsed())
    });
    assert_eq!(r.unwrap(), None);
    assert!(took >= Duration::from_millis(9500), "{took:?}");
    assert!(took < Duration::from_secs(20), "sleep was not capped: {took:?}");
    assert_eq!(msgs, vec!["x: woke"]);
}

// @spec EXT-API-004
#[test]
fn toml_decode_parses_and_raises_on_invalid_input() {
    let (_l, mut rt) = single(
        r#"return { before_start = function(ctx)
  local t = gbiv.toml.decode('name = "vat"\ncount = 2\n[user]\nids = ["a", "b"]\n')
  gbiv.log(t.name .. "," .. math.type(t.count) .. "," .. t.user.ids[2])
  local ok, err = pcall(gbiv.toml.decode, "this is = = not toml")
  gbiv.log(tostring(ok) .. "," .. type(err))
end }"#,
    );
    rt.before_start("t", "red").unwrap();
    assert_eq!(rt.take_messages(), vec!["x: vat,integer,b", "x: false,string"]);
}

// ---- Host API: capability gating -------------------------------------------

const PRESENCE: &str = r#"gbiv.log(type(gbiv.fs) .. "," .. type(gbiv.git) .. "," .. type(gbiv.http) .. "," .. type(gbiv.env))"#;

fn presence(caps: &[&str], grant_paths: bool, http: bool, env: &[&str]) -> String {
    let l = layout();
    let settings = ExtensionSettings {
        paths: if grant_paths { vec![l.main_repo.clone()] } else { vec![] },
        http,
        env: env.iter().map(|s| s.to_string()).collect(),
        ..Default::default()
    };
    let msgs = run_ok(&l, caps, settings, PRESENCE);
    assert_eq!(msgs.len(), 1, "{msgs:?}");
    msgs[0].strip_prefix("x: ").expect("prefixed").to_string()
}

// @spec EXT-API-005
#[test]
fn capability_functions_need_both_declaration_and_grant() {
    let all = ["fs", "git", "http", "env"];
    assert_eq!(presence(&[], true, true, &["HOME"]), "nil,nil,nil,nil");
    assert_eq!(presence(&all, false, false, &[]), "nil,nil,nil,nil");
    assert_eq!(presence(&["fs"], true, false, &[]), "table,nil,nil,nil");
    assert_eq!(presence(&["git"], true, false, &[]), "nil,function,nil,nil");
    assert_eq!(presence(&["http"], false, true, &[]), "nil,nil,function,nil");
    assert_eq!(presence(&["env"], false, false, &["HOME"]), "nil,nil,nil,function");
    assert_eq!(presence(&all, true, true, &["HOME"]), "table,function,function,function");
}

// ---- Host API: fs -------------------------------------------------------------

// @spec EXT-API-006
#[test]
fn fs_read_write_remove_exists_list() {
    let l = layout();
    let (backlog, settings) = backlog_grant(&l);
    let msgs = run_ok(
        &l,
        &["fs"],
        settings,
        r#"local fs = gbiv.fs
fs.write("backlog/a.txt", "alpha")
gbiv.log("read=" .. fs.read("backlog/a.txt"))
gbiv.log("missing=" .. tostring(fs.read("backlog/nope.txt")))
fs.write("backlog/deep/er/b.txt", "beta")
gbiv.log("exists=" .. tostring(fs.exists("backlog/deep/er/b.txt")) .. "," .. tostring(fs.exists("backlog/nope.txt")) .. "," .. tostring(fs.exists("backlog/deep")))
local names = fs.list("backlog")
table.sort(names)
gbiv.log("list=" .. table.concat(names, ","))
gbiv.log("listmissing=" .. tostring(fs.list("backlog/nope")))
fs.remove("backlog/a.txt")
fs.remove("backlog/a.txt")
gbiv.log("after=" .. tostring(fs.exists("backlog/a.txt")))"#,
    );
    assert_eq!(
        msgs,
        vec![
            "x: read=alpha",
            "x: missing=nil",
            "x: exists=true,false,true",
            "x: list=a.txt,deep",
            "x: listmissing=nil",
            "x: after=false",
        ]
    );
    assert_eq!(fs::read_to_string(backlog.join("deep/er/b.txt")).unwrap(), "beta");
    assert!(!backlog.join("a.txt").exists());
}

// @spec EXT-API-007
#[test]
fn fs_paths_resolve_relative_tilde_and_absolute() {
    let l = layout();
    let backlog = l.main_repo.join("backlog");
    let vat_cfg = l.home.join(".config/vat");
    fs::create_dir_all(&backlog).unwrap();
    fs::create_dir_all(&vat_cfg).unwrap();
    fs::write(backlog.join("abs.txt"), "absolute").unwrap();
    let body = format!(
        r#"gbiv.fs.write("backlog/rel.txt", "relative")
gbiv.fs.write("~/.config/vat/config.toml", "tilde")
gbiv.log(gbiv.fs.read({abs}))
gbiv.fs.write("backlog/sub/../in.txt", "dotdot inside")
gbiv.fs.write("backlog/not/yet/made.txt", "deep")"#,
        abs = lua_path(&backlog.join("abs.txt"))
    );
    let msgs = run_ok(&l, &["fs"], paths_grant(vec![backlog.clone(), vat_cfg.clone()]), &body);
    assert_eq!(msgs, vec!["x: absolute"]);
    assert_eq!(fs::read_to_string(backlog.join("rel.txt")).unwrap(), "relative");
    assert_eq!(fs::read_to_string(vat_cfg.join("config.toml")).unwrap(), "tilde");
    assert_eq!(fs::read_to_string(backlog.join("in.txt")).unwrap(), "dotdot inside");
    assert_eq!(fs::read_to_string(backlog.join("not/yet/made.txt")).unwrap(), "deep");
}

// @spec EXT-API-007
#[test]
fn fs_refuses_paths_outside_the_grants() {
    let l = layout();
    let (backlog, settings) = backlog_grant(&l);
    fs::write(l.main_repo.join("secret.txt"), "secret").unwrap();
    fs::create_dir_all(l.main_repo.join("backlog2")).unwrap();
    let outdir = l.root.join("outdir");
    fs::create_dir_all(&outdir).unwrap();
    fs::write(outdir.join("f.txt"), "outside").unwrap();
    std::os::unix::fs::symlink(&outdir, backlog.join("link")).unwrap();
    let body = format!(
        r#"try("dotdot_read", gbiv.fs.read, "backlog/../secret.txt")
try("dotdot_write", gbiv.fs.write, "backlog/../escape.txt", "x")
try("sibling_prefix", gbiv.fs.write, "backlog2/x.txt", "x")
try("absolute_outside", gbiv.fs.read, {outside})
try("tilde_outside", gbiv.fs.exists, "~/anything")
try("symlink_read", gbiv.fs.read, "backlog/link/f.txt")
try("symlink_list", gbiv.fs.list, "backlog/link")
try("symlink_write", gbiv.fs.write, "backlog/link/new.txt", "x")
try("symlink_remove", gbiv.fs.remove, "backlog/link/f.txt")
try("inside", gbiv.fs.read, "backlog/link_does_not_exist.txt")"#,
        outside = lua_path(&outdir.join("f.txt"))
    );
    let msgs = run_ok(&l, &["fs"], settings, &body);
    assert_eq!(
        msgs,
        vec![
            "x: dotdot_read=false",
            "x: dotdot_write=false",
            "x: sibling_prefix=false",
            "x: absolute_outside=false",
            "x: tilde_outside=false",
            "x: symlink_read=false",
            "x: symlink_list=false",
            "x: symlink_write=false",
            "x: symlink_remove=false",
            "x: inside=true",
        ]
    );
    assert!(!l.main_repo.join("escape.txt").exists());
    assert!(!l.main_repo.join("backlog2/x.txt").exists());
    assert!(!outdir.join("new.txt").exists());
    assert!(outdir.join("f.txt").exists());
}

// @spec EXT-API-007
#[test]
fn grants_are_canonicalized_and_rechecked_when_missing_at_load() {
    let l = layout();
    let backlog = l.main_repo.join("backlog");
    fs::create_dir_all(&backlog).unwrap();
    fs::write(backlog.join("a.txt"), "via canonical grant").unwrap();
    let glink = l.main_repo.join("glink");
    std::os::unix::fs::symlink(&backlog, &glink).unwrap();
    let later = l.main_repo.join("later");

    let init = format!(
        "{PRELUDE}\nreturn {{ before_done = function(ctx)\n\
         gbiv.log(gbiv.fs.read(\"backlog/a.txt\"))\n\
         gbiv.fs.write(\"later/x.txt\", \"created later\")\n\
         end }}\n"
    );
    l.project_ext("x", &["fs"], &init);
    let mut rt = load(&l, &config_with("x", paths_grant(vec![glink, later.clone()])));
    // The grant `later` did not exist at load; it is created before the event.
    fs::create_dir_all(&later).unwrap();
    let _ = rt.take_messages();
    rt.before_done("red", "t").unwrap();
    assert_eq!(rt.take_messages(), vec!["x: via canonical grant"]);
    assert_eq!(fs::read_to_string(later.join("x.txt")).unwrap(), "created later");
}

// @spec EXT-API-008
#[test]
fn fs_write_refuses_symlinks_pointing_outside() {
    let l = layout();
    let (backlog, settings) = backlog_grant(&l);
    let target = l.root.join("target.txt");
    fs::write(&target, "orig").unwrap();
    std::os::unix::fs::symlink(&target, backlog.join("out.txt")).unwrap();
    let dangling_target = l.root.join("would-be-created.txt");
    std::os::unix::fs::symlink(&dangling_target, backlog.join("dangle.txt")).unwrap();
    let msgs = run_ok(
        &l,
        &["fs"],
        settings,
        r#"try("existing_target", gbiv.fs.write, "backlog/out.txt", "pwned")
try("dangling_target", gbiv.fs.write, "backlog/dangle.txt", "pwned")"#,
    );
    assert_eq!(msgs, vec!["x: existing_target=false", "x: dangling_target=false"]);
    assert_eq!(fs::read_to_string(&target).unwrap(), "orig");
    assert!(!dangling_target.exists(), "write followed a dangling symlink outside");
}

// @spec EXT-API-009
#[test]
fn fs_write_replaces_the_file_by_rename() {
    let l = layout();
    let (backlog, settings) = backlog_grant(&l);
    fs::write(backlog.join("a.txt"), "old").unwrap();
    fs::hard_link(backlog.join("a.txt"), backlog.join("b.txt")).unwrap();
    let msgs = run_ok(&l, &["fs"], settings, r#"gbiv.fs.write("backlog/a.txt", "new")"#);
    assert!(msgs.is_empty(), "{msgs:?}");
    assert_eq!(fs::read_to_string(backlog.join("a.txt")).unwrap(), "new");
    // A rename gives a.txt a new inode; the old one (still b.txt) is untouched.
    assert_eq!(fs::read_to_string(backlog.join("b.txt")).unwrap(), "old");
    let mut entries: Vec<String> = fs::read_dir(&backlog)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    entries.sort();
    assert_eq!(entries, vec!["a.txt", "b.txt"], "no temporary file left behind");
}

// @spec EXT-API-010
#[test]
fn fs_wrong_kinds_and_io_errors_raise() {
    let l = layout();
    let (backlog, settings) = backlog_grant(&l);
    fs::create_dir_all(backlog.join("dir")).unwrap();
    fs::write(backlog.join("file.txt"), "f").unwrap();
    let msgs = run_ok(
        &l,
        &["fs"],
        settings,
        r#"try("read_dir", gbiv.fs.read, "backlog/dir")
try("list_file", gbiv.fs.list, "backlog/file.txt")
try("write_under_file", gbiv.fs.write, "backlog/file.txt/child.txt", "x")
try("write_over_dir", gbiv.fs.write, "backlog/dir", "x")"#,
    );
    assert_eq!(
        msgs,
        vec![
            "x: read_dir=false",
            "x: list_file=false",
            "x: write_under_file=false",
            "x: write_over_dir=false",
        ]
    );
    assert!(backlog.join("dir").is_dir());
}

// ---- Host API: git ------------------------------------------------------------

/// A granted repo at `<main_repo>/backlog` with one commit.
fn git_fixture() -> (Layout, PathBuf, ExtensionSettings) {
    let l = layout();
    let (backlog, settings) = backlog_grant(&l);
    init_repo(&backlog);
    (l, backlog, settings)
}

// @spec EXT-API-011
#[test]
fn git_runs_without_a_shell_and_returns_code_stdout_stderr() {
    let (l, backlog, settings) = git_fixture();
    let body = format!(
        r#"local r = gbiv.git("backlog", {{ "rev-parse", "--is-inside-work-tree" }})
gbiv.log("ok code=" .. tostring(r.code) .. " out=" .. (r.stdout:gsub("\n", "")) .. " err=" .. #r.stderr)
local bad = gbiv.git({abs}, {{ "rev-parse", "--verify", "no-such-ref" }})
gbiv.log("bad code=" .. tostring(bad.code) .. " out=" .. #bad.stdout .. " err=" .. tostring(#bad.stderr > 0))
gbiv.git("backlog", {{ "config", "--local", "test.key", "$(touch PWNED); echo hi" }})
local v = gbiv.git("backlog", {{ "config", "--get", "test.key" }})
gbiv.log("literal=" .. v.stdout)"#,
        abs = lua_path(&backlog)
    );
    let msgs = run_ok(&l, &["git"], settings, &body);
    assert_eq!(
        msgs,
        vec![
            "x: ok code=0 out=true err=0",
            "x: bad code=128 out=0 err=true",
            "x: literal=$(touch PWNED); echo hi\n",
        ]
    );
    assert!(!backlog.join("PWNED").exists());
    assert!(!l.main_repo.join("PWNED").exists());
}

// @spec EXT-API-012
#[test]
fn git_refuses_dirs_outside_the_grants() {
    let (l, _backlog, settings) = git_fixture();
    let other = l.root.join("other");
    init_repo(&other);
    let body = format!(
        r#"try("outside", gbiv.git, {other}, {{ "config", "--local", "gbiv.touched", "yes" }})
try("dotdot", gbiv.git, "backlog/..", {{ "status" }})"#,
        other = lua_path(&other)
    );
    let msgs = run_ok(&l, &["git"], settings, &body);
    assert_eq!(msgs, vec!["x: outside=false", "x: dotdot=false"]);
    assert!(!git(&other, &["config", "--get", "gbiv.touched"]).status.success());
}

// @spec EXT-API-012
#[test]
fn git_refuses_options_before_the_subcommand() {
    let (l, _backlog, settings) = git_fixture();
    let other = l.root.join("other");
    init_repo(&other);
    let marker = l.root.join("PWNED");
    let body = format!(
        r#"try("dash_c", gbiv.git, "backlog", {{ "-c", "alias.pwn=!touch " .. {marker}, "pwn" }})
try("git_dir", gbiv.git, "backlog", {{ "--git-dir=" .. {other_git}, "config", "gbiv.touched", "yes" }})
try("dash_C", gbiv.git, "backlog", {{ "-C", {other}, "config", "gbiv.touched", "yes" }})
try("work_tree", gbiv.git, "backlog", {{ "--work-tree", {other}, "status" }})
try("exec_path", gbiv.git, "backlog", {{ "--exec-path=/tmp", "status" }})
try("after_subcommand", gbiv.git, "backlog", {{ "log", "--oneline", "-n", "1" }})"#,
        marker = lua_path(&marker),
        other_git = lua_path(&other.join(".git")),
        other = lua_path(&other)
    );
    let msgs = run_ok(&l, &["git"], settings, &body);
    assert_eq!(
        msgs,
        vec![
            "x: dash_c=false",
            "x: git_dir=false",
            "x: dash_C=false",
            "x: work_tree=false",
            "x: exec_path=false",
            "x: after_subcommand=true",
        ]
    );
    assert!(!marker.exists(), "git ran an injected alias");
    assert!(!git(&other, &["config", "--get", "gbiv.touched"]).status.success());
}

// @spec EXT-API-013
#[test]
fn git_runs_with_stdin_closed_and_a_fixed_environment() {
    let (l, backlog, settings) = git_fixture();
    git_ok(&backlog, &["config", "alias.showenv", "!env"]);
    git_ok(&backlog, &["config", "alias.readin", "!cat"]);
    let msgs = run_ok(
        &l,
        &["git"],
        settings,
        r#"local h = gbiv.git("backlog", { "hash-object", "--stdin" })
gbiv.log("hash=" .. h.stdout)
local c = gbiv.git("backlog", { "readin" })
gbiv.log("cat=" .. c.code .. ":" .. c.stdout)
local e = gbiv.git("backlog", { "showenv" })
gbiv.log("env=" .. e.stdout)"#,
    );
    assert_eq!(msgs.len(), 3, "{msgs:?}");
    // The empty blob: nothing was read from stdin.
    assert_eq!(msgs[0], "x: hash=e69de29bb2d1d6434b8b29ae775ad8c2e48c5391\n");
    assert_eq!(msgs[1], "x: cat=0:");
    let env_lines: Vec<&str> = msgs[2].lines().collect();
    assert!(env_lines.contains(&"GIT_TERMINAL_PROMPT=0"), "{env_lines:?}");
    assert!(env_lines.contains(&"LC_ALL=C"), "{env_lines:?}");
}

// ---- Host API: http -----------------------------------------------------------

struct Seen {
    method: String,
    url: String,
    x_test: Option<String>,
    body: String,
}

/// A local server answering `n` requests: `/ok` 200 (header `X-Reply: pong`,
/// body echoing the `X-Test` header), `/echo` 201 echoing the body, `/missing`
/// 404, `/boom` 500.
fn http_server(n: usize) -> (String, thread::JoinHandle<Vec<Seen>>) {
    let server = tiny_http::Server::http("127.0.0.1:0").expect("bind");
    let port = server.server_addr().to_ip().expect("ip").port();
    let handle = thread::spawn(move || {
        let mut seen = Vec::new();
        for _ in 0..n {
            let Ok(Some(mut req)) = server.recv_timeout(Duration::from_secs(30)) else {
                break;
            };
            let mut body = String::new();
            let _ = req.as_reader().read_to_string(&mut body);
            let x_test = req
                .headers()
                .iter()
                .find(|h| h.field.equiv("X-Test"))
                .map(|h| h.value.as_str().to_string());
            let url = req.url().to_string();
            let (status, text) = match url.as_str() {
                "/ok" => (200, format!("hello xtest={}", x_test.clone().unwrap_or_default())),
                "/echo" => (201, body.clone()),
                "/missing" => (404, "nope".to_string()),
                _ => (500, "boom".to_string()),
            };
            seen.push(Seen {
                method: req.method().as_str().to_string(),
                url,
                x_test,
                body,
            });
            let header = tiny_http::Header::from_bytes(&b"X-Reply"[..], &b"pong"[..]).unwrap();
            let _ = req.respond(
                tiny_http::Response::from_string(text)
                    .with_status_code(status)
                    .with_header(header),
            );
        }
        seen
    });
    (format!("http://127.0.0.1:{port}"), handle)
}

fn http_grant() -> ExtensionSettings {
    ExtensionSettings {
        http: true,
        ..Default::default()
    }
}

// @spec EXT-API-014
#[test]
fn http_performs_requests_and_returns_status_headers_body() {
    let l = layout();
    let (base, server) = http_server(2);
    let body = format!(
        r#"local base = {base}
local r = gbiv.http({{ method = "GET", url = base .. "/ok", headers = {{ ["X-Test"] = "abc" }} }})
local reply
for k, v in pairs(r.headers) do if k:lower() == "x-reply" then reply = v end end
gbiv.log("get=" .. r.status .. ":" .. tostring(reply) .. ":" .. r.body)
local p = gbiv.http({{ method = "POST", url = base .. "/echo", body = "payload" }})
gbiv.log("post=" .. p.status .. ":" .. p.body)"#,
        base = lua_str(&base)
    );
    let msgs = run_ok(&l, &["http"], http_grant(), &body);
    assert_eq!(msgs, vec!["x: get=200:pong:hello xtest=abc", "x: post=201:payload"]);
    let seen = server.join().unwrap();
    assert_eq!(seen.len(), 2);
    assert_eq!((seen[0].method.as_str(), seen[0].url.as_str()), ("GET", "/ok"));
    assert_eq!(seen[0].x_test.as_deref(), Some("abc"));
    assert_eq!((seen[1].method.as_str(), seen[1].body.as_str()), ("POST", "payload"));
}

// @spec EXT-API-014
#[test]
fn http_error_statuses_are_returned_not_raised() {
    let l = layout();
    let (base, server) = http_server(2);
    let body = format!(
        r#"local base = {base}
local m = gbiv.http({{ method = "GET", url = base .. "/missing" }})
gbiv.log("missing=" .. m.status .. ":" .. m.body)
local b = gbiv.http({{ method = "GET", url = base .. "/boom" }})
gbiv.log("boom=" .. b.status .. ":" .. b.body)"#,
        base = lua_str(&base)
    );
    let msgs = run_ok(&l, &["http"], http_grant(), &body);
    assert_eq!(msgs, vec!["x: missing=404:nope", "x: boom=500:boom"]);
    server.join().unwrap();
}

// @spec EXT-API-014
#[test]
fn http_transport_failure_raises() {
    let l = layout();
    let port = {
        let probe = TcpListener::bind("127.0.0.1:0").unwrap();
        probe.local_addr().unwrap().port()
    };
    let body = format!(
        r#"try("refused", gbiv.http, {{ method = "GET", url = {url} }})
try("bad_url", gbiv.http, {{ method = "GET", url = "not a url" }})"#,
        url = lua_str(&format!("http://127.0.0.1:{port}/x"))
    );
    let msgs = run_ok(&l, &["http"], http_grant(), &body);
    assert_eq!(msgs, vec!["x: refused=false", "x: bad_url=false"]);
}

// ---- Host API: env ------------------------------------------------------------

// @spec EXT-API-015
#[test]
fn env_reads_only_granted_variables() {
    let set = "GBIV_EXT_RT_TEST_SET_7Q4Z";
    let unset = "GBIV_EXT_RT_TEST_UNSET_7Q4Z";
    std::env::set_var(set, "hello env");
    std::env::remove_var(unset);
    let l = layout();
    let settings = ExtensionSettings {
        env: vec![set.to_string(), unset.to_string()],
        ..Default::default()
    };
    let body = format!(
        r#"gbiv.log("set=" .. tostring(gbiv.env({set})))
gbiv.log("unset=" .. tostring(gbiv.env({unset})))
try("ungranted", gbiv.env, "PATH")"#,
        set = lua_str(set),
        unset = lua_str(unset)
    );
    let msgs = run_ok(&l, &["env"], settings, &body);
    assert_eq!(msgs, vec!["x: set=hello env", "x: unset=nil", "x: ungranted=false"]);
}

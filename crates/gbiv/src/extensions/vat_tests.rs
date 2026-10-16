//! gbv-1ht tests (Phase 5): the bundled vat extension (`extensions/vat/`),
//! driven through the real extension runtime. Specs: `docs/specs/vat-extension.md`
//! (VATX-*). The `testdata/vat/{start,done}` fixtures are copied from
//! `jmbeach/vat`'s `tests/fixtures/commands/` (see `testdata/vat/README.md`).
//!
//! Every test copies the repo's real `extensions/vat/` into a throwaway
//! layout's project extension directory, enables it with grants for the
//! backlog and a fake `~/.config/vat`, and calls `before_start` /
//! `before_done` the way `gbiv start` / `gbiv done` would.
//!
//! Nested-repo tests build a bare remote plus clones in the layout's temp dir.
//! Contention is made deterministic with client-side hooks in the backlog
//! clone: a `post-commit` hook makes a second clone push first, so the
//! extension's own `git push` is then rejected (`[rejected] ... (fetch first)`).

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use super::test_support::{config_with, fast_limits, layout, Layout};
use super::{ExtensionError, ExtensionSettings, Limits, Runtime};

const HEADER: &str = "---\nversion: 1\n---\n\n";
const VAT_TOML: &str = "[project]\nid = \"vat\"\n";
const COLOR: &str = "red";
const SET_USER_MSG: &str = "set user.name first: vat config set user.name <name>";

// ════════════════════════════════════════════════════════════════════════════
// Harness
// ════════════════════════════════════════════════════════════════════════════

fn bundled_ext_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../extensions/vat")
}

fn fixture_dir(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src/extensions/testdata/vat")
        .join(name)
}

fn copy_tree(src: &Path, dst: &Path) {
    fs::create_dir_all(dst).expect("mkdir dst");
    for entry in fs::read_dir(src).expect("read_dir") {
        let entry = entry.expect("dir entry");
        let to = dst.join(entry.file_name());
        if entry.file_type().expect("file_type").is_dir() {
            copy_tree(&entry.path(), &to);
        } else {
            fs::copy(entry.path(), &to).expect("copy");
        }
    }
}

/// Every file under `dir` (skipping any `.git`), relative path → bytes.
fn snapshot(dir: &Path) -> BTreeMap<String, Vec<u8>> {
    fn walk(base: &Path, dir: &Path, out: &mut BTreeMap<String, Vec<u8>>) {
        let Ok(rd) = fs::read_dir(dir) else { return };
        for entry in rd {
            let entry = entry.expect("dir entry");
            if entry.file_name() == ".git" {
                continue;
            }
            let p = entry.path();
            if entry.file_type().expect("file_type").is_dir() {
                walk(base, &p, out);
            } else {
                let rel = p.strip_prefix(base).unwrap().to_string_lossy().into_owned();
                out.insert(rel, fs::read(&p).expect("read"));
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(dir, dir, &mut out);
    out
}

fn opts(pairs: &[(&str, &str)]) -> toml::Table {
    let mut t = toml::Table::new();
    for (k, v) in pairs {
        t.insert((*k).to_string(), toml::Value::String((*v).to_string()));
    }
    t
}

fn user(name: &str) -> toml::Table {
    opts(&[("user", name)])
}

/// A layout with the bundled vat extension installed in the project scope,
/// a backlog directory at `<main_repo>/backlog` and a fake `~/.config/vat`.
struct Vat {
    layout: Layout,
    backlog: PathBuf,
    vat_cfg: PathBuf,
}

impl Vat {
    fn new() -> Vat {
        let layout = layout();
        copy_tree(&bundled_ext_dir(), &layout.project_ext.join("vat"));
        let backlog = layout.main_repo.join("backlog");
        fs::create_dir_all(&backlog).expect("mkdir backlog");
        let vat_cfg = layout.home.join(".config").join("vat");
        fs::create_dir_all(&vat_cfg).expect("mkdir ~/.config/vat");
        Vat {
            layout,
            backlog,
            vat_cfg,
        }
    }

    /// A backlog copied from `testdata/vat/<name>`.
    fn fixture(name: &str) -> Vat {
        let v = Vat::new();
        copy_tree(&fixture_dir(name), &v.backlog);
        v
    }

    /// A backlog with `vat.toml` (prefix `vat`) and the given `backlog.md`.
    fn with_backlog(content: &str) -> Vat {
        let v = Vat::new();
        v.write("vat.toml", VAT_TOML);
        v.write("backlog.md", content);
        v
    }

    fn write(&self, rel: &str, content: &str) {
        write_file(&self.backlog.join(rel), content);
    }

    fn read(&self, rel: &str) -> String {
        fs::read_to_string(self.backlog.join(rel)).unwrap_or_else(|e| panic!("read {rel}: {e}"))
    }

    fn exists(&self, rel: &str) -> bool {
        self.backlog.join(rel).exists()
    }

    fn snapshot(&self) -> BTreeMap<String, Vec<u8>> {
        snapshot(&self.backlog)
    }

    /// Default grants: the backlog and the fake `~/.config/vat`, plus `XDG_CONFIG_HOME`.
    fn settings(&self, options: toml::Table) -> ExtensionSettings {
        ExtensionSettings {
            paths: vec![self.backlog.clone(), self.vat_cfg.clone()],
            env: vec!["XDG_CONFIG_HOME".into()],
            options,
            ..Default::default()
        }
    }

    fn load(&self, settings: ExtensionSettings, limits: Limits) -> Runtime {
        Runtime::load(&self.layout.paths(), &config_with("vat", settings), limits)
            .expect("vat extension loads")
    }

    fn rt_opts(&self, options: toml::Table) -> Runtime {
        self.load(self.settings(options), fast_limits())
    }

    /// Runtime with the claiming user set through the `user` option.
    fn rt(&self, name: &str) -> Runtime {
        self.rt_opts(user(name))
    }

    /// Runtime with no `user` option.
    fn rt_anon(&self) -> Runtime {
        self.rt_opts(toml::Table::new())
    }
}

fn write_file(path: &Path, content: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("mkdir parent");
    }
    fs::write(path, content).expect("write file");
}

fn start(rt: &mut Runtime, arg: &str) -> Result<Option<String>, ExtensionError> {
    rt.before_start(arg, COLOR)
}

fn done(rt: &mut Runtime, text: &str) -> Result<(), ExtensionError> {
    rt.before_done(COLOR, text)
}

/// The message of a veto raised by the vat extension.
fn veto<T: std::fmt::Debug>(r: Result<T, ExtensionError>) -> String {
    match r {
        Err(ExtensionError::Veto { name, message }) => {
            assert_eq!(
                name, "vat",
                "veto should be attributed to the vat extension"
            );
            message
        }
        other => panic!("expected a veto from vat, got {other:?}"),
    }
}

fn assert_init_pointer(msg: &str) {
    assert!(
        msg.contains("vat init"),
        "expected vat's init-pointer message (mentioning `vat init`), got {msg:?}"
    );
}

fn assert_duplicate_msg(msg: &str, id: &str) {
    assert!(
        msg.to_lowercase().contains("duplicate") && msg.contains(id),
        "expected vat's duplicate-ID message naming {id}, got {msg:?}"
    );
}

fn assert_upgrade_msg(msg: &str) {
    assert!(
        msg.contains("version 2") && msg.to_lowercase().contains("upgrade vat"),
        "expected vat's upgrade message, got {msg:?}"
    );
}

/// Sets (or unsets) an environment variable for the life of the guard.
struct EnvGuard {
    key: &'static str,
    old: Option<OsString>,
}

impl EnvGuard {
    fn set(key: &'static str, val: Option<&Path>) -> EnvGuard {
        let old = std::env::var_os(key);
        match val {
            Some(v) => std::env::set_var(key, v),
            None => std::env::remove_var(key),
        }
        EnvGuard { key, old }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        match &self.old {
            Some(v) => std::env::set_var(self.key, v),
            None => std::env::remove_var(self.key),
        }
    }
}

// ════════════════════════════════════════════════════════════════════════════
// Git harness
// ════════════════════════════════════════════════════════════════════════════

fn git_out(dir: &Path, args: &[&str]) -> std::process::Output {
    Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("LC_ALL", "C")
        .stdin(Stdio::null())
        .output()
        .expect("git runs")
}

/// Run git in `dir`, require success, return trimmed stdout.
fn git(dir: &Path, args: &[&str]) -> String {
    let out = git_out(dir, args);
    assert!(
        out.status.success(),
        "git {args:?} in {} failed: {}",
        dir.display(),
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

/// Local identity and settings that keep the host's global git config
/// (signing, push negotiation) out of the tests.
fn configure(dir: &Path, name: &str) {
    git(dir, &["config", "user.name", name]);
    git(
        dir,
        &["config", "user.email", &format!("{name}@example.com")],
    );
    git(dir, &["config", "commit.gpgsign", "false"]);
    git(dir, &["config", "tag.gpgsign", "false"]);
    git(dir, &["config", "push.negotiate", "false"]);
}

fn head(dir: &Path) -> String {
    git(dir, &["rev-parse", "HEAD"])
}

fn porcelain(dir: &Path) -> String {
    git(dir, &["status", "--porcelain"])
}

fn commit_all(dir: &Path, msg: &str) {
    git(dir, &["add", "-A"]);
    git(dir, &["commit", "-q", "-m", msg]);
}

fn write_hook(repo: &Path, name: &str, body: &str) {
    let path = repo.join(".git").join("hooks").join(name);
    let script = format!(
        "#!/bin/sh\nunset GIT_DIR GIT_INDEX_FILE GIT_WORK_TREE GIT_PREFIX GIT_OBJECT_DIRECTORY\n{body}\n"
    );
    write_file(&path, &script);
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).expect("chmod hook");
}

fn line_count(path: &Path) -> usize {
    fs::read_to_string(path)
        .map(|s| s.lines().count())
        .unwrap_or(0)
}

/// A backlog that is its own git clone of a bare remote, plus a second
/// clone ("other", a different user) that can push competing changes.
struct Nested {
    scratch: PathBuf,
    bare: PathBuf,
    other: PathBuf,
}

impl Nested {
    /// Make `v.backlog` a clone of a fresh bare remote seeded with `files`.
    fn setup(v: &Vat, files: &[(&str, &str)]) -> Nested {
        let scratch = v.layout.root.join("git");
        fs::create_dir_all(&scratch).expect("mkdir scratch");
        let bare = scratch.join("remote.git");
        git(
            &scratch,
            &["init", "-q", "--bare", "-b", "main", "remote.git"],
        );
        git(
            &scratch,
            &[
                "clone",
                "-q",
                bare.to_str().unwrap(),
                v.backlog.to_str().unwrap(),
            ],
        );
        git(&v.backlog, &["symbolic-ref", "HEAD", "refs/heads/main"]);
        configure(&v.backlog, "tester");
        for (rel, content) in files {
            v.write(rel, content);
        }
        commit_all(&v.backlog, "seed");
        git(&v.backlog, &["push", "-q", "-u", "origin", "main"]);
        let other = scratch.join("other");
        git(
            &scratch,
            &[
                "clone",
                "-q",
                bare.to_str().unwrap(),
                other.to_str().unwrap(),
            ],
        );
        configure(&other, "other");
        Nested {
            scratch,
            bare,
            other,
        }
    }

    fn remote_head(&self) -> String {
        git(&self.bare, &["rev-parse", "main"])
    }

    fn remote_subjects(&self) -> Vec<String> {
        git(&self.bare, &["log", "--format=%s", "main"])
            .lines()
            .map(str::to_string)
            .collect()
    }

    fn remote_file(&self, rel: &str) -> Option<String> {
        let out = git_out(&self.bare, &["show", &format!("main:{rel}")]);
        out.status
            .success()
            .then(|| String::from_utf8_lossy(&out.stdout).into_owned())
    }

    /// From the other clone: sync to the remote, apply `files` (None removes),
    /// commit and push.
    fn other_push(&self, files: &[(&str, Option<&str>)], msg: &str) {
        git(&self.other, &["fetch", "-q"]);
        git(&self.other, &["reset", "-q", "--hard", "origin/main"]);
        for (rel, content) in files {
            let p = self.other.join(rel);
            match content {
                Some(c) => write_file(&p, c),
                None => {
                    let _ = fs::remove_file(&p);
                }
            }
        }
        commit_all(&self.other, msg);
        git(&self.other, &["push", "-q"]);
    }

    /// Install a `post-commit` hook in the backlog clone that, on each of the
    /// extension's commits (only the first, when `once`), makes the other
    /// clone push a commit first: `mutate` is a shell snippet run with `$O`
    /// set to the other clone. Each invocation appends a line to the returned
    /// counter file.
    fn race_hook(&self, backlog: &Path, once: bool, mutate: &str, msg: &str) -> PathBuf {
        let count = self.scratch.join("post-commit-count");
        let flag = self.scratch.join("post-commit-fired");
        let once_guard = if once {
            format!("[ -e '{f}' ] && exit 0\ntouch '{f}'\n", f = flag.display())
        } else {
            String::new()
        };
        let body = format!(
            "echo x >> '{count}'\n{once_guard}O='{other}'\n\
             git -C \"$O\" fetch -q && git -C \"$O\" reset -q --hard origin/main && {mutate} && \
             git -C \"$O\" add -A && git -C \"$O\" commit -qm '{msg}' && git -C \"$O\" push -q\n",
            count = count.display(),
            other = self.other.display(),
        );
        write_hook(backlog, "post-commit", &body);
        count
    }
}

/// Generous limits for claim-loop tests (backoff alone can take ~9.5 s).
fn loop_limits() -> Limits {
    Limits {
        handler_timeout: Duration::from_secs(60),
        ..fast_limits()
    }
}

fn nested_seed() -> Vec<(&'static str, &'static str)> {
    vec![
        ("vat.toml", VAT_TOML),
        ("backlog.md", SEED_BACKLOG),
        (".used-ids", "vat-aaa\n"),
        ("items/vat-g5y.md", "---\nid: vat-g5y\n---\n\nNotes.\n"),
    ]
}

const SEED_BACKLOG: &str = "---\nversion: 1\n---\n\n- [vat-g5y] A task\n- [vat-h8x] Other task\n";
const SEED_CLAIMED: &str =
    "---\nversion: 1\n---\n\n- [vat-g5y] [in-progress] [by:tester] A task\n- [vat-h8x] Other task\n";
const SEED_DONE: &str = "---\nversion: 1\n---\n\n- [vat-h8x] Other task\n";

// ════════════════════════════════════════════════════════════════════════════
// Packaging and configuration
// ════════════════════════════════════════════════════════════════════════════

// @spec VATX-PKG-001
#[test]
fn manifest_declares_name_api_version_and_capabilities_and_loads() {
    let text = fs::read_to_string(bundled_ext_dir().join("extension.toml")).expect("manifest");
    let m: toml::Table = toml::from_str(&text).expect("manifest is TOML");
    assert_eq!(m.get("name").and_then(|v| v.as_str()), Some("vat"));
    assert_eq!(m.get("api_version").and_then(|v| v.as_integer()), Some(1));
    let mut caps: Vec<&str> = m
        .get("capabilities")
        .and_then(|v| v.as_array())
        .expect("capabilities array")
        .iter()
        .map(|c| c.as_str().expect("capability string"))
        .collect();
    caps.sort_unstable();
    assert_eq!(caps, vec!["env", "fs", "git"]);
    assert!(
        bundled_ext_dir().join("init.lua").is_file(),
        "init.lua ships"
    );

    let v = Vat::fixture("start");
    let rt = v.rt("tester");
    assert_eq!(rt.names(), vec!["vat".to_string()]);
}

// @spec VATX-PKG-002
#[test]
fn defines_before_handlers_and_no_after_handlers() {
    let v = Vat::fixture("start");
    let before = v.snapshot();
    let mut rt = v.rt("tester");
    // No after_* handlers: firing them does nothing and produces no message.
    rt.after_start(COLOR, "[vat-g5y] A task");
    rt.after_done(COLOR, "[vat-g5y] A task");
    assert!(
        rt.take_messages().is_empty(),
        "after_* must not run any vat code"
    );
    assert_eq!(v.snapshot(), before);
    // before_start is defined: it names the entry for a vat ID.
    assert_eq!(
        start(&mut rt, "vat-g5y").unwrap().as_deref(),
        Some("[vat-g5y] A task")
    );

    // before_done is defined: it completes the task.
    let d = Vat::fixture("done");
    let mut rt = d.rt_anon();
    done(&mut rt, "[vat-g5y] Blocker").unwrap();
    assert!(!d.read("backlog.md").contains("[vat-g5y]"));
}

// @spec VATX-CFG-001
#[test]
fn backlog_defaults_to_backlog_dir_in_main_repo() {
    let v = Vat::fixture("start");
    // No `backlog` option: `<main_repo>/backlog` is used.
    let mut rt = v.rt("tester");
    start(&mut rt, "vat-g5y").unwrap();
    assert!(v
        .read("backlog.md")
        .contains("- [vat-g5y] [in-progress] [by:tester] A task\n"));
}

// @spec VATX-CFG-001
#[test]
fn backlog_option_relative_path_resolves_against_main_repo() {
    let v = Vat::fixture("start");
    let custom = v.layout.main_repo.join("tasks").join("bl");
    copy_tree(&fixture_dir("start"), &custom);
    let default_before = v.snapshot();
    let mut settings = v.settings(opts(&[("user", "tester"), ("backlog", "tasks/bl")]));
    settings.paths.push(custom.clone());
    let mut rt = v.load(settings, fast_limits());
    start(&mut rt, "vat-g5y").unwrap();
    let edited = fs::read_to_string(custom.join("backlog.md")).unwrap();
    assert!(
        edited.contains("- [vat-g5y] [in-progress] [by:tester] A task\n"),
        "{edited}"
    );
    assert_eq!(
        v.snapshot(),
        default_before,
        "default backlog/ must be untouched"
    );
}

// @spec VATX-CFG-001
#[test]
fn backlog_option_absolute_path_is_used_as_is() {
    let v = Vat::new();
    let elsewhere = v.layout.root.join("elsewhere").join("bl");
    copy_tree(&fixture_dir("start"), &elsewhere);
    let mut settings = v.settings(opts(&[
        ("user", "tester"),
        ("backlog", elsewhere.to_str().unwrap()),
    ]));
    settings.paths.push(elsewhere.clone());
    let mut rt = v.load(settings, fast_limits());
    start(&mut rt, "vat-g5y").unwrap();
    let edited = fs::read_to_string(elsewhere.join("backlog.md")).unwrap();
    assert!(
        edited.contains("- [vat-g5y] [in-progress] [by:tester] A task\n"),
        "{edited}"
    );
}

// @spec VATX-CFG-001
#[test]
fn backlog_option_tilde_path_expands_to_home() {
    let v = Vat::new();
    let in_home = v.layout.home.join("bl");
    copy_tree(&fixture_dir("start"), &in_home);
    let mut settings = v.settings(opts(&[("user", "tester"), ("backlog", "~/bl")]));
    settings.paths.push(in_home.clone());
    let mut rt = v.load(settings, fast_limits());
    start(&mut rt, "vat-g5y").unwrap();
    let edited = fs::read_to_string(in_home.join("backlog.md")).unwrap();
    assert!(
        edited.contains("- [vat-g5y] [in-progress] [by:tester] A task\n"),
        "{edited}"
    );
}

// @spec VATX-CFG-002
#[test]
fn user_option_wins_over_vat_user_config() {
    let v = Vat::fixture("start");
    write_file(&v.vat_cfg.join("config.toml"), "[user]\nname = \"alice\"\n");
    let mut rt = v.rt("tester");
    start(&mut rt, "vat-g5y").unwrap();
    assert!(v.read("backlog.md").contains("[by:tester]"));
    assert!(!v.read("backlog.md").contains("[by:alice]"));
}

// Hermetic: no `env` grant, so XDG_CONFIG_HOME is not readable and the
// extension must use ~/.config/vat/config.toml (the layout's fake home).
// @spec VATX-CFG-002
#[test]
fn user_from_home_vat_config_when_xdg_not_readable() {
    let v = Vat::fixture("start");
    write_file(&v.vat_cfg.join("config.toml"), "[user]\nname = \"alice\"\n");
    let mut settings = v.settings(toml::Table::new());
    settings.env.clear();
    let mut rt = v.load(settings, fast_limits());
    start(&mut rt, "vat-g5y").unwrap();
    assert!(
        v.read("backlog.md")
            .contains("- [vat-g5y] [in-progress] [by:alice] A task\n"),
        "{}",
        v.read("backlog.md")
    );
}

// @spec VATX-CFG-002
#[test]
#[serial_test::serial]
fn user_from_home_vat_config_when_xdg_granted_but_unset() {
    let v = Vat::fixture("start");
    write_file(&v.vat_cfg.join("config.toml"), "[user]\nname = \"alice\"\n");
    let _env = EnvGuard::set("XDG_CONFIG_HOME", None);
    let mut rt = v.rt_anon();
    start(&mut rt, "vat-g5y").unwrap();
    assert!(v.read("backlog.md").contains("[by:alice]"));
}

// @spec VATX-CFG-002
#[test]
#[serial_test::serial]
fn user_from_xdg_vat_config_when_xdg_set() {
    let v = Vat::fixture("start");
    write_file(&v.vat_cfg.join("config.toml"), "[user]\nname = \"alice\"\n");
    let xdg = v.layout.root.join("xdg");
    write_file(
        &xdg.join("vat").join("config.toml"),
        "[user]\nname = \"carol\"\n",
    );
    let _env = EnvGuard::set("XDG_CONFIG_HOME", Some(&xdg));
    let mut settings = v.settings(toml::Table::new());
    settings.paths.push(xdg.join("vat"));
    let mut rt = v.load(settings, fast_limits());
    start(&mut rt, "vat-g5y").unwrap();
    assert!(
        v.read("backlog.md")
            .contains("- [vat-g5y] [in-progress] [by:carol] A task\n"),
        "{}",
        v.read("backlog.md")
    );
}

// With XDG_CONFIG_HOME set, vat looks only there (not ~/.config) for the user.
// @spec VATX-CFG-002, VATX-CFG-003
#[test]
#[serial_test::serial]
fn xdg_set_without_config_does_not_fall_back_to_home() {
    let v = Vat::fixture("start");
    write_file(&v.vat_cfg.join("config.toml"), "[user]\nname = \"alice\"\n");
    let xdg = v.layout.root.join("xdg");
    fs::create_dir_all(xdg.join("vat")).unwrap();
    let _env = EnvGuard::set("XDG_CONFIG_HOME", Some(&xdg));
    let mut settings = v.settings(toml::Table::new());
    settings.paths.push(xdg.join("vat"));
    let before = v.snapshot();
    let mut rt = v.load(settings, fast_limits());
    assert_eq!(veto(start(&mut rt, "vat-g5y")), SET_USER_MSG);
    assert_eq!(v.snapshot(), before);
}

// XDG_CONFIG_HOME pointing outside the granted paths: the read fails and the
// extension falls back to requiring `user` (LLD § Configuration).
// @spec VATX-CFG-002, VATX-CFG-003
#[test]
#[serial_test::serial]
fn xdg_outside_grants_requires_user_option() {
    let v = Vat::fixture("start");
    let xdg = v.layout.root.join("xdg-ungranted");
    write_file(
        &xdg.join("vat").join("config.toml"),
        "[user]\nname = \"carol\"\n",
    );
    let _env = EnvGuard::set("XDG_CONFIG_HOME", Some(&xdg));
    let before = v.snapshot();
    let mut rt = v.rt_anon();
    assert_eq!(veto(start(&mut rt, "vat-g5y")), SET_USER_MSG);
    assert_eq!(v.snapshot(), before);

    let mut rt = v.rt("tester");
    start(&mut rt, "vat-g5y").unwrap();
    assert!(v.read("backlog.md").contains("[by:tester]"));
}

// @spec VATX-CFG-003
#[test]
fn no_user_name_anywhere_vetoes_claim() {
    let v = Vat::fixture("start");
    let mut settings = v.settings(toml::Table::new());
    settings.env.clear();
    let before = v.snapshot();
    let mut rt = v.load(settings, fast_limits());
    assert_eq!(veto(start(&mut rt, "vat-g5y")), SET_USER_MSG);
    assert_eq!(v.snapshot(), before);
}

// @spec VATX-CFG-003
#[test]
fn user_config_without_user_name_vetoes_claim() {
    let v = Vat::fixture("start");
    write_file(
        &v.vat_cfg.join("config.toml"),
        "[user]\n\n[other]\nkey = 1\n",
    );
    let mut settings = v.settings(toml::Table::new());
    settings.env.clear();
    let before = v.snapshot();
    let mut rt = v.load(settings, fast_limits());
    assert_eq!(veto(start(&mut rt, "vat-g5y")), SET_USER_MSG);
    assert_eq!(v.snapshot(), before);
}

// The user name is only needed when claiming.
// @spec VATX-CFG-003, VATX-ID-003
#[test]
fn no_user_name_is_fine_for_non_vat_arguments() {
    let v = Vat::fixture("start");
    let mut settings = v.settings(toml::Table::new());
    settings.env.clear();
    let before = v.snapshot();
    let mut rt = v.load(settings.clone(), fast_limits());
    assert_eq!(start(&mut rt, "Fix login bug").unwrap(), None);
    let mut rt = v.load(settings, fast_limits());
    assert_eq!(start(&mut rt, "abc-123").unwrap(), None);
    assert_eq!(v.snapshot(), before);
}

// ════════════════════════════════════════════════════════════════════════════
// Recognizing vat IDs
// ════════════════════════════════════════════════════════════════════════════

// @spec VATX-ID-001, VATX-ID-003
#[test]
fn non_candidate_arguments_return_nil_and_change_nothing() {
    let v = Vat::fixture("start");
    let before = v.snapshot();
    for arg in [
        "Fix login bug",
        "vat-g5y fix",
        "fix vat-g5y",
        "[vat-g5y]",
        "vat-g5",
        "vat-g5yy",
        "vatg5y",
        "vat_g5y",
        "va-g5y",
        "vat-g5u", // `u` is not Crockford base32
        "vat-g5i", // nor `i`
        "VAT-G5L", // nor `l`
        "vat-g5o", // nor `o`
        "vat-g5y-h8x",
    ] {
        let mut rt = v.rt("tester");
        assert_eq!(
            start(&mut rt, arg).unwrap(),
            None,
            "arg {arg:?} is not a candidate"
        );
        assert_eq!(v.snapshot(), before, "arg {arg:?} must change nothing");
    }
}

// @spec VATX-ID-001
#[test]
fn candidate_is_trimmed_before_matching() {
    let v = Vat::fixture("start");
    let mut rt = v.rt("tester");
    assert_eq!(
        start(&mut rt, "  vat-g5y \t").unwrap().as_deref(),
        Some("[vat-g5y] A task")
    );
    assert!(v
        .read("backlog.md")
        .contains("- [vat-g5y] [in-progress] [by:tester] A task\n"));
}

// @spec VATX-ID-001
#[test]
fn candidate_with_unknown_suffix_for_this_backlog_is_handled_not_ignored() {
    // A well-formed candidate with this prefix is ours even if it is not in
    // the backlog (contrast with the non-candidates above, which return nil).
    let v = Vat::fixture("start");
    let mut rt = v.rt("tester");
    assert_eq!(veto(start(&mut rt, "vat-0zz")), "unknown id: vat-0zz");
}

// @spec VATX-ID-002, VATX-ID-003
#[test]
fn candidate_with_other_prefix_returns_nil_and_changes_nothing() {
    let v = Vat::fixture("start");
    let before = v.snapshot();
    for arg in ["abc-123", "gbv-1ht", "ABC-123"] {
        let mut rt = v.rt("tester");
        assert_eq!(
            start(&mut rt, arg).unwrap(),
            None,
            "{arg:?} belongs to another backlog"
        );
    }
    assert_eq!(v.snapshot(), before);
}

// @spec VATX-ID-002
#[test]
fn prefix_compared_case_insensitively_with_project_id() {
    let v = Vat::fixture("start");
    v.write("vat.toml", "[project]\nid = \"VAT\"\n");
    let mut rt = v.rt("tester");
    assert_eq!(
        start(&mut rt, "vat-g5y").unwrap().as_deref(),
        Some("[vat-g5y] A task")
    );
    assert!(v
        .read("backlog.md")
        .contains("[vat-g5y] [in-progress] [by:tester]"));
}

// @spec VATX-ID-004
#[test]
fn missing_vat_toml_vetoes_candidates_with_init_pointer() {
    let v = Vat::fixture("start");
    fs::remove_file(v.backlog.join("vat.toml")).unwrap();
    let before = v.snapshot();
    for arg in ["vat-g5y", "abc-123"] {
        let mut rt = v.rt("tester");
        assert_init_pointer(&veto(start(&mut rt, arg)));
    }
    assert_eq!(v.snapshot(), before);
}

// @spec VATX-ID-004
#[test]
fn missing_vat_toml_still_returns_nil_for_non_candidates() {
    let v = Vat::fixture("start");
    fs::remove_file(v.backlog.join("vat.toml")).unwrap();
    let before = v.snapshot();
    let mut rt = v.rt("tester");
    assert_eq!(start(&mut rt, "Fix login bug").unwrap(), None);
    assert_eq!(v.snapshot(), before);
}

// @spec VATX-ID-004
#[test]
fn invalid_project_id_vetoes_candidates_with_init_pointer() {
    for toml_text in [
        "[project]\nid = \"vatt\"\n",  // 4 chars
        "[project]\nid = \"va\"\n",    // 2 chars
        "[project]\nid = \"vau\"\n",   // `u` not in the alphabet
        "[project]\nid = 7\n",         // not a string
        "[project]\nname = \"vat\"\n", // no id
        "[other]\nid = \"vat\"\n",     // no [project]
    ] {
        let v = Vat::fixture("start");
        v.write("vat.toml", toml_text);
        let before = v.snapshot();
        let mut rt = v.rt("tester");
        let msg = veto(start(&mut rt, "vat-g5y"));
        assert!(
            msg.contains("vat init"),
            "vat.toml {toml_text:?}: got {msg:?}"
        );
        let mut rt = v.rt("tester");
        assert_eq!(
            start(&mut rt, "Fix login bug").unwrap(),
            None,
            "vat.toml {toml_text:?}"
        );
        assert_eq!(v.snapshot(), before);
    }
}

// @spec VATX-ID-005
#[test]
fn uppercase_argument_is_normalized_to_lowercase() {
    let v = Vat::fixture("start");
    let mut rt = v.rt("tester");
    assert_eq!(
        start(&mut rt, "VAT-G5Y").unwrap().as_deref(),
        Some("[vat-g5y] A task")
    );
    assert_eq!(
        v.read("backlog.md"),
        format!("{HEADER}- [vat-g5y] [in-progress] [by:tester] A task\n- [vat-h8x] [in-progress] [by:bob] Claimed task\n")
    );
}

// @spec VATX-ID-005
#[test]
fn uppercase_id_in_done_text_is_normalized_to_lowercase() {
    let v = Vat::fixture("done");
    let mut rt = v.rt_anon();
    done(&mut rt, "[VAT-G5Y] Blocker").unwrap();
    assert_eq!(
        v.read("backlog.md"),
        format!("{HEADER}- [vat-h8x] Dependent\n")
    );
    assert_eq!(v.read(".used-ids"), "vat-aaa\nvat-g5y\n");
    assert!(!v.exists("items/vat-g5y.md"));
}

// ════════════════════════════════════════════════════════════════════════════
// before_start (claim)
// ════════════════════════════════════════════════════════════════════════════

// @spec VATX-START-001
#[test]
fn unknown_id_vetoes() {
    let v = Vat::fixture("start");
    let before = v.snapshot();
    let mut rt = v.rt("tester");
    assert_eq!(veto(start(&mut rt, "vat-zzz")), "unknown id: vat-zzz");
    assert_eq!(v.snapshot(), before);
}

// @spec VATX-START-001
#[test]
fn missing_backlog_md_vetoes_unknown_id() {
    let v = Vat::new();
    v.write("vat.toml", VAT_TOML);
    let before = v.snapshot();
    let mut rt = v.rt("tester");
    assert_eq!(veto(start(&mut rt, "vat-g5y")), "unknown id: vat-g5y");
    assert_eq!(v.snapshot(), before, "nothing may be created");
}

// Only the parsed region holds tasks; a bullet below the first `---` is freeform.
// @spec VATX-START-001, VATX-FMT-003
#[test]
fn id_only_in_freeform_region_is_unknown() {
    let v = Vat::with_backlog(&format!(
        "{HEADER}- [vat-g5y] A task\n\n---\n\n- [vat-k2m] Freeform bullet\n"
    ));
    let before = v.snapshot();
    let mut rt = v.rt("tester");
    assert_eq!(veto(start(&mut rt, "vat-k2m")), "unknown id: vat-k2m");
    assert_eq!(v.snapshot(), before);
}

// @spec VATX-START-002
#[test]
fn duplicate_id_vetoes() {
    let v = Vat::with_backlog(&format!("{HEADER}- [vat-g5y] First\n- [vat-g5y] Second\n"));
    let before = v.snapshot();
    let mut rt = v.rt("tester");
    assert_duplicate_msg(&veto(start(&mut rt, "vat-g5y")), "vat-g5y");
    assert_eq!(v.snapshot(), before);
}

// Golden: jmbeach/vat tests/fixtures/commands/start (start_claim_succeeds_and_adds_markers).
// @spec VATX-START-003, VATX-START-006
#[test]
fn claim_inserts_markers_in_canonical_position_golden() {
    let v = Vat::fixture("start");
    let mut rt = v.rt("tester");
    assert_eq!(
        start(&mut rt, "vat-g5y").unwrap().as_deref(),
        Some("[vat-g5y] A task")
    );
    assert_eq!(
        v.read("backlog.md"),
        "---\nversion: 1\n---\n\n- [vat-g5y] [in-progress] [by:tester] A task\n- [vat-h8x] [in-progress] [by:bob] Claimed task\n"
    );
    assert!(rt.take_messages().is_empty());
}

// @spec VATX-START-003
#[test]
fn claim_allowed_on_blocked_bullet() {
    let v = Vat::with_backlog(&format!(
        "{HEADER}- [vat-g5y] Blocker\n- [vat-h8x] [blocked-by:vat-g5y] Dependent\n"
    ));
    let mut rt = v.rt("tester");
    assert_eq!(
        start(&mut rt, "vat-h8x").unwrap().as_deref(),
        Some("[vat-h8x] [blocked-by:vat-g5y] Dependent")
    );
    assert_eq!(
        v.read("backlog.md"),
        format!("{HEADER}- [vat-g5y] Blocker\n- [vat-h8x] [in-progress] [by:tester] [blocked-by:vat-g5y] Dependent\n")
    );
}

// @spec VATX-START-004, VATX-START-006
#[test]
fn already_claimed_by_this_user_is_unchanged_and_named() {
    let v = Vat::fixture("start");
    let before = v.snapshot();
    let mut rt = v.rt("bob");
    assert_eq!(
        start(&mut rt, "vat-h8x").unwrap().as_deref(),
        Some("[vat-h8x] Claimed task")
    );
    assert_eq!(v.snapshot(), before);
}

// @spec VATX-START-004
#[test]
fn by_this_user_without_in_progress_is_unchanged_and_named() {
    let v = Vat::with_backlog(&format!("{HEADER}- [vat-g5y] [by:tester] A task\n"));
    let before = v.snapshot();
    let mut rt = v.rt("tester");
    assert_eq!(
        start(&mut rt, "vat-g5y").unwrap().as_deref(),
        Some("[vat-g5y] A task")
    );
    assert_eq!(v.snapshot(), before);
}

// Golden: jmbeach/vat tests/fixtures/commands/start (start_already_claimed_is_refused...).
// @spec VATX-START-005
#[test]
fn claimed_by_other_user_vetoes() {
    let v = Vat::fixture("start");
    let before = v.snapshot();
    let mut rt = v.rt("tester");
    assert_eq!(
        veto(start(&mut rt, "vat-h8x")),
        "vat-h8x already claimed by bob"
    );
    assert_eq!(v.snapshot(), before);
}

// @spec VATX-START-004, VATX-START-005
#[test]
fn claimer_name_compared_case_sensitively() {
    let v = Vat::fixture("start");
    let before = v.snapshot();
    let mut rt = v.rt("Bob");
    assert_eq!(
        veto(start(&mut rt, "vat-h8x")),
        "vat-h8x already claimed by bob"
    );
    assert_eq!(v.snapshot(), before);
}

// @spec VATX-START-005
#[test]
fn in_progress_without_by_vetoes() {
    let v = Vat::with_backlog(&format!("{HEADER}- [vat-g5y] [in-progress] A task\n"));
    let before = v.snapshot();
    let mut rt = v.rt("tester");
    assert_eq!(
        veto(start(&mut rt, "vat-g5y")),
        "vat-g5y already in progress"
    );
    assert_eq!(v.snapshot(), before);
}

// @spec VATX-START-005
#[test]
fn by_other_without_in_progress_vetoes() {
    let v = Vat::with_backlog(&format!("{HEADER}- [vat-g5y] [by:bob] A task\n"));
    let before = v.snapshot();
    let mut rt = v.rt("tester");
    assert_eq!(
        veto(start(&mut rt, "vat-g5y")),
        "vat-g5y already claimed by bob"
    );
    assert_eq!(v.snapshot(), before);
}

// @spec VATX-START-006
#[test]
fn entry_text_rewrites_item_pointer_relative_to_main_repo() {
    let v = Vat::with_backlog(&format!(
        "{HEADER}- [vat-g5y] A task (see ./items/vat-g5y.md)\n"
    ));
    v.write("items/vat-g5y.md", "---\nid: vat-g5y\n---\n\nNotes.\n");
    let mut rt = v.rt("tester");
    assert_eq!(
        start(&mut rt, "vat-g5y").unwrap().as_deref(),
        Some("[vat-g5y] A task (see backlog/items/vat-g5y.md)")
    );
    // The backlog keeps vat's own `./items/` pointer.
    assert_eq!(
        v.read("backlog.md"),
        format!("{HEADER}- [vat-g5y] [in-progress] [by:tester] A task (see ./items/vat-g5y.md)\n")
    );
}

// @spec VATX-START-006, VATX-CFG-001
#[test]
fn entry_text_pointer_uses_configured_backlog_path() {
    let v = Vat::new();
    let custom = v.layout.main_repo.join("tasks").join("bl");
    write_file(&custom.join("vat.toml"), VAT_TOML);
    write_file(
        &custom.join("backlog.md"),
        &format!("{HEADER}- [vat-g5y] A task (see ./items/vat-g5y.md)\n"),
    );
    write_file(
        &custom.join("items/vat-g5y.md"),
        "---\nid: vat-g5y\n---\n\nNotes.\n",
    );
    let mut settings = v.settings(opts(&[("user", "tester"), ("backlog", "tasks/bl")]));
    settings.paths.push(custom.clone());
    let mut rt = v.load(settings, fast_limits());
    assert_eq!(
        start(&mut rt, "vat-g5y").unwrap().as_deref(),
        Some("[vat-g5y] A task (see tasks/bl/items/vat-g5y.md)")
    );
}

// @spec VATX-START-006
#[test]
fn entry_text_keeps_other_tags_and_drops_claim_markers() {
    let v = Vat::with_backlog(&format!(
        "{HEADER}- [vat-g5y] [blocked-by:vat-h8x] [parent:vat-k2m] A task\n- [vat-h8x] Blocker\n- [vat-k2m] Parent\n"
    ));
    let mut rt = v.rt("tester");
    assert_eq!(
        start(&mut rt, "vat-g5y").unwrap().as_deref(),
        Some("[vat-g5y] [blocked-by:vat-h8x] [parent:vat-k2m] A task")
    );
}

// The returned text is computed from the bullet as it stands after the claim,
// even when it was already ours (START-004) with non-canonical spacing.
// @spec VATX-START-006
#[test]
fn entry_text_from_already_owned_bullet_drops_claim_markers() {
    let v = Vat::with_backlog(&format!(
        "{HEADER}- [vat-g5y] [in-progress] [by:tester] [blocked-by:vat-h8x] A task (see ./items/vat-g5y.md)\n- [vat-h8x] Blocker\n"
    ));
    v.write("items/vat-g5y.md", "---\nid: vat-g5y\n---\n\nNotes.\n");
    let mut rt = v.rt("tester");
    assert_eq!(
        start(&mut rt, "vat-g5y").unwrap().as_deref(),
        Some("[vat-g5y] [blocked-by:vat-h8x] A task (see backlog/items/vat-g5y.md)")
    );
}

// ════════════════════════════════════════════════════════════════════════════
// before_done (complete)
// ════════════════════════════════════════════════════════════════════════════

// @spec VATX-DONE-001
#[test]
fn done_text_without_this_backlogs_id_tag_changes_nothing() {
    let v = Vat::fixture("done");
    let before = v.snapshot();
    for text in [
        "Fix login bug",
        "vat-g5y Blocker",        // not bracketed
        "[abc-123] Foreign task", // another backlog's ID
        "[vat-g5] Malformed",     // not an ID
        "[parent:vat-g5y] Child", // bracketed, but the tag is not an ID tag
        "[TODO] vat-g5y",
    ] {
        let mut rt = v.rt_anon();
        done(&mut rt, text).unwrap_or_else(|e| panic!("text {text:?}: {e:?}"));
        assert_eq!(v.snapshot(), before, "text {text:?} must change nothing");
    }
}

// @spec VATX-DONE-002
#[test]
fn done_uses_first_id_tag_of_this_backlog() {
    let v = Vat::fixture("done");
    let mut rt = v.rt_anon();
    done(&mut rt, "[abc-123] [vat-h8x] Dependent, see [vat-g5y]").unwrap();
    assert_eq!(
        v.read("backlog.md"),
        format!("{HEADER}- [vat-g5y] Blocker\n")
    );
    assert_eq!(v.read(".used-ids"), "vat-aaa\nvat-h8x\n");
    assert!(
        v.exists("items/vat-g5y.md"),
        "only the first ID's task is completed"
    );
}

// @spec VATX-DONE-002
#[test]
fn done_skips_non_id_tags_before_the_id_tag() {
    let v = Vat::fixture("done");
    let mut rt = v.rt_anon();
    done(&mut rt, "[parent:vat-h8x] [vat-g5y] Blocker").unwrap();
    assert_eq!(
        v.read("backlog.md"),
        format!("{HEADER}- [vat-h8x] Dependent\n")
    );
}

// Golden: jmbeach/vat tests/fixtures/commands/done
// (done_removes_entry_deletes_item_tombstones_and_auto_unblocks).
// @spec VATX-DONE-003
#[test]
fn done_completes_task_golden() {
    let v = Vat::fixture("done");
    let mut rt = v.rt_anon();
    done(&mut rt, "[vat-g5y] Blocker").unwrap();
    assert_eq!(
        v.read("backlog.md"),
        "---\nversion: 1\n---\n\n- [vat-h8x] Dependent\n"
    );
    assert_eq!(v.read(".used-ids"), "vat-aaa\nvat-g5y\n");
    assert!(!v.exists("items/vat-g5y.md"), "item file deleted");
    assert_eq!(
        v.snapshot().keys().cloned().collect::<Vec<_>>(),
        vec![".used-ids", "backlog.md", "vat.toml"]
    );
}

// @spec VATX-DONE-003
#[test]
fn done_completes_regardless_of_claimer() {
    let v = Vat::with_backlog(&format!(
        "{HEADER}- [vat-g5y] [in-progress] [by:bob] A task\n- [vat-h8x] Other\n"
    ));
    v.write(".used-ids", "vat-aaa\n");
    // No user configured at all: done does not need one.
    let mut settings = v.settings(toml::Table::new());
    settings.env.clear();
    let mut rt = v.load(settings, fast_limits());
    done(&mut rt, "[vat-g5y] A task").unwrap();
    assert_eq!(v.read("backlog.md"), format!("{HEADER}- [vat-h8x] Other\n"));
    assert_eq!(v.read(".used-ids"), "vat-aaa\nvat-g5y\n");
}

// @spec VATX-DONE-003
#[test]
fn done_does_not_duplicate_used_id() {
    let v = Vat::with_backlog(&format!("{HEADER}- [vat-g5y] A task\n- [vat-h8x] Other\n"));
    v.write(".used-ids", "vat-g5y\nvat-h8x\n");
    let mut rt = v.rt_anon();
    done(&mut rt, "[vat-g5y] A task").unwrap();
    assert_eq!(v.read(".used-ids"), "vat-g5y\nvat-h8x\n");
    assert_eq!(v.read("backlog.md"), format!("{HEADER}- [vat-h8x] Other\n"));
}

// @spec VATX-DONE-003
#[test]
fn done_creates_used_ids_and_tolerates_missing_item_file() {
    let v = Vat::with_backlog(&format!("{HEADER}- [vat-g5y] A task\n"));
    let mut rt = v.rt_anon();
    done(&mut rt, "[vat-g5y] A task").unwrap();
    assert_eq!(v.read(".used-ids"), "vat-g5y\n");
    assert_eq!(v.read("backlog.md"), HEADER);
}

// @spec VATX-DONE-003
#[test]
fn done_drops_one_blank_line_to_avoid_double_blank() {
    let v = Vat::with_backlog(&format!(
        "{HEADER}- [vat-aaa] First\n\n- [vat-g5y] Middle\n\n- [vat-h8x] Last\n"
    ));
    let mut rt = v.rt_anon();
    done(&mut rt, "[vat-g5y] Middle").unwrap();
    assert_eq!(
        v.read("backlog.md"),
        format!("{HEADER}- [vat-aaa] First\n\n- [vat-h8x] Last\n")
    );
}

// @spec VATX-DONE-003
#[test]
fn done_strips_blocked_by_from_every_other_bullet() {
    let v = Vat::with_backlog(&format!(
        "{HEADER}- [vat-g5y] Blocker\n- [vat-h8x] [blocked-by:vat-g5y] One\n- [vat-k2m] [in-progress] [by:bob] [blocked-by:vat-g5y] Two\n- [vat-m3n] [blocked-by:vat-h8x] Three\n"
    ));
    let mut rt = v.rt_anon();
    done(&mut rt, "[vat-g5y] Blocker").unwrap();
    assert_eq!(
        v.read("backlog.md"),
        format!("{HEADER}- [vat-h8x] One\n- [vat-k2m] [in-progress] [by:bob] Two\n- [vat-m3n] [blocked-by:vat-h8x] Three\n")
    );
}

// @spec VATX-DONE-004
#[test]
fn done_on_already_completed_id_changes_nothing() {
    let v = Vat::with_backlog(&format!("{HEADER}- [vat-h8x] Other\n"));
    v.write(".used-ids", "vat-aaa\nvat-g5y\n");
    let before = v.snapshot();
    let mut rt = v.rt_anon();
    done(&mut rt, "[vat-g5y] A task").unwrap();
    assert_eq!(v.snapshot(), before);
}

// @spec VATX-DONE-005
#[test]
fn done_on_unknown_id_vetoes() {
    let v = Vat::fixture("done");
    let before = v.snapshot();
    let mut rt = v.rt_anon();
    assert_eq!(veto(done(&mut rt, "[vat-zzz] Gone")), "unknown id: vat-zzz");
    assert_eq!(v.snapshot(), before);
}

// @spec VATX-DONE-005
#[test]
fn done_with_missing_backlog_md_vetoes() {
    let v = Vat::new();
    v.write("vat.toml", VAT_TOML);
    v.write(".used-ids", "vat-aaa\n");
    let before = v.snapshot();
    let mut rt = v.rt_anon();
    assert_eq!(
        veto(done(&mut rt, "[vat-g5y] A task")),
        "unknown id: vat-g5y"
    );
    assert_eq!(v.snapshot(), before);
}

// @spec VATX-DONE-006
#[test]
fn done_on_duplicate_id_vetoes() {
    let v = Vat::with_backlog(&format!("{HEADER}- [vat-g5y] First\n- [vat-g5y] Second\n"));
    let before = v.snapshot();
    let mut rt = v.rt_anon();
    assert_duplicate_msg(&veto(done(&mut rt, "[vat-g5y] First")), "vat-g5y");
    assert_eq!(v.snapshot(), before);
}

// @spec VATX-DONE-007
#[test]
fn done_without_vat_toml_vetoes_id_shaped_tags() {
    let v = Vat::fixture("done");
    fs::remove_file(v.backlog.join("vat.toml")).unwrap();
    let before = v.snapshot();
    for text in [
        "[vat-g5y] Blocker",
        "[abc-123] Foreign",
        "Note [VAT-G5Y] later",
    ] {
        let mut rt = v.rt_anon();
        assert_init_pointer(&veto(done(&mut rt, text)));
    }
    assert_eq!(v.snapshot(), before);
}

// @spec VATX-DONE-007
#[test]
fn done_without_vat_toml_ignores_text_without_id_shaped_tag() {
    let v = Vat::fixture("done");
    fs::remove_file(v.backlog.join("vat.toml")).unwrap();
    let before = v.snapshot();
    for text in [
        "Fix login bug",
        "[TODO] thing",
        "vat-g5y unbracketed",
        "[vat-g5] short",
    ] {
        let mut rt = v.rt_anon();
        done(&mut rt, text).unwrap_or_else(|e| panic!("text {text:?}: {e:?}"));
    }
    assert_eq!(v.snapshot(), before);
}

// @spec VATX-DONE-007
#[test]
fn done_with_invalid_project_id_vetoes_id_shaped_tags() {
    let v = Vat::fixture("done");
    v.write("vat.toml", "[project]\nid = \"vatt\"\n");
    let before = v.snapshot();
    let mut rt = v.rt_anon();
    assert_init_pointer(&veto(done(&mut rt, "[vat-g5y] Blocker")));
    let mut rt = v.rt_anon();
    done(&mut rt, "Fix login bug").unwrap();
    assert_eq!(v.snapshot(), before);
}

// ════════════════════════════════════════════════════════════════════════════
// Backlog file handling
// ════════════════════════════════════════════════════════════════════════════

// @spec VATX-FMT-001
#[test]
fn crlf_backlog_is_normalized_on_claim() {
    let v = Vat::with_backlog(
        "---\r\nversion: 1\r\n---\r\n\r\n# Tasks\r\n- [vat-g5y] A task\r\n- [vat-h8x] Other\r\n",
    );
    let mut rt = v.rt("tester");
    assert_eq!(
        start(&mut rt, "vat-g5y").unwrap().as_deref(),
        Some("[vat-g5y] A task")
    );
    assert_eq!(
        v.read("backlog.md"),
        format!(
            "{HEADER}# Tasks\n- [vat-g5y] [in-progress] [by:tester] A task\n- [vat-h8x] Other\n"
        )
    );
}

// @spec VATX-FMT-001
#[test]
fn crlf_backlog_is_normalized_on_done() {
    let v = Vat::with_backlog(
        "---\r\nversion: 1\r\n---\r\n\r\n- [vat-g5y] A task\r\n- [vat-h8x] [blocked-by:vat-g5y] Other\r\n",
    );
    let mut rt = v.rt_anon();
    done(&mut rt, "[vat-g5y] A task").unwrap();
    assert_eq!(v.read("backlog.md"), format!("{HEADER}- [vat-h8x] Other\n"));
    assert_eq!(v.read(".used-ids"), "vat-g5y\n");
}

// @spec VATX-FMT-001
#[test]
fn written_files_end_with_exactly_one_newline() {
    let v = Vat::with_backlog(&format!("{HEADER}- [vat-g5y] A task\n- [vat-h8x] Other"));
    let mut rt = v.rt("tester");
    start(&mut rt, "vat-g5y").unwrap();
    assert_eq!(
        v.read("backlog.md"),
        format!("{HEADER}- [vat-g5y] [in-progress] [by:tester] A task\n- [vat-h8x] Other\n")
    );

    let v = Vat::with_backlog(&format!("{HEADER}- [vat-g5y] A task\n- [vat-h8x] Other"));
    v.write(".used-ids", "vat-aaa");
    let mut rt = v.rt_anon();
    done(&mut rt, "[vat-g5y] A task").unwrap();
    assert_eq!(v.read("backlog.md"), format!("{HEADER}- [vat-h8x] Other\n"));
    assert_eq!(v.read(".used-ids"), "vat-aaa\nvat-g5y\n");
}

// @spec VATX-FMT-002
#[test]
fn newer_backlog_version_vetoes_start_and_writes_nothing() {
    let v = Vat::with_backlog("---\nversion: 2\n---\n\n- [vat-g5y] A task\n");
    let before = v.snapshot();
    let mut rt = v.rt("tester");
    assert_upgrade_msg(&veto(start(&mut rt, "vat-g5y")));
    assert_eq!(v.snapshot(), before);
}

// @spec VATX-FMT-002
#[test]
fn newer_backlog_version_vetoes_done_and_writes_nothing() {
    let v = Vat::with_backlog("---\nversion: 2\n---\n\n- [vat-g5y] A task\n");
    v.write("items/vat-g5y.md", "---\nid: vat-g5y\n---\n\nNotes.\n");
    let before = v.snapshot();
    let mut rt = v.rt_anon();
    assert_upgrade_msg(&veto(done(&mut rt, "[vat-g5y] A task")));
    assert_eq!(v.snapshot(), before);
}

const RICH_HEAD: &str = "---\nversion: 1\nowner: someone   # unknown key\nlabels: [a, b]\n---\n\n# Backlog\n\nSome preamble paragraph.\n* star line in preamble\n\n";
const RICH_TAIL: &str = "- [vat-h8x]  [by:bob]   odd   spacing   \n  indented note under h8x\n- [vat-k2m] [blocked-by:vat-g5y] Blocked task\n- [vat-m3n] [TODO] keep me\n\n---\n\n## Freeform\n- [vat-p4q] [blocked-by:vat-g5y] not parsed\nrandom text   with spaces  \n";

// @spec VATX-FMT-003
#[test]
fn claim_preserves_everything_it_does_not_change_byte_for_byte() {
    let v = Vat::with_backlog(&format!(
        "{RICH_HEAD}- [vat-g5y] A task\n  a note line\n    deeper note\n{RICH_TAIL}"
    ));
    let mut rt = v.rt("tester");
    start(&mut rt, "vat-g5y").unwrap();
    assert_eq!(
        v.read("backlog.md"),
        format!(
            "{RICH_HEAD}- [vat-g5y] [in-progress] [by:tester] A task\n  a note line\n    deeper note\n{RICH_TAIL}"
        )
    );
}

// @spec VATX-FMT-003, VATX-DONE-003
#[test]
fn done_preserves_everything_it_does_not_change_byte_for_byte() {
    let v = Vat::with_backlog(&format!("{RICH_HEAD}- [vat-g5y] A task\n{RICH_TAIL}"));
    let mut rt = v.rt_anon();
    done(&mut rt, "[vat-g5y] A task").unwrap();
    // Only the done bullet goes and only k2m's marker is stripped; the freeform
    // region's `[blocked-by:vat-g5y]` stays.
    let tail = RICH_TAIL.replace(
        "- [vat-k2m] [blocked-by:vat-g5y] Blocked task\n",
        "- [vat-k2m] Blocked task\n",
    );
    assert_eq!(v.read("backlog.md"), format!("{RICH_HEAD}{tail}"));
}

// @spec VATX-FMT-004
#[test]
fn unknown_markers_are_title_and_preserved_in_place() {
    let v = Vat::with_backlog(&format!(
        "{HEADER}- [vat-g5y] [parent:vat-h8x] Child task\n- [vat-h8x] Parent\n"
    ));
    let mut rt = v.rt("tester");
    assert_eq!(
        start(&mut rt, "vat-g5y").unwrap().as_deref(),
        Some("[vat-g5y] [parent:vat-h8x] Child task")
    );
    assert_eq!(
        v.read("backlog.md"),
        format!("{HEADER}- [vat-g5y] [in-progress] [by:tester] [parent:vat-h8x] Child task\n- [vat-h8x] Parent\n")
    );
}

// A `[by:...]` after the first unknown token is title text, not a claim.
// @spec VATX-FMT-004
#[test]
fn known_marker_after_title_start_is_not_a_marker() {
    let v = Vat::with_backlog(&format!("{HEADER}- [vat-g5y] [TODO] [by:bob] thing\n"));
    let mut rt = v.rt("tester");
    let text = start(&mut rt, "vat-g5y").unwrap().expect("names the entry");
    assert!(
        text.starts_with("[vat-g5y] [TODO] ") && text.ends_with(" thing"),
        "title kept: {text:?}"
    );
    assert_eq!(
        v.read("backlog.md"),
        format!("{HEADER}- [vat-g5y] [in-progress] [by:tester] [TODO] [by:bob] thing\n")
    );
}

// Known markers are matched left to right whatever their order.
// @spec VATX-FMT-004
#[test]
fn known_markers_matched_in_any_order_before_title() {
    let v = Vat::with_backlog(&format!(
        "{HEADER}- [vat-g5y] [blocked-by:vat-h8x] [by:bob] A task\n- [vat-h8x] Blocker\n"
    ));
    let before = v.snapshot();
    let mut rt = v.rt("tester");
    assert_eq!(
        veto(start(&mut rt, "vat-g5y")),
        "vat-g5y already claimed by bob"
    );
    assert_eq!(v.snapshot(), before);
}

// ════════════════════════════════════════════════════════════════════════════
// Nested-repo backlogs
// ════════════════════════════════════════════════════════════════════════════

// The backlog sits inside an ordinary git repo (the main repo) but has no
// `.git` of its own: edited in place, nothing committed.
// @spec VATX-GIT-001
#[test]
fn backlog_without_own_git_is_edited_in_place_without_git() {
    let v = Vat::new();
    let main = v.layout.main_repo.clone();
    git(&main, &["init", "-q", "-b", "main"]);
    configure(&main, "tester");
    for (rel, content) in nested_seed() {
        v.write(rel, content);
    }
    commit_all(&main, "seed");
    let main_head = head(&main);

    let mut rt = v.rt("tester");
    assert_eq!(
        start(&mut rt, "vat-g5y").unwrap().as_deref(),
        Some("[vat-g5y] A task")
    );
    assert_eq!(v.read("backlog.md"), SEED_CLAIMED);
    assert_eq!(head(&main), main_head, "no commit");
    assert_eq!(porcelain(&main), "M backlog/backlog.md");
    assert!(!v.exists(".git"));

    let mut rt = v.rt("tester");
    done(&mut rt, "[vat-g5y] A task").unwrap();
    assert_eq!(head(&main), main_head, "no commit");
    assert!(
        rt.take_messages().is_empty(),
        "no not-pushed warning for a plain backlog"
    );
}

// The guard's fetch runs before anything changes: the remote-tracking ref
// is updated even on the local-edit-only path.
// @spec VATX-GIT-002
#[test]
fn guard_fetches_before_changing_anything() {
    let v = Vat::new();
    let n = Nested::setup(&v, &nested_seed());
    n.other_push(&[("noise.txt", Some("x\n"))], "noise");
    v.write("scratch.txt", "uncommitted\n");
    let local_head = head(&v.backlog);

    let mut rt = v.load(v.settings(user("tester")), loop_limits());
    start(&mut rt, "vat-g5y").unwrap();
    assert_eq!(
        git(&v.backlog, &["rev-parse", "refs/remotes/origin/main"]),
        n.remote_head(),
        "guard ran git fetch"
    );
    assert_eq!(
        head(&v.backlog),
        local_head,
        "dirty tree: no reset or commit"
    );
}

// A `.git` *file* (linked worktree / submodule) also marks a nested backlog.
// @spec VATX-GIT-002, VATX-GIT-005
#[test]
fn git_file_backlog_is_treated_as_nested_repo() {
    let v = Vat::new();
    let scratch = v.layout.root.join("git");
    fs::create_dir_all(&scratch).unwrap();
    git(
        &scratch,
        &["init", "-q", "--bare", "-b", "main", "remote.git"],
    );
    let bare = scratch.join("remote.git");
    let seed = scratch.join("seed");
    git(
        &scratch,
        &[
            "clone",
            "-q",
            bare.to_str().unwrap(),
            seed.to_str().unwrap(),
        ],
    );
    git(&seed, &["symbolic-ref", "HEAD", "refs/heads/main"]);
    configure(&seed, "tester");
    for (rel, content) in nested_seed() {
        write_file(&seed.join(rel), content);
    }
    commit_all(&seed, "seed");
    git(&seed, &["push", "-q", "-u", "origin", "main"]);
    git(&seed, &["checkout", "-q", "-b", "parking"]);
    fs::remove_dir(&v.backlog).unwrap();
    git(
        &seed,
        &["worktree", "add", "-q", v.backlog.to_str().unwrap(), "main"],
    );
    assert!(v.backlog.join(".git").is_file());

    let mut rt = v.load(v.settings(user("tester")), loop_limits());
    start(&mut rt, "vat-g5y").unwrap();
    assert_eq!(
        git(&bare, &["log", "-1", "--format=%s", "main"]),
        "vat start vat-g5y"
    );
    assert_eq!(
        git(&bare, &["show", "main:backlog.md"]) + "\n",
        SEED_CLAIMED
    );
}

// @spec VATX-GIT-003
#[test]
fn no_upstream_vetoes_with_git_error_and_changes_nothing() {
    let v = Vat::new();
    git(&v.backlog, &["init", "-q", "-b", "main"]);
    configure(&v.backlog, "tester");
    for (rel, content) in nested_seed() {
        v.write(rel, content);
    }
    commit_all(&v.backlog, "seed");
    let before = v.snapshot();
    let local_head = head(&v.backlog);

    let mut rt = v.load(v.settings(user("tester")), loop_limits());
    let msg = veto(start(&mut rt, "vat-g5y"));
    assert!(
        msg.contains("upstream"),
        "git's no-upstream error, got {msg:?}"
    );
    assert_eq!(v.snapshot(), before);
    assert_eq!(head(&v.backlog), local_head);

    let mut rt = v.load(v.settings(user("tester")), loop_limits());
    let msg = veto(done(&mut rt, "[vat-g5y] A task"));
    assert!(
        msg.contains("upstream"),
        "git's no-upstream error, got {msg:?}"
    );
    assert_eq!(v.snapshot(), before);
}

// @spec VATX-GIT-003
#[test]
fn failed_fetch_vetoes_with_git_error_and_changes_nothing() {
    let v = Vat::new();
    let n = Nested::setup(&v, &nested_seed());
    let missing = n.scratch.join("no-such-remote.git");
    git(
        &v.backlog,
        &["remote", "set-url", "origin", missing.to_str().unwrap()],
    );
    let before = v.snapshot();
    let local_head = head(&v.backlog);

    let mut rt = v.load(v.settings(user("tester")), loop_limits());
    let msg = veto(start(&mut rt, "vat-g5y"));
    assert!(
        msg.contains("does not appear to be a git repository"),
        "git's fetch error, got {msg:?}"
    );
    assert_eq!(v.snapshot(), before);
    assert_eq!(head(&v.backlog), local_head);
}

// @spec VATX-GIT-004
#[test]
fn dirty_nested_backlog_edits_locally_and_warns() {
    let v = Vat::new();
    let n = Nested::setup(&v, &nested_seed());
    v.write("scratch.txt", "in-flight work\n");
    let local_head = head(&v.backlog);
    let remote_head = n.remote_head();

    let mut rt = v.load(v.settings(user("tester")), loop_limits());
    assert_eq!(
        start(&mut rt, "vat-g5y").unwrap().as_deref(),
        Some("[vat-g5y] A task")
    );
    assert_eq!(v.read("backlog.md"), SEED_CLAIMED);
    assert_eq!(
        v.read("scratch.txt"),
        "in-flight work\n",
        "inherited work untouched"
    );
    assert_eq!(head(&v.backlog), local_head, "no commit");
    assert_eq!(n.remote_head(), remote_head, "no push");
    let msgs = rt.take_messages();
    assert!(
        msgs.iter()
            .any(|m| m.contains("vat-g5y: not pushed; backlog has uncommitted changes")),
        "{msgs:?}"
    );
}

// @spec VATX-GIT-004
#[test]
fn dirty_nested_backlog_done_edits_locally_and_warns() {
    let v = Vat::new();
    let n = Nested::setup(&v, &nested_seed());
    v.write("scratch.txt", "in-flight work\n");
    let local_head = head(&v.backlog);
    let remote_head = n.remote_head();

    let mut rt = v.load(v.settings(toml::Table::new()), loop_limits());
    done(&mut rt, "[vat-g5y] A task").unwrap();
    assert_eq!(v.read("backlog.md"), SEED_DONE);
    assert_eq!(v.read(".used-ids"), "vat-aaa\nvat-g5y\n");
    assert!(!v.exists("items/vat-g5y.md"));
    assert_eq!(head(&v.backlog), local_head, "no commit");
    assert_eq!(n.remote_head(), remote_head, "no push");
    let msgs = rt.take_messages();
    assert!(
        msgs.iter()
            .any(|m| m.contains("vat-g5y: not pushed; backlog has uncommitted changes")),
        "{msgs:?}"
    );
}

// @spec VATX-GIT-004
#[test]
fn ahead_nested_backlog_edits_locally_and_warns() {
    let v = Vat::new();
    let n = Nested::setup(&v, &nested_seed());
    v.write("notes.txt", "local commit\n");
    commit_all(&v.backlog, "unpushed");
    let local_head = head(&v.backlog);
    let remote_head = n.remote_head();

    let mut rt = v.load(v.settings(user("tester")), loop_limits());
    start(&mut rt, "vat-g5y").unwrap();
    assert_eq!(v.read("backlog.md"), SEED_CLAIMED);
    assert_eq!(
        head(&v.backlog),
        local_head,
        "local commit kept, nothing new committed"
    );
    assert_eq!(n.remote_head(), remote_head, "no push");
    let msgs = rt.take_messages();
    assert!(
        msgs.iter()
            .any(|m| m.contains("vat-g5y: not pushed; backlog has unpushed local commits")),
        "{msgs:?}"
    );
}

// @spec VATX-GIT-005
#[test]
fn clean_nested_backlog_claim_is_committed_and_pushed() {
    let v = Vat::new();
    let n = Nested::setup(&v, &nested_seed());

    let mut rt = v.load(v.settings(user("tester")), loop_limits());
    assert_eq!(
        start(&mut rt, "vat-g5y").unwrap().as_deref(),
        Some("[vat-g5y] A task")
    );
    assert_eq!(n.remote_subjects(), vec!["vat start vat-g5y", "seed"]);
    assert_eq!(n.remote_file("backlog.md").as_deref(), Some(SEED_CLAIMED));
    assert_eq!(head(&v.backlog), n.remote_head());
    assert_eq!(porcelain(&v.backlog), "");
    assert!(rt.take_messages().iter().all(|m| !m.contains("not pushed")));
}

// @spec VATX-GIT-005
#[test]
fn clean_nested_backlog_done_is_committed_and_pushed() {
    let v = Vat::new();
    let n = Nested::setup(&v, &nested_seed());

    let mut rt = v.load(v.settings(toml::Table::new()), loop_limits());
    done(&mut rt, "[vat-g5y] A task").unwrap();
    assert_eq!(n.remote_subjects(), vec!["vat done vat-g5y", "seed"]);
    assert_eq!(n.remote_file("backlog.md").as_deref(), Some(SEED_DONE));
    assert_eq!(
        n.remote_file(".used-ids").as_deref(),
        Some("vat-aaa\nvat-g5y\n")
    );
    assert_eq!(n.remote_file("items/vat-g5y.md"), None);
    assert_eq!(head(&v.backlog), n.remote_head());
    assert_eq!(porcelain(&v.backlog), "");
}

// Behind upstream counts as synced; the loop resets to @{u} and applies the
// change to the fresh files, keeping the other user's commit.
// @spec VATX-GIT-005
#[test]
fn claim_loop_applies_change_to_fresh_upstream_state() {
    let v = Vat::new();
    let n = Nested::setup(&v, &nested_seed());
    let theirs =
        format!("{HEADER}- [vat-g5y] A task\n- [vat-h8x] Other task\n- [vat-k2m] Added upstream\n");
    n.other_push(&[("backlog.md", Some(&theirs))], "other adds a task");

    let mut rt = v.load(v.settings(user("tester")), loop_limits());
    start(&mut rt, "vat-g5y").unwrap();
    assert_eq!(
        n.remote_subjects(),
        vec!["vat start vat-g5y", "other adds a task", "seed"]
    );
    assert_eq!(
        n.remote_file("backlog.md").as_deref(),
        Some(
            format!("{HEADER}- [vat-g5y] [in-progress] [by:tester] A task\n- [vat-h8x] Other task\n- [vat-k2m] Added upstream\n")
                .as_str()
        )
    );
    assert_eq!(head(&v.backlog), n.remote_head());
}

// A rejected push is reset away, backed off and retried; the retry wins.
// @spec VATX-GIT-006, VATX-GIT-005
#[test]
fn rejected_push_is_retried_after_reset_and_backoff() {
    let v = Vat::new();
    let n = Nested::setup(&v, &nested_seed());
    let count = n.race_hook(&v.backlog, true, "echo x >> \"$O/noise.txt\"", "noise");

    let mut rt = v.load(v.settings(user("tester")), loop_limits());
    let t0 = Instant::now();
    assert_eq!(
        start(&mut rt, "vat-g5y").unwrap().as_deref(),
        Some("[vat-g5y] A task")
    );
    let elapsed = t0.elapsed();

    assert_eq!(
        n.remote_subjects(),
        vec!["vat start vat-g5y", "noise", "seed"]
    );
    assert_eq!(n.remote_file("backlog.md").as_deref(), Some(SEED_CLAIMED));
    assert_eq!(n.remote_file("noise.txt").as_deref(), Some("x\n"));
    assert_eq!(
        line_count(&count),
        2,
        "one rejected attempt, one winning attempt"
    );
    assert_eq!(head(&v.backlog), n.remote_head());
    assert_eq!(porcelain(&v.backlog), "");
    assert!(
        elapsed >= Duration::from_millis(500),
        "first retry sleeps at least 0.5 s, took {elapsed:?}"
    );
}

// @spec VATX-GIT-007, VATX-GIT-006
#[test]
fn five_rejected_pushes_veto_lost_race() {
    let v = Vat::new();
    let n = Nested::setup(&v, &nested_seed());
    let count = n.race_hook(&v.backlog, false, "echo x >> \"$O/noise.txt\"", "noise");

    let mut rt = v.load(v.settings(user("tester")), loop_limits());
    let t0 = Instant::now();
    assert_eq!(
        veto(start(&mut rt, "vat-g5y")),
        "lost race claiming vat-g5y"
    );
    let elapsed = t0.elapsed();

    assert_eq!(line_count(&count), 5, "exactly 5 attempts");
    assert!(
        !n.remote_subjects()
            .iter()
            .any(|s| s.starts_with("vat start")),
        "no claim reached the remote"
    );
    assert_eq!(
        head(&v.backlog),
        git(&v.backlog, &["rev-parse", "@{u}"]),
        "no local commit remains"
    );
    // Backoff before attempts 2..5: 0.5 + 1 + 2 + 4 s (plus jitter).
    assert!(
        elapsed >= Duration::from_millis(7500),
        "backoff schedule, took {elapsed:?}"
    );
}

// @spec VATX-GIT-008
#[test]
fn failed_commit_in_loop_resets_and_vetoes_with_git_error() {
    let v = Vat::new();
    let n = Nested::setup(&v, &nested_seed());
    write_hook(
        &v.backlog,
        "pre-commit",
        "echo 'pre-commit hook says no' >&2\nexit 1",
    );
    let remote_head = n.remote_head();

    let mut rt = v.load(v.settings(user("tester")), loop_limits());
    let msg = veto(start(&mut rt, "vat-g5y"));
    assert!(
        msg.contains("pre-commit hook says no"),
        "git's error output, got {msg:?}"
    );
    assert_eq!(n.remote_head(), remote_head);
    assert_eq!(head(&v.backlog), remote_head, "no local commit");
    assert_eq!(v.read("backlog.md"), SEED_BACKLOG, "reset to upstream");
    assert_eq!(porcelain(&v.backlog), "");
}

// A push failure that is not contention fails fast (no retries).
// @spec VATX-GIT-008, VATX-GIT-006
#[test]
fn non_contention_push_failure_vetoes_without_retry() {
    let v = Vat::new();
    let n = Nested::setup(&v, &nested_seed());
    let count = n.scratch.join("pre-push-count");
    write_hook(
        &v.backlog,
        "pre-push",
        &format!(
            "echo x >> '{}'\necho 'pre-push hook refuses' >&2\nexit 1",
            count.display()
        ),
    );
    let remote_head = n.remote_head();

    let mut rt = v.load(v.settings(user("tester")), loop_limits());
    let msg = veto(start(&mut rt, "vat-g5y"));
    assert!(
        msg.contains("pre-push hook refuses"),
        "git's error output, got {msg:?}"
    );
    assert_eq!(
        line_count(&count),
        1,
        "no retry for a non-contention failure"
    );
    assert_eq!(n.remote_head(), remote_head);
    assert_eq!(head(&v.backlog), remote_head, "no local commit remains");
}

// The handler's time budget runs out mid-loop (a slow pre-push hook): the
// extension still resets so no local commit remains, and vetoes.
// @spec VATX-GIT-008
#[test]
fn timeout_in_loop_resets_and_vetoes() {
    let v = Vat::new();
    let n = Nested::setup(&v, &nested_seed());
    write_hook(&v.backlog, "pre-push", "sleep 4");
    let remote_head = n.remote_head();

    let mut rt = v.load(v.settings(user("tester")), fast_limits());
    let _ = veto(start(&mut rt, "vat-g5y"));
    assert_eq!(n.remote_head(), remote_head);
    assert_eq!(head(&v.backlog), remote_head, "no local commit remains");
}

// Another user's claim lands between our commit and push: after the reset,
// fresh state shows their claim.
// @spec VATX-GIT-009
#[test]
fn lost_claim_when_fresh_state_claimed_by_other_after_rejection() {
    let v = Vat::new();
    let n = Nested::setup(&v, &nested_seed());
    let bob = n.scratch.join("bob-backlog.md");
    write_file(
        &bob,
        &format!("{HEADER}- [vat-g5y] [in-progress] [by:bob] A task\n- [vat-h8x] Other task\n"),
    );
    let count = n.race_hook(
        &v.backlog,
        true,
        &format!("cp '{}' \"$O/backlog.md\"", bob.display()),
        "bob claims",
    );

    let mut rt = v.load(v.settings(user("tester")), loop_limits());
    assert_eq!(
        veto(start(&mut rt, "vat-g5y")),
        "lost claim: vat-g5y already claimed by bob"
    );
    assert_eq!(n.remote_subjects(), vec!["bob claims", "seed"]);
    assert_eq!(line_count(&count), 1, "no second commit");
    assert_eq!(head(&v.backlog), n.remote_head());
    assert_eq!(porcelain(&v.backlog), "");
}

// The local clone is behind (clean, so synced); upstream already has bob's claim.
// @spec VATX-GIT-009
#[test]
fn lost_claim_when_upstream_already_claimed_by_other() {
    let v = Vat::new();
    let n = Nested::setup(&v, &nested_seed());
    let bobs =
        format!("{HEADER}- [vat-g5y] [in-progress] [by:bob] A task\n- [vat-h8x] Other task\n");
    n.other_push(&[("backlog.md", Some(&bobs))], "bob claims");
    let remote_head = n.remote_head();

    let mut rt = v.load(v.settings(user("tester")), loop_limits());
    assert_eq!(
        veto(start(&mut rt, "vat-g5y")),
        "lost claim: vat-g5y already claimed by bob"
    );
    assert_eq!(n.remote_head(), remote_head);
}

// @spec VATX-GIT-009, VATX-START-004
#[test]
fn fresh_state_claimed_by_this_user_succeeds_without_commit() {
    let v = Vat::new();
    let n = Nested::setup(&v, &nested_seed());
    n.other_push(
        &[("backlog.md", Some(SEED_CLAIMED))],
        "claimed elsewhere by tester",
    );
    let remote_head = n.remote_head();

    let mut rt = v.load(v.settings(user("tester")), loop_limits());
    assert_eq!(
        start(&mut rt, "vat-g5y").unwrap().as_deref(),
        Some("[vat-g5y] A task")
    );
    assert_eq!(n.remote_head(), remote_head, "no new commit");
    assert_eq!(head(&v.backlog), remote_head);
    assert_eq!(v.read("backlog.md"), SEED_CLAIMED);
}

// @spec VATX-GIT-009, VATX-DONE-004
#[test]
fn fresh_state_already_done_succeeds_without_commit() {
    let v = Vat::new();
    let n = Nested::setup(&v, &nested_seed());
    n.other_push(
        &[
            ("backlog.md", Some(SEED_DONE)),
            (".used-ids", Some("vat-aaa\nvat-g5y\n")),
            ("items/vat-g5y.md", None),
        ],
        "done elsewhere",
    );
    let remote_head = n.remote_head();

    let mut rt = v.load(v.settings(toml::Table::new()), loop_limits());
    done(&mut rt, "[vat-g5y] A task").unwrap();
    assert_eq!(n.remote_head(), remote_head, "no new commit");
    assert_eq!(head(&v.backlog), remote_head);
}

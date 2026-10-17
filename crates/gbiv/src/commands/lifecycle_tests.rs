//! gbv-1ht tests for `gbiv start` / `gbiv done` (Phase 5).
//!
//! Each test builds a throwaway gbiv workspace (`<base>/proj/main/proj` as a git
//! repo plus empty color dirs), drives the command through `CmdEnv` + a
//! recording `Sink`, and asserts on GBIV.md, the error, and the output order.
//! Extensions are small inline Lua files under `<root>/.gbiv/extensions/<name>/`,
//! enabled through `<root>/.gbiv/config.toml`.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use tempfile::TempDir;

use crate::commands::done::done_command;
use crate::commands::lifecycle::{CmdEnv, Sink};
use crate::commands::start::start_command;
use crate::extensions::test_support::{fast_limits, manifest};

const BASE: [&str; 7] = [
    "red", "orange", "yellow", "green", "blue", "indigo", "violet",
];

// ---- recording sink ---------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
enum Line {
    Out(String),
    Err(String),
}

/// Records every line in order. For each stdout line it also snapshots the
/// contents of the `watch` files, so a test can tell what had happened on disk
/// at the moment the line was printed.
#[derive(Default)]
struct Rec {
    lines: Vec<Line>,
    watch: Vec<PathBuf>,
    at_out: Vec<Vec<Option<String>>>,
}

impl Sink for Rec {
    fn out(&mut self, line: &str) {
        self.lines.push(Line::Out(line.to_string()));
        let snap = self
            .watch
            .iter()
            .map(|p| fs::read_to_string(p).ok())
            .collect();
        self.at_out.push(snap);
    }
    fn err(&mut self, line: &str) {
        self.lines.push(Line::Err(line.to_string()));
    }
}

impl Rec {
    fn outs(&self) -> Vec<String> {
        self.lines
            .iter()
            .filter_map(|l| match l {
                Line::Out(s) => Some(s.clone()),
                _ => None,
            })
            .collect()
    }
    fn errs(&self) -> Vec<String> {
        self.lines
            .iter()
            .filter_map(|l| match l {
                Line::Err(s) => Some(s.clone()),
                _ => None,
            })
            .collect()
    }
    fn any_line_contains(&self, needle: &str) -> bool {
        self.lines.iter().any(|l| match l {
            Line::Out(s) | Line::Err(s) => s.contains(needle),
        })
    }
    fn index_of(&self, pred: impl Fn(&Line) -> bool) -> Option<usize> {
        self.lines.iter().position(pred)
    }
}

// ---- workspace --------------------------------------------------------------

fn git(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .expect("run git");
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

fn init_repo_with_commit(dir: &Path, branch: &str) {
    fs::create_dir_all(dir).unwrap();
    git(dir, &["init", "-q"]);
    git(dir, &["config", "user.email", "test@example.com"]);
    git(dir, &["config", "user.name", "Test"]);
    git(dir, &["config", "commit.gpgsign", "false"]);
    fs::write(dir.join("README.md"), "hello\n").unwrap();
    git(dir, &["add", "."]);
    git(dir, &["commit", "-q", "-m", "initial"]);
    git(dir, &["branch", "-M", branch]);
}

struct Ws {
    _tmp: TempDir,
    base: PathBuf,
    root: PathBuf,
    main_repo: PathBuf,
    home: PathBuf,
    xdg: PathBuf,
}

/// `<base>/proj/{main/proj (git repo), red..violet (empty)}` plus an empty
/// temp home and XDG config dir, so no real user config leaks in.
fn ws() -> Ws {
    let tmp = TempDir::new().unwrap();
    let base = tmp.path().to_path_buf();
    let root = base.join("proj");
    let main_repo = root.join("main").join("proj");
    init_repo_with_commit(&main_repo, "main");
    for c in BASE {
        fs::create_dir_all(root.join(c)).unwrap();
    }
    let home = base.join("home");
    let xdg = base.join("xdg");
    fs::create_dir_all(&home).unwrap();
    fs::create_dir_all(&xdg).unwrap();
    Ws {
        _tmp: tmp,
        base,
        root,
        main_repo,
        home,
        xdg,
    }
}

impl Ws {
    fn gbiv_md(&self) -> PathBuf {
        self.main_repo.join("GBIV.md")
    }
    fn write_md(&self, content: &str) {
        fs::write(self.gbiv_md(), content).unwrap();
    }
    fn read_md(&self) -> Option<String> {
        fs::read_to_string(self.gbiv_md()).ok()
    }
    /// Write the project config `<root>/.gbiv/config.toml`.
    fn config(&self, toml: &str) {
        let dir = self.root.join(".gbiv");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("config.toml"), toml).unwrap();
    }
    /// Install an extension in the project extension dir.
    fn ext(&self, name: &str, caps: &[&str], init_lua: &str) {
        let dir = self.root.join(".gbiv").join("extensions").join(name);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("extension.toml"), manifest(name, caps)).unwrap();
        fs::write(dir.join("init.lua"), init_lua).unwrap();
    }
    /// Project config enabling `names`, followed by `extra` TOML.
    fn enable(&self, names: &[&str], extra: &str) {
        let list: Vec<String> = names.iter().map(|n| format!("{n:?}")).collect();
        self.config(&format!(
            "[extensions]\nenabled = [{}]\n{extra}",
            list.join(", ")
        ));
    }
    /// A directory extensions may write marker files into, and the TOML that
    /// grants it to `ext` (requires the `fs` capability in the manifest).
    fn marks(&self, ext: &str) -> (PathBuf, String) {
        let dir = self.base.join("marks");
        fs::create_dir_all(&dir).unwrap();
        let grant = format!(
            "[extensions.{ext}]\npaths = [{:?}]\n",
            dir.to_str().unwrap()
        );
        (dir, grant)
    }
    fn env_at(&self, cwd: &Path) -> CmdEnv {
        CmdEnv {
            cwd: cwd.to_path_buf(),
            home: self.home.clone(),
            xdg_config_home: Some(self.xdg.clone()),
            limits: fast_limits(),
        }
    }
    fn start(&self, task: &str, color: Option<&str>) -> (anyhow::Result<()>, Rec) {
        let mut rec = Rec::default();
        let r = start_command(task, color, &self.env_at(&self.root), &mut rec);
        (r, rec)
    }
    fn start_rec(
        &self,
        task: &str,
        color: Option<&str>,
        mut rec: Rec,
    ) -> (anyhow::Result<()>, Rec) {
        let r = start_command(task, color, &self.env_at(&self.root), &mut rec);
        (r, rec)
    }
    fn done(&self, arg: Option<&str>) -> (anyhow::Result<()>, Rec) {
        self.done_at(arg, &self.root.clone())
    }
    fn done_at(&self, arg: Option<&str>, cwd: &Path) -> (anyhow::Result<()>, Rec) {
        let mut rec = Rec::default();
        let r = done_command(arg, &self.env_at(cwd), &mut rec);
        (r, rec)
    }
    fn done_rec(&self, arg: Option<&str>, mut rec: Rec) -> (anyhow::Result<()>, Rec) {
        let r = done_command(arg, &self.env_at(&self.root), &mut rec);
        (r, rec)
    }
}

/// The error message with its whole context chain.
fn err_of(r: anyhow::Result<()>) -> String {
    match r {
        Ok(()) => panic!("expected an error, got Ok"),
        Err(e) => format!("{e:#}"),
    }
}

fn all_seven_taken() -> String {
    BASE.iter().map(|c| format!("- [{c}] task {c}\n")).collect()
}

/// Logs `ARG=<..> COLOR=..` in before_start and `AFTER` in after_start.
const LOGGER_START: &str = r#"
return {
  before_start = function(ctx)
    gbiv.log("ARG=<" .. ctx.arg .. "> COLOR=" .. ctx.color)
  end,
  after_start = function(ctx)
    gbiv.log("AFTER " .. ctx.color .. " " .. ctx.text)
  end,
}
"#;

// =============================================================================
// gbiv start — argument validation
// =============================================================================

// @spec FL-START-002
#[test]
fn start_rejects_empty_and_whitespace_only_task() {
    for task in ["", "   ", "\t \t"] {
        let w = ws();
        w.write_md("- [red] existing\n");
        let (r, rec) = w.start(task, None);
        assert!(r.is_err(), "task {task:?} should be rejected");
        assert_eq!(w.read_md().as_deref(), Some("- [red] existing\n"));
        assert!(
            rec.outs().is_empty(),
            "no stdout for rejected task {task:?}"
        );
    }
}

// @spec FL-START-002
#[test]
fn start_rejects_multi_line_task() {
    for task in ["fix\nlogin", "fix\r\nlogin", "fix login\n  more"] {
        let w = ws();
        let (r, rec) = w.start(task, None);
        assert!(r.is_err(), "task {task:?} should be rejected");
        assert_eq!(w.read_md(), None, "GBIV.md must not be created");
        assert!(rec.outs().is_empty());
    }
}

// @spec FL-START-002
#[test]
fn start_rejects_task_starting_with_legacy_status_tag() {
    for task in [
        "[done] fix login",
        "[in-progress] fix login",
        "  [done] fix login  ",
    ] {
        let w = ws();
        w.write_md("- [red] existing\n");
        let (r, rec) = w.start(task, None);
        assert!(r.is_err(), "task {task:?} should be rejected");
        assert_eq!(w.read_md().as_deref(), Some("- [red] existing\n"));
        assert!(rec.outs().is_empty());
    }
}

// @spec FL-START-002
#[test]
fn start_rejected_task_fires_no_extension_event() {
    let w = ws();
    w.ext("logger", &[], LOGGER_START);
    w.enable(&["logger"], "");
    let (r, rec) = w.start("", None);
    assert!(r.is_err());
    assert!(
        !rec.any_line_contains("ARG="),
        "before_start must not fire: {:?}",
        rec.lines
    );
}

// @spec FL-START-002
#[test]
fn start_accepts_task_with_leading_non_status_bracket() {
    let w = ws();
    let (r, _rec) = w.start("[urgent] fix login", None);
    r.unwrap();
    assert_eq!(w.read_md().as_deref(), Some("- [red] [urgent] fix login\n"));
}

// =============================================================================
// gbiv start — choosing the color
// =============================================================================

// @spec FL-START-003, FL-MUTATE-014
#[test]
fn start_picks_red_when_gbiv_md_missing() {
    let w = ws();
    let (r, rec) = w.start("Fix login bug", None);
    r.unwrap();
    assert_eq!(w.read_md().as_deref(), Some("- [red] Fix login bug\n"));
    assert_eq!(
        rec.outs(),
        vec!["started on red: Fix login bug".to_string()]
    );
}

// @spec FL-START-003
#[test]
fn start_picks_first_free_color_in_palette_order() {
    let w = ws();
    // red and yellow taken (out of order), orange free.
    w.write_md("- [yellow] y\n- [red] r\n");
    let (r, rec) = w.start("next", None);
    r.unwrap();
    assert_eq!(rec.outs(), vec!["started on orange: next".to_string()]);
    assert_eq!(
        w.read_md().as_deref(),
        Some("- [yellow] y\n- [red] r\n- [orange] next\n")
    );
}

// @spec FL-START-003
#[test]
fn start_counts_legacy_status_entries_and_ignores_untagged_and_non_palette_tags() {
    let w = ws();
    // red is taken (legacy status still counts); untagged and [purple] take nothing.
    w.write_md("- [red] [done] old\n- backlog item\n- [purple] not a palette color\n");
    let (r, rec) = w.start("next", None);
    r.unwrap();
    assert_eq!(rec.outs(), vec!["started on orange: next".to_string()]);
}

// @spec FL-START-003
#[test]
fn start_ignores_entries_below_separator_when_choosing() {
    let w = ws();
    w.write_md("- [red] r\n---\n- [orange] documented example\n");
    let (r, rec) = w.start("next", None);
    r.unwrap();
    assert_eq!(rec.outs(), vec!["started on orange: next".to_string()]);
}

// @spec FL-START-003
#[test]
fn start_chooses_color_regardless_of_worktree_state() {
    let w = ws();
    // red has no worktree dir at all; orange's is a dirty git repo.
    fs::remove_dir(w.root.join("red")).unwrap();
    let orange = w.root.join("orange").join("proj");
    init_repo_with_commit(&orange, "orange");
    fs::write(orange.join("dirty.txt"), "x").unwrap();
    let (r, rec) = w.start("first", None);
    r.unwrap();
    assert_eq!(rec.outs(), vec!["started on red: first".to_string()]);
    let (r, rec) = w.start("second", None);
    r.unwrap();
    assert_eq!(rec.outs(), vec!["started on orange: second".to_string()]);
}

// @spec FL-START-003
#[test]
fn start_falls_through_to_configured_extras_in_order() {
    let w = ws();
    w.config("[palette]\nextra = [\"alpha\", \"beta\"]\n");
    w.write_md(&all_seven_taken());
    let (r, rec) = w.start("eighth", None);
    r.unwrap();
    assert_eq!(rec.outs(), vec!["started on alpha: eighth".to_string()]);
    let (r, rec) = w.start("ninth", None);
    r.unwrap();
    assert_eq!(rec.outs(), vec!["started on beta: ninth".to_string()]);
    assert!(w
        .read_md()
        .unwrap()
        .ends_with("- [alpha] eighth\n- [beta] ninth\n"));
}

// @spec FL-START-004
#[test]
fn start_with_color_uses_that_color_even_if_earlier_ones_are_free() {
    let w = ws();
    let (r, rec) = w.start("Fix login bug", Some("blue"));
    r.unwrap();
    assert_eq!(
        rec.outs(),
        vec!["started on blue: Fix login bug".to_string()]
    );
    assert_eq!(w.read_md().as_deref(), Some("- [blue] Fix login bug\n"));
}

// @spec FL-START-004
#[test]
fn start_with_color_accepts_configured_extra() {
    let w = ws();
    w.config("[palette]\nextra = [\"alpha\"]\n");
    let (r, rec) = w.start("x", Some("alpha"));
    r.unwrap();
    assert_eq!(rec.outs(), vec!["started on alpha: x".to_string()]);
}

// @spec FL-START-004
#[test]
fn start_with_color_not_in_palette_errors() {
    for c in ["purple", "Red", "main", "alpha"] {
        let w = ws();
        w.write_md("- [orange] o\n");
        let (r, rec) = w.start("x", Some(c));
        let e = err_of(r);
        assert!(e.contains(c), "error should name {c:?}: {e}");
        assert_eq!(w.read_md().as_deref(), Some("- [orange] o\n"));
        assert!(rec.outs().is_empty());
    }
}

// @spec FL-START-004
#[test]
fn start_with_color_that_already_has_an_entry_errors() {
    let w = ws();
    w.write_md("- [blue] b\n");
    let (r, rec) = w.start("x", Some("blue"));
    let e = err_of(r);
    assert!(e.contains("blue"), "{e}");
    assert_eq!(w.read_md().as_deref(), Some("- [blue] b\n"));
    assert!(rec.outs().is_empty());
}

// @spec FL-START-005
#[test]
fn start_errors_when_no_color_is_free() {
    let w = ws();
    w.write_md(&all_seven_taken());
    let (r, rec) = w.start("x", None);
    let e = err_of(r);
    assert!(
        e.contains("no free color (all 7 colors have GBIV.md entries)"),
        "{e}"
    );
    assert_eq!(w.read_md(), Some(all_seven_taken()));
    assert!(rec.outs().is_empty());
}

// @spec FL-START-005
#[test]
fn start_no_free_color_counts_extras() {
    let w = ws();
    w.config("[palette]\nextra = [\"alpha\", \"beta\"]\n");
    w.write_md(&format!("{}- [alpha] a\n- [beta] b\n", all_seven_taken()));
    let (r, _rec) = w.start("x", None);
    let e = err_of(r);
    assert!(
        e.contains("no free color (all 9 colors have GBIV.md entries)"),
        "{e}"
    );
}

// @spec FL-START-005, FL-START-014
#[test]
fn start_no_free_color_errors_before_loading_extensions() {
    let w = ws();
    // Enabled but not installed: loading would fail with a not-found error.
    w.enable(&["ghost"], "");
    w.write_md(&all_seven_taken());
    let (r, _rec) = w.start("x", None);
    let e = err_of(r);
    assert!(e.contains("no free color"), "{e}");
    assert!(
        !e.contains("install-extension"),
        "extension must not load: {e}"
    );
}

// =============================================================================
// gbiv start — duplicate checks
// =============================================================================

// @spec FL-START-006, FL-TAG-001
#[test]
fn start_errors_when_single_token_task_is_carried_as_a_tag() {
    let w = ws();
    let md = "- [red] [gbv-1ht] Pluggable integrations\n";
    w.write_md(md);
    let (r, rec) = w.start("gbv-1ht", None);
    let e = err_of(r);
    assert!(e.contains("gbv-1ht is already on red"), "{e}");
    assert_eq!(w.read_md().as_deref(), Some(md));
    assert!(rec.outs().is_empty());
}

// @spec FL-START-006, FL-TAG-001
#[test]
fn start_duplicate_tag_check_is_case_insensitive_and_any_position() {
    let w = ws();
    let md = "- [orange] Title [GBV-1HT] trailing\n";
    w.write_md(md);
    let (r, _rec) = w.start("  gbv-1ht  ", None);
    let e = err_of(r);
    assert!(e.contains("gbv-1ht is already on orange"), "{e}");
    assert_eq!(w.read_md().as_deref(), Some(md));
}

// @spec FL-START-006
#[test]
fn start_duplicate_tag_check_ignores_color_tags() {
    let w = ws();
    // "orange" is the color tag of the entry, not a text tag it carries.
    w.write_md("- [orange] something\n");
    let (r, rec) = w.start("orange", None);
    r.unwrap();
    assert_eq!(rec.outs(), vec!["started on red: orange".to_string()]);
}

// @spec FL-START-006
#[test]
fn start_task_with_whitespace_skips_the_tag_duplicate_check() {
    let w = ws();
    w.write_md("- [red] [gbv-1ht] Title\n");
    let (r, rec) = w.start("gbv-1ht follow-up", None);
    r.unwrap();
    assert_eq!(
        rec.outs(),
        vec!["started on orange: gbv-1ht follow-up".to_string()]
    );
}

// @spec FL-START-006, FL-START-014
#[test]
fn start_duplicate_argument_errors_before_loading_extensions() {
    let w = ws();
    w.enable(&["ghost"], "");
    w.write_md("- [red] [gbv-1ht] Title\n");
    let (r, _rec) = w.start("gbv-1ht", None);
    let e = err_of(r);
    assert!(e.contains("gbv-1ht is already on red"), "{e}");
    assert!(!e.contains("install-extension"), "{e}");
}

// @spec FL-START-013
#[test]
fn start_errors_when_text_equals_existing_entry_text_case_insensitively() {
    let w = ws();
    let md = "- [red] Fix login bug\n";
    w.write_md(md);
    let (r, rec) = w.start("fix LOGIN bug", None);
    let e = err_of(r);
    assert!(e.contains("is already on red"), "{e}");
    assert!(e.to_lowercase().contains("fix login bug"), "{e}");
    assert_eq!(w.read_md().as_deref(), Some(md));
    assert!(rec.outs().is_empty());
}

// @spec FL-START-013
#[test]
fn start_errors_when_free_text_leading_tag_is_carried_by_an_entry() {
    let w = ws();
    let md = "- [red] [urgent] something else\n";
    w.write_md(md);
    let (r, _rec) = w.start("[Urgent] fix login", None);
    let e = err_of(r);
    assert!(e.contains("is already on red"), "{e}");
    assert!(e.to_lowercase().contains("urgent"), "{e}");
    assert_eq!(w.read_md().as_deref(), Some(md));
}

// @spec FL-START-013
#[test]
fn start_non_leading_tag_in_text_is_not_a_duplicate() {
    let w = ws();
    w.write_md("- [red] [x] a\n");
    let (r, rec) = w.start("fix [x] later", None);
    r.unwrap();
    assert_eq!(
        rec.outs(),
        vec!["started on orange: fix [x] later".to_string()]
    );
}

// @spec FL-START-013, FL-START-008
#[test]
fn start_errors_when_named_text_leading_tag_is_already_on_a_color() {
    let w = ws();
    w.ext(
        "namer",
        &[],
        r#"return { before_start = function(ctx) return { text = "[abc-1] Renamed by tracker" } end,
                    after_start = function(ctx) gbiv.log("AFTER") end }"#,
    );
    w.enable(&["namer"], "");
    let md = "- [orange] [ABC-1] Claimed earlier\n";
    w.write_md(md);
    let (r, rec) = w.start("my ticket", None);
    let e = err_of(r);
    assert!(e.contains("is already on orange"), "{e}");
    assert!(e.to_lowercase().contains("abc-1"), "{e}");
    assert_eq!(w.read_md().as_deref(), Some(md));
    assert!(rec.outs().is_empty());
    assert!(!rec.any_line_contains("AFTER"));
}

// @spec FL-START-013, FL-START-008
#[test]
fn start_errors_when_named_text_equals_existing_text() {
    let w = ws();
    w.ext(
        "namer",
        &[],
        r#"return { before_start = function(ctx) return { text = "Same Title" } end }"#,
    );
    w.enable(&["namer"], "");
    let md = "- [red] same title\n";
    w.write_md(md);
    let (r, _rec) = w.start("tkt-2", None);
    let e = err_of(r);
    assert!(e.contains("is already on red"), "{e}");
    assert_eq!(w.read_md().as_deref(), Some(md));
}

// =============================================================================
// gbiv start — extensions, naming, warning
// =============================================================================

// @spec FL-START-007, FL-START-008
#[test]
fn start_fires_before_start_with_untrimmed_arg_and_chosen_color() {
    let w = ws();
    w.ext("logger", &[], LOGGER_START);
    w.enable(&["logger"], "");
    w.write_md("- [red] r\n");
    let (r, rec) = w.start("  fix login  ", None);
    r.unwrap();
    assert!(
        rec.errs()
            .iter()
            .any(|l| l == "logger: ARG=<  fix login  > COLOR=orange"),
        "{:?}",
        rec.lines
    );
    // Not named → the trimmed argument is the text.
    assert_eq!(
        w.read_md().as_deref(),
        Some("- [red] r\n- [orange] fix login\n")
    );
    assert_eq!(rec.outs(), vec!["started on orange: fix login".to_string()]);
}

// @spec FL-START-007
#[test]
fn start_before_start_sees_the_color_flag() {
    let w = ws();
    w.ext("logger", &[], LOGGER_START);
    w.enable(&["logger"], "");
    let (r, rec) = w.start("x y", Some("indigo"));
    r.unwrap();
    assert!(
        rec.errs()
            .iter()
            .any(|l| l == "logger: ARG=<x y> COLOR=indigo"),
        "{:?}",
        rec.lines
    );
}

// @spec FL-START-007
#[test]
fn start_veto_writes_nothing_and_fires_no_after_start() {
    let w = ws();
    w.ext(
        "vet",
        &[],
        r#"return {
  before_start = function(ctx) error("claimed by sam") end,
  after_start = function(ctx) gbiv.log("AFTER") end,
}"#,
    );
    w.enable(&["vet"], "");
    let md = "- [red] r\n";
    w.write_md(md);
    let (r, rec) = w.start("gbv-2", None);
    let e = err_of(r);
    assert!(e.contains("vet: claimed by sam"), "{e}");
    assert_eq!(w.read_md().as_deref(), Some(md));
    assert!(rec.outs().is_empty(), "{:?}", rec.lines);
    assert!(!rec.any_line_contains("AFTER"));
}

// @spec FL-START-007
#[test]
fn start_veto_with_missing_gbiv_md_creates_no_file() {
    let w = ws();
    w.ext(
        "vet",
        &[],
        r#"return { before_start = function(ctx) error("no") end }"#,
    );
    w.enable(&["vet"], "");
    let (r, _rec) = w.start("free text", None);
    assert!(r.is_err());
    assert_eq!(w.read_md(), None);
}

// @spec FL-START-008, FL-START-010
#[test]
fn start_writes_named_text_trimmed() {
    let w = ws();
    w.ext(
        "namer",
        &[],
        r#"return { before_start = function(ctx) return { text = "  [gbv-9] Named title  " } end }"#,
    );
    w.enable(&["namer"], "");
    let (r, rec) = w.start("gbv-9", None);
    r.unwrap();
    assert_eq!(
        w.read_md().as_deref(),
        Some("- [red] [gbv-9] Named title\n")
    );
    assert_eq!(
        rec.outs(),
        vec!["started on red: [gbv-9] Named title".to_string()]
    );
    assert!(
        !rec.errs().iter().any(|l| l.contains("free text")),
        "named entry must not warn: {:?}",
        rec.lines
    );
}

// @spec FL-START-009
#[test]
fn start_warns_when_enabled_extensions_do_not_recognize_single_token() {
    let w = ws();
    w.ext("quiet", &[], "return {}");
    w.enable(&["quiet"], "");
    let (r, rec) = w.start("gbv-404", None);
    r.unwrap();
    let warned = rec
        .errs()
        .iter()
        .any(|l| l.contains("gbv-404") && l.to_lowercase().contains("free text"));
    assert!(warned, "expected a free-text warning: {:?}", rec.lines);
    assert_eq!(w.read_md().as_deref(), Some("- [red] gbv-404\n"));
}

// @spec FL-START-009
#[test]
fn start_warns_when_before_start_returns_nil() {
    let w = ws();
    w.ext(
        "nil_ext",
        &[],
        r#"return { before_start = function(ctx) return nil end }"#,
    );
    w.enable(&["nil_ext"], "");
    let (r, rec) = w.start("refactor", None);
    r.unwrap();
    assert!(
        rec.errs()
            .iter()
            .any(|l| l.to_lowercase().contains("free text")),
        "{:?}",
        rec.lines
    );
}

// @spec FL-START-009
#[test]
fn start_no_warning_without_enabled_extensions() {
    let w = ws();
    // An installed but not enabled extension does not count.
    w.ext("quiet", &[], "return {}");
    let (r, rec) = w.start("gbv-404", None);
    r.unwrap();
    assert!(rec.errs().is_empty(), "{:?}", rec.lines);
}

// @spec FL-START-009
#[test]
fn start_no_warning_for_multi_word_task() {
    let w = ws();
    w.ext("quiet", &[], "return {}");
    w.enable(&["quiet"], "");
    let (r, rec) = w.start("Fix login bug", None);
    r.unwrap();
    assert!(
        !rec.errs()
            .iter()
            .any(|l| l.to_lowercase().contains("free text")),
        "{:?}",
        rec.lines
    );
}

// @spec FL-START-009
#[test]
fn start_no_warning_when_extension_disabled_in_config() {
    let w = ws();
    w.ext("quiet", &[], "return {}");
    w.config("[extensions]\nenabled = [\"quiet\"]\ndisabled = [\"quiet\"]\n");
    let (r, rec) = w.start("gbv-404", None);
    r.unwrap();
    assert!(
        !rec.errs()
            .iter()
            .any(|l| l.to_lowercase().contains("free text")),
        "{:?}",
        rec.lines
    );
}

// =============================================================================
// gbiv start — write, print, after_start
// =============================================================================

// @spec FL-START-010, EXT-EVT-012
#[test]
fn start_writes_then_prints_then_fires_after_start() {
    let w = ws();
    let (marks, grant) = w.marks("ord");
    let marker = marks.join("after_start");
    w.ext(
        "ord",
        &["fs"],
        &format!(
            r#"return {{
  after_start = function(ctx)
    gbiv.fs.write({marker:?}, ctx.color .. " " .. ctx.text)
    gbiv.log("AFTER")
  end,
}}"#,
            marker = marker.to_str().unwrap()
        ),
    );
    w.enable(&["ord"], &grant);
    w.write_md("- [red] r\n");
    let rec = Rec {
        watch: vec![w.gbiv_md(), marker.clone()],
        ..Rec::default()
    };
    let (r, rec) = w.start_rec("Fix login bug", None, rec);
    r.unwrap();

    assert_eq!(
        rec.outs(),
        vec!["started on orange: Fix login bug".to_string()]
    );
    let snap = &rec.at_out[0];
    assert_eq!(
        snap[0].as_deref(),
        Some("- [red] r\n- [orange] Fix login bug\n"),
        "GBIV.md must be written before the success line"
    );
    assert_eq!(
        snap[1], None,
        "after_start must not run before the success line"
    );
    // after_start did run, with the written color and text.
    assert_eq!(fs::read_to_string(&marker).unwrap(), "orange Fix login bug");
    let out_i = rec.index_of(|l| matches!(l, Line::Out(_))).unwrap();
    let after_i = rec
        .index_of(|l| matches!(l, Line::Err(s) if s == "ord: AFTER"))
        .expect("after_start log printed");
    assert!(out_i < after_i, "{:?}", rec.lines);
}

// @spec FL-START-010, EXT-EVT-011
#[test]
fn start_after_start_failure_is_a_warning_and_still_succeeds() {
    let w = ws();
    w.ext(
        "boom",
        &[],
        r#"return { after_start = function(ctx) error("notify failed") end }"#,
    );
    w.enable(&["boom"], "");
    let (r, rec) = w.start("Fix login bug", None);
    r.unwrap();
    assert_eq!(w.read_md().as_deref(), Some("- [red] Fix login bug\n"));
    assert_eq!(
        rec.outs(),
        vec!["started on red: Fix login bug".to_string()]
    );
    let out_i = rec.index_of(|l| matches!(l, Line::Out(_))).unwrap();
    let warn_i = rec
        .index_of(|l| matches!(l, Line::Err(s) if s.contains("boom: notify failed")))
        .expect("after_start failure printed as a warning");
    assert!(out_i < warn_i);
}

// @spec FL-START-011
#[test]
fn start_write_failure_after_before_start_reports_extensions_ran() {
    let w = ws();
    w.ext("logger", &[], LOGGER_START);
    w.enable(&["logger"], "");
    // A directory where GBIV.md should be: it reads as empty (with a warning)
    // and cannot be written.
    fs::create_dir_all(w.gbiv_md()).unwrap();
    let (r, rec) = w.start("Fix login bug", None);
    let e = err_of(r).to_lowercase();
    assert!(
        e.contains("extension"),
        "error must say extensions already ran: {e}"
    );
    assert!(
        e.contains("undo"),
        "error must say changes may need undoing: {e}"
    );
    assert!(rec.outs().is_empty(), "{:?}", rec.lines);
    assert!(
        !rec.any_line_contains("AFTER"),
        "no after_start: {:?}",
        rec.lines
    );
    assert!(w.gbiv_md().is_dir());
}

// @spec FL-START-012
#[test]
fn start_leaves_worktrees_and_branches_untouched() {
    let w = ws();
    let red = w.root.join("red").join("proj");
    init_repo_with_commit(&red, "red");
    fs::write(red.join("wip.txt"), "uncommitted").unwrap();
    let snapshot = |dir: &Path| {
        (
            git(dir, &["rev-parse", "HEAD"]),
            git(dir, &["symbolic-ref", "HEAD"]),
            git(dir, &["branch", "--list"]),
            git(dir, &["status", "--porcelain"]),
        )
    };
    let red_before = snapshot(&red);
    let main_before = (
        git(&w.main_repo, &["rev-parse", "HEAD"]),
        git(&w.main_repo, &["symbolic-ref", "HEAD"]),
        git(&w.main_repo, &["branch", "--list"]),
    );
    let orange_entries: Vec<_> = fs::read_dir(w.root.join("orange")).unwrap().collect();
    assert!(orange_entries.is_empty());

    let (r, _rec) = w.start("gbv-77", None);
    r.unwrap();
    let (r, _rec) = w.start("another task", Some("orange"));
    r.unwrap();

    assert_eq!(snapshot(&red), red_before);
    assert_eq!(
        (
            git(&w.main_repo, &["rev-parse", "HEAD"]),
            git(&w.main_repo, &["symbolic-ref", "HEAD"]),
            git(&w.main_repo, &["branch", "--list"]),
        ),
        main_before
    );
    assert_eq!(
        fs::read_dir(w.root.join("orange")).unwrap().count(),
        0,
        "no worktree created for orange"
    );
}

// =============================================================================
// gbiv start — check order
// =============================================================================

// @spec FL-START-014
#[test]
fn start_order_config_parse_error_beats_invalid_task() {
    let w = ws();
    w.config("this is = = not toml [[[\n");
    let (r, _rec) = w.start("", None);
    let e = err_of(r);
    assert!(
        e.contains("config.toml"),
        "config parse error expected first: {e}"
    );
}

// @spec FL-START-014
#[test]
fn start_order_global_config_parse_error_beats_invalid_task() {
    let w = ws();
    let g = w.xdg.join("gbiv");
    fs::create_dir_all(&g).unwrap();
    fs::write(g.join("config.toml"), "[[[ broken\n").unwrap();
    let (r, _rec) = w.start("[done] x", None);
    let e = err_of(r);
    assert!(e.contains("config.toml"), "{e}");
}

// @spec FL-START-014
#[test]
fn start_order_invalid_task_beats_no_free_color() {
    let w = ws();
    w.write_md(&all_seven_taken());
    let (r, _rec) = w.start("two\nlines", None);
    let e = err_of(r);
    assert!(
        !e.contains("no free color"),
        "task validation comes first: {e}"
    );
}

// @spec FL-START-014
#[test]
fn start_order_no_free_color_beats_malformed_extensions_table() {
    let w = ws();
    w.config("[extensions]\nenabled = \"vat\"\n");
    w.write_md(&all_seven_taken());
    let (r, _rec) = w.start("x", None);
    let e = err_of(r);
    assert!(e.contains("no free color"), "{e}");
}

// @spec FL-START-014
#[test]
fn start_order_bad_color_flag_beats_malformed_extensions_table() {
    let w = ws();
    w.config("[extensions]\nenabled = \"vat\"\n");
    let (r, _rec) = w.start("x", Some("purple"));
    let e = err_of(r);
    assert!(e.contains("purple"), "{e}");
}

// @spec FL-START-014
#[test]
fn start_order_duplicate_argument_beats_malformed_extensions_table() {
    let w = ws();
    w.config("[extensions]\nenabeld = [\"vat\"]\n");
    w.write_md("- [red] [gbv-1ht] Title\n");
    let (r, _rec) = w.start("gbv-1ht", None);
    let e = err_of(r);
    assert!(e.contains("gbv-1ht is already on red"), "{e}");
}

// @spec FL-START-014
#[test]
fn start_order_malformed_extensions_table_errors_before_writing() {
    let w = ws();
    w.config("[extensions]\nenabled = \"vat\"\n");
    let (r, rec) = w.start("Fix login bug", None);
    assert!(r.is_err());
    assert_eq!(w.read_md(), None);
    assert!(rec.outs().is_empty());
}

// @spec FL-START-014
#[test]
fn start_order_all_handlers_run_before_named_text_check() {
    // `namer` names a duplicate, then `vet` vetoes: the veto wins because
    // the named-text check only happens after before_start finishes.
    let w = ws();
    w.ext(
        "namer",
        &[],
        r#"return { before_start = function(ctx) return { text = "[abc-1] dup" } end }"#,
    );
    w.ext(
        "vet",
        &[],
        r#"return { before_start = function(ctx) error("nope") end }"#,
    );
    w.enable(&["namer", "vet"], "");
    let md = "- [red] [abc-1] existing\n";
    w.write_md(md);
    let (r, _rec) = w.start("my ticket", None);
    let e = err_of(r);
    assert!(e.contains("vet: nope"), "{e}");
    assert!(!e.contains("is already on"), "{e}");
    assert_eq!(w.read_md().as_deref(), Some(md));
}

// @spec FL-START-014
#[test]
fn start_order_missing_extension_errors_before_any_handler_or_write() {
    let w = ws();
    w.ext("logger", &[], LOGGER_START);
    w.enable(&["logger", "ghost"], "");
    let (r, rec) = w.start("Fix login bug", None);
    let e = err_of(r);
    assert!(e.contains("ghost"), "{e}");
    assert!(!rec.any_line_contains("ARG="), "{:?}", rec.lines);
    assert_eq!(w.read_md(), None);
}

// =============================================================================
// gbiv done — resolution
// =============================================================================

/// Logs `COLOR=.. TEXT=..` in before_done and `AFTER ..` in after_done.
const LOGGER_DONE: &str = r#"
return {
  before_done = function(ctx)
    gbiv.log("COLOR=" .. ctx.color .. " TEXT=" .. ctx.text)
  end,
  after_done = function(ctx)
    gbiv.log("AFTER " .. ctx.color .. " " .. ctx.text)
  end,
}
"#;

// @spec FL-DONE-002, FL-DONE-008
#[test]
fn done_color_removes_that_entry() {
    let w = ws();
    w.write_md("- [red] A\n- [blue] B\n---\nfooter\n");
    let (r, rec) = w.done(Some("blue"));
    r.unwrap();
    assert_eq!(w.read_md().as_deref(), Some("- [red] A\n---\nfooter\n"));
    assert_eq!(rec.outs(), vec!["done blue: B".to_string()]);
}

// @spec FL-DONE-002
#[test]
fn done_configured_extra_resolves_as_color() {
    let w = ws();
    w.config("[palette]\nextra = [\"alpha\"]\n");
    w.write_md("- [red] A\n- [alpha] X\n");
    let (r, rec) = w.done(Some("alpha"));
    r.unwrap();
    assert_eq!(w.read_md().as_deref(), Some("- [red] A\n"));
    assert_eq!(rec.outs(), vec!["done alpha: X".to_string()]);
}

// @spec FL-DONE-002, FL-DONE-003, FL-DONE-005
#[test]
fn done_palette_match_is_case_sensitive() {
    let w = ws();
    let md = "- [blue] B\n";
    w.write_md(md);
    // "Blue" is not a palette name, so it is looked up as a text tag.
    let (r, _rec) = w.done(Some("Blue"));
    let e = err_of(r);
    assert!(e.contains("no GBIV.md entry carries [Blue]"), "{e}");
    assert_eq!(w.read_md().as_deref(), Some(md));
}

// @spec FL-DONE-003, FL-DONE-008
#[test]
fn done_text_tag_removes_entry_with_its_notes() {
    let w = ws();
    w.write_md("- [red] [gbv-1ht] Pluggable\n  a note\n- [blue] B\n");
    let (r, rec) = w.done(Some("GBV-1HT"));
    r.unwrap();
    assert_eq!(w.read_md().as_deref(), Some("- [blue] B\n"));
    assert_eq!(
        rec.outs(),
        vec!["done red: [gbv-1ht] Pluggable".to_string()]
    );
}

// @spec FL-DONE-003, FL-TAG-001
#[test]
fn done_text_tag_matches_anywhere_in_text() {
    let w = ws();
    w.write_md("- [green] Title [blocked-by:gbv-x2v] more\n- [blue] B\n");
    let (r, rec) = w.done(Some("blocked-by:gbv-x2v"));
    r.unwrap();
    assert_eq!(w.read_md().as_deref(), Some("- [blue] B\n"));
    assert_eq!(
        rec.outs(),
        vec!["done green: Title [blocked-by:gbv-x2v] more".to_string()]
    );
}

// @spec FL-DONE-003, FL-DONE-005
#[test]
fn done_untagged_text_is_not_addressable_by_its_text() {
    let w = ws();
    let md = "- [red] refactor\n- [blue] B\n";
    w.write_md(md);
    let (r, _rec) = w.done(Some("refactor"));
    let e = err_of(r);
    assert!(e.contains("no GBIV.md entry carries [refactor]"), "{e}");
    assert_eq!(w.read_md().as_deref(), Some(md));
}

// @spec FL-DONE-003, FL-DONE-005
#[test]
fn done_partial_text_or_unbracketed_word_does_not_match() {
    let w = ws();
    let md = "- [red] refactor auth\n- [blue] [gbv-1ht2] x\n";
    w.write_md(md);
    for arg in ["refactor", "gbv-1ht"] {
        let (r, _rec) = w.done(Some(arg));
        let e = err_of(r);
        assert!(
            e.contains(&format!("no GBIV.md entry carries [{arg}]")),
            "{e}"
        );
        assert_eq!(w.read_md().as_deref(), Some(md));
    }
}

// @spec FL-DONE-003
#[test]
fn done_tag_lookup_ignores_untagged_and_below_separator_entries() {
    let w = ws();
    let md = "- [gbv-5] untagged-looking backlog line\n---\n- [red] [gbv-5] example\n";
    w.write_md(md);
    // The first line's [gbv-5] is its color tag (not a text tag) and the second
    // entry is below the separator; neither carries [gbv-5].
    let (r, _rec) = w.done(Some("gbv-5"));
    let e = err_of(r);
    assert!(e.contains("no GBIV.md entry carries [gbv-5]"), "{e}");
    assert_eq!(w.read_md().as_deref(), Some(md));
}

// @spec FL-DONE-004
#[test]
fn done_without_argument_infers_color_from_cwd() {
    let w = ws();
    w.write_md("- [red] A\n- [green] G\n");
    let cwd = w.root.join("green").join("proj").join("src");
    fs::create_dir_all(&cwd).unwrap();
    let (r, rec) = w.done_at(None, &cwd);
    r.unwrap();
    assert_eq!(w.read_md().as_deref(), Some("- [red] A\n"));
    assert_eq!(rec.outs(), vec!["done green: G".to_string()]);
}

// @spec FL-DONE-004
#[test]
fn done_without_argument_errors_when_color_cannot_be_inferred() {
    let w = ws();
    let md = "- [red] A\n";
    w.write_md(md);
    for cwd in [w.root.clone(), w.main_repo.clone()] {
        let (r, rec) = w.done_at(None, &cwd);
        assert!(r.is_err(), "cwd {cwd:?} should not infer a color");
        assert_eq!(w.read_md().as_deref(), Some(md));
        assert!(rec.outs().is_empty());
    }
}

// @spec FL-DONE-004, FL-DONE-005
#[test]
fn done_inferred_color_without_entry_errors() {
    let w = ws();
    w.write_md("- [red] A\n");
    let cwd = w.root.join("blue");
    let (r, _rec) = w.done_at(None, &cwd);
    let e = err_of(r);
    assert!(e.contains("no GBIV.md entry for blue"), "{e}");
}

// @spec FL-DONE-005
#[test]
fn done_errors_when_no_entry_for_color() {
    let w = ws();
    let md = "- [blue] B\n";
    w.write_md(md);
    let (r, rec) = w.done(Some("red"));
    let e = err_of(r);
    assert!(e.contains("no GBIV.md entry for red"), "{e}");
    assert_eq!(w.read_md().as_deref(), Some(md));
    assert!(rec.outs().is_empty());
}

// @spec FL-DONE-005
#[test]
fn done_errors_when_no_entry_carries_tag() {
    let w = ws();
    let md = "- [blue] [gbv-1] B\n";
    w.write_md(md);
    let (r, rec) = w.done(Some("gbv-2"));
    let e = err_of(r);
    assert!(e.contains("no GBIV.md entry carries [gbv-2]"), "{e}");
    assert_eq!(w.read_md().as_deref(), Some(md));
    assert!(rec.outs().is_empty());
}

// @spec FL-DONE-005
#[test]
fn done_errors_when_gbiv_md_missing() {
    let w = ws();
    let (r, _rec) = w.done(Some("red"));
    let e = err_of(r);
    assert!(e.contains("no GBIV.md entry for red"), "{e}");
    assert_eq!(w.read_md(), None);
}

// @spec FL-DONE-006
#[test]
fn done_errors_listing_entries_when_color_matches_several() {
    let w = ws();
    let md = "- [red] First thing\n- [red] Second thing\n";
    w.write_md(md);
    let (r, rec) = w.done(Some("red"));
    let e = err_of(r);
    assert!(
        e.contains("First thing") && e.contains("Second thing"),
        "{e}"
    );
    assert_eq!(w.read_md().as_deref(), Some(md));
    assert!(rec.outs().is_empty());
}

// @spec FL-DONE-006
#[test]
fn done_errors_listing_entries_when_tag_matches_several() {
    let w = ws();
    let md = "- [red] [gbv-1] part one\n- [blue] [GBV-1] part two\n";
    w.write_md(md);
    let (r, _rec) = w.done(Some("gbv-1"));
    let e = err_of(r);
    assert!(e.contains("part one") && e.contains("part two"), "{e}");
    assert_eq!(w.read_md().as_deref(), Some(md));
}

// @spec FL-DONE-003
#[test]
fn done_tag_match_ignores_an_untagged_entry_with_the_same_text() {
    let w = ws();
    w.write_md("- [red] refactor\n- [blue] [refactor] the auth module\n");
    let (r, rec) = w.done(Some("refactor"));
    r.unwrap();
    assert_eq!(w.read_md().as_deref(), Some("- [red] refactor\n"));
    assert_eq!(
        rec.outs(),
        vec!["done blue: [refactor] the auth module".to_string()]
    );
}

// @spec FL-DONE-010
#[test]
fn done_strips_one_pair_of_brackets() {
    let w = ws();
    w.write_md("- [red] [gbv-1ht] Pluggable\n- [blue] B\n");
    let (r, rec) = w.done(Some("[gbv-1ht]"));
    r.unwrap();
    assert_eq!(w.read_md().as_deref(), Some("- [blue] B\n"));
    assert_eq!(
        rec.outs(),
        vec!["done red: [gbv-1ht] Pluggable".to_string()]
    );
}

// @spec FL-DONE-010, FL-DONE-002
#[test]
fn done_bracketed_color_resolves_as_color() {
    let w = ws();
    w.write_md("- [red] A\n- [blue] B\n");
    let (r, rec) = w.done(Some("[blue]"));
    r.unwrap();
    assert_eq!(w.read_md().as_deref(), Some("- [red] A\n"));
    assert_eq!(rec.outs(), vec!["done blue: B".to_string()]);
}

// @spec FL-DONE-011
#[test]
fn done_palette_name_wins_over_text_tag() {
    let w = ws();
    w.write_md("- [blue] [red] tagged red\n- [red] the red one\n");
    let (r, rec) = w.done(Some("red"));
    r.unwrap();
    assert_eq!(w.read_md().as_deref(), Some("- [blue] [red] tagged red\n"));
    assert_eq!(rec.outs(), vec!["done red: the red one".to_string()]);
}

// @spec FL-DONE-011, FL-DONE-005
#[test]
fn done_palette_name_never_falls_back_to_text_tag() {
    let w = ws();
    let md = "- [blue] [red] tagged red\n";
    w.write_md(md);
    let (r, _rec) = w.done(Some("red"));
    let e = err_of(r);
    assert!(e.contains("no GBIV.md entry for red"), "{e}");
    assert_eq!(w.read_md().as_deref(), Some(md));
}

// =============================================================================
// gbiv done — extensions, removal, output
// =============================================================================

// @spec FL-DONE-007, FL-DONE-008
#[test]
fn done_fires_before_done_with_color_and_text() {
    let w = ws();
    w.ext("logger", &[], LOGGER_DONE);
    w.enable(&["logger"], "");
    w.write_md("- [red] [gbv-1ht] Pluggable\n");
    let (r, rec) = w.done(Some("gbv-1ht"));
    r.unwrap();
    assert!(
        rec.errs()
            .iter()
            .any(|l| l == "logger: COLOR=red TEXT=[gbv-1ht] Pluggable"),
        "{:?}",
        rec.lines
    );
    assert!(
        crate::gbiv_md::parse_gbiv_md(&w.gbiv_md()).is_empty(),
        "{:?}",
        w.read_md()
    );
}

// @spec FL-DONE-007
#[test]
fn done_veto_keeps_entry_and_fires_no_after_done() {
    let w = ws();
    w.ext(
        "vet",
        &[],
        r#"return {
  before_done = function(ctx) error("tests are failing") end,
  after_done = function(ctx) gbiv.log("AFTER") end,
}"#,
    );
    w.enable(&["vet"], "");
    let md = "- [red] A\n  note\n- [blue] B\n";
    w.write_md(md);
    let (r, rec) = w.done(Some("red"));
    let e = err_of(r);
    assert!(e.contains("vet: tests are failing"), "{e}");
    assert_eq!(w.read_md().as_deref(), Some(md));
    assert!(rec.outs().is_empty(), "{:?}", rec.lines);
    assert!(!rec.any_line_contains("AFTER"));
}

// @spec FL-DONE-007
#[test]
fn done_resolution_failure_errors_before_loading_extensions() {
    let w = ws();
    w.enable(&["ghost"], "");
    w.write_md("- [blue] B\n");
    let (r, _rec) = w.done(Some("red"));
    let e = err_of(r);
    assert!(e.contains("no GBIV.md entry for red"), "{e}");
    assert!(!e.contains("install-extension"), "{e}");
}

// @spec FL-DONE-007
#[test]
fn done_extension_load_failure_keeps_entry() {
    let w = ws();
    w.enable(&["ghost"], "");
    let md = "- [red] A\n";
    w.write_md(md);
    let (r, _rec) = w.done(Some("red"));
    let e = err_of(r);
    assert!(e.contains("ghost"), "{e}");
    assert_eq!(w.read_md().as_deref(), Some(md));
}

// @spec FL-DONE-008, FL-MUTATE-016
#[test]
fn done_removes_only_the_entry_and_cleans_following_blank_line() {
    let w = ws();
    w.write_md("# Tasks\n- [red] A\n  note a\n\n- [blue] B\n\n---\nfooter\n");
    let (r, rec) = w.done(Some("red"));
    r.unwrap();
    assert_eq!(
        w.read_md().as_deref(),
        Some("# Tasks\n- [blue] B\n\n---\nfooter\n")
    );
    assert_eq!(rec.outs(), vec!["done red: A".to_string()]);
}

// @spec FL-DONE-008, EXT-EVT-012
#[test]
fn done_removes_then_prints_then_fires_after_done() {
    let w = ws();
    let (marks, grant) = w.marks("ord");
    let marker = marks.join("after_done");
    w.ext(
        "ord",
        &["fs"],
        &format!(
            r#"return {{
  after_done = function(ctx)
    gbiv.fs.write({marker:?}, ctx.color .. " " .. ctx.text)
    gbiv.log("AFTER")
  end,
}}"#,
            marker = marker.to_str().unwrap()
        ),
    );
    w.enable(&["ord"], &grant);
    w.write_md("- [red] A\n- [blue] B\n");
    let rec = Rec {
        watch: vec![w.gbiv_md(), marker.clone()],
        ..Rec::default()
    };
    let (r, rec) = w.done_rec(Some("blue"), rec);
    r.unwrap();
    assert_eq!(rec.outs(), vec!["done blue: B".to_string()]);
    let snap = &rec.at_out[0];
    assert_eq!(
        snap[0].as_deref(),
        Some("- [red] A\n"),
        "entry removed before the success line"
    );
    assert_eq!(
        snap[1], None,
        "after_done must not run before the success line"
    );
    assert_eq!(fs::read_to_string(&marker).unwrap(), "blue B");
    let out_i = rec.index_of(|l| matches!(l, Line::Out(_))).unwrap();
    let after_i = rec
        .index_of(|l| matches!(l, Line::Err(s) if s == "ord: AFTER"))
        .expect("after_done log printed");
    assert!(out_i < after_i, "{:?}", rec.lines);
}

// @spec FL-DONE-008, EXT-EVT-011
#[test]
fn done_after_done_failure_is_a_warning_and_still_succeeds() {
    let w = ws();
    w.ext(
        "boom",
        &[],
        r#"return { after_done = function(ctx) error("notify failed") end }"#,
    );
    w.enable(&["boom"], "");
    w.write_md("- [red] A\n");
    let (r, rec) = w.done(Some("red"));
    r.unwrap();
    assert!(
        crate::gbiv_md::parse_gbiv_md(&w.gbiv_md()).is_empty(),
        "{:?}",
        w.read_md()
    );
    assert_eq!(rec.outs(), vec!["done red: A".to_string()]);
    assert!(
        rec.errs().iter().any(|l| l.contains("boom: notify failed")),
        "{:?}",
        rec.lines
    );
}

// @spec FL-DONE-008, FL-MUTATE-016
#[test]
fn done_fails_when_entry_changed_while_before_done_ran() {
    // A before_done handler rewrites the entry (standing in for a hand edit made
    // while extensions ran): remove_gbiv_entry re-reads and finds no match.
    let w = ws();
    let md_path = w.gbiv_md();
    let grant = format!(
        "[extensions.editor]\npaths = [{:?}]\n",
        w.main_repo.to_str().unwrap()
    );
    w.ext(
        "editor",
        &["fs"],
        &format!(
            r#"return {{
  before_done = function(ctx)
    gbiv.fs.write({p:?}, "- [red] A edited\n")
  end,
  after_done = function(ctx) gbiv.log("AFTER") end,
}}"#,
            p = md_path.to_str().unwrap()
        ),
    );
    w.enable(&["editor"], &grant);
    w.write_md("- [red] A\n");
    let (r, rec) = w.done(Some("red"));
    assert!(r.is_err());
    assert_eq!(w.read_md().as_deref(), Some("- [red] A edited\n"));
    assert!(rec.outs().is_empty(), "{:?}", rec.lines);
    assert!(!rec.any_line_contains("AFTER"));
}

// @spec FL-DONE-009
#[test]
fn done_leaves_worktrees_and_branches_untouched() {
    let w = ws();
    let red = w.root.join("red").join("proj");
    init_repo_with_commit(&red, "red");
    git(&red, &["checkout", "-q", "-b", "feature-x"]);
    fs::write(red.join("wip.txt"), "uncommitted").unwrap();
    let snapshot = |dir: &Path| {
        (
            git(dir, &["rev-parse", "HEAD"]),
            git(dir, &["symbolic-ref", "HEAD"]),
            git(dir, &["branch", "--list"]),
            git(dir, &["status", "--porcelain"]),
        )
    };
    let red_before = snapshot(&red);
    let main_before = (
        git(&w.main_repo, &["rev-parse", "HEAD"]),
        git(&w.main_repo, &["branch", "--list"]),
    );
    w.write_md("- [red] A\n");

    let cwd = red.clone();
    let (r, _rec) = w.done_at(None, &cwd);
    r.unwrap();

    assert_eq!(snapshot(&red), red_before);
    assert_eq!(
        (
            git(&w.main_repo, &["rev-parse", "HEAD"]),
            git(&w.main_repo, &["branch", "--list"]),
        ),
        main_before
    );
}

//! gbv-1ht tests (Phase 5) for `gbiv install-extension`
//! (`docs/specs/extension-runtime.md` § EXT-INST). Every test runs against
//! temp dirs only: `cwd` is a throwaway gbiv workspace (or a plain temp dir)
//! and `user_config_dir` stands in for `$XDG_CONFIG_HOME`.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use tempfile::TempDir;

use super::install::{bundled_names, run_install_extension, InstallEnv, Scope};

const EXIT_OK: i32 = 0;
const EXIT_OTHER: i32 = 1;
const EXIT_NOT_A_GBIV_WORKSPACE: i32 = 2;
const EXIT_REFUSED: i32 = 7;

/// The vat manifest version shipped in `extensions/vat/extension.toml`.
const VAT_VERSION: &str = "0.1.0";

struct Ws {
    _tmp: TempDir,
    root: PathBuf,
    main_repo: PathBuf,
    user_config_dir: PathBuf,
}

impl Ws {
    fn env(&self) -> InstallEnv {
        InstallEnv {
            cwd: self.main_repo.clone(),
            user_config_dir: self.user_config_dir.clone(),
        }
    }

    fn user_ext_dir(&self, name: &str) -> PathBuf {
        self.user_config_dir
            .join("gbiv")
            .join("extensions")
            .join(name)
    }

    fn project_ext_dir(&self, name: &str) -> PathBuf {
        self.root.join(".gbiv").join("extensions").join(name)
    }

    fn global_config(&self) -> PathBuf {
        self.user_config_dir.join("gbiv").join("config.toml")
    }

    fn project_config(&self) -> PathBuf {
        self.root.join(".gbiv").join("config.toml")
    }
}

/// A gbiv-shaped workspace: `<tmp>/proj/{main/proj (git repo), red/}` plus a
/// separate `<tmp>/xdg` user config dir.
fn workspace() -> Ws {
    let tmp = TempDir::new().expect("tempdir");
    let root = tmp.path().join("proj");
    let main_repo = root.join("main").join("proj");
    fs::create_dir_all(&main_repo).unwrap();
    Command::new("git")
        .args(["init", "-q"])
        .current_dir(&main_repo)
        .output()
        .unwrap();
    fs::create_dir_all(root.join("red")).unwrap();
    let user_config_dir = tmp.path().join("xdg");
    fs::create_dir_all(&user_config_dir).unwrap();
    Ws {
        _tmp: tmp,
        root,
        main_repo,
        user_config_dir,
    }
}

/// The source tree of the bundled vat extension, read at test time only to
/// compare against what the command installs.
fn bundled_source_dir(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("extensions")
        .join(name)
}

/// Every file under `dir`, keyed by its path relative to `dir`.
fn dir_contents(dir: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    fn walk(base: &Path, dir: &Path, out: &mut BTreeMap<PathBuf, Vec<u8>>) {
        for entry in fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                walk(base, &path, out);
            } else {
                let rel = path.strip_prefix(base).unwrap().to_path_buf();
                out.insert(rel, fs::read(&path).unwrap());
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(dir, dir, &mut out);
    out
}

fn stdout_json(outcome: &crate::orchestration::fleet_client::Outcome) -> serde_json::Value {
    let stdout = outcome
        .stdout
        .as_deref()
        .unwrap_or_else(|| panic!("expected JSON on stdout, got: {outcome:?}"));
    serde_json::from_str(stdout).unwrap_or_else(|e| panic!("stdout is not JSON ({e}): {stdout}"))
}

fn action(outcome: &crate::orchestration::fleet_client::Outcome) -> String {
    stdout_json(outcome)["action"]
        .as_str()
        .expect("action is a string")
        .to_string()
}

fn read_toml(path: &Path) -> toml::Table {
    let text =
        fs::read_to_string(path).unwrap_or_else(|e| panic!("reading {}: {e}", path.display()));
    text.parse::<toml::Table>()
        .unwrap_or_else(|e| panic!("parsing {}: {e}", path.display()))
}

fn enabled_list(path: &Path) -> Vec<String> {
    let table = read_toml(path);
    table["extensions"]["enabled"]
        .as_array()
        .expect("[extensions] enabled is an array")
        .iter()
        .map(|v| v.as_str().expect("enabled entries are strings").to_string())
        .collect()
}

fn write(path: &Path, body: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, body).unwrap();
}

/// Rewrite the installed manifest's `version` (keeping the rest of the file)
/// and hand-edit `init.lua`, so the installed copy differs from the bundle.
fn tamper_installed(ext_dir: &Path, version: &str) {
    let manifest = ext_dir.join("extension.toml");
    let text = fs::read_to_string(&manifest).unwrap();
    let rewritten: String = text
        .lines()
        .map(|l| {
            if l.trim_start().starts_with("version") {
                format!("version = \"{version}\"")
            } else {
                l.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    fs::write(&manifest, rewritten + "\n").unwrap();
    let init = ext_dir.join("init.lua");
    let mut lua = fs::read_to_string(&init).unwrap();
    lua.push_str("\n-- hand edited\n");
    fs::write(&init, lua).unwrap();
}

// ---- EXT-INST-002: bundled extensions ---------------------------------------

// @spec EXT-INST-002
#[test]
fn bundled_names_is_exactly_vat() {
    assert_eq!(bundled_names(), vec!["vat"]);
}

// @spec EXT-INST-002, EXT-INST-004
#[test]
fn user_scope_installs_every_bundled_file_byte_for_byte() {
    let ws = workspace();
    let outcome = run_install_extension("vat", Scope::User, false, &ws.env());
    assert_eq!(outcome.exit_code, EXIT_OK, "{outcome:?}");

    let installed = dir_contents(&ws.user_ext_dir("vat"));
    let bundled = dir_contents(&bundled_source_dir("vat"));
    assert!(!bundled.is_empty(), "bundled source tree must not be empty");
    assert_eq!(
        installed.keys().collect::<Vec<_>>(),
        bundled.keys().collect::<Vec<_>>(),
        "installed file set must match the bundled extension"
    );
    assert_eq!(
        installed, bundled,
        "installed contents must match the bundle"
    );
}

// ---- EXT-INST-003: unknown name ---------------------------------------------

// @spec EXT-INST-003, EXT-INST-013
#[test]
fn unknown_name_exits_1_listing_bundled_names_and_writes_nothing() {
    let ws = workspace();
    let outcome = run_install_extension("no-such-ext", Scope::User, false, &ws.env());
    assert_eq!(outcome.exit_code, EXIT_OTHER, "{outcome:?}");
    let message = format!(
        "{}{}",
        outcome.stderr.as_deref().unwrap_or_default(),
        outcome.stdout.as_deref().unwrap_or_default()
    );
    assert!(
        message.contains("vat"),
        "message must list the bundled names: {message:?}"
    );
    assert!(!ws.user_ext_dir("no-such-ext").exists());
    assert!(!ws.global_config().exists(), "no config written");
}

// ---- EXT-INST-004 / EXT-INST-005: scope -------------------------------------

// @spec EXT-INST-004
#[test]
fn user_scope_installs_under_user_config_dir() {
    let ws = workspace();
    let outcome = run_install_extension("vat", Scope::User, false, &ws.env());
    assert_eq!(outcome.exit_code, EXIT_OK, "{outcome:?}");
    assert!(ws.user_ext_dir("vat").join("extension.toml").is_file());
    assert!(ws.user_ext_dir("vat").join("init.lua").is_file());
    assert!(
        !ws.project_ext_dir("vat").exists(),
        "user scope must not write the project dir"
    );
    let json = stdout_json(&outcome);
    assert_eq!(json["scope"], "user");
    let dest = json["destination"].as_str().unwrap();
    assert!(
        dest.contains(&ws.user_ext_dir("vat").display().to_string()),
        "destination {dest:?} should name the user extension dir"
    );
}

// @spec EXT-INST-004
#[test]
fn project_scope_installs_under_gbiv_root() {
    let ws = workspace();
    let outcome = run_install_extension("vat", Scope::Project, false, &ws.env());
    assert_eq!(outcome.exit_code, EXIT_OK, "{outcome:?}");
    assert!(ws.project_ext_dir("vat").join("extension.toml").is_file());
    assert!(
        !ws.user_ext_dir("vat").exists(),
        "project scope must not write the user dir"
    );
    let json = stdout_json(&outcome);
    assert_eq!(json["scope"], "project");
    let dest = json["destination"].as_str().unwrap();
    assert!(
        dest.contains(&ws.project_ext_dir("vat").display().to_string()),
        "destination {dest:?} should name the project extension dir"
    );
}

// @spec EXT-INST-004
#[test]
fn project_scope_works_from_a_color_worktree_subdir() {
    let ws = workspace();
    let env = InstallEnv {
        cwd: ws.root.join("red"),
        user_config_dir: ws.user_config_dir.clone(),
    };
    let outcome = run_install_extension("vat", Scope::Project, false, &env);
    assert_eq!(outcome.exit_code, EXIT_OK, "{outcome:?}");
    assert!(ws.project_ext_dir("vat").join("extension.toml").is_file());
}

// @spec EXT-INST-005, EXT-INST-013
#[test]
fn project_scope_outside_a_workspace_exits_2() {
    let tmp = TempDir::new().unwrap();
    let env = InstallEnv {
        cwd: tmp.path().to_path_buf(),
        user_config_dir: tmp.path().join("xdg"),
    };
    let outcome = run_install_extension("vat", Scope::Project, false, &env);
    assert_eq!(outcome.exit_code, EXIT_NOT_A_GBIV_WORKSPACE, "{outcome:?}");
    assert!(!tmp.path().join(".gbiv").exists());
    assert!(!tmp.path().join("xdg").join("gbiv").exists());
}

// @spec EXT-INST-004
#[test]
fn user_scope_outside_a_workspace_still_installs() {
    let tmp = TempDir::new().unwrap();
    let xdg = tmp.path().join("xdg");
    let env = InstallEnv {
        cwd: tmp.path().to_path_buf(),
        user_config_dir: xdg.clone(),
    };
    let outcome = run_install_extension("vat", Scope::User, false, &env);
    assert_eq!(outcome.exit_code, EXIT_OK, "{outcome:?}");
    assert!(xdg
        .join("gbiv")
        .join("extensions")
        .join("vat")
        .join("extension.toml")
        .is_file());
}

// ---- EXT-INST-006: idempotency decision -------------------------------------

// @spec EXT-INST-006, EXT-INST-013
#[test]
fn missing_destination_is_installed() {
    let ws = workspace();
    let outcome = run_install_extension("vat", Scope::User, false, &ws.env());
    assert_eq!(outcome.exit_code, EXIT_OK);
    assert_eq!(action(&outcome), "installed");
}

// @spec EXT-INST-006, EXT-INST-013
#[test]
fn identical_destination_is_unchanged() {
    let ws = workspace();
    run_install_extension("vat", Scope::User, false, &ws.env());
    let outcome = run_install_extension("vat", Scope::User, false, &ws.env());
    assert_eq!(outcome.exit_code, EXIT_OK, "{outcome:?}");
    assert_eq!(action(&outcome), "unchanged");
}

// @spec EXT-INST-006
#[test]
fn identical_destination_with_force_is_updated() {
    let ws = workspace();
    run_install_extension("vat", Scope::User, false, &ws.env());
    let outcome = run_install_extension("vat", Scope::User, true, &ws.env());
    assert_eq!(outcome.exit_code, EXIT_OK, "{outcome:?}");
    assert_eq!(action(&outcome), "updated");
}

// @spec EXT-INST-006, EXT-INST-013
#[test]
fn hand_edited_same_version_is_refused_and_nothing_written() {
    let ws = workspace();
    run_install_extension("vat", Scope::User, false, &ws.env());
    let dir = ws.user_ext_dir("vat");
    tamper_installed(&dir, VAT_VERSION);
    let before = dir_contents(&dir);

    let outcome = run_install_extension("vat", Scope::User, false, &ws.env());
    assert_eq!(outcome.exit_code, EXIT_REFUSED, "{outcome:?}");
    let json = stdout_json(&outcome);
    assert_eq!(json["action"], "refused");
    assert!(
        json["reason"]
            .as_str()
            .unwrap_or_default()
            .contains("--force"),
        "reason should point at --force: {json}"
    );
    assert_eq!(dir_contents(&dir), before, "refusal must not write");
}

// @spec EXT-INST-006
#[test]
fn an_extra_file_counts_as_a_content_difference() {
    let ws = workspace();
    run_install_extension("vat", Scope::User, false, &ws.env());
    let dir = ws.user_ext_dir("vat");
    fs::write(dir.join("local.lua"), "-- my own module\n").unwrap();

    let outcome = run_install_extension("vat", Scope::User, false, &ws.env());
    assert_eq!(outcome.exit_code, EXIT_REFUSED, "{outcome:?}");
    assert_eq!(action(&outcome), "refused");
    assert!(
        dir.join("local.lua").exists(),
        "refusal must not touch files"
    );
}

// @spec EXT-INST-006
#[test]
fn older_installed_version_is_updated_without_force() {
    let ws = workspace();
    run_install_extension("vat", Scope::User, false, &ws.env());
    let dir = ws.user_ext_dir("vat");
    tamper_installed(&dir, "0.0.1");

    let outcome = run_install_extension("vat", Scope::User, false, &ws.env());
    assert_eq!(outcome.exit_code, EXIT_OK, "{outcome:?}");
    let json = stdout_json(&outcome);
    assert_eq!(json["action"], "updated");
    assert_eq!(json["previous_version"], "0.0.1");
    assert_eq!(
        dir_contents(&dir),
        dir_contents(&bundled_source_dir("vat")),
        "update must restore the bundled files"
    );
}

// @spec EXT-INST-006, EXT-INST-013
#[test]
fn newer_installed_version_is_refused_without_force() {
    let ws = workspace();
    run_install_extension("vat", Scope::User, false, &ws.env());
    let dir = ws.user_ext_dir("vat");
    tamper_installed(&dir, "9.9.9");
    let before = dir_contents(&dir);

    let outcome = run_install_extension("vat", Scope::User, false, &ws.env());
    assert_eq!(outcome.exit_code, EXIT_REFUSED, "{outcome:?}");
    let json = stdout_json(&outcome);
    assert_eq!(json["action"], "refused");
    assert_eq!(json["previous_version"], "9.9.9");
    assert_eq!(dir_contents(&dir), before);
}

// @spec EXT-INST-006
#[test]
fn unparseable_installed_manifest_is_refused_without_force() {
    let ws = workspace();
    run_install_extension("vat", Scope::User, false, &ws.env());
    let dir = ws.user_ext_dir("vat");
    fs::write(dir.join("extension.toml"), "this is = = not toml [[[").unwrap();

    let outcome = run_install_extension("vat", Scope::User, false, &ws.env());
    assert_eq!(outcome.exit_code, EXIT_REFUSED, "{outcome:?}");
    let json = stdout_json(&outcome);
    assert_eq!(json["action"], "refused");
    assert!(json["previous_version"].is_null(), "{json}");
}

// @spec EXT-INST-006
#[test]
fn force_overwrites_a_different_destination() {
    let ws = workspace();
    run_install_extension("vat", Scope::User, false, &ws.env());
    let dir = ws.user_ext_dir("vat");
    tamper_installed(&dir, "9.9.9");

    let outcome = run_install_extension("vat", Scope::User, true, &ws.env());
    assert_eq!(outcome.exit_code, EXIT_OK, "{outcome:?}");
    assert_eq!(action(&outcome), "updated");
    let installed = dir_contents(&dir);
    for (path, bytes) in dir_contents(&bundled_source_dir("vat")) {
        assert_eq!(
            installed.get(&path),
            Some(&bytes),
            "{} must be restored",
            path.display()
        );
    }
}

// ---- EXT-INST-007: enabling -------------------------------------------------

// @spec EXT-INST-007
#[test]
fn user_install_creates_global_config_enabling_the_extension() {
    let ws = workspace();
    assert!(!ws.global_config().exists());
    let outcome = run_install_extension("vat", Scope::User, false, &ws.env());
    assert_eq!(outcome.exit_code, EXIT_OK, "{outcome:?}");
    assert_eq!(enabled_list(&ws.global_config()), vec!["vat"]);
    assert!(
        !ws.project_config().exists(),
        "user scope must not create the project config"
    );
}

// @spec EXT-INST-007
#[test]
fn project_install_enables_in_project_config_only() {
    let ws = workspace();
    let outcome = run_install_extension("vat", Scope::Project, false, &ws.env());
    assert_eq!(outcome.exit_code, EXIT_OK, "{outcome:?}");
    assert_eq!(enabled_list(&ws.project_config()), vec!["vat"]);
    assert!(
        !ws.global_config().exists(),
        "project scope must not create the global config"
    );
}

// @spec EXT-INST-007
#[test]
fn enabling_appends_to_an_existing_enabled_list() {
    let ws = workspace();
    write(&ws.global_config(), "[extensions]\nenabled = [\"other\"]\n");
    let outcome = run_install_extension("vat", Scope::User, false, &ws.env());
    assert_eq!(outcome.exit_code, EXIT_OK, "{outcome:?}");
    assert_eq!(enabled_list(&ws.global_config()), vec!["other", "vat"]);
}

// @spec EXT-INST-007
#[test]
fn rerunning_does_not_duplicate_the_enabled_entry() {
    let ws = workspace();
    run_install_extension("vat", Scope::User, false, &ws.env());
    let outcome = run_install_extension("vat", Scope::User, false, &ws.env());
    assert_eq!(action(&outcome), "unchanged");
    assert_eq!(enabled_list(&ws.global_config()), vec!["vat"]);
}

// @spec EXT-INST-007
#[test]
fn unchanged_install_still_enables_when_missing_from_config() {
    let ws = workspace();
    run_install_extension("vat", Scope::User, false, &ws.env());
    // User removed it from `enabled` by hand; files are still identical.
    write(&ws.global_config(), "[extensions]\nenabled = []\n");

    let outcome = run_install_extension("vat", Scope::User, false, &ws.env());
    assert_eq!(action(&outcome), "unchanged");
    assert_eq!(enabled_list(&ws.global_config()), vec!["vat"]);
    assert_eq!(stdout_json(&outcome)["enabled"], true);
}

// @spec EXT-INST-007
#[test]
fn refused_install_does_not_touch_the_config() {
    let ws = workspace();
    run_install_extension("vat", Scope::User, false, &ws.env());
    tamper_installed(&ws.user_ext_dir("vat"), VAT_VERSION);
    write(&ws.global_config(), "[extensions]\nenabled = []\n");
    let before = fs::read_to_string(ws.global_config()).unwrap();

    let outcome = run_install_extension("vat", Scope::User, false, &ws.env());
    assert_eq!(outcome.exit_code, EXIT_REFUSED, "{outcome:?}");
    assert_eq!(fs::read_to_string(ws.global_config()).unwrap(), before);
}

// ---- EXT-INST-008: default grants -------------------------------------------

// @spec EXT-INST-008
#[test]
fn vat_default_grants_are_written_when_table_absent() {
    let ws = workspace();
    run_install_extension("vat", Scope::User, false, &ws.env());
    let table = read_toml(&ws.global_config());
    let vat = table["extensions"]["vat"]
        .as_table()
        .expect("[extensions.vat] table written");
    let paths: Vec<&str> = vat["paths"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert_eq!(paths, vec!["backlog", "~/.config/vat"]);
    let env: Vec<&str> = vat["env"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert_eq!(env, vec!["XDG_CONFIG_HOME"]);
}

// @spec EXT-INST-008
#[test]
fn vat_default_grants_written_to_project_config_for_project_scope() {
    let ws = workspace();
    run_install_extension("vat", Scope::Project, false, &ws.env());
    let table = read_toml(&ws.project_config());
    assert!(
        table["extensions"]["vat"].as_table().is_some(),
        "[extensions.vat] belongs in the project config for --scope project"
    );
}

// @spec EXT-INST-008
#[test]
fn existing_extension_table_is_left_alone() {
    let ws = workspace();
    write(
        &ws.global_config(),
        "[extensions]\nenabled = []\n\n[extensions.vat]\npaths = [\"custom\"]\n",
    );
    run_install_extension("vat", Scope::User, false, &ws.env());
    let table = read_toml(&ws.global_config());
    let vat = table["extensions"]["vat"].as_table().unwrap();
    assert_eq!(
        vat["paths"].as_array().unwrap(),
        &vec![toml::Value::String("custom".into())]
    );
    assert!(
        vat.get("env").is_none(),
        "no default keys may be merged into an existing table: {vat:?}"
    );
    assert_eq!(enabled_list(&ws.global_config()), vec!["vat"]);
}

// ---- EXT-INST-009: formatting preserved -------------------------------------

// @spec EXT-INST-009
#[test]
fn comments_and_formatting_outside_changed_keys_survive() {
    let ws = workspace();
    let original = "# my global gbiv config\n\
                    [other]\n\
                    key   =   1   # trailing comment\n\
                    \n\
                    [extensions]\n\
                    # keep this note\n\
                    enabled = [\"other\"]\n";
    write(&ws.global_config(), original);

    let outcome = run_install_extension("vat", Scope::User, false, &ws.env());
    assert_eq!(outcome.exit_code, EXIT_OK, "{outcome:?}");
    let after = fs::read_to_string(ws.global_config()).unwrap();
    for kept in [
        "# my global gbiv config",
        "key   =   1   # trailing comment",
        "# keep this note",
    ] {
        assert!(after.contains(kept), "lost {kept:?}; file now:\n{after}");
    }
    assert_eq!(enabled_list(&ws.global_config()), vec!["other", "vat"]);
}

// ---- EXT-INST-010: bad config aborts before copying -------------------------

// @spec EXT-INST-010, EXT-INST-013
#[test]
fn invalid_toml_config_exits_1_and_changes_nothing() {
    let ws = workspace();
    let broken = "[extensions\nenabled = [\"x\"\n";
    write(&ws.global_config(), broken);

    let outcome = run_install_extension("vat", Scope::User, false, &ws.env());
    assert_eq!(outcome.exit_code, EXIT_OTHER, "{outcome:?}");
    assert!(
        !ws.user_ext_dir("vat").exists(),
        "no extension files may be copied"
    );
    assert_eq!(fs::read_to_string(ws.global_config()).unwrap(), broken);
}

// @spec EXT-INST-010
#[test]
fn non_array_enabled_exits_1_and_changes_nothing() {
    let ws = workspace();
    let body = "[extensions]\nenabled = \"vat\"\n";
    write(&ws.project_config(), body);

    let outcome = run_install_extension("vat", Scope::Project, false, &ws.env());
    assert_eq!(outcome.exit_code, EXIT_OTHER, "{outcome:?}");
    assert!(!ws.project_ext_dir("vat").exists());
    assert_eq!(fs::read_to_string(ws.project_config()).unwrap(), body);
}

// ---- EXT-INST-011: disabled warning -----------------------------------------

// @spec EXT-INST-011
#[test]
fn warns_when_name_is_disabled_in_the_chosen_scope() {
    let ws = workspace();
    write(&ws.global_config(), "[extensions]\ndisabled = [\"vat\"]\n");
    let outcome = run_install_extension("vat", Scope::User, false, &ws.env());
    assert_eq!(outcome.exit_code, EXIT_OK, "{outcome:?}");
    let stderr = outcome.stderr.as_deref().unwrap_or_default();
    assert!(
        stderr.contains("disabled") && stderr.contains("vat"),
        "expected a disabled warning on stderr, got: {stderr:?}"
    );
}

// @spec EXT-INST-011
#[test]
fn warns_when_name_is_disabled_in_the_other_config_file() {
    let ws = workspace();
    // Installing for the user, but this project switches vat off.
    write(&ws.project_config(), "[extensions]\ndisabled = [\"vat\"]\n");
    let outcome = run_install_extension("vat", Scope::User, false, &ws.env());
    assert_eq!(outcome.exit_code, EXIT_OK, "{outcome:?}");
    let stderr = outcome.stderr.as_deref().unwrap_or_default();
    assert!(
        stderr.contains("disabled") && stderr.contains("vat"),
        "expected a disabled warning on stderr, got: {stderr:?}"
    );
}

// @spec EXT-INST-011
#[test]
fn no_disabled_warning_when_not_disabled() {
    let ws = workspace();
    let outcome = run_install_extension("vat", Scope::User, false, &ws.env());
    assert_eq!(outcome.exit_code, EXIT_OK, "{outcome:?}");
    let stderr = outcome.stderr.as_deref().unwrap_or_default();
    assert!(
        !stderr.contains("disabled"),
        "unexpected warning: {stderr:?}"
    );
}

// ---- EXT-INST-012: JSON result ----------------------------------------------

// @spec EXT-INST-012
#[test]
fn json_result_has_install_skill_fields_plus_enabled() {
    let ws = workspace();
    let outcome = run_install_extension("vat", Scope::User, false, &ws.env());
    assert_eq!(outcome.exit_code, EXIT_OK, "{outcome:?}");
    let json = stdout_json(&outcome);
    let obj = json.as_object().expect("a JSON object");
    let mut keys: Vec<&str> = obj.keys().map(String::as_str).collect();
    keys.sort_unstable();
    assert_eq!(
        keys,
        vec![
            "action",
            "bundled_version",
            "destination",
            "enabled",
            "previous_version",
            "reason",
            "scope",
        ]
    );
    assert_eq!(json["action"], "installed");
    assert_eq!(json["bundled_version"], VAT_VERSION);
    assert!(json["previous_version"].is_null());
    assert!(json["reason"].is_null());
    assert_eq!(json["enabled"], true);
}

// @spec EXT-INST-012
#[test]
fn enabled_is_false_when_already_enabled() {
    let ws = workspace();
    write(&ws.global_config(), "[extensions]\nenabled = [\"vat\"]\n");
    let outcome = run_install_extension("vat", Scope::User, false, &ws.env());
    assert_eq!(outcome.exit_code, EXIT_OK, "{outcome:?}");
    let json = stdout_json(&outcome);
    assert_eq!(json["action"], "installed");
    assert_eq!(json["enabled"], false);
}

// @spec EXT-INST-012
#[test]
fn second_run_reports_previous_version_and_enabled_false() {
    let ws = workspace();
    run_install_extension("vat", Scope::User, false, &ws.env());
    let outcome = run_install_extension("vat", Scope::User, false, &ws.env());
    let json = stdout_json(&outcome);
    assert_eq!(json["action"], "unchanged");
    assert_eq!(json["previous_version"], VAT_VERSION);
    assert_eq!(json["enabled"], false);
}

// @spec EXT-INST-012, EXT-INST-013
#[test]
fn refused_result_still_prints_json_with_enabled_false() {
    let ws = workspace();
    run_install_extension("vat", Scope::User, false, &ws.env());
    tamper_installed(&ws.user_ext_dir("vat"), VAT_VERSION);
    let outcome = run_install_extension("vat", Scope::User, false, &ws.env());
    assert_eq!(outcome.exit_code, EXIT_REFUSED);
    let json = stdout_json(&outcome);
    assert_eq!(json["action"], "refused");
    assert_eq!(json["enabled"], false);
    assert!(json["reason"].is_string());
}

// @spec EXT-INST-012, EXT-INST-013
#[test]
fn hard_failures_print_no_json() {
    let tmp = TempDir::new().unwrap();
    let env = InstallEnv {
        cwd: tmp.path().to_path_buf(),
        user_config_dir: tmp.path().join("xdg"),
    };
    let outcome = run_install_extension("vat", Scope::Project, false, &env);
    assert_eq!(outcome.exit_code, EXIT_NOT_A_GBIV_WORKSPACE);
    assert_eq!(
        outcome.stdout, None,
        "hard failures print nothing to stdout"
    );
    assert!(outcome.stderr.is_some());
}

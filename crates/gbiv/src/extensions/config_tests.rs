//! gbv-1ht tests (Phase 5).
//! `[extensions]` config merging — EXT-CFG-001 through EXT-CFG-007, against
//! `ExtensionsConfig::from_layers`.
//!
//! The layers passed in are whole config files parsed as TOML tables (the
//! shape `gbiv_core::config::Layers` holds); `from_layers` picks out each
//! layer's `[extensions]` table itself. The main repo and home paths below do
//! not exist, so no canonicalization can change them.

use std::path::{Path, PathBuf};

use super::{ExtensionError, ExtensionSettings, ExtensionsConfig};

const MAIN_REPO: &str = "/gbiv-cfg-test-nonexistent/root/main/repo";
const HOME: &str = "/gbiv-cfg-test-nonexistent/home";

fn table(src: &str) -> toml::Table {
    src.parse::<toml::Table>()
        .unwrap_or_else(|e| panic!("test TOML must parse: {e}\n{src}"))
}

fn merge(global: Option<&str>, project: Option<&str>) -> Result<ExtensionsConfig, ExtensionError> {
    let g = global.map(table);
    let p = project.map(table);
    ExtensionsConfig::from_layers(g.as_ref(), p.as_ref(), Path::new(MAIN_REPO), Path::new(HOME))
}

fn merged(global: Option<&str>, project: Option<&str>) -> ExtensionsConfig {
    merge(global, project).unwrap_or_else(|e| panic!("expected config to merge, got: {e}"))
}

/// Settings for `name`, or the defaults when no per-extension table applied.
fn settings(cfg: &ExtensionsConfig, name: &str) -> ExtensionSettings {
    cfg.settings.get(name).cloned().unwrap_or_default()
}

/// Assert a config error and return its detail.
fn config_err(global: Option<&str>, project: Option<&str>) -> String {
    match merge(global, project) {
        Err(ExtensionError::Config { detail }) => detail,
        Err(other) => panic!("expected ExtensionError::Config, got {other:?}"),
        Ok(cfg) => panic!("expected a config error, got {cfg:?}"),
    }
}

fn repo(rel: &str) -> PathBuf {
    Path::new(MAIN_REPO).join(rel)
}

fn names(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| s.to_string()).collect()
}

// ---- EXT-CFG-001: effective list -------------------------------------------

// @spec EXT-CFG-001
#[test]
fn effective_list_is_global_then_new_project_names_in_order() {
    let cfg = merged(
        Some("[extensions]\nenabled = [\"b\", \"a\"]\n"),
        Some("[extensions]\nenabled = [\"c\", \"a\", \"d\"]\n"),
    );
    assert_eq!(cfg.enabled, names(&["b", "a", "c", "d"]));
}

// @spec EXT-CFG-001
#[test]
fn project_disabled_removes_a_globally_enabled_name() {
    let cfg = merged(
        Some("[extensions]\nenabled = [\"vat\", \"jira\"]\n"),
        Some("[extensions]\ndisabled = [\"vat\"]\n"),
    );
    assert_eq!(cfg.enabled, names(&["jira"]));
    assert!(!cfg.settings.contains_key("vat"));
}

// @spec EXT-CFG-001
#[test]
fn global_disabled_removes_a_project_enabled_name() {
    let cfg = merged(
        Some("[extensions]\ndisabled = [\"c\"]\n"),
        Some("[extensions]\nenabled = [\"c\", \"d\"]\n"),
    );
    assert_eq!(cfg.enabled, names(&["d"]));
}

// @spec EXT-CFG-001
#[test]
fn a_name_disabled_in_its_own_layer_is_off() {
    let cfg = merged(
        Some("[extensions]\nenabled = [\"a\", \"b\"]\ndisabled = [\"a\"]\n"),
        None,
    );
    assert_eq!(cfg.enabled, names(&["b"]));
}

// @spec EXT-CFG-001
#[test]
fn only_one_layer_or_none_present() {
    assert_eq!(
        merged(None, Some("[extensions]\nenabled = [\"p\"]\n")).enabled,
        names(&["p"])
    );
    assert_eq!(
        merged(Some("[extensions]\nenabled = [\"g\"]\n"), None).enabled,
        names(&["g"])
    );
    let empty = merged(None, None);
    assert!(empty.enabled.is_empty());
    assert!(empty.settings.is_empty());
}

// @spec EXT-CFG-001
#[test]
fn files_without_an_extensions_table_enable_nothing() {
    let cfg = merged(
        Some("[palette]\nred = \"#ff0000\"\n"),
        Some("other = 1\n"),
    );
    assert!(cfg.enabled.is_empty());
}

// @spec EXT-CFG-001
#[test]
fn same_name_enabled_in_both_files_is_not_a_duplicate_error() {
    let cfg = merged(
        Some("[extensions]\nenabled = [\"vat\"]\n"),
        Some("[extensions]\nenabled = [\"vat\"]\n"),
    );
    assert_eq!(cfg.enabled, names(&["vat"]));
}

// ---- EXT-CFG-002: per-extension table merge ---------------------------------

// @spec EXT-CFG-002
#[test]
fn per_extension_tables_merge_key_by_key_project_wins() {
    let cfg = merged(
        Some(
            "[extensions]\nenabled = [\"vat\"]\n\n[extensions.vat]\n\
             paths = [\"backlog\"]\nbacklog = \"global-backlog\"\nmode = \"strict\"\n",
        ),
        Some("[extensions.vat]\nbacklog = \"project-backlog\"\n"),
    );
    let s = settings(&cfg, "vat");
    // The project table set only `backlog`: the global `paths` and `mode` survive.
    assert_eq!(s.paths, vec![repo("backlog")]);
    assert_eq!(
        s.options.get("backlog").and_then(|v| v.as_str()),
        Some("project-backlog")
    );
    assert_eq!(s.options.get("mode").and_then(|v| v.as_str()), Some("strict"));
}

// @spec EXT-CFG-002
#[test]
fn list_values_are_replaced_whole_not_concatenated() {
    let cfg = merged(
        Some(
            "[extensions]\nenabled = [\"vat\"]\n\n[extensions.vat]\n\
             paths = [\"/g/one\", \"/g/two\"]\nenv = [\"A\", \"B\"]\ntags = [\"x\", \"y\"]\n",
        ),
        Some(
            "[extensions.vat]\npaths = [\"/p/three\"]\nenv = [\"C\"]\ntags = [\"z\"]\n",
        ),
    );
    let s = settings(&cfg, "vat");
    assert_eq!(s.paths, vec![PathBuf::from("/p/three")]);
    assert_eq!(s.env, names(&["C"]));
    let tags = s.options.get("tags").and_then(|v| v.as_array()).expect("tags array");
    let tags: Vec<&str> = tags.iter().filter_map(|v| v.as_str()).collect();
    assert_eq!(tags, vec!["z"]);
}

// @spec EXT-CFG-002
#[test]
fn http_flag_merges_like_any_other_key() {
    let keep = merged(
        Some("[extensions]\nenabled = [\"x\"]\n[extensions.x]\nhttp = true\n"),
        Some("[extensions.x]\nother = 1\n"),
    );
    assert!(settings(&keep, "x").http, "project table without `http` keeps the global value");

    let over = merged(
        Some("[extensions]\nenabled = [\"x\"]\n[extensions.x]\nhttp = true\n"),
        Some("[extensions.x]\nhttp = false\n"),
    );
    assert!(!settings(&over, "x").http, "project value wins for the same key");
}

// @spec EXT-CFG-002
#[test]
fn a_table_in_only_one_file_is_used_as_is() {
    let cfg = merged(
        Some("[extensions]\nenabled = [\"x\"]\n"),
        Some("[extensions.x]\nenv = [\"TOKEN\"]\nflavor = \"mint\"\n"),
    );
    let s = settings(&cfg, "x");
    assert_eq!(s.env, names(&["TOKEN"]));
    assert_eq!(s.options.get("flavor").and_then(|v| v.as_str()), Some("mint"));
}

// ---- EXT-CFG-003: enabled / disabled validation ------------------------------

// @spec EXT-CFG-003
#[test]
fn enabled_that_is_not_an_array_is_a_config_error_naming_the_file() {
    let detail = config_err(None, Some("[extensions]\nenabled = \"vat\"\n"));
    assert!(detail.contains("project"), "detail should name the project config: {detail}");
    assert!(detail.contains("enabled"), "detail should name the key: {detail}");

    let detail = config_err(Some("[extensions]\nenabled = \"vat\"\n"), None);
    assert!(detail.contains("global"), "detail should name the global config: {detail}");
}

// @spec EXT-CFG-003
#[test]
fn enabled_or_disabled_with_non_string_elements_is_a_config_error() {
    let detail = config_err(Some("[extensions]\nenabled = [\"vat\", 3]\n"), None);
    assert!(detail.contains("global"), "{detail}");

    let detail = config_err(None, Some("[extensions]\ndisabled = [true]\n"));
    assert!(detail.contains("project"), "{detail}");
    assert!(detail.contains("disabled"), "{detail}");

    let detail = config_err(None, Some("[extensions]\ndisabled = 5\n"));
    assert!(detail.contains("project"), "{detail}");
}

// @spec EXT-CFG-003
#[test]
fn a_name_repeated_within_one_list_is_a_config_error() {
    let detail = config_err(Some("[extensions]\nenabled = [\"vat\", \"jira\", \"vat\"]\n"), None);
    assert!(detail.contains("global"), "{detail}");
    assert!(detail.contains("vat"), "detail should name the repeated extension: {detail}");

    let detail = config_err(None, Some("[extensions]\ndisabled = [\"x\", \"x\"]\n"));
    assert!(detail.contains("project"), "{detail}");
    assert!(detail.contains('x'), "{detail}");
}

// ---- EXT-CFG-004: non-table keys ---------------------------------------------

// @spec EXT-CFG-004
#[test]
fn a_misspelled_non_table_key_is_a_config_error_naming_key_and_file() {
    let detail = config_err(None, Some("[extensions]\nenabeld = [\"vat\"]\n"));
    assert!(detail.contains("enabeld"), "detail should name the key: {detail}");
    assert!(detail.contains("project"), "detail should name the file: {detail}");

    let detail = config_err(Some("[extensions]\nvat = 1\n"), None);
    assert!(detail.contains("vat"), "{detail}");
    assert!(detail.contains("global"), "{detail}");
}

// @spec EXT-CFG-004
#[test]
fn table_valued_keys_are_per_extension_tables_not_errors() {
    let cfg = merged(
        Some("[extensions]\nenabled = [\"vat\"]\n\n[extensions.vat]\nbacklog = \"b\"\n\n[extensions.unused]\nk = 1\n"),
        None,
    );
    assert_eq!(cfg.enabled, names(&["vat"]));
}

// ---- EXT-CFG-005: reserved keys vs options ----------------------------------

// @spec EXT-CFG-005
#[test]
fn reserved_keys_become_grants_and_everything_else_is_options() {
    let cfg = merged(
        None,
        Some(
            "[extensions]\nenabled = [\"x\"]\n\n[extensions.x]\n\
             paths = [\"/abs/dir\"]\nhttp = true\nenv = [\"TOKEN\"]\n\
             name = \"n\"\ncount = 3\nratio = 0.5\nflag = false\nlist = [1, 2]\n\
             [extensions.x.nested]\ninner = \"v\"\n",
        ),
    );
    let s = settings(&cfg, "x");
    assert_eq!(s.paths, vec![PathBuf::from("/abs/dir")]);
    assert!(s.http);
    assert_eq!(s.env, names(&["TOKEN"]));

    for reserved in ["paths", "http", "env"] {
        assert!(!s.options.contains_key(reserved), "{reserved} must not be an option");
    }
    let expected = table(
        "name = \"n\"\ncount = 3\nratio = 0.5\nflag = false\nlist = [1, 2]\n[nested]\ninner = \"v\"\n",
    );
    assert_eq!(s.options, expected);
}

// @spec EXT-CFG-005
#[test]
fn an_enabled_extension_without_a_table_has_no_grants_or_options() {
    let cfg = merged(Some("[extensions]\nenabled = [\"x\"]\n"), None);
    assert_eq!(cfg.enabled, names(&["x"]));
    assert_eq!(settings(&cfg, "x"), ExtensionSettings::default());
}

// ---- EXT-CFG-006: path resolution -------------------------------------------

// @spec EXT-CFG-006
#[test]
fn grant_paths_expand_tilde_and_resolve_relative_against_main_repo() {
    let cfg = merged(
        Some(
            "[extensions]\nenabled = [\"x\", \"y\"]\n\n[extensions.x]\n\
             paths = [\"backlog\", \"~/.config/vat\", \"/abs/elsewhere\", \"nested/dir\"]\n",
        ),
        // A relative path from the project file resolves against the main repo too.
        Some("[extensions.y]\npaths = [\"docs\", \"~/notes\"]\n"),
    );
    assert_eq!(
        settings(&cfg, "x").paths,
        vec![
            repo("backlog"),
            Path::new(HOME).join(".config/vat"),
            PathBuf::from("/abs/elsewhere"),
            repo("nested/dir"),
        ]
    );
    assert_eq!(
        settings(&cfg, "y").paths,
        vec![repo("docs"), Path::new(HOME).join("notes")]
    );
}

// ---- EXT-CFG-007: tables for extensions not in the effective list ------------

// @spec EXT-CFG-007
#[test]
fn tables_for_extensions_not_enabled_are_ignored() {
    let cfg = merged(
        Some("[extensions]\nenabled = [\"a\"]\n\n[extensions.ghost]\npaths = [\"/x\"]\n"),
        Some("[extensions]\ndisabled = [\"off\"]\n\n[extensions.off]\nhttp = true\n"),
    );
    assert_eq!(cfg.enabled, names(&["a"]));
    assert!(!cfg.settings.contains_key("ghost"));
    assert!(!cfg.settings.contains_key("off"));
    assert!(cfg.settings.keys().all(|k| cfg.enabled.contains(k)));
}

// @spec EXT-CFG-007
#[test]
fn a_disabled_extension_drops_its_merged_table() {
    let cfg = merged(
        Some("[extensions]\nenabled = [\"vat\", \"b\"]\n\n[extensions.vat]\npaths = [\"backlog\"]\n"),
        Some("[extensions]\ndisabled = [\"vat\"]\n"),
    );
    assert_eq!(cfg.enabled, names(&["b"]));
    assert!(!cfg.settings.contains_key("vat"));
}

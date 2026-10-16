use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::colors::BASE_COLORS;

/// Reserved worktree names that an extra palette entry may not use.
const RESERVED_NAMES: [&str; 2] = ["main", "all"];

// @spec CLI-COLOR-022, CLI-COLOR-023, CLI-COLOR-024, CLI-COLOR-025
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("reading {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("parsing {path}: {source}")]
    Parse {
        path: PathBuf,
        #[source]
        source: toml::de::Error,
    },
    #[error("invalid palette name {name:?} in {path}: {reason}")]
    InvalidName {
        path: PathBuf,
        name: String,
        reason: String,
    },
}

#[derive(Debug, Default, Deserialize)]
struct RawConfig {
    #[serde(default)]
    palette: PaletteSection,
}

// @spec CLI-COLOR-028
// `deny_unknown_fields` rejects a mistyped key inside `[palette]` (e.g. `extras`
// or `exclude`) so a typo fails loudly instead of silently yielding a base-only
// palette. It applies only within `[palette]`; unknown *top-level* tables remain
// ignored (CLI-COLOR-020) so the shared config file can host other sections.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct PaletteSection {
    #[serde(default)]
    extra: Vec<String>,
}

/// The path of the config file relative to the gbiv root.
pub fn config_path(gbiv_root: &Path) -> PathBuf {
    gbiv_root.join(".gbiv").join("config.toml")
}

// @spec CLI-COLOR-019, CLI-COLOR-020, CLI-COLOR-021, CLI-COLOR-025
/// Read and validate the extra palette names from `<root>/.gbiv/config.toml`.
/// A missing file, a missing `[palette]` table, or an empty `extra` list all
/// yield an empty vec (i.e. a base-only palette).
pub fn load_extra_names(gbiv_root: &Path) -> Result<Vec<String>, ConfigError> {
    let path = config_path(gbiv_root);
    let contents = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(ConfigError::Io { path, source: e }),
    };
    let raw: RawConfig = toml::from_str(&contents).map_err(|source| ConfigError::Parse {
        path: path.clone(),
        source,
    })?;
    validate(&raw.palette.extra, &path)?;
    Ok(raw.palette.extra)
}

// @spec CLI-COLOR-022, CLI-COLOR-023, CLI-COLOR-024
fn validate(extra: &[String], path: &Path) -> Result<(), ConfigError> {
    let mut seen: Vec<String> = Vec::new();
    for name in extra {
        let invalid = |reason: &str| ConfigError::InvalidName {
            path: path.to_path_buf(),
            name: name.clone(),
            reason: reason.to_string(),
        };

        let first = match name.chars().next() {
            None => return Err(invalid("name must not be empty")),
            Some(c) => c,
        };
        if first == '.' || first == '-' {
            return Err(invalid("name must not begin with '.' or '-'"));
        }
        if !name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-')
        {
            return Err(invalid(
                "name may only contain ASCII letters, digits, '.', '_', or '-'",
            ));
        }

        let lower = name.to_ascii_lowercase();
        if RESERVED_NAMES.contains(&lower.as_str()) {
            return Err(invalid("name is reserved (\"main\", \"all\")"));
        }
        if BASE_COLORS.iter().any(|b| b.eq_ignore_ascii_case(name)) {
            return Err(invalid(
                "name collides with a base ROYGBIV color (case-insensitive)",
            ));
        }
        if seen.contains(&lower) {
            return Err(invalid("duplicate name (case-insensitive)"));
        }
        seen.push(lower);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn write_config(root: &Path, body: &str) {
        let dir = root.join(".gbiv");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("config.toml"), body).unwrap();
    }

    // @spec CLI-COLOR-020
    #[test]
    fn missing_file_yields_empty_extras() {
        let root = TempDir::new().unwrap();
        assert_eq!(load_extra_names(root.path()).unwrap(), Vec::<String>::new());
    }

    // @spec CLI-COLOR-020
    #[test]
    fn missing_palette_section_yields_empty_extras() {
        let root = TempDir::new().unwrap();
        write_config(root.path(), "[other]\nkey = 1\n");
        assert_eq!(load_extra_names(root.path()).unwrap(), Vec::<String>::new());
    }

    // @spec CLI-COLOR-020
    #[test]
    fn empty_extra_list_yields_empty_extras() {
        let root = TempDir::new().unwrap();
        write_config(root.path(), "[palette]\nextra = []\n");
        assert_eq!(load_extra_names(root.path()).unwrap(), Vec::<String>::new());
    }

    // @spec CLI-COLOR-021
    #[test]
    fn valid_extras_returned_in_declared_order() {
        let root = TempDir::new().unwrap();
        write_config(root.path(), "[palette]\nextra = [\"amber\", \"cobalt\"]\n");
        assert_eq!(
            load_extra_names(root.path()).unwrap(),
            vec!["amber".to_string(), "cobalt".to_string()]
        );
    }

    // @spec CLI-COLOR-025
    #[test]
    fn malformed_toml_is_parse_error() {
        let root = TempDir::new().unwrap();
        write_config(root.path(), "[palette]\nextra = [\"unterminated\n");
        assert!(matches!(
            load_extra_names(root.path()),
            Err(ConfigError::Parse { .. })
        ));
    }

    // @spec CLI-COLOR-028
    #[test]
    fn unknown_key_in_palette_table_is_parse_error() {
        let root = TempDir::new().unwrap();
        // A misspelled key ("extras" instead of "extra") must not silently yield
        // a base-only palette — it is a parse error naming the file.
        write_config(root.path(), "[palette]\nextras = [\"amber\"]\n");
        assert!(matches!(
            load_extra_names(root.path()),
            Err(ConfigError::Parse { .. })
        ));
    }

    // @spec CLI-COLOR-020
    #[test]
    fn unknown_top_level_table_is_still_ignored() {
        // Unknown *top-level* tables remain permissive so the config file can host
        // other sections; only keys inside [palette] are strict.
        let root = TempDir::new().unwrap();
        write_config(
            root.path(),
            "[other]\nkey = 1\n[palette]\nextra = [\"amber\"]\n",
        );
        assert_eq!(
            load_extra_names(root.path()).unwrap(),
            vec!["amber".to_string()]
        );
    }

    // @spec CLI-COLOR-022
    #[test]
    fn empty_name_is_invalid() {
        let root = TempDir::new().unwrap();
        write_config(root.path(), "[palette]\nextra = [\"\"]\n");
        assert!(matches!(
            load_extra_names(root.path()),
            Err(ConfigError::InvalidName { .. })
        ));
    }

    // @spec CLI-COLOR-022
    #[test]
    fn name_with_illegal_char_is_invalid() {
        let root = TempDir::new().unwrap();
        write_config(root.path(), "[palette]\nextra = [\"has/slash\"]\n");
        assert!(matches!(
            load_extra_names(root.path()),
            Err(ConfigError::InvalidName { .. })
        ));
    }

    // @spec CLI-COLOR-022
    #[test]
    fn name_starting_with_dot_or_dash_is_invalid() {
        let root = TempDir::new().unwrap();
        write_config(root.path(), "[palette]\nextra = [\".hidden\"]\n");
        assert!(matches!(
            load_extra_names(root.path()),
            Err(ConfigError::InvalidName { .. })
        ));
        write_config(root.path(), "[palette]\nextra = [\"-dash\"]\n");
        assert!(matches!(
            load_extra_names(root.path()),
            Err(ConfigError::InvalidName { .. })
        ));
    }

    // @spec CLI-COLOR-024
    #[test]
    fn reserved_name_is_invalid() {
        let root = TempDir::new().unwrap();
        for reserved in ["main", "all", "ALL", "Main"] {
            write_config(
                root.path(),
                &format!("[palette]\nextra = [\"{reserved}\"]\n"),
            );
            assert!(
                matches!(
                    load_extra_names(root.path()),
                    Err(ConfigError::InvalidName { .. })
                ),
                "{reserved} should be reserved"
            );
        }
    }

    // @spec CLI-COLOR-023
    #[test]
    fn case_insensitive_collision_with_base_color_is_invalid() {
        let root = TempDir::new().unwrap();
        write_config(root.path(), "[palette]\nextra = [\"Red\"]\n");
        assert!(matches!(
            load_extra_names(root.path()),
            Err(ConfigError::InvalidName { .. })
        ));
    }

    // @spec CLI-COLOR-023
    #[test]
    fn case_insensitive_duplicate_extra_is_invalid() {
        let root = TempDir::new().unwrap();
        write_config(root.path(), "[palette]\nextra = [\"amber\", \"Amber\"]\n");
        assert!(matches!(
            load_extra_names(root.path()),
            Err(ConfigError::InvalidName { .. })
        ));
    }

    // ---- Layered configuration (gbv-1ht) ------------------------------------

    fn write_file(path: &Path, body: &str) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, body).unwrap();
    }

    // @spec CLI-CFG-001
    #[test]
    fn global_config_path_prefers_xdg_config_home() {
        let xdg = Path::new("/x/cfg");
        let home = Path::new("/h/me");
        assert_eq!(
            global_config_path(Some(xdg), Some(home)),
            Some(PathBuf::from("/x/cfg/gbiv/config.toml"))
        );
        assert_eq!(
            global_config_path(Some(xdg), None),
            Some(PathBuf::from("/x/cfg/gbiv/config.toml"))
        );
    }

    // @spec CLI-CFG-001
    #[test]
    fn global_config_path_falls_back_to_home_dot_config() {
        assert_eq!(
            global_config_path(None, Some(Path::new("/h/me"))),
            Some(PathBuf::from("/h/me/.config/gbiv/config.toml"))
        );
    }

    // @spec CLI-CFG-001
    #[test]
    fn global_config_path_is_none_without_xdg_or_home() {
        assert_eq!(global_config_path(None, None), None);
    }

    // @spec CLI-CFG-001
    #[test]
    fn read_layers_reads_both_files() {
        let root = TempDir::new().unwrap();
        let cfg = TempDir::new().unwrap();
        write_config(root.path(), "[extensions]\nenabled = [\"proj\"]\n");
        let global = cfg.path().join("gbiv").join("config.toml");
        write_file(&global, "[extensions]\nenabled = [\"glob\"]\n");

        let layers = read_layers(root.path(), Some(&global)).unwrap();
        let project = layers.project.expect("project layer read");
        let global = layers.global.expect("global layer read");
        assert_eq!(
            project["extensions"]["enabled"].as_array().unwrap()[0].as_str(),
            Some("proj")
        );
        assert_eq!(
            global["extensions"]["enabled"].as_array().unwrap()[0].as_str(),
            Some("glob")
        );
    }

    // @spec CLI-CFG-001
    #[test]
    fn read_layers_keeps_unknown_top_level_tables_raw() {
        // The layer reader is a plain TOML parse; owners of each table apply
        // their own rules later, so nothing is dropped or validated here.
        let root = TempDir::new().unwrap();
        write_config(
            root.path(),
            "[future]\nx = 1\n[palette]\nextra = [\"amber\"]\n",
        );
        let layers = read_layers(root.path(), None).unwrap();
        let project = layers.project.unwrap();
        assert!(project.contains_key("future"));
        assert!(project.contains_key("palette"));
    }

    // @spec CLI-CFG-002
    #[test]
    fn read_layers_treats_missing_files_as_absent() {
        let root = TempDir::new().unwrap();
        let cfg = TempDir::new().unwrap();
        let missing_global = cfg.path().join("gbiv").join("config.toml");

        let layers = read_layers(root.path(), Some(&missing_global)).unwrap();
        assert!(layers.project.is_none(), "no project file -> None");
        assert!(layers.global.is_none(), "no global file -> None");
    }

    // @spec CLI-CFG-002
    #[test]
    fn read_layers_without_a_global_path_reads_only_the_project() {
        let root = TempDir::new().unwrap();
        write_config(root.path(), "[extensions]\nenabled = []\n");
        let layers = read_layers(root.path(), None).unwrap();
        assert!(layers.global.is_none());
        assert!(layers.project.is_some());
    }

    // @spec CLI-CFG-002
    #[test]
    fn read_layers_global_only() {
        let root = TempDir::new().unwrap();
        let cfg = TempDir::new().unwrap();
        let global = cfg.path().join("config.toml");
        write_file(&global, "[extensions]\nenabled = [\"vat\"]\n");
        let layers = read_layers(root.path(), Some(&global)).unwrap();
        assert!(layers.project.is_none());
        assert!(layers.global.is_some());
    }

    // @spec CLI-CFG-005
    #[test]
    fn unparseable_global_file_is_a_parse_error_naming_it() {
        let root = TempDir::new().unwrap();
        let cfg = TempDir::new().unwrap();
        let global = cfg.path().join("gbiv").join("config.toml");
        write_file(&global, "[extensions\nenabled = [\n");

        let err = read_layers(root.path(), Some(&global)).unwrap_err();
        match &err {
            ConfigError::Parse { path, .. } => assert_eq!(path, &global),
            other => panic!("expected ConfigError::Parse, got {other:?}"),
        }
        assert!(
            err.to_string().contains(&global.display().to_string()),
            "message must name the file: {err}"
        );
    }

    // @spec CLI-CFG-005
    #[test]
    fn unparseable_project_file_is_a_parse_error_naming_it() {
        let root = TempDir::new().unwrap();
        write_config(root.path(), "not = = toml");
        let err = read_layers(root.path(), None).unwrap_err();
        match &err {
            ConfigError::Parse { path, .. } => assert_eq!(path, &config_path(root.path())),
            other => panic!("expected ConfigError::Parse, got {other:?}"),
        }
    }

    /// Point `XDG_CONFIG_HOME` and `HOME` at `dir` for the duration of `f`,
    /// so any global-config read would land in a temp dir (never the real
    /// `~/.config`). Callers are `#[serial]`.
    fn with_config_home<T>(dir: &Path, f: impl FnOnce() -> T) -> T {
        let old_xdg = std::env::var_os("XDG_CONFIG_HOME");
        let old_home = std::env::var_os("HOME");
        std::env::set_var("XDG_CONFIG_HOME", dir);
        std::env::set_var("HOME", dir);
        let out = f();
        match old_xdg {
            Some(v) => std::env::set_var("XDG_CONFIG_HOME", v),
            None => std::env::remove_var("XDG_CONFIG_HOME"),
        }
        match old_home {
            Some(v) => std::env::set_var("HOME", v),
            None => std::env::remove_var("HOME"),
        }
        out
    }

    // @spec CLI-CFG-003
    #[test]
    #[serial_test::serial]
    fn palette_ignores_global_palette_table() {
        let root = TempDir::new().unwrap();
        let cfg = TempDir::new().unwrap();
        write_file(
            &cfg.path().join("gbiv").join("config.toml"),
            "[palette]\nextra = [\"cobalt\"]\n",
        );
        write_file(
            &cfg.path().join(".config").join("gbiv").join("config.toml"),
            "[palette]\nextra = [\"cobalt\"]\n",
        );
        write_config(root.path(), "[palette]\nextra = [\"amber\"]\n");

        let palette = with_config_home(cfg.path(), || {
            crate::palette::Palette::load(root.path()).unwrap()
        });
        assert!(palette.contains("amber"), "project extras apply");
        assert!(
            !palette.contains("cobalt"),
            "global [palette] must be ignored"
        );
    }

    // @spec CLI-CFG-006
    #[test]
    #[serial_test::serial]
    fn palette_load_survives_malformed_global_and_extensions_tables() {
        let root = TempDir::new().unwrap();
        let cfg = TempDir::new().unwrap();
        write_file(
            &cfg.path().join("gbiv").join("config.toml"),
            "this is [[[ not toml",
        );
        write_file(
            &cfg.path().join(".config").join("gbiv").join("config.toml"),
            "this is [[[ not toml",
        );
        // `[extensions]` content that the extension runtime would reject.
        write_config(
            root.path(),
            "[palette]\nextra = [\"amber\"]\n\n[extensions]\nenabled = 5\n\n[extensions.vat]\npaths = \"not-a-list\"\n",
        );

        let palette = with_config_home(cfg.path(), || crate::palette::Palette::load(root.path()));
        let palette = palette.expect("palette load must not read global or [extensions]");
        assert!(palette.contains("amber"));
    }
}

// ---- Layered configuration (gbv-1ht) ----------------------------------------

/// The two optional config layers, parsed as raw TOML tables. Owners of each
/// top-level table (`[palette]`, `[extensions]`) apply their own rules on top.
#[derive(Debug, Default, Clone)]
pub struct Layers {
    pub global: Option<toml::Table>,
    pub project: Option<toml::Table>,
}

/// Path of the global config file: `$XDG_CONFIG_HOME/gbiv/config.toml`, else
/// `<home>/.config/gbiv/config.toml`. `None` when neither is known.
// @spec CLI-CFG-001
pub fn global_config_path(xdg_config_home: Option<&Path>, home: Option<&Path>) -> Option<PathBuf> {
    let _ = (xdg_config_home, home);
    todo!("gbv-1ht: global_config_path")
}

/// Read both layers (TOML parse only). A missing file yields `None` for that
/// layer; an unparseable file is a `ConfigError::Parse` naming it.
// @spec CLI-CFG-001, CLI-CFG-002, CLI-CFG-005
pub fn read_layers(gbiv_root: &Path, global_path: Option<&Path>) -> Result<Layers, ConfigError> {
    let _ = (gbiv_root, global_path);
    todo!("gbv-1ht: read_layers")
}

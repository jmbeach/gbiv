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

#[derive(Debug, Default, Deserialize)]
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
}

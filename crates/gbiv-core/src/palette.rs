use std::path::Path;

use crate::colors::BASE_COLORS;
use crate::config::{load_extra_names, ConfigError};
use crate::root::find_repo_in_worktree;

/// The active palette: the fixed base ROYGBIV colors followed by any extra
/// worktree names declared in `.gbiv/config.toml`, in declared order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Palette {
    names: Vec<String>,
}

impl Palette {
    // @spec CLI-COLOR-018, CLI-COLOR-019, CLI-COLOR-021, CLI-COLOR-026
    /// Load the active palette from the gbiv root. Returns the base colors plus
    /// any validated extras. A malformed or invalid config is a hard error.
    pub fn load(gbiv_root: &Path) -> Result<Palette, ConfigError> {
        let extras = load_extra_names(gbiv_root)?;
        let mut names: Vec<String> = BASE_COLORS.iter().map(|s| s.to_string()).collect();
        names.extend(extras);
        Ok(Palette { names })
    }

    /// Construct a palette from an explicit list of names (base + extras).
    /// Primarily for tests and callers that already hold the resolved names.
    pub fn from_names(extras: Vec<String>) -> Palette {
        let mut names: Vec<String> = BASE_COLORS.iter().map(|s| s.to_string()).collect();
        names.extend(extras);
        Palette { names }
    }

    /// The full active palette in canonical order (base colors, then extras).
    pub fn names(&self) -> &[String] {
        &self.names
    }

    /// The extra names beyond the base colors.
    pub fn extras(&self) -> &[String] {
        &self.names[BASE_COLORS.len()..]
    }

    // @spec CLI-COLOR-015, CLI-COLOR-016
    /// Whether a name is in the active palette (base color or configured extra).
    pub fn contains(&self, name: &str) -> bool {
        self.names.iter().any(|n| n == name)
    }

    /// Whether a name is one of the fixed base ROYGBIV colors.
    pub fn is_base(name: &str) -> bool {
        BASE_COLORS.contains(&name)
    }
}

// @spec CLI-COLOR-020
/// The default palette: exactly the base ROYGBIV colors, no extras.
impl Default for Palette {
    fn default() -> Palette {
        Palette {
            names: BASE_COLORS.iter().map(|s| s.to_string()).collect(),
        }
    }
}

// @spec WTL-REPAIR-013
/// The active-palette names that have no worktree on disk (no git repo found
/// within `<root>/<name>`). Used to warn about drift and suggest `gbiv repair`.
pub fn palette_drift(gbiv_root: &Path, palette: &Palette) -> Vec<String> {
    palette
        .names()
        .iter()
        .filter(|name| find_repo_in_worktree(&gbiv_root.join(name)).is_none())
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    // @spec CLI-COLOR-020
    #[test]
    fn default_palette_is_base_colors() {
        let p = Palette::default();
        assert_eq!(p.names(), BASE_COLORS);
        assert!(p.extras().is_empty());
    }

    // @spec CLI-COLOR-015, CLI-COLOR-016
    #[test]
    fn contains_matches_base_and_extra_but_not_others() {
        let p = Palette::from_names(vec!["my-extra".to_string()]);
        assert!(p.contains("red"));
        assert!(p.contains("my-extra"));
        assert!(!p.contains("purple"));
        assert!(!p.contains("main"));
    }

    // @spec CLI-COLOR-021
    #[test]
    fn extras_follow_base_in_order() {
        let p = Palette::from_names(vec!["a".to_string(), "b".to_string()]);
        assert_eq!(p.extras(), ["a".to_string(), "b".to_string()]);
        assert_eq!(p.names().len(), BASE_COLORS.len() + 2);
        assert_eq!(&p.names()[0], "red");
        assert_eq!(&p.names()[BASE_COLORS.len()], "a");
    }

    #[test]
    fn is_base_only_true_for_roygbiv() {
        assert!(Palette::is_base("violet"));
        assert!(!Palette::is_base("my-extra"));
    }

    // @spec CLI-COLOR-021
    #[test]
    fn load_appends_validated_extras() {
        let root = TempDir::new().unwrap();
        let dir = root.path().join(".gbiv");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("config.toml"), "[palette]\nextra = [\"amber\"]\n").unwrap();
        let p = Palette::load(root.path()).unwrap();
        assert!(p.contains("amber"));
        assert_eq!(p.extras(), ["amber".to_string()]);
    }

    // @spec WTL-REPAIR-013
    #[test]
    fn palette_drift_lists_names_without_worktrees() {
        let root = TempDir::new().unwrap();
        // Create a real repo for "red" only.
        let red_repo = root.path().join("red").join("proj");
        fs::create_dir_all(red_repo.join(".git")).unwrap();
        let palette = Palette::default();
        let drift = palette_drift(root.path(), &palette);
        assert!(!drift.contains(&"red".to_string()), "red exists, not drift");
        assert!(drift.contains(&"orange".to_string()), "orange is missing");
        assert_eq!(drift.len(), BASE_COLORS.len() - 1);
    }
}

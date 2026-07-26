use crate::colors::BASE_COLORS;
use crate::config::{load_extra_names, ConfigError};
use std::path::Path;

/// The active palette: the fixed base ROYGBIV colors followed by any extra
/// worktree names declared in `.gbiv/config.toml`, in declared order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Palette {
    names: Vec<String>,
}

impl Palette {
    /// The base ROYGBIV colors followed by `extras`, in order. The single place
    /// the base-prefix is assembled, shared by `load`, `from_extras`, and `Default`.
    fn with_extras(extras: Vec<String>) -> Palette {
        let mut names: Vec<String> = BASE_COLORS.iter().map(|s| s.to_string()).collect();
        names.extend(extras);
        Palette { names }
    }

    // @spec CLI-COLOR-018, CLI-COLOR-019, CLI-COLOR-021, CLI-COLOR-026
    /// Load the active palette from the gbiv root. Returns the base colors plus
    /// any validated extras. A malformed or invalid config is a hard error.
    pub fn load(gbiv_root: &Path) -> Result<Palette, ConfigError> {
        Ok(Palette::with_extras(load_extra_names(gbiv_root)?))
    }

    /// Construct a palette from an explicit list of extra names (appended after
    /// the base colors). Primarily for tests and callers that already hold the
    /// resolved extras.
    pub fn from_extras(extras: Vec<String>) -> Palette {
        Palette::with_extras(extras)
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
        Palette::with_extras(Vec::new())
    }
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
        let p = Palette::from_extras(vec!["my-extra".to_string()]);
        assert!(p.contains("red"));
        assert!(p.contains("my-extra"));
        assert!(!p.contains("purple"));
        assert!(!p.contains("main"));
    }

    // @spec CLI-COLOR-021
    #[test]
    fn extras_follow_base_in_order() {
        let p = Palette::from_extras(vec!["a".to_string(), "b".to_string()]);
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
}

use std::path::Path;

// @spec CLI-COLOR-001
pub const COLORS: [&str; 7] = ["red", "orange", "yellow", "green", "blue", "indigo", "violet"];

// @spec CLI-COLOR-015, CLI-COLOR-016
pub fn is_valid_color(name: &str) -> bool {
    COLORS.contains(&name)
}

// @spec WTL-UTIL-004, WTL-UTIL-005, WTL-UTIL-006
/// Infer the color by finding the path component directly under the gbiv root.
pub fn infer_color_from_path(cwd: &Path, gbiv_root: &Path) -> Option<&'static str> {
    let relative = cwd.strip_prefix(gbiv_root).ok()?;
    let first_component = relative.components().next()?;
    let name = first_component.as_os_str().to_str()?;
    COLORS.iter().find(|&&c| c == name).copied()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    // @spec CLI-COLOR-015
    #[test]
    fn is_valid_color_returns_true_for_known_color() {
        for c in COLORS {
            assert!(is_valid_color(c), "{} should be valid", c);
        }
    }

    // @spec CLI-COLOR-016
    #[test]
    fn is_valid_color_returns_false_for_unknown_name() {
        for name in ["", "rainbow", "Red", "RED", "purple", "black"] {
            assert!(!is_valid_color(name), "{:?} should be invalid", name);
        }
    }

    // @spec WTL-UTIL-004
    #[test]
    fn infer_color_from_path_returns_color_when_under_color_worktree() {
        let root = PathBuf::from("/tmp/gbiv-root");
        let cwd = root.join("red").join("project").join("src");
        assert_eq!(infer_color_from_path(&cwd, &root), Some("red"));
    }

    // @spec WTL-UTIL-005
    #[test]
    fn infer_color_from_path_returns_none_when_first_component_not_a_color() {
        let root = PathBuf::from("/tmp/gbiv-root");
        let cwd = root.join("main").join("project");
        assert!(infer_color_from_path(&cwd, &root).is_none());
    }

    // @spec WTL-UTIL-006
    #[test]
    fn infer_color_from_path_returns_none_when_cwd_not_under_root() {
        let root = PathBuf::from("/tmp/gbiv-root");
        let cwd = PathBuf::from("/elsewhere/red");
        assert!(infer_color_from_path(&cwd, &root).is_none());
    }
}

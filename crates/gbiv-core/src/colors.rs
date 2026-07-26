use std::path::Path;

use crate::palette::Palette;

// @spec CLI-COLOR-001
/// The immutable base ROYGBIV palette. This is the default palette and the fixed
/// prefix of every active palette; the config may append extra names but never
/// renames or removes a base color. Root discovery keys off this constant.
pub const BASE_COLORS: [&str; 7] = [
    "red", "orange", "yellow", "green", "blue", "indigo", "violet",
];

// @spec WTL-UTIL-004, WTL-UTIL-005, WTL-UTIL-006
/// Infer the worktree name by matching the first path component under the gbiv
/// root against the active palette. Returns an owned name because the active
/// palette is runtime data, not `&'static`.
pub fn infer_color_from_path(cwd: &Path, gbiv_root: &Path, palette: &Palette) -> Option<String> {
    let relative = cwd.strip_prefix(gbiv_root).ok()?;
    let first_component = relative.components().next()?;
    let name = first_component.as_os_str().to_str()?;
    if palette.contains(name) {
        Some(name.to_string())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    // @spec WTL-UTIL-004
    #[test]
    fn infer_color_from_path_returns_color_when_under_color_worktree() {
        let root = PathBuf::from("/tmp/gbiv-root");
        let cwd = root.join("red").join("project").join("src");
        let palette = Palette::default();
        assert_eq!(
            infer_color_from_path(&cwd, &root, &palette).as_deref(),
            Some("red")
        );
    }

    // @spec WTL-UTIL-005
    #[test]
    fn infer_color_from_path_returns_extra_name_when_under_extra_worktree() {
        let root = PathBuf::from("/tmp/gbiv-root");
        let cwd = root.join("my-extra").join("project");
        let palette = Palette::from_extras(vec!["my-extra".to_string()]);
        assert_eq!(
            infer_color_from_path(&cwd, &root, &palette).as_deref(),
            Some("my-extra")
        );
    }

    // @spec WTL-UTIL-006
    #[test]
    fn infer_color_from_path_returns_none_when_first_component_not_in_palette() {
        let root = PathBuf::from("/tmp/gbiv-root");
        let cwd = root.join("main").join("project");
        let palette = Palette::default();
        assert!(infer_color_from_path(&cwd, &root, &palette).is_none());
    }

    // @spec WTL-UTIL-006
    #[test]
    fn infer_color_from_path_returns_none_when_cwd_not_under_root() {
        let root = PathBuf::from("/tmp/gbiv-root");
        let cwd = PathBuf::from("/elsewhere/red");
        let palette = Palette::default();
        assert!(infer_color_from_path(&cwd, &root, &palette).is_none());
    }
}

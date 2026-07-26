use std::path::{Path, PathBuf};
use std::process::Command;

use crate::colors::BASE_COLORS;

pub struct GbivRoot {
    pub root: PathBuf,
    pub folder_name: String,
}

// @spec WTL-UTIL-001, WTL-UTIL-002, WTL-UTIL-003
pub fn find_gbiv_root(start: &Path) -> Option<GbivRoot> {
    let mut current = start.to_path_buf();
    loop {
        if let Some(folder_name) = current.file_name().and_then(|n| n.to_str()) {
            let candidate = current.join("main").join(folder_name);
            let has_color_dir = BASE_COLORS.iter().any(|c| current.join(c).is_dir());
            if candidate.exists() && is_git_repo(&candidate) && has_color_dir {
                return Some(GbivRoot {
                    root: current.clone(),
                    folder_name: folder_name.to_string(),
                });
            }
        }
        if !current.pop() {
            break;
        }
    }
    None
}

// @spec WTL-UTIL-019
pub fn is_git_repo(path: &Path) -> bool {
    let output = Command::new("git")
        .args(["rev-parse", "--git-dir"])
        .current_dir(path)
        .output();
    matches!(output, Ok(o) if o.status.success())
}

// @spec WTL-UTIL-014, WTL-UTIL-015
pub fn find_repo_in_worktree(worktree_dir: &Path) -> Option<PathBuf> {
    for entry in std::fs::read_dir(worktree_dir).ok()? {
        let Ok(entry) = entry else { continue };
        let path = entry.path();
        if path.is_dir() && path.join(".git").exists() {
            return Some(path);
        }
    }
    None
}

/// The on-disk state of a single palette worktree slot, as seen from the gbiv root.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorktreePresence {
    /// A git repo was found within `<root>/<name>`; carries its path.
    Present(PathBuf),
    /// The directory is absent or empty — repairable by `gbiv repair`.
    Missing,
    /// The directory exists and is non-empty but contains no git repo —
    /// an unresolved problem that `gbiv repair` deliberately will not overwrite.
    Broken,
}

// @spec WTL-UTIL-020
/// Classify a palette worktree slot at `<gbiv_root>/<name>` as Present, Missing,
/// or Broken. This is the single definition of those states shared by `status`
/// (row + drift hints) and `repair` (create vs. skip vs. flag), so the two never
/// disagree about what "missing" and "broken" mean.
pub fn classify_worktree(gbiv_root: &Path, name: &str) -> WorktreePresence {
    let dir = gbiv_root.join(name);
    if let Some(repo) = find_repo_in_worktree(&dir) {
        WorktreePresence::Present(repo)
    } else if dir
        .read_dir()
        .map(|mut entries| entries.next().is_some())
        .unwrap_or(false)
    {
        WorktreePresence::Broken
    } else {
        WorktreePresence::Missing
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn init_git_repo(path: &Path) {
        Command::new("git")
            .args(["init"])
            .current_dir(path)
            .output()
            .unwrap();
        Command::new("git")
            .args(["config", "user.email", "test@test.com"])
            .current_dir(path)
            .output()
            .unwrap();
        Command::new("git")
            .args(["config", "user.name", "Test"])
            .current_dir(path)
            .output()
            .unwrap();
        fs::write(path.join("test.txt"), "test").unwrap();
        Command::new("git")
            .args(["add", "."])
            .current_dir(path)
            .output()
            .unwrap();
        Command::new("git")
            .args(["commit", "-m", "initial"])
            .current_dir(path)
            .output()
            .unwrap();
    }

    /// Build a minimal gbiv-shaped layout under `base/<project>` and return the
    /// project root + main repo path. Caller's TempDir owns cleanup.
    fn setup_gbiv_layout(base: &Path, project: &str) -> (PathBuf, PathBuf) {
        let project_root = base.join(project);
        let main_repo = project_root.join("main").join(project);
        fs::create_dir_all(&main_repo).unwrap();
        init_git_repo(&main_repo);
        fs::create_dir_all(project_root.join("red")).unwrap();
        (project_root, main_repo)
    }

    // @spec WTL-UTIL-001, WTL-UTIL-002
    #[test]
    fn test_find_gbiv_root_some() {
        let base = TempDir::new().unwrap();
        let (project_root, _) = setup_gbiv_layout(base.path(), "myproject");

        let result = find_gbiv_root(&project_root).expect("expected Some");
        assert_eq!(result.folder_name, "myproject");
        assert_eq!(result.root, project_root);
    }

    // @spec WTL-UTIL-001, WTL-UTIL-002
    #[test]
    fn test_find_gbiv_root_some_from_nested() {
        let base = TempDir::new().unwrap();
        let (_, main_repo) = setup_gbiv_layout(base.path(), "myproject");

        let result = find_gbiv_root(&main_repo).expect("expected Some");
        assert_eq!(result.folder_name, "myproject");
    }

    // @spec WTL-UTIL-003
    #[test]
    fn test_find_gbiv_root_none() {
        let base = TempDir::new().unwrap();
        assert!(find_gbiv_root(base.path()).is_none());
    }

    // @spec WTL-UTIL-020
    #[test]
    fn classify_worktree_present_missing_broken() {
        let base = TempDir::new().unwrap();
        let root = base.path();

        // Present: a real repo inside <root>/red/proj.
        let red_repo = root.join("red").join("proj");
        fs::create_dir_all(red_repo.join(".git")).unwrap();
        assert!(matches!(
            classify_worktree(root, "red"),
            WorktreePresence::Present(_)
        ));

        // Missing: directory absent entirely.
        assert_eq!(classify_worktree(root, "orange"), WorktreePresence::Missing);

        // Missing: directory exists but is empty (e.g. leftover parent).
        fs::create_dir_all(root.join("green")).unwrap();
        assert_eq!(classify_worktree(root, "green"), WorktreePresence::Missing);

        // Broken: directory non-empty but has no git repo — including for a
        // configured extra name, so extras are classified the same as base colors.
        let amber = root.join("amber");
        fs::create_dir_all(&amber).unwrap();
        fs::write(amber.join("stray.txt"), "x").unwrap();
        assert_eq!(classify_worktree(root, "amber"), WorktreePresence::Broken);
    }
}

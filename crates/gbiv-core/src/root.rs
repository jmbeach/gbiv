use std::path::{Path, PathBuf};
use std::process::Command;

use crate::colors::COLORS;

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
            let has_color_dir = COLORS.iter().any(|c| current.join(c).is_dir());
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn init_git_repo(path: &Path) {
        Command::new("git").args(["init"]).current_dir(path).output().unwrap();
        Command::new("git").args(["config", "user.email", "test@test.com"]).current_dir(path).output().unwrap();
        Command::new("git").args(["config", "user.name", "Test"]).current_dir(path).output().unwrap();
        fs::write(path.join("test.txt"), "test").unwrap();
        Command::new("git").args(["add", "."]).current_dir(path).output().unwrap();
        Command::new("git").args(["commit", "-m", "initial"]).current_dir(path).output().unwrap();
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
}

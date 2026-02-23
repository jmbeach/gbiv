use std::path::{Path, PathBuf};
use std::process::Command as ProcessCommand;
use std::time::Duration;

use crate::colors::COLORS;

pub struct GbivRoot {
    pub root: PathBuf,
    pub folder_name: String,
}

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

pub fn is_git_repo(path: &Path) -> bool {
    let output = ProcessCommand::new("git")
        .args(["rev-parse", "--git-dir"])
        .current_dir(path)
        .output();
    matches!(output, Ok(o) if o.status.success())
}

pub fn has_commits(path: &Path) -> bool {
    let output = ProcessCommand::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(path)
        .output();
    matches!(output, Ok(o) if o.status.success())
}

pub fn get_main_branch(path: &Path) -> Option<String> {
    let output = ProcessCommand::new("git")
        .args(["symbolic-ref", "--short", "HEAD"])
        .current_dir(path)
        .output()
        .ok()?;
    if output.status.success() {
        Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        None
    }
}

pub fn get_existing_branches(path: &Path) -> Vec<String> {
    let output = ProcessCommand::new("git")
        .args(["branch", "--list"])
        .current_dir(path)
        .output();
    match output {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout)
            .lines()
            .map(|l| l.trim().trim_start_matches("* ").to_string())
            .collect(),
        _ => vec![],
    }
}

pub struct QuickStatus {
    pub branch: Option<String>,
    pub is_dirty: bool,
    pub ahead_behind: Option<(u32, u32)>,
}

pub fn get_quick_status(path: &Path) -> QuickStatus {
    let output = ProcessCommand::new("git")
        .args(["status", "--porcelain=v2", "--branch"])
        .current_dir(path)
        .output();

    let mut branch = None;
    let mut is_dirty = false;
    let mut ahead_behind = None;

    if let Ok(o) = output {
        if o.status.success() {
            for line in String::from_utf8_lossy(&o.stdout).lines() {
                if line.starts_with("# branch.head ") {
                    branch = Some(line.trim_start_matches("# branch.head ").to_string());
                } else if line.starts_with("# branch.ab ") {
                    let ab = line.trim_start_matches("# branch.ab ");
                    let parts: Vec<&str> = ab.split_whitespace().collect();
                    if parts.len() == 2 {
                        let ahead: u32 = parts[0].trim_start_matches('+').parse().unwrap_or(0);
                        let behind: u32 = parts[1].trim_start_matches('-').parse().unwrap_or(0);
                        ahead_behind = Some((ahead, behind));
                    }
                } else if !line.starts_with('#') && !line.is_empty() {
                    is_dirty = true;
                }
            }
        }
    }

    QuickStatus { branch, is_dirty, ahead_behind }
}

pub fn get_ahead_behind_vs(path: &Path, target: &str) -> Option<(u32, u32)> {
    let output = ProcessCommand::new("git")
        .args(["rev-list", "--left-right", "--count", &format!("HEAD...{}", target)])
        .current_dir(path)
        .output()
        .ok()?;
    if output.status.success() {
        let text = String::from_utf8_lossy(&output.stdout);
        let parts: Vec<&str> = text.trim().split('\t').collect();
        if parts.len() == 2 {
            let ahead = parts[0].parse().unwrap_or(0);
            let behind = parts[1].parse().unwrap_or(0);
            return Some((ahead, behind));
        }
    }
    None
}

pub fn is_merged_into(path: &Path, branch: &str, target: &str) -> bool {
    let output = ProcessCommand::new("git")
        .args(["merge-base", "--is-ancestor", branch, target])
        .current_dir(path)
        .output();
    matches!(output, Ok(o) if o.status.success())
}

pub fn get_last_commit_age(path: &Path) -> Option<Duration> {
    let output = ProcessCommand::new("git")
        .args(["log", "-1", "--format=%ct"])
        .current_dir(path)
        .output()
        .ok()?;
    if output.status.success() {
        let timestamp: u64 = String::from_utf8_lossy(&output.stdout)
            .trim()
            .parse()
            .ok()?;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .ok()?
            .as_secs();
        Some(Duration::from_secs(now.saturating_sub(timestamp)))
    } else {
        None
    }
}

pub fn get_remote_main_branch(path: &Path) -> Option<String> {
    for candidate in ["origin/main", "origin/master", "origin/develop"] {
        let output = ProcessCommand::new("git")
            .args(["rev-parse", "--verify", candidate])
            .current_dir(path)
            .output();
        if matches!(output, Ok(o) if o.status.success()) {
            return Some(candidate.to_string());
        }
    }
    None
}

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

/// Resolves the actual git directory for a repo, handling both the normal case
/// (`.git/` is a directory) and the gitlink case (`.git` is a file containing
/// `gitdir: <path>`, as produced by `git worktree add`).
pub fn resolve_git_dir(repo: &Path) -> Option<PathBuf> {
    let dot_git = repo.join(".git");
    if dot_git.is_dir() {
        return Some(dot_git);
    }
    if dot_git.is_file() {
        let contents = std::fs::read_to_string(&dot_git).ok()?;
        for line in contents.lines() {
            if let Some(gitdir) = line.strip_prefix("gitdir:") {
                let gitdir = gitdir.trim();
                let resolved = if std::path::Path::new(gitdir).is_absolute() {
                    PathBuf::from(gitdir)
                } else {
                    repo.join(gitdir)
                };
                return resolved.canonicalize().ok();
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::process::Command;

    fn init_git_repo(path: &Path) {
        Command::new("git").args(["init"]).current_dir(path).output().unwrap();
        Command::new("git").args(["config", "user.email", "test@test.com"]).current_dir(path).output().unwrap();
        Command::new("git").args(["config", "user.name", "Test"]).current_dir(path).output().unwrap();
        fs::write(path.join("test.txt"), "test").unwrap();
        Command::new("git").args(["add", "."]).current_dir(path).output().unwrap();
        Command::new("git").args(["commit", "-m", "initial"]).current_dir(path).output().unwrap();
    }

    #[test]
    fn test_find_gbiv_root_some() {
        let base = PathBuf::from("/tmp/gbiv_test_find_root_some");
        let _ = fs::remove_dir_all(&base);
        let project_name = "myproject";
        let main_repo = base.join(project_name).join("main").join(project_name);
        fs::create_dir_all(&main_repo).unwrap();
        init_git_repo(&main_repo);
        // Create a color dir so the heuristic recognises this as a gbiv project
        fs::create_dir_all(base.join(project_name).join("red")).unwrap();

        // Call from inside the gbiv root: <base>/myproject/
        let result = find_gbiv_root(&base.join(project_name));
        assert!(result.is_some(), "expected Some but got None");
        let gbiv_root = result.unwrap();
        assert_eq!(gbiv_root.folder_name, project_name);
        assert_eq!(gbiv_root.root, base.join(project_name));

        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn test_find_gbiv_root_some_from_nested() {
        let base = PathBuf::from("/tmp/gbiv_test_find_root_nested");
        let _ = fs::remove_dir_all(&base);
        let project_name = "myproject";
        let main_repo = base.join(project_name).join("main").join(project_name);
        fs::create_dir_all(&main_repo).unwrap();
        init_git_repo(&main_repo);
        // Create a color dir so the heuristic recognises this as a gbiv project
        fs::create_dir_all(base.join(project_name).join("red")).unwrap();

        // Call from inside main/<project>/ — should still find the root
        let result = find_gbiv_root(&main_repo);
        assert!(result.is_some(), "expected Some but got None");
        let gbiv_root = result.unwrap();
        assert_eq!(gbiv_root.folder_name, project_name);

        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn test_find_gbiv_root_none() {
        let base = PathBuf::from("/tmp/gbiv_test_find_root_none");
        let _ = fs::remove_dir_all(&base);
        fs::create_dir_all(&base).unwrap();

        let result = find_gbiv_root(&base);
        assert!(result.is_none(), "expected None but got Some");

        let _ = fs::remove_dir_all(&base);
    }
}

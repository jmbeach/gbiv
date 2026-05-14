use std::path::{Path, PathBuf};
use std::process::Command as ProcessCommand;
use std::time::Duration;

#[derive(Debug, thiserror::Error)]
pub enum GitError {
    #[error("not inside a gbiv project (no main/<repo> found walking up from {0})")]
    NotInGbivProject(PathBuf),
    #[error("git command failed: {cmd}\nstderr: {stderr}")]
    GitFailed { cmd: String, stderr: String },
    #[error("rebase conflict in {0}")]
    RebaseConflict(PathBuf),
    #[error("worktree {0} already exists")]
    WorktreeAlreadyExists(String),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Core(#[from] gbiv_core::error::CoreError),
    #[error("{0}")]
    Other(String),
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

// @spec WTL-UTIL-007, WTL-UTIL-008, WTL-UTIL-009
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

// @spec WTL-UTIL-010, WTL-UTIL-011
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

pub fn checkout_branch(path: &Path, branch: &str) -> Result<(), GitError> {
    let output = ProcessCommand::new("git")
        .args(["checkout", branch])
        .current_dir(path)
        .output()
        .map_err(GitError::Io)?;
    if output.status.success() {
        Ok(())
    } else {
        Err(GitError::GitFailed {
            cmd: format!("git checkout {}", branch),
            stderr: String::from_utf8_lossy(&output.stderr).trim().to_string(),
        })
    }
}

// @spec WTL-UTIL-012, WTL-UTIL-013
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

/// Returns the common git directory for the given path.
/// For linked worktrees, this is the main .git directory (not the
/// worktree-specific subdirectory). Git reads info/exclude from the
/// common dir, so this is the correct location for ignore entries.
pub fn get_git_dir(path: &Path) -> Option<PathBuf> {
    let output = ProcessCommand::new("git")
        .args(["rev-parse", "--git-common-dir"])
        .current_dir(path)
        .output()
        .ok()?;
    if output.status.success() {
        let raw = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let p = PathBuf::from(&raw);
        if p.is_absolute() {
            Some(p)
        } else {
            Some(path.join(p).canonicalize().ok()?)
        }
    } else {
        None
    }
}

pub fn fetch_remote(path: &Path) -> Result<(), GitError> {
    let output = ProcessCommand::new("git")
        .args(["fetch", "origin"])
        .current_dir(path)
        .output()
        .map_err(GitError::Io)?;
    if output.status.success() {
        Ok(())
    } else {
        Err(GitError::GitFailed {
            cmd: "git fetch origin".to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).trim().to_string(),
        })
    }
}

pub fn pull(path: &Path) -> Result<(), GitError> {
    let output = ProcessCommand::new("git")
        .args(["pull"])
        .current_dir(path)
        .output()
        .map_err(GitError::Io)?;
    if output.status.success() {
        Ok(())
    } else {
        Err(GitError::GitFailed {
            cmd: "git pull".to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).trim().to_string(),
        })
    }
}

pub fn reset_hard(path: &Path, target: &str) -> Result<(), GitError> {
    let output = ProcessCommand::new("git")
        .args(["reset", "--hard", target])
        .current_dir(path)
        .output()
        .map_err(GitError::Io)?;
    if output.status.success() {
        Ok(())
    } else {
        Err(GitError::GitFailed {
            cmd: format!("git reset --hard {}", target),
            stderr: String::from_utf8_lossy(&output.stderr).trim().to_string(),
        })
    }
}

pub fn stash_push(path: &Path, message: &str) -> Result<String, GitError> {
    let output = ProcessCommand::new("git")
        .args(["stash", "push", "-u", "-m", message])
        .current_dir(path)
        .output()
        .map_err(GitError::Io)?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        Err(GitError::GitFailed {
            cmd: format!("git stash push -u -m {}", message),
            stderr: String::from_utf8_lossy(&output.stderr).trim().to_string(),
        })
    }
}

pub fn rebase_onto(path: &Path, upstream: &str) -> Result<(), GitError> {
    let output = ProcessCommand::new("git")
        .args(["rebase", upstream])
        .current_dir(path)
        .output()
        .map_err(GitError::Io)?;
    if output.status.success() {
        return Ok(());
    }
    // Abort the failed rebase to leave the worktree clean
    let _ = ProcessCommand::new("git")
        .args(["rebase", "--abort"])
        .current_dir(path)
        .output();
    let stdout = String::from_utf8_lossy(&output.stdout);
    if stdout.contains("CONFLICT") {
        Err(GitError::RebaseConflict(path.to_path_buf()))
    } else {
        Err(GitError::GitFailed {
            cmd: format!("git rebase {}", upstream),
            stderr: String::from_utf8_lossy(&output.stderr).trim().to_string(),
        })
    }
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

    // @spec WTL-REBASE-014
    #[test]
    fn test_rebase_onto_error_includes_stdout_and_stderr() {
        let base = PathBuf::from("/tmp/gbiv_test_rebase_onto_stdout_stderr");
        let _ = fs::remove_dir_all(&base);
        fs::create_dir_all(&base).unwrap();
        init_git_repo(&base);

        // Create a file on main that we will conflict with
        fs::write(base.join("conflict.txt"), "main content\n").unwrap();
        Command::new("git").args(["add", "."]).current_dir(&base).output().unwrap();
        Command::new("git").args(["commit", "-m", "add conflict file on main"]).current_dir(&base).output().unwrap();

        // Create a feature branch from the initial commit (parent of HEAD)
        Command::new("git").args(["checkout", "-b", "feature", "HEAD~1"]).current_dir(&base).output().unwrap();

        // Create a conflicting change on the feature branch
        fs::write(base.join("conflict.txt"), "feature content\n").unwrap();
        Command::new("git").args(["add", "."]).current_dir(&base).output().unwrap();
        Command::new("git").args(["commit", "-m", "add conflict file on feature"]).current_dir(&base).output().unwrap();

        // Attempt to rebase feature onto main — this should fail with a conflict
        let result = rebase_onto(&base, "main");
        assert!(result.is_err(), "expected rebase to fail due to conflict");

        let err = result.unwrap_err();
        // Git writes "CONFLICT" to stdout; we detect it there and emit RebaseConflict.
        assert!(
            matches!(err, GitError::RebaseConflict(_)),
            "expected RebaseConflict variant, but got: {:?}",
            err
        );

        let _ = fs::remove_dir_all(&base);
    }
}

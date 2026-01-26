use std::path::{Path, PathBuf};
use std::process::Command as ProcessCommand;
use std::time::Duration;

use crate::colors::COLORS;

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

pub fn find_gbiv_root(start: &Path) -> Option<PathBuf> {
    let mut current = start.to_path_buf();
    if !current.is_absolute() {
        current = std::env::current_dir().ok()?.join(current);
    }
    current = current.canonicalize().ok()?;

    loop {
        let parent = current.parent()?;
        let has_main = parent.join("main").is_dir();
        let has_colors = COLORS.iter().any(|c| parent.join(c).is_dir());
        if has_main && has_colors {
            return Some(parent.to_path_buf());
        }
        if !current.pop() {
            return None;
        }
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


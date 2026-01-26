use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;

use crate::colors::{ansi_color, COLORS, DIM, GREEN, RED, RESET, YELLOW};
use crate::git_utils::{
    find_gbiv_root, get_ahead_behind_vs, get_last_commit_age,
    get_quick_status, get_remote_main_branch, is_merged_into,
};

struct WorktreeStatus {
    branch: Option<String>,
    is_dirty: bool,
    merged: Option<bool>,
    age: Option<Duration>,
    ahead_behind: Option<(u32, u32)>,
}

fn format_age(duration: Duration) -> String {
    let secs = duration.as_secs();
    if secs < 60 {
        format!("{} secs", secs)
    } else if secs < 3600 {
        format!("{} mins", secs / 60)
    } else if secs < 86400 {
        format!("{} hours", secs / 3600)
    } else {
        format!("{} days", secs / 86400)
    }
}

fn collect_worktree_status(color: &'static str, repo_path: PathBuf) -> WorktreeStatus {
    let quick = get_quick_status(&repo_path);
    let branch = quick.branch;
    let is_dirty = quick.is_dirty;

    let (merged, age, ahead_behind) = if branch.as_deref() != Some(color) {
        let remote_main = get_remote_main_branch(&repo_path);
        let merged = match (&branch, &remote_main) {
            (Some(b), Some(rm)) => Some(is_merged_into(&repo_path, b, rm)),
            _ => None,
        };
        let age = get_last_commit_age(&repo_path);
        let ahead_behind = quick.ahead_behind.or_else(|| {
            remote_main.as_ref().and_then(|rm| get_ahead_behind_vs(&repo_path, rm))
        });
        (merged, age, ahead_behind)
    } else {
        (None, None, None)
    };
    WorktreeStatus { branch, is_dirty, merged, age, ahead_behind }
}

pub fn status_command() -> Result<(), String> {
    let cwd = std::env::current_dir()
        .map_err(|e| format!("Failed to get current directory: {}", e))?;

    let gbiv_root = find_gbiv_root(&cwd)
        .ok_or_else(|| "Not in a gbiv-structured repository".to_string())?;

    let handles: Vec<_> = COLORS
        .iter()
        .map(|&color| {
            let worktree_dir = gbiv_root.join(color);
            thread::spawn(move || {
                if !worktree_dir.exists() {
                    return None;
                }
                let repo_path = find_repo_in_worktree(&worktree_dir)?;
                Some(collect_worktree_status(color, repo_path))
            })
        })
        .collect();

    let results: Vec<_> = handles
        .into_iter()
        .map(|h| h.join().unwrap_or(None))
        .collect();

    for (i, result) in results.into_iter().enumerate() {
        let color = COLORS[i];
        let color_code = ansi_color(color);

        match result {
            None => println!("{}{:<8}{}  missing", color_code, color, RESET),
            Some(status) => {
                let branch = status.branch.as_deref().unwrap_or("???");
                let is_dirty = status.is_dirty;

                if branch == color {
                    if is_dirty {
                        println!("{}{:<8}{}  {}{:<24}{} {}dirty{}", color_code, color, RESET, DIM, branch, RESET, YELLOW, RESET);
                    } else {
                        println!("{}{:<8}{}  {}{:<24} clean{}", color_code, color, RESET, DIM, branch, RESET);
                    }
                } else {
                    let dirty_str = if is_dirty {
                        format!("{}dirty{}", YELLOW, RESET)
                    } else {
                        "clean".to_string()
                    };
                    let (merged_str, merged_color) = match status.merged {
                        Some(true) => ("merged", DIM),
                        Some(false) => ("not merged", YELLOW),
                        None => ("no remote", DIM),
                    };
                    let age_str = status.age.map(format_age).unwrap_or_else(|| "???".to_string());
                    let ab_str = match status.ahead_behind {
                        Some((ahead, behind)) => {
                            let ahead_fmt = if ahead > 0 {
                                format!("{}↑{}{}", GREEN, ahead, RESET)
                            } else {
                                format!("{}↑{}{}", DIM, ahead, RESET)
                            };
                            let behind_fmt = if behind > 0 {
                                format!("{}↓{}{}", RED, behind, RESET)
                            } else {
                                format!("{}↓{}{}", DIM, behind, RESET)
                            };
                            format!("{} {}", ahead_fmt, behind_fmt)
                        }
                        None => format!("{}???{}", DIM, RESET),
                    };
                    println!(
                        "{}{:<8}{}  {:<24} {:<5}  {}{}{}  {}{}  {}{}",
                        color_code, color, RESET, branch, dirty_str, merged_color, merged_str, RESET, DIM, age_str, ab_str, RESET
                    );
                }
            }
        }
    }

    Ok(())
}

fn find_repo_in_worktree(worktree_dir: &Path) -> Option<std::path::PathBuf> {
    for entry in std::fs::read_dir(worktree_dir).ok()? {
        let entry = entry.ok()?;
        let path = entry.path();
        if path.is_dir() && path.join(".git").exists() {
            return Some(path);
        }
    }
    None
}

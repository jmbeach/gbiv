use std::process::Command as ProcessCommand;

use crate::colors::{ansi_color, COLORS, GREEN, RED, RESET};
use crate::git_utils::{find_gbiv_root, find_repo_in_worktree, get_remote_main_branch, resolve_git_dir};

pub fn rebase_all_command() -> Result<(), String> {
    let cwd = std::env::current_dir()
        .map_err(|e| format!("Failed to get current directory: {}", e))?;

    let gbiv_root = find_gbiv_root(&cwd)
        .ok_or_else(|| "Could not find gbiv root".to_string())?;

    let main_worktree_dir = gbiv_root.root.join("main");
    let main_repo = find_repo_in_worktree(&main_worktree_dir)
        .ok_or_else(|| "Could not find git repo in main worktree".to_string())?;

    let remote_main = get_remote_main_branch(&main_repo)
        .ok_or_else(|| "Could not determine remote main branch (tried origin/main, origin/master, origin/develop)".to_string())?;

    println!("Pulling main worktree ({remote_main})...");
    let pull_status = ProcessCommand::new("git")
        .arg("pull")
        .current_dir(&main_repo)
        .status()
        .map_err(|e| format!("Failed to run git pull: {}", e))?;

    if !pull_status.success() {
        return Err("git pull failed in main worktree".to_string());
    }

    let mut any_failed = false;

    for color in COLORS.iter() {
        let color_dir = gbiv_root.root.join(color);
        if !color_dir.exists() {
            println!("{}[{}]{} SKIP (no worktree)", ansi_color(color), color, RESET);
            continue;
        }

        let color_repo = match find_repo_in_worktree(&color_dir) {
            Some(repo) => repo,
            None => {
                println!("{}[{}]{} SKIP (no git repo found)", ansi_color(color), color, RESET);
                continue;
            }
        };

        let git_dir = resolve_git_dir(&color_repo).unwrap_or_else(|| color_repo.join(".git"));
        let rebase_merge = git_dir.join("rebase-merge");
        let rebase_apply = git_dir.join("rebase-apply");
        if rebase_merge.exists() || rebase_apply.exists() {
            println!("{}[{}]{} SKIP (rebase in progress)", ansi_color(color), color, RESET);
            continue;
        }

        let rebase_result = ProcessCommand::new("git")
            .args(["rebase", &remote_main])
            .current_dir(&color_repo)
            .status();

        let rebase_success = match rebase_result {
            Ok(s) => s.success(),
            Err(e) => {
                println!("{}[{}]{} {}FAILED ✗{} ({})", ansi_color(color), color, RESET, RED, RESET, e);
                any_failed = true;
                continue;
            }
        };

        if rebase_success {
            println!("{}[{}]{} {}OK ✓{}", ansi_color(color), color, RESET, GREEN, RESET);
        } else {
            println!("{}[{}]{} {}FAILED ✗{}", ansi_color(color), color, RESET, RED, RESET);
            any_failed = true;
        }
    }

    if any_failed {
        Err("One or more worktrees failed to rebase".to_string())
    } else {
        Ok(())
    }
}

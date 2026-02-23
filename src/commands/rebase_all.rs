use crate::colors::{ansi_color, COLORS, DIM, GREEN, RED, RESET, YELLOW};
use crate::git_utils::{
    ensure_gitignore_entry, fetch_remote, find_gbiv_root, find_repo_in_worktree,
    get_ahead_behind_vs, get_git_dir, get_remote_main_branch, pull, rebase_onto, resolve_git_dir,
};

const GBIV_STATE_FILES: &[&str] = &[".last-branch"];

pub fn rebase_all_command() -> Result<(), String> {
    let cwd = std::env::current_dir()
        .map_err(|e| format!("Failed to get current directory: {}", e))?;

    let gbiv_root = find_gbiv_root(&cwd)
        .ok_or_else(|| "Could not find gbiv root".to_string())?;

    let mut succeeded = 0u32;
    let mut failed = 0u32;

    // Pull the main worktree first so colour rebases are based on the latest main
    let main_worktree_dir = gbiv_root.root.join("main");
    let main_repo = find_repo_in_worktree(&main_worktree_dir)
        .ok_or_else(|| "Could not find git repo in main worktree".to_string())?;

    let remote_main = get_remote_main_branch(&main_repo)
        .ok_or_else(|| "Could not determine remote main branch (tried origin/main, origin/master, origin/develop)".to_string())?;

    match pull(&main_repo) {
        Ok(()) => println!("{}main    {}  {}pulled{}", "\x1b[0m", RESET, GREEN, RESET),
        Err(e) => {
            println!("{}main    {}  {}pull failed: {}{}", "\x1b[0m", RESET, RED, e, RESET);
            return Err("git pull failed in main worktree".to_string());
        }
    }

    for &color in &COLORS {
        let color_code = ansi_color(color);
        let worktree_dir = gbiv_root.root.join(color);

        if !worktree_dir.exists() {
            println!("{}{:<8}{}  skipped (not found)", color_code, color, RESET);
            continue;
        }

        let repo_path = match find_repo_in_worktree(&worktree_dir) {
            Some(p) => p,
            None => {
                println!("{}{:<8}{}  skipped (no repo in worktree)", color_code, color, RESET);
                continue;
            }
        };

        // Skip if a rebase is already in progress
        let git_dir_path = resolve_git_dir(&repo_path).unwrap_or_else(|| repo_path.join(".git"));
        if git_dir_path.join("rebase-merge").exists() || git_dir_path.join("rebase-apply").exists() {
            println!("{}{:<8}{}  {}skipped (rebase in progress){}", color_code, color, RESET, YELLOW, RESET);
            failed += 1;
            continue;
        }

        // Register gbiv state files in info/exclude so that tool-managed files
        // (e.g. .last-branch) are never seen as untracked and never block checkout.
        if let Some(common_git_dir) = get_git_dir(&repo_path) {
            for &state_file in GBIV_STATE_FILES {
                if let Err(e) = ensure_gitignore_entry(&common_git_dir, state_file) {
                    eprintln!("  warning: could not update info/exclude for {}: {}", color, e);
                }
            }
        }

        // Skip if already up-to-date (uses the locally cached ref — main pull already fetched)
        if let Some((_, behind)) = get_ahead_behind_vs(&repo_path, &remote_main) {
            if behind == 0 {
                println!(
                    "{}{:<8}{}  {}already up to date{}",
                    color_code, color, RESET, GREEN, RESET
                );
                succeeded += 1;
                continue;
            }
        }

        // Fetch to ensure the remote ref is current, then rebase
        if let Err(e) = fetch_remote(&repo_path) {
            println!(
                "{}{:<8}{}  {}fetch failed: {}{}",
                color_code, color, RESET, RED, e, RESET
            );
            failed += 1;
            continue;
        }

        // Print the label before rebasing so the user knows which worktree's
        // git output (conflicts etc.) they are looking at.
        println!("{}{:<8}{}  rebasing {}{}{}...", color_code, color, RESET, DIM, remote_main, RESET);

        match rebase_onto(&repo_path, &remote_main) {
            Ok(()) => {
                println!("{}{:<8}{}  {}rebased onto {}{}",
                    color_code, color, RESET, GREEN, remote_main, RESET);
                succeeded += 1;
            }
            Err(e) => {
                println!(
                    "{}{:<8}{}  {}rebase failed: {}{}",
                    color_code, color, RESET, RED, e, RESET
                );
                failed += 1;
            }
        }
    }

    println!();
    if failed == 0 {
        println!("{}{}/{} worktrees rebased successfully{}", GREEN, succeeded, succeeded + failed, RESET);
        Ok(())
    } else {
        println!(
            "{}{}/{} worktrees rebased successfully{} — {} failed",
            YELLOW, succeeded, succeeded + failed, RESET, failed
        );
        Err(format!("{} worktree(s) failed to rebase", failed))
    }
}

#[cfg(test)]
mod tests {
    use crate::git_utils::{ensure_gitignore_entry, get_git_dir, get_quick_status};
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::process::Command;

    fn setup_test_dir(name: &str) -> String {
        let test_dir = format!("/tmp/gbiv_rebase_test_{}", name);
        let _ = fs::remove_dir_all(&test_dir);
        fs::create_dir_all(&test_dir).unwrap();
        test_dir
    }

    fn cleanup(path: &str) {
        let _ = fs::remove_dir_all(path);
    }

    #[test]
    fn test_ensure_gitignore_entry_no_duplicate() {
        let dir = setup_test_dir("no_dup");
        let git_dir = PathBuf::from(&dir);
        fs::create_dir_all(git_dir.join("info")).unwrap();

        ensure_gitignore_entry(&git_dir, ".last-branch").unwrap();
        ensure_gitignore_entry(&git_dir, ".last-branch").unwrap();

        let content = fs::read_to_string(git_dir.join("info/exclude")).unwrap();
        let count = content.lines().filter(|l| l.trim() == ".last-branch").count();
        assert_eq!(count, 1, "Entry should appear exactly once, got:\n{}", content);

        cleanup(&dir);
    }

    #[test]
    fn test_ensure_gitignore_entry_creates_info_dir() {
        let dir = setup_test_dir("creates_info");
        let git_dir = PathBuf::from(&dir);

        ensure_gitignore_entry(&git_dir, ".last-branch").unwrap();

        let exclude = git_dir.join("info/exclude");
        assert!(exclude.exists(), "info/exclude should have been created");
        let content = fs::read_to_string(&exclude).unwrap();
        assert!(content.contains(".last-branch"));

        cleanup(&dir);
    }

    fn init_git_repo(path: &str) {
        Command::new("git").args(["init"]).current_dir(path).output().unwrap();
        Command::new("git").args(["config", "user.email", "t@t.com"]).current_dir(path).output().unwrap();
        Command::new("git").args(["config", "user.name", "T"]).current_dir(path).output().unwrap();
    }

    fn add_commit(path: &str) {
        fs::write(format!("{}/f.txt", path), "x").unwrap();
        Command::new("git").args(["add", "."]).current_dir(path).output().unwrap();
        Command::new("git").args(["commit", "-m", "init"]).current_dir(path).output().unwrap();
    }

    #[test]
    fn test_rebase_all_registers_last_branch_in_exclude() {
        let base = setup_test_dir("rebase_exclude");
        let repo_name = "proj";
        let main_repo = format!("{}/main/{}", base, repo_name);
        fs::create_dir_all(&main_repo).unwrap();
        init_git_repo(&main_repo);
        add_commit(&main_repo);

        let red_wt = format!("../../red/{}", repo_name);
        Command::new("git")
            .args(["worktree", "add", "-b", "red", &red_wt, "HEAD"])
            .current_dir(&main_repo)
            .output()
            .unwrap();

        let red_repo = format!("{}/red/{}", base, repo_name);
        fs::write(format!("{}/.last-branch", red_repo), "main").unwrap();

        let git_dir = get_git_dir(Path::new(&red_repo)).expect("should find git dir");
        ensure_gitignore_entry(&git_dir, ".last-branch").unwrap();

        let exclude = fs::read_to_string(git_dir.join("info/exclude")).unwrap_or_default();
        assert!(
            exclude.contains(".last-branch"),
            "info/exclude should contain .last-branch, got:\n{}", exclude
        );

        let status = get_quick_status(Path::new(&red_repo));
        assert!(
            !status.is_dirty,
            "worktree should not appear dirty after .last-branch is registered in info/exclude"
        );

        cleanup(&base);
    }
}

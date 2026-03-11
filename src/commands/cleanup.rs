use std::path::Path;

use crate::colors::COLORS;
use crate::gbiv_md::remove_gbiv_features_by_tag;
use crate::git_utils::{
    checkout_branch, find_gbiv_root, find_repo_in_worktree, get_quick_status,
    get_remote_main_branch, is_merged_into, reset_hard,
};

pub fn cleanup_one(gbiv_root: &Path, color: &str) -> Result<String, String> {
    let worktree_dir = gbiv_root.join(color);

    let repo_path = find_repo_in_worktree(&worktree_dir)
        .ok_or_else(|| format!("No git repo found in {} worktree", color))?;

    let status = get_quick_status(&repo_path);
    let branch = status
        .branch
        .ok_or_else(|| format!("Could not determine current branch for {} worktree", color))?;

    if branch == color {
        return Ok(format!("{} worktree is already on the {} branch, skipping", color, color));
    }

    let remote_main = get_remote_main_branch(&repo_path)
        .ok_or_else(|| format!("No remote configured for {} worktree", color))?;

    if !is_merged_into(&repo_path, &branch, &remote_main) {
        return Err(format!(
            "Branch {} is not merged into {} in {} worktree",
            branch, remote_main, color
        ));
    }

    checkout_branch(&repo_path, color)?;
    reset_hard(&repo_path, &remote_main)?;

    let message = format!("{} worktree cleaned up (was on {}), reset to {}", color, branch, remote_main);

    match find_repo_in_worktree(&gbiv_root.join("main")) {
        Some(main_repo) => {
            let gbiv_md_path = main_repo.join("GBIV.md");
            remove_gbiv_features_by_tag(&gbiv_md_path, color)?;
        }
        None => {
            eprintln!("Warning [{}]: could not find main repo to update GBIV.md", color);
        }
    }

    Ok(message)
}

pub fn cleanup_command(color: Option<&str>) -> Result<(), String> {
    let cwd = std::env::current_dir()
        .map_err(|e| format!("Failed to get current directory: {}", e))?;
    let gbiv_root = find_gbiv_root(&cwd)
        .ok_or_else(|| "Not in a gbiv-structured repository".to_string())?;

    if let Some(c) = color {
        let msg = cleanup_one(&gbiv_root.root, c)?;
        println!("{}", msg);
        Ok(())
    } else {
        for &c in COLORS.iter() {
            match cleanup_one(&gbiv_root.root, c) {
                Ok(msg) => println!("{}", msg),
                Err(e) => eprintln!("Warning [{}]: {}", c, e),
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command as Cmd;
    use tempfile::TempDir;

    fn git(args: &[&str], dir: &std::path::Path) {
        Cmd::new("git")
            .args(args)
            .current_dir(dir)
            .output()
            .expect("git command failed");
    }

    /// Set up a bare-bones repo whose HEAD points to `branch` (no commits needed).
    fn setup_empty_repo_on_branch(path: &std::path::Path, branch: &str) {
        std::fs::create_dir_all(path).unwrap();
        git(&["init"], path);
        git(
            &["symbolic-ref", "HEAD", &format!("refs/heads/{}", branch)],
            path,
        );
    }

    /// Set up a repo with one commit on `branch`.
    fn setup_repo_with_commit(path: &std::path::Path, branch: &str) {
        std::fs::create_dir_all(path).unwrap();
        git(&["init"], path);
        git(&["config", "user.email", "test@example.com"], path);
        git(&["config", "user.name", "Test"], path);
        std::fs::write(path.join("README.md"), "hello").unwrap();
        git(&["add", "."], path);
        git(&["commit", "-m", "initial"], path);
        git(&["branch", "-m", branch], path);
    }

    #[test]
    fn returns_ok_when_already_on_color_branch() {
        let root = TempDir::new().unwrap();
        let repo_path = root.path().join("red").join("myrepo");
        setup_empty_repo_on_branch(&repo_path, "red");

        let result = cleanup_one(root.path(), "red");
        assert!(result.is_ok(), "expected Ok but got: {:?}", result);
    }

    #[test]
    fn returns_err_when_no_remote_configured() {
        let root = TempDir::new().unwrap();
        let repo_path = root.path().join("red").join("myrepo");
        setup_repo_with_commit(&repo_path, "feature-branch");

        let result = cleanup_one(root.path(), "red");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.contains("No remote"),
            "expected 'No remote' in error: {}",
            err
        );
    }

    /// Helper: create a source repo (origin) with one commit on main, then create
    /// a worktree-style repo that has origin pointing to source, a feature branch
    /// that is already merged (same commit as origin/main), and a local-only "red"
    /// color branch. Returns (source_dir, root) so TempDirs stay alive.
    fn setup_worktree_with_merged_feature(
    ) -> (TempDir, TempDir, std::path::PathBuf) {
        // Source repo acts as "origin" — has one commit on main
        let source_dir = TempDir::new().unwrap();
        let source_path = source_dir.path().join("source");
        setup_repo_with_commit(&source_path, "main");

        // Worktree repo
        let root = TempDir::new().unwrap();
        let repo_path = root.path().join("red").join("myrepo");
        std::fs::create_dir_all(&repo_path).unwrap();
        git(&["init"], &repo_path);
        git(&["config", "user.email", "test@example.com"], &repo_path);
        git(&["config", "user.name", "Test"], &repo_path);
        git(
            &["remote", "add", "origin", source_path.to_str().unwrap()],
            &repo_path,
        );
        git(&["fetch", "origin"], &repo_path);

        // Create the local-only "red" color branch from origin/main
        git(&["checkout", "-b", "red", "origin/main"], &repo_path);

        // Create a feature branch from origin/main (already merged since same commit)
        git(&["checkout", "-b", "feature-branch", "origin/main"], &repo_path);

        // Also set up main worktree dir so GBIV.md step doesn't warn
        let main_repo = root.path().join("main").join("myrepo");
        std::fs::create_dir_all(&main_repo).unwrap();
        git(&["init"], &main_repo);

        (source_dir, root, repo_path)
    }

    #[test]
    fn cleanup_resets_color_branch_head_to_origin_main() {
        let (_source_dir, root, repo_path) = setup_worktree_with_merged_feature();

        // Confirm we're on feature-branch before cleanup
        let output = Cmd::new("git")
            .args(["rev-parse", "--abbrev-ref", "HEAD"])
            .current_dir(&repo_path)
            .output()
            .unwrap();
        let branch = String::from_utf8_lossy(&output.stdout).trim().to_string();
        assert_eq!(branch, "feature-branch");

        // Run cleanup — this should succeed but currently fails because
        // pull_remote tries `git pull origin red` and "red" doesn't exist on remote
        let result = cleanup_one(root.path(), "red");
        assert!(result.is_ok(), "expected Ok but got: {:?}", result);

        // After cleanup, HEAD should be on the "red" branch
        let output = Cmd::new("git")
            .args(["rev-parse", "--abbrev-ref", "HEAD"])
            .current_dir(&repo_path)
            .output()
            .unwrap();
        let current_branch = String::from_utf8_lossy(&output.stdout).trim().to_string();
        assert_eq!(current_branch, "red");

        // The "red" branch should be at the same commit as origin/main
        let red_rev = Cmd::new("git")
            .args(["rev-parse", "red"])
            .current_dir(&repo_path)
            .output()
            .unwrap();
        let origin_main_rev = Cmd::new("git")
            .args(["rev-parse", "origin/main"])
            .current_dir(&repo_path)
            .output()
            .unwrap();
        let red_commit = String::from_utf8_lossy(&red_rev.stdout).trim().to_string();
        let main_commit = String::from_utf8_lossy(&origin_main_rev.stdout)
            .trim()
            .to_string();
        assert_eq!(
            red_commit, main_commit,
            "red branch should be at origin/main after cleanup"
        );
    }

    #[test]
    fn cleanup_succeeds_when_color_branch_has_no_remote_tracking() {
        let (_source_dir, root, repo_path) = setup_worktree_with_merged_feature();

        // cleanup_one should succeed — the color branch is local-only
        // and doesn't exist on the remote
        let result = cleanup_one(root.path(), "red");
        assert!(result.is_ok(), "expected Ok but got: {:?}", result);

        // Verify we're on the color branch after cleanup
        let output = Cmd::new("git")
            .args(["rev-parse", "--abbrev-ref", "HEAD"])
            .current_dir(&repo_path)
            .output()
            .unwrap();
        let current_branch = String::from_utf8_lossy(&output.stdout).trim().to_string();
        assert_eq!(
            current_branch, "red",
            "worktree should be on the color branch after cleanup"
        );
    }

    #[test]
    fn cleanup_one_returns_success_message_with_previous_branch() {
        let (_source_dir, root, _repo_path) = setup_worktree_with_merged_feature();

        let result = cleanup_one(root.path(), "red");
        assert!(result.is_ok(), "expected Ok but got: {:?}", result);

        // cleanup_one should return a message describing what happened.
        // Currently it returns Ok(()) with no message; this test will fail
        // until the return type is changed to carry a descriptive message
        // (e.g., Result<String, String>).
        let message = format!("{:?}", result.unwrap());
        assert!(
            message.contains("red worktree cleaned up"),
            "expected success message containing 'red worktree cleaned up', got: {:?}",
            message
        );
        assert!(
            message.contains("feature-branch"),
            "expected success message to mention previous branch 'feature-branch', got: {:?}",
            message
        );
        assert!(
            message.contains("origin/main"),
            "expected success message to mention reset target 'origin/main', got: {:?}",
            message
        );
    }

    #[test]
    fn cleanup_one_returns_skip_message_when_on_color_branch() {
        let root = TempDir::new().unwrap();
        let repo_path = root.path().join("red").join("myrepo");
        setup_empty_repo_on_branch(&repo_path, "red");

        let result = cleanup_one(root.path(), "red");
        assert!(result.is_ok(), "expected Ok but got: {:?}", result);

        // cleanup_one should return a message indicating it skipped.
        // Currently it returns Ok(()) with no message; this test will fail
        // until the return type is changed to carry a descriptive message
        // (e.g., Result<String, String>).
        let message = format!("{:?}", result.unwrap());
        assert!(
            message.contains("red worktree is already on the red branch, skipping"),
            "expected skip message 'red worktree is already on the red branch, skipping', got: {:?}",
            message
        );
    }

    #[test]
    fn returns_err_when_feature_branch_not_merged() {
        // Create a source repo with a commit on main (serves as origin)
        let source_dir = TempDir::new().unwrap();
        let source_path = source_dir.path().join("source");
        setup_repo_with_commit(&source_path, "main");

        // Create the worktree repo with origin pointing to source
        let root = TempDir::new().unwrap();
        let repo_path = root.path().join("red").join("myrepo");
        std::fs::create_dir_all(&repo_path).unwrap();
        git(&["init"], &repo_path);
        git(&["config", "user.email", "test@example.com"], &repo_path);
        git(&["config", "user.name", "Test"], &repo_path);
        git(
            &["remote", "add", "origin", source_path.to_str().unwrap()],
            &repo_path,
        );
        git(&["fetch", "origin"], &repo_path);
        git(&["checkout", "-b", "feature-branch"], &repo_path);
        // Add a commit on feature-branch that is not in origin/main
        std::fs::write(repo_path.join("feature.txt"), "new work").unwrap();
        git(&["add", "."], &repo_path);
        git(&["commit", "-m", "feature work"], &repo_path);

        let result = cleanup_one(root.path(), "red");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.contains("not merged"),
            "expected 'not merged' in error: {}",
            err
        );
    }
}

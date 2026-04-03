use std::path::Path;
use crate::colors::COLORS;
use crate::git_utils::{
    checkout_branch, find_gbiv_root, find_repo_in_worktree, get_remote_main_branch, reset_hard,
};

pub fn reset_one(gbiv_root: &Path, color: &str) -> Result<String, String> {
    let worktree_dir = gbiv_root.join(color);

    let repo_path = find_repo_in_worktree(&worktree_dir)
        .ok_or_else(|| format!("No git repo found in {} worktree", color))?;

    let remote_main = get_remote_main_branch(&repo_path)
        .ok_or_else(|| format!("No remote configured for {} worktree", color))?;

    checkout_branch(&repo_path, color)?;
    reset_hard(&repo_path, &remote_main)?;

    Ok(format!("{} worktree reset to {}", color, remote_main))
}

pub fn reset_command(color: Option<&str>) -> Result<(), String> {
    let cwd = std::env::current_dir()
        .map_err(|e| format!("Failed to get current directory: {}", e))?;
    let gbiv_root = find_gbiv_root(&cwd)
        .ok_or_else(|| "Not in a gbiv-structured repository".to_string())?;

    let color = match color {
        Some(c) => c.to_string(),
        None => {
            let cwd_str = cwd.to_string_lossy();
            COLORS.iter()
                .find(|&&c| cwd_str.contains(&format!("/{}/", c)))
                .map(|&c| c.to_string())
                .ok_or_else(|| "Could not determine color from current directory. Run from a color worktree or specify a color.".to_string())?
        }
    };

    let msg = reset_one(&gbiv_root.root, &color)?;
    println!("{}", msg);
    Ok(())
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

    /// Set up a worktree-style repo with origin pointing to a source repo,
    /// a local color branch from origin/main, and a feature branch that has
    /// an extra commit NOT in origin/main (i.e., unmerged).
    /// Returns (source_dir, root, repo_path).
    fn setup_worktree_with_unmerged_feature() -> (TempDir, TempDir, std::path::PathBuf) {
        let source_dir = TempDir::new().unwrap();
        let source_path = source_dir.path().join("source");
        setup_repo_with_commit(&source_path, "main");

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

        // Create local "red" color branch from origin/main
        git(&["checkout", "-b", "red", "origin/main"], &repo_path);

        // Create a feature branch with an extra commit (NOT in origin/main)
        git(&["checkout", "-b", "feature-branch"], &repo_path);
        std::fs::write(repo_path.join("feature.txt"), "new work").unwrap();
        git(&["add", "."], &repo_path);
        git(&["commit", "-m", "unmerged feature work"], &repo_path);

        // Also set up main worktree so find_gbiv_root can resolve it
        let main_repo = root.path().join("main").join("myrepo");
        std::fs::create_dir_all(&main_repo).unwrap();
        git(&["init"], &main_repo);
        std::fs::write(main_repo.join("README.md"), "main").unwrap();
        git(&["config", "user.email", "test@example.com"], &main_repo);
        git(&["config", "user.name", "Test"], &main_repo);
        git(&["add", "."], &main_repo);
        git(&["commit", "-m", "init"], &main_repo);

        (source_dir, root, repo_path)
    }

    /// Set up a gbiv-structured worktree where find_gbiv_root works from inside.
    /// Uses a named "myrepo" folder so the gbiv root detection matches.
    /// Returns (source_dir, root_tmpdir, gbiv_root, repo_path).
    fn setup_worktree_for_gbiv_root() -> (TempDir, TempDir, std::path::PathBuf, std::path::PathBuf) {
        let source_dir = TempDir::new().unwrap();
        let source_path = source_dir.path().join("source");
        setup_repo_with_commit(&source_path, "main");

        let root = TempDir::new().unwrap();
        let gbiv_root = root.path().join("myrepo");
        std::fs::create_dir_all(&gbiv_root).unwrap();
        let repo_path = gbiv_root.join("red").join("myrepo");
        std::fs::create_dir_all(&repo_path).unwrap();
        git(&["init"], &repo_path);
        git(&["config", "user.email", "test@example.com"], &repo_path);
        git(&["config", "user.name", "Test"], &repo_path);
        git(
            &["remote", "add", "origin", source_path.to_str().unwrap()],
            &repo_path,
        );
        git(&["fetch", "origin"], &repo_path);

        git(&["checkout", "-b", "red", "origin/main"], &repo_path);
        git(&["checkout", "-b", "feature-branch"], &repo_path);
        std::fs::write(repo_path.join("feature.txt"), "new work").unwrap();
        git(&["add", "."], &repo_path);
        git(&["commit", "-m", "unmerged feature work"], &repo_path);

        let main_repo = gbiv_root.join("main").join("myrepo");
        std::fs::create_dir_all(&main_repo).unwrap();
        git(&["init"], &main_repo);
        std::fs::write(main_repo.join("README.md"), "main").unwrap();
        git(&["config", "user.email", "test@example.com"], &main_repo);
        git(&["config", "user.name", "Test"], &main_repo);
        git(&["add", "."], &main_repo);
        git(&["commit", "-m", "init"], &main_repo);

        (source_dir, root, gbiv_root, repo_path)
    }

    /// Set up a worktree already on the color branch (no feature branch checked out).
    /// Returns (source_dir, root, repo_path).
    fn setup_worktree_on_color_branch() -> (TempDir, TempDir, std::path::PathBuf) {
        let source_dir = TempDir::new().unwrap();
        let source_path = source_dir.path().join("source");
        setup_repo_with_commit(&source_path, "main");

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
        // Already on the color branch
        git(&["checkout", "-b", "red", "origin/main"], &repo_path);

        // Set up main worktree
        let main_repo = root.path().join("main").join("myrepo");
        std::fs::create_dir_all(&main_repo).unwrap();
        git(&["init"], &main_repo);
        std::fs::write(main_repo.join("README.md"), "main").unwrap();
        git(&["config", "user.email", "test@example.com"], &main_repo);
        git(&["config", "user.name", "Test"], &main_repo);
        git(&["add", "."], &main_repo);
        git(&["commit", "-m", "init"], &main_repo);

        (source_dir, root, repo_path)
    }

    /// Set up a worktree with GBIV.md containing an entry tagged with color "red".
    /// Returns (source_dir, root, repo_path, gbiv_md_path).
    fn setup_worktree_with_gbiv_entry() -> (TempDir, TempDir, std::path::PathBuf, std::path::PathBuf) {
        let source_dir = TempDir::new().unwrap();
        let source_path = source_dir.path().join("source");
        setup_repo_with_commit(&source_path, "main");

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
        git(&["checkout", "-b", "red", "origin/main"], &repo_path);
        git(&["checkout", "-b", "feature-branch"], &repo_path);
        std::fs::write(repo_path.join("feature.txt"), "work").unwrap();
        git(&["add", "."], &repo_path);
        git(&["commit", "-m", "feature work"], &repo_path);

        // Set up main worktree with a real git repo and GBIV.md
        let main_repo = root.path().join("main").join("myrepo");
        std::fs::create_dir_all(&main_repo).unwrap();
        git(&["init"], &main_repo);
        std::fs::write(main_repo.join("README.md"), "main").unwrap();
        git(&["config", "user.email", "test@example.com"], &main_repo);
        git(&["config", "user.name", "Test"], &main_repo);
        git(&["add", "."], &main_repo);
        git(&["commit", "-m", "init"], &main_repo);

        let gbiv_md_path = main_repo.join("GBIV.md");
        std::fs::write(&gbiv_md_path, "- [red] [in-progress] Fix critical bug\n").unwrap();

        (source_dir, root, repo_path, gbiv_md_path)
    }

    // gbi-a06u: Test reset with explicit color on unmerged feature branch
    #[test]
    fn reset_one_succeeds_on_unmerged_feature_branch() {
        let (_source_dir, root, repo_path) = setup_worktree_with_unmerged_feature();

        // Confirm we're on feature-branch with an unmerged commit
        let output = Cmd::new("git")
            .args(["rev-parse", "--abbrev-ref", "HEAD"])
            .current_dir(&repo_path)
            .output()
            .unwrap();
        let branch = String::from_utf8_lossy(&output.stdout).trim().to_string();
        assert_eq!(branch, "feature-branch");

        let result = reset_one(root.path(), "red");
        assert!(result.is_ok(), "expected Ok but got: {:?}", result);

        // After reset, HEAD should be on the "red" branch
        let output = Cmd::new("git")
            .args(["rev-parse", "--abbrev-ref", "HEAD"])
            .current_dir(&repo_path)
            .output()
            .unwrap();
        let current_branch = String::from_utf8_lossy(&output.stdout).trim().to_string();
        assert_eq!(
            current_branch, "red",
            "expected HEAD to be on 'red' branch after reset"
        );

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
            "red branch should be at origin/main after reset"
        );
    }

    // gbi-23r1: Test reset when already on color branch
    #[test]
    fn reset_one_succeeds_when_already_on_color_branch() {
        let (_source_dir, root, repo_path) = setup_worktree_on_color_branch();

        // Confirm we're already on the red branch
        let output = Cmd::new("git")
            .args(["rev-parse", "--abbrev-ref", "HEAD"])
            .current_dir(&repo_path)
            .output()
            .unwrap();
        let branch = String::from_utf8_lossy(&output.stdout).trim().to_string();
        assert_eq!(branch, "red");

        let result = reset_one(root.path(), "red");
        assert!(result.is_ok(), "expected Ok but got: {:?}", result);

        // After reset, HEAD should still be on the "red" branch
        let output = Cmd::new("git")
            .args(["rev-parse", "--abbrev-ref", "HEAD"])
            .current_dir(&repo_path)
            .output()
            .unwrap();
        let current_branch = String::from_utf8_lossy(&output.stdout).trim().to_string();
        assert_eq!(current_branch, "red");

        // The "red" branch should be at origin/main
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
            "red branch should be at origin/main after reset"
        );
    }

    // gbi-ujdr: Test reset with inferred color from current directory
    #[test]
    #[serial_test::serial]
    fn reset_command_infers_color_from_current_directory() {
        let (_source_dir, _root, _gbiv_root, repo_path) = setup_worktree_for_gbiv_root();

        // Change cwd into the color worktree so reset_command can infer "red"
        std::env::set_current_dir(&repo_path).unwrap();

        let result = reset_command(None);
        assert!(result.is_ok(), "expected Ok but got: {:?}", result);

        // After reset, HEAD should be on the "red" branch
        let output = Cmd::new("git")
            .args(["rev-parse", "--abbrev-ref", "HEAD"])
            .current_dir(&repo_path)
            .output()
            .unwrap();
        let current_branch = String::from_utf8_lossy(&output.stdout).trim().to_string();
        assert_eq!(
            current_branch, "red",
            "expected HEAD to be on 'red' branch after inferred reset"
        );
    }

    // gbi-werx: Test reset errors when no remote main branch found
    #[test]
    fn reset_one_errors_when_no_remote_main_branch() {
        // Set up a repo with no remote configured
        let root = TempDir::new().unwrap();
        let repo_path = root.path().join("red").join("myrepo");
        setup_repo_with_commit(&repo_path, "feature-branch");

        let result = reset_one(root.path(), "red");
        assert!(result.is_err(), "expected Err but got: {:?}", result);
        let err = result.unwrap_err();
        assert!(
            err.contains("remote") || err.contains("No remote") || err.contains("not found"),
            "expected error about remote/default branch not found, got: {}",
            err
        );
    }

    // gbi-w6v8: Test reset does not modify GBIV.md
    #[test]
    fn reset_one_does_not_modify_gbiv_md() {
        let (_source_dir, root, _repo_path, gbiv_md_path) = setup_worktree_with_gbiv_entry();

        let content_before = std::fs::read_to_string(&gbiv_md_path).unwrap();

        let result = reset_one(root.path(), "red");
        assert!(result.is_ok(), "expected Ok but got: {:?}", result);

        let content_after = std::fs::read_to_string(&gbiv_md_path).unwrap();
        assert_eq!(
            content_before, content_after,
            "GBIV.md should be unchanged after reset"
        );
    }

    // gbi-jd1h: Test reset errors from non-color directory without argument
    #[test]
    #[serial_test::serial]
    fn reset_command_errors_from_non_color_directory() {
        let (_source_dir, _root, gbiv_root, _repo_path) = setup_worktree_for_gbiv_root();

        // Change cwd to the main worktree (not a color directory)
        let main_repo = gbiv_root.join("main").join("myrepo");
        std::env::set_current_dir(&main_repo).unwrap();

        let result = reset_command(None);
        assert!(result.is_err(), "expected Err but got: {:?}", result);
        let err = result.unwrap_err();
        assert!(
            err.contains("color") || err.contains("determine") || err.contains("not in"),
            "expected error about not being able to determine color, got: {}",
            err
        );
    }
}

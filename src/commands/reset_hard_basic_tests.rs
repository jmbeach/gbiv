use crate::commands::reset::reset_one;
use std::process::Command as Cmd;
use tempfile::TempDir;

fn git(args: &[&str], dir: &std::path::Path) {
    Cmd::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .expect("git command failed");
}

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

fn setup_empty_repo_on_branch(path: &std::path::Path, branch: &str) {
    std::fs::create_dir_all(path).unwrap();
    git(&["init"], path);
    git(
        &["symbolic-ref", "HEAD", &format!("refs/heads/{}", branch)],
        path,
    );
}

/// Set up a worktree repo with an origin, a "red" color branch, and a feature
/// branch that has an extra commit NOT present in origin/main (unmerged).
/// Returns (source_dir, root, repo_path) so TempDirs stay alive.
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

    // Create the "red" color branch from origin/main
    git(&["checkout", "-b", "red", "origin/main"], &repo_path);

    // Create a feature branch with an extra commit — NOT merged into origin/main
    git(&["checkout", "-b", "feature-unmerged", "origin/main"], &repo_path);
    std::fs::write(repo_path.join("feature.txt"), "unmerged work").unwrap();
    git(&["add", "."], &repo_path);
    git(&["commit", "-m", "unmerged feature work"], &repo_path);

    // Set up main worktree dir so GBIV.md step doesn't warn
    let main_repo = root.path().join("main").join("myrepo");
    std::fs::create_dir_all(&main_repo).unwrap();
    git(&["init"], &main_repo);

    (source_dir, root, repo_path)
}

/// Set up a worktree repo with an origin, a "red" color branch, and a feature
/// branch that is already merged (same commit as origin/main).
/// Returns (source_dir, root, repo_path) so TempDirs stay alive.
fn setup_worktree_with_merged_feature() -> (TempDir, TempDir, std::path::PathBuf) {
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

    // Create the "red" color branch from origin/main
    git(&["checkout", "-b", "red", "origin/main"], &repo_path);

    // Create a feature branch at the same commit as origin/main (already merged)
    git(&["checkout", "-b", "feature-merged", "origin/main"], &repo_path);

    // Set up main worktree dir so GBIV.md step doesn't warn
    let main_repo = root.path().join("main").join("myrepo");
    std::fs::create_dir_all(&main_repo).unwrap();
    git(&["init"], &main_repo);

    (source_dir, root, repo_path)
}

// @spec HRESET-001
// Hard reset should succeed even when the current branch has commits not merged
// into origin/main — the merge check is skipped when hard=true.
#[test]
fn hard_reset_one_succeeds_with_unmerged_branch() {
    let (_source_dir, root, repo_path) = setup_worktree_with_unmerged_feature();

    // Confirm we're on the unmerged feature branch before calling reset
    let output = Cmd::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .current_dir(&repo_path)
        .output()
        .unwrap();
    let branch = String::from_utf8_lossy(&output.stdout).trim().to_string();
    assert_eq!(branch, "feature-unmerged");

    // reset_one with hard=true should bypass the merge check
    let result = reset_one(root.path(), "red", true);
    assert!(
        result.is_ok(),
        "--hard reset should succeed even when branch is not merged into origin/main, got: {:?}",
        result
    );

    // After hard reset, HEAD should be on the "red" branch
    let output = Cmd::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .current_dir(&repo_path)
        .output()
        .unwrap();
    let current_branch = String::from_utf8_lossy(&output.stdout).trim().to_string();
    assert_eq!(
        current_branch, "red",
        "--hard reset should check out the color branch, got: {}",
        current_branch
    );
}

// @spec HRESET-001
// Hard reset should succeed and reset to origin/main when the current branch is
// already merged — same happy-path as a normal reset but invoked with hard=true.
#[test]
fn hard_reset_one_succeeds_with_merged_branch() {
    let (_source_dir, root, repo_path) = setup_worktree_with_merged_feature();

    // Confirm we're on the merged feature branch before calling reset
    let output = Cmd::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .current_dir(&repo_path)
        .output()
        .unwrap();
    let branch = String::from_utf8_lossy(&output.stdout).trim().to_string();
    assert_eq!(branch, "feature-merged");

    let result = reset_one(root.path(), "red", true);
    assert!(
        result.is_ok(),
        "--hard reset should succeed for a merged branch, got: {:?}",
        result
    );

    // After hard reset, HEAD should be on the "red" branch
    let output = Cmd::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .current_dir(&repo_path)
        .output()
        .unwrap();
    let current_branch = String::from_utf8_lossy(&output.stdout).trim().to_string();
    assert_eq!(
        current_branch, "red",
        "--hard reset should check out the color branch, got: {}",
        current_branch
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
        "red branch should be at origin/main after --hard reset"
    );
}

// @spec HRESET-001
// Hard reset should proceed with checkout and reset even when the worktree is
// already on the color branch — the early-return skip is bypassed when hard=true.
#[test]
fn hard_reset_one_proceeds_when_already_on_color_branch() {
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

    let main_repo = root.path().join("main").join("myrepo");
    std::fs::create_dir_all(&main_repo).unwrap();
    git(&["init"], &main_repo);

    // Confirm we are already on the color branch
    let output = Cmd::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .current_dir(&repo_path)
        .output()
        .unwrap();
    let branch = String::from_utf8_lossy(&output.stdout).trim().to_string();
    assert_eq!(branch, "red");

    let result = reset_one(root.path(), "red", true);
    assert!(
        result.is_ok(),
        "--hard reset should succeed (not skip) when already on color branch, got: {:?}",
        result
    );

    let message = result.unwrap();
    assert!(
        !message.contains("skipping"),
        "--hard reset should NOT return an 'already on ... skipping' message, got: {:?}",
        message
    );
}

// @spec HRESET-001
// Regression: calling reset_one with hard=false should behave exactly like the
// current two-argument form.
#[test]
fn default_reset_behavior_unchanged_when_already_on_color_branch() {
    let root = TempDir::new().unwrap();
    let repo_path = root.path().join("red").join("myrepo");
    setup_empty_repo_on_branch(&repo_path, "red");

    let result = reset_one(root.path(), "red", false);
    assert!(
        result.is_ok(),
        "default (hard=false) reset should return Ok when already on color branch, got: {:?}",
        result
    );

    let message = result.unwrap();
    assert!(
        message.contains("already on the red branch"),
        "default (hard=false) reset should still return 'already on the red branch' skip message, got: {:?}",
        message
    );
}

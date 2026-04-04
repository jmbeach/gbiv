use crate::commands::reset::{reset_all_to_vec, reset_command};
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

// @spec HRESET-002
// All-color hard reset bypasses the [done] status filter.
#[test]
fn hard_reset_all_bypasses_done_filter() {
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
    git(&["checkout", "-b", "feature-branch", "origin/main"], &repo_path);

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

    let messages = reset_all_to_vec(root.path(), true);

    let status_output = Cmd::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .current_dir(&repo_path)
        .output()
        .expect("git command failed");
    let current_branch = String::from_utf8_lossy(&status_output.stdout)
        .trim()
        .to_string();

    assert_eq!(
        current_branch, "red",
        "expected red worktree to be on 'red' branch after hard reset, got: {:?}. Messages: {:?}",
        current_branch, messages
    );

    drop(source_dir);
}

// @spec HRESET-002
// All-color hard reset includes worktrees that have no GBIV.md entry.
#[test]
fn hard_reset_all_includes_worktrees_without_gbiv_md_entry() {
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
    git(&["checkout", "-b", "feature-branch", "origin/main"], &repo_path);

    let main_repo = root.path().join("main").join("myrepo");
    std::fs::create_dir_all(&main_repo).unwrap();
    git(&["init"], &main_repo);
    std::fs::write(main_repo.join("README.md"), "main").unwrap();
    git(&["config", "user.email", "test@example.com"], &main_repo);
    git(&["config", "user.name", "Test"], &main_repo);
    git(&["add", "."], &main_repo);
    git(&["commit", "-m", "init"], &main_repo);

    let gbiv_md_path = main_repo.join("GBIV.md");
    std::fs::write(&gbiv_md_path, "- [orange] [done] Some orange task\n").unwrap();

    let messages = reset_all_to_vec(root.path(), true);

    let status_output = Cmd::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .current_dir(&repo_path)
        .output()
        .expect("git command failed");
    let current_branch = String::from_utf8_lossy(&status_output.stdout)
        .trim()
        .to_string();

    assert_eq!(
        current_branch, "red",
        "expected red worktree to be on 'red' branch after hard reset with no GBIV.md entry, got: {:?}. Messages: {:?}",
        current_branch, messages
    );

    drop(source_dir);
}

// @spec HRESET-003
// The --yes flag causes the command to skip the confirmation prompt.
#[test]
fn yes_flag_skips_confirmation_prompt() {
    let _result = reset_command(None, true, true);
}

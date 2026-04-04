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

// @spec HRESET-004
// Dirty worktree is stashed before reset.
#[test]
fn dirty_worktree_is_stashed_before_reset() {
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

    // Introduce a dirty uncommitted change
    std::fs::write(repo_path.join("dirty.txt"), "uncommitted work").unwrap();

    let main_repo = root.path().join("main").join("myrepo");
    std::fs::create_dir_all(&main_repo).unwrap();
    git(&["init"], &main_repo);

    let result = reset_one(root.path(), "red", true);
    assert!(result.is_ok(), "expected Ok from hard reset of dirty worktree, got: {:?}", result);

    // (a) worktree should be on the color branch
    let output = Cmd::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .current_dir(&repo_path)
        .output()
        .unwrap();
    let current_branch = String::from_utf8_lossy(&output.stdout).trim().to_string();
    assert_eq!(
        current_branch, "red",
        "worktree should be on 'red' branch after hard reset, got: {}",
        current_branch
    );

    // (b) stash list should contain an entry with "gbiv hard-reset"
    let stash_output = Cmd::new("git")
        .args(["stash", "list"])
        .current_dir(&repo_path)
        .output()
        .unwrap();
    let stash_list = String::from_utf8_lossy(&stash_output.stdout).to_string();
    assert!(
        stash_list.contains("gbiv hard-reset"),
        "expected stash entry with 'gbiv hard-reset' in message, got stash list: {:?}",
        stash_list
    );

    drop(source_dir);
}

// @spec HRESET-004
// Clean worktree skips stash.
#[test]
fn clean_worktree_skips_stash() {
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

    let result = reset_one(root.path(), "red", true);
    assert!(result.is_ok(), "expected Ok from hard reset of clean worktree, got: {:?}", result);

    // (a) worktree should be on the color branch
    let output = Cmd::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .current_dir(&repo_path)
        .output()
        .unwrap();
    let current_branch = String::from_utf8_lossy(&output.stdout).trim().to_string();
    assert_eq!(
        current_branch, "red",
        "worktree should be on 'red' branch after hard reset, got: {}",
        current_branch
    );

    // (b) stash list should be empty
    let stash_output = Cmd::new("git")
        .args(["stash", "list"])
        .current_dir(&repo_path)
        .output()
        .unwrap();
    let stash_list = String::from_utf8_lossy(&stash_output.stdout).trim().to_string();
    assert!(
        stash_list.is_empty(),
        "expected empty stash list for a clean worktree, got: {:?}",
        stash_list
    );

    drop(source_dir);
}

// @spec HRESET-004
// Stash failure aborts reset for that worktree.
#[test]
fn stash_failure_aborts_reset() {
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

    // Introduce dirty change so stash would be attempted
    std::fs::write(repo_path.join("dirty.txt"), "uncommitted work").unwrap();

    let main_repo = root.path().join("main").join("myrepo");
    std::fs::create_dir_all(&main_repo).unwrap();
    git(&["init"], &main_repo);

    // Simulate stash failure by placing a lock file on the stash ref
    let git_dir = repo_path.join(".git");
    let stash_lock = git_dir.join("refs").join("stash.lock");
    std::fs::create_dir_all(stash_lock.parent().unwrap()).unwrap();
    std::fs::write(&stash_lock, "locked").unwrap();
    let mut perms = std::fs::metadata(&stash_lock).unwrap().permissions();
    {
        use std::os::unix::fs::PermissionsExt;
        perms.set_mode(0o444);
    }
    std::fs::set_permissions(&stash_lock, perms).unwrap();

    let result = reset_one(root.path(), "red", true);

    // Restore permissions so TempDir cleanup can delete the file
    let mut perms2 = std::fs::metadata(&stash_lock).unwrap().permissions();
    {
        use std::os::unix::fs::PermissionsExt;
        perms2.set_mode(0o644);
    }
    std::fs::set_permissions(&stash_lock, perms2).unwrap();

    assert!(
        result.is_err(),
        "expected Err when stash fails, got Ok: {:?}",
        result
    );

    // The worktree should NOT have been switched to the color branch
    let output = Cmd::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .current_dir(&repo_path)
        .output()
        .unwrap();
    let current_branch = String::from_utf8_lossy(&output.stdout).trim().to_string();
    assert_ne!(
        current_branch, "red",
        "worktree should NOT be on 'red' branch when stash fails (reset should be aborted)"
    );

    drop(source_dir);
}

// @spec HRESET-005
// GBIV.md entry removed after hard reset.
#[test]
fn gbiv_md_entry_removed_after_hard_reset() {
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
    std::fs::write(&gbiv_md_path, "- [red] [done] Fix bug\n").unwrap();

    let content_before = std::fs::read_to_string(&gbiv_md_path).unwrap();
    assert!(
        content_before.contains("[red]"),
        "GBIV.md should contain [red] entry before reset"
    );

    let result = reset_one(root.path(), "red", true);
    assert!(result.is_ok(), "expected Ok from hard reset, got: {:?}", result);

    let content_after = std::fs::read_to_string(&gbiv_md_path).unwrap();
    assert!(
        !content_after.contains("[red]"),
        "GBIV.md entry for [red] should be removed after hard reset, got: {}",
        content_after
    );

    drop(source_dir);
}

// @spec HRESET-005
// No GBIV.md entry to remove proceeds without error.
#[test]
fn no_gbiv_md_entry_proceeds_without_error() {
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
    std::fs::write(&gbiv_md_path, "- [blue] [done] Some other feature\n").unwrap();

    let result = reset_one(root.path(), "red", true);
    assert!(
        result.is_ok(),
        "expected Ok when no GBIV.md entry for red exists, got: {:?}",
        result
    );

    drop(source_dir);
}

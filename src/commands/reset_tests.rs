use crate::commands::reset::{reset_all_to_vec, reset_command, reset_one};
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

/// Helper: create a source repo (origin) with one commit on main, then create
/// a worktree-style repo that has origin pointing to source, a feature branch
/// that is already merged (same commit as origin/main), and a local-only "red"
/// color branch. Returns (source_dir, root, repo_path) so TempDirs stay alive.
fn setup_worktree_with_merged_feature() -> (TempDir, TempDir, std::path::PathBuf) {
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

// @spec WTL-RESET-001
#[test]
fn reset_one_exists_and_returns_ok_when_already_on_color_branch() {
    let root = TempDir::new().unwrap();
    let repo_path = root.path().join("red").join("myrepo");
    setup_empty_repo_on_branch(&repo_path, "red");

    let result = reset_one(root.path(), "red", false);
    assert!(result.is_ok(), "expected Ok but got: {:?}", result);

    let message = result.unwrap();
    assert!(
        message.contains("already on the red branch"),
        "expected message containing 'already on the red branch', got: {:?}",
        message
    );
}

// @spec WTL-RESET-004
#[test]
fn reset_one_success_message_contains_reset_not_cleaned_up() {
    let (_source_dir, root, _repo_path) = setup_worktree_with_merged_feature();

    let result = reset_one(root.path(), "red", false);
    assert!(result.is_ok(), "expected Ok but got: {:?}", result);

    let message = result.unwrap();
    assert!(
        message.contains("reset"),
        "expected success message to contain 'reset', got: {:?}",
        message
    );
    assert!(
        !message.contains("cleaned up"),
        "expected success message NOT to contain 'cleaned up', got: {:?}",
        message
    );
}

// @spec WTL-RESET-011, WTL-RESET-012
#[test]
fn reset_all_to_vec_exists_and_processes_done_entries() {
    // Source repo acts as "origin"
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

    // Set up main worktree with a real git repo and a GBIV.md with [done] entry
    let main_repo = root.path().join("main").join("myrepo");
    std::fs::create_dir_all(&main_repo).unwrap();
    git(&["init"], &main_repo);
    std::fs::write(main_repo.join("README.md"), "main").unwrap();
    git(&["config", "user.email", "test@example.com"], &main_repo);
    git(&["config", "user.name", "Test"], &main_repo);
    git(&["add", "."], &main_repo);
    git(&["commit", "-m", "init"], &main_repo);

    let gbiv_md_path = main_repo.join("GBIV.md");
    std::fs::write(&gbiv_md_path, "- [red] [done] Fix critical bug\n").unwrap();

    let messages = reset_all_to_vec(root.path(), false);

    // Should return a non-empty Vec (at minimum a summary line)
    assert!(
        !messages.is_empty(),
        "reset_all_to_vec should return at least a summary line, got empty vec"
    );

    // Drop source_dir explicitly so it stays alive through the test
    drop(source_dir);
}

// @spec CLI-DISPATCH-002
#[test]
fn reset_command_exists_and_is_callable() {
    // Verify reset_command compiles and is callable. The result depends on
    // whether CWD is a gbiv repo, so we just check it doesn't panic.
    let _result = reset_command(None, false, false);
    // If we reach here, the function exists and is callable — test passes.
}

// @spec CLI-DISPATCH-004
#[test]
fn cli_registers_reset_subcommand() {
    // Verify the CLI definition includes a "reset" subcommand by building the
    // clap Command and checking it accepts "reset --help" without error.
    let cli = crate::cli();
    let result = cli.try_get_matches_from(["gbiv", "reset", "--help"]);
    // clap returns Err with DisplayHelp kind for --help, which is success
    match result {
        Ok(_) => {} // matches parsed fine
        Err(e) => {
            assert_eq!(
                e.kind(),
                clap::error::ErrorKind::DisplayHelp,
                "expected DisplayHelp for 'reset --help', got: {:?}",
                e.kind()
            );
        }
    }
}

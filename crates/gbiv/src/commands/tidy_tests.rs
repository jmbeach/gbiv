// @spec CLI-DISPATCH-004
#[test]
fn cli_registers_tidy_subcommand() {
    let m = crate::cli().try_get_matches_from(["gbiv", "tidy"]);
    assert!(m.is_ok(), "tidy subcommand should be registered");
}

use std::path::{Path, PathBuf};
use std::process::Command as Cmd;

fn git(args: &[&str], dir: &Path) -> String {
    let out = Cmd::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .expect("spawn git");
    assert!(
        out.status.success(),
        "git {args:?} in {} failed: {}",
        dir.display(),
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

/// A gbiv workspace whose `red` worktree sits on a feature branch that is
/// already merged into `origin/main`, with a `[done]` GBIV.md entry for red —
/// exactly the state the old tidy "reset merged branches" step acted on.
/// Returns (tmp, root, main_repo, red_repo).
fn workspace_with_merged_done_red() -> (tempfile::TempDir, PathBuf, PathBuf, PathBuf) {
    let tmp = tempfile::TempDir::new().unwrap();
    // A unique project name so `tmux clean` (if tmux is installed) finds no session.
    let project = "gbivtidynoresetproj";

    let seed = tmp.path().join("seed");
    std::fs::create_dir_all(&seed).unwrap();
    git(&["init", "-q", "-b", "main"], &seed);
    git(&["config", "user.email", "t@t.com"], &seed);
    git(&["config", "user.name", "T"], &seed);
    std::fs::write(seed.join("README.md"), "hello\n").unwrap();
    git(&["add", "."], &seed);
    git(&["commit", "-q", "-m", "init"], &seed);
    let origin = tmp.path().join("origin.git");
    git(
        &[
            "clone",
            "-q",
            "--bare",
            seed.to_str().unwrap(),
            origin.to_str().unwrap(),
        ],
        tmp.path(),
    );

    let root = tmp.path().join(project);
    std::fs::create_dir_all(root.join("main")).unwrap();
    let main_repo = root.join("main").join(project);
    git(
        &[
            "clone",
            "-q",
            origin.to_str().unwrap(),
            main_repo.to_str().unwrap(),
        ],
        tmp.path(),
    );
    git(&["config", "user.email", "t@t.com"], &main_repo);
    git(&["config", "user.name", "T"], &main_repo);

    git(&["branch", "red", "origin/main"], &main_repo);
    std::fs::create_dir_all(root.join("red")).unwrap();
    let red_repo = root.join("red").join(project);
    git(
        &[
            "worktree",
            "add",
            "-q",
            "-b",
            "feature-x",
            red_repo.to_str().unwrap(),
            "origin/main",
        ],
        &main_repo,
    );

    std::fs::write(main_repo.join("GBIV.md"), "- [red] [done] Feature X\n").unwrap();
    (tmp, root, main_repo, red_repo)
}

// @spec WTL-TIDY-003
#[test]
#[serial_test::serial]
fn tidy_runs_no_reset_step() {
    let (_tmp, root, main_repo, red_repo) = workspace_with_merged_done_red();

    let original_dir = std::env::current_dir().unwrap_or_else(|_| std::env::temp_dir());
    std::env::set_current_dir(&root).unwrap();
    // tidy's own result depends on the tmux step; only its side effects matter here.
    let _ = crate::commands::tidy::tidy_command();
    let _ = std::env::set_current_dir(&original_dir);

    assert_eq!(
        git(&["rev-parse", "--abbrev-ref", "HEAD"], &red_repo),
        "feature-x",
        "tidy must not check the merged worktree back out to its color branch"
    );
    let gbiv_md = std::fs::read_to_string(main_repo.join("GBIV.md")).unwrap();
    assert!(
        gbiv_md.contains("[red] [done] Feature X"),
        "tidy must not remove GBIV.md entries; file now:\n{gbiv_md}"
    );
}

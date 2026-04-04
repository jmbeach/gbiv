use std::path::Path;
use crate::colors::COLORS;
use crate::gbiv_md::remove_gbiv_features_by_tag;
use crate::git_utils::{
    checkout_branch, find_gbiv_root, find_repo_in_worktree, get_quick_status,
    get_remote_main_branch, is_merged_into, reset_hard,
};

pub fn reset_one(gbiv_root: &Path, color: &str) -> Result<String, String> {
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

    let message = format!("{} worktree reset (was on {}), reset to {}", color, branch, remote_main);

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

/// Returns all output lines (including a summary) produced by all-color reset.
pub fn reset_all_to_vec(gbiv_root: &std::path::Path) -> Vec<String> {
    use crate::gbiv_md::parse_gbiv_md;

    let mut messages: Vec<String> = vec![];
    let mut cleaned = 0u32;
    let mut without_done = 0u32;
    let mut not_merged = 0u32;
    let mut already_clean = 0u32;
    let mut missing_worktree = 0u32;
    let mut other_errors = 0u32;

    // Parse GBIV.md to get feature statuses
    let features = find_repo_in_worktree(&gbiv_root.join("main"))
        .map(|p| parse_gbiv_md(&p.join("GBIV.md")))
        .unwrap_or_default();

    for &color in COLORS.iter() {
        // Check if the worktree directory exists
        let worktree_dir = gbiv_root.join(color);
        if !worktree_dir.exists() {
            missing_worktree += 1;
            messages.push(format!("Warning [{}]: worktree directory missing", color));
            continue;
        }

        // Find the feature entry for this color
        let feature = features.iter().find(|f| f.tag.as_deref() == Some(color));

        // If no GBIV.md entry, skip silently
        let feature = match feature {
            Some(f) => f,
            None => continue,
        };

        // Check status: only process [done] entries
        match feature.status.as_deref() {
            Some("done") => {
                // Proceed with reset
            }
            Some(_status) => {
                without_done += 1;
                messages.push(format!("Skipping [{}]: without [done] status", color));
                continue;
            }
            None => {
                without_done += 1;
                messages.push(format!("Skipping [{}]: without [done] status", color));
                continue;
            }
        }

        match reset_one(gbiv_root, color) {
            Ok(msg) => {
                if msg.contains("already on the") && msg.contains("skipping") {
                    already_clean += 1;
                    messages.push(msg);
                } else {
                    cleaned += 1;
                    messages.push(msg);
                }
            }
            Err(e) => {
                if e.contains("not merged") {
                    not_merged += 1;
                    messages.push(format!("Warning [{}]: {}", color, e));
                } else {
                    other_errors += 1;
                    messages.push(format!("Warning [{}]: {}", color, e));
                }
            }
        }
    }

    // Build summary
    let mut skip_parts: Vec<String> = vec![];
    if not_merged > 0 {
        skip_parts.push(format!("{} not merged", not_merged));
    }
    if without_done > 0 {
        skip_parts.push(format!("{} without [done] status", without_done));
    }
    if already_clean > 0 {
        skip_parts.push(format!("{} already reset", already_clean));
    }
    if missing_worktree > 0 {
        skip_parts.push(format!("{} missing worktree", missing_worktree));
    }
    if other_errors > 0 {
        skip_parts.push(format!("{} errors", other_errors));
    }

    let summary = if skip_parts.is_empty() {
        format!("{} reset", cleaned)
    } else {
        format!("{} reset ({})", cleaned, skip_parts.join(", "))
    };
    messages.push(summary);

    messages
}

pub fn reset_command(color: Option<&str>) -> Result<(), String> {
    let cwd = std::env::current_dir()
        .map_err(|e| format!("Failed to get current directory: {}", e))?;
    let gbiv_root = find_gbiv_root(&cwd)
        .ok_or_else(|| "Not in a gbiv-structured repository".to_string())?;

    if let Some(c) = color {
        let msg = reset_one(&gbiv_root.root, c)?;
        println!("{}", msg);
        Ok(())
    } else {
        let messages = reset_all_to_vec(&gbiv_root.root);
        for msg in &messages {
            println!("{}", msg);
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

        let result = reset_one(root.path(), "red");
        assert!(result.is_ok(), "expected Ok but got: {:?}", result);
    }

    #[test]
    fn returns_err_when_no_remote_configured() {
        let root = TempDir::new().unwrap();
        let repo_path = root.path().join("red").join("myrepo");
        setup_repo_with_commit(&repo_path, "feature-branch");

        let result = reset_one(root.path(), "red");
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
    fn reset_resets_color_branch_head_to_origin_main() {
        let (_source_dir, root, repo_path) = setup_worktree_with_merged_feature();

        // Confirm we're on feature-branch before reset
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
            "red branch should be at origin/main after reset"
        );
    }

    #[test]
    fn reset_succeeds_when_color_branch_has_no_remote_tracking() {
        let (_source_dir, root, repo_path) = setup_worktree_with_merged_feature();

        let result = reset_one(root.path(), "red");
        assert!(result.is_ok(), "expected Ok but got: {:?}", result);

        // Verify we're on the color branch after reset
        let output = Cmd::new("git")
            .args(["rev-parse", "--abbrev-ref", "HEAD"])
            .current_dir(&repo_path)
            .output()
            .unwrap();
        let current_branch = String::from_utf8_lossy(&output.stdout).trim().to_string();
        assert_eq!(
            current_branch, "red",
            "worktree should be on the color branch after reset"
        );
    }

    #[test]
    fn reset_one_returns_success_message_with_previous_branch() {
        let (_source_dir, root, _repo_path) = setup_worktree_with_merged_feature();

        let result = reset_one(root.path(), "red");
        assert!(result.is_ok(), "expected Ok but got: {:?}", result);

        let message = result.unwrap();
        assert!(
            message.contains("red worktree reset"),
            "expected success message containing 'red worktree reset', got: {:?}",
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
    fn reset_one_returns_skip_message_when_on_color_branch() {
        let root = TempDir::new().unwrap();
        let repo_path = root.path().join("red").join("myrepo");
        setup_empty_repo_on_branch(&repo_path, "red");

        let result = reset_one(root.path(), "red");
        assert!(result.is_ok(), "expected Ok but got: {:?}", result);

        let message = result.unwrap();
        assert!(
            message.contains("red worktree is already on the red branch, skipping"),
            "expected skip message 'red worktree is already on the red branch, skipping', got: {:?}",
            message
        );
    }

    /// Helper: set up a worktree with a GBIV.md entry that has a given status tag.
    /// Returns (source_dir, root, repo_path, main_repo_path, gbiv_md_path).
    fn setup_worktree_with_gbiv_entry(
        status_tag: Option<&str>,
    ) -> (TempDir, TempDir, std::path::PathBuf, std::path::PathBuf, std::path::PathBuf) {
        // Source repo acts as "origin" with one commit on main
        let source_dir = TempDir::new().unwrap();
        let source_path = source_dir.path().join("source");
        setup_repo_with_commit(&source_path, "main");

        // Gbiv root
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

        // Set up main worktree with a real git repo so GBIV.md can be written
        let main_repo = root.path().join("main").join("myrepo");
        std::fs::create_dir_all(&main_repo).unwrap();
        git(&["init"], &main_repo);
        std::fs::write(main_repo.join("README.md"), "main").unwrap();
        git(&["config", "user.email", "test@example.com"], &main_repo);
        git(&["config", "user.name", "Test"], &main_repo);
        git(&["add", "."], &main_repo);
        git(&["commit", "-m", "init"], &main_repo);

        // Write GBIV.md with a status-tagged entry for red
        let gbiv_md_path = main_repo.join("GBIV.md");
        let entry = match status_tag {
            Some(tag) => format!("- [red] [{}] Fix critical bug\n", tag),
            None => "- [red] Fix critical bug\n".to_string(),
        };
        std::fs::write(&gbiv_md_path, &entry).unwrap();

        (source_dir, root, repo_path, main_repo, gbiv_md_path)
    }

    // all-color reset skips entries without [done] status
    #[test]
    fn all_color_reset_skips_entries_without_done_status() {
        let (_source_dir, root, repo_path, _main_repo, gbiv_md_path) =
            setup_worktree_with_gbiv_entry(Some("in-progress"));

        let content_before = std::fs::read_to_string(&gbiv_md_path).unwrap();
        assert!(
            content_before.contains("[in-progress]"),
            "setup should produce an [in-progress] entry"
        );

        let messages = reset_all_to_vec(root.path());

        // The worktree should NOT have been reset because it has [in-progress] status
        let output = Cmd::new("git")
            .args(["rev-parse", "--abbrev-ref", "HEAD"])
            .current_dir(&repo_path)
            .output()
            .unwrap();
        let current_branch = String::from_utf8_lossy(&output.stdout).trim().to_string();
        assert_eq!(
            current_branch, "feature-branch",
            "worktree with [in-progress] status should NOT be reset by all-color reset (was reset)"
        );

        // Should have a skip message mentioning status
        let has_skip = messages.iter().any(|msg| msg.contains("without [done] status"));
        assert!(
            has_skip,
            "expected skip message about missing [done] status, got: {:?}",
            messages
        );
    }

    // all-color reset processes [done] entries
    #[test]
    fn all_color_reset_processes_done_entries() {
        let (_source_dir, root, repo_path, _main_repo, gbiv_md_path) =
            setup_worktree_with_gbiv_entry(Some("done"));

        let content_before = std::fs::read_to_string(&gbiv_md_path).unwrap();
        assert!(
            content_before.contains("[done]"),
            "setup should produce a [done] entry, got: {}",
            content_before
        );

        let result = reset_one(root.path(), "red");
        assert!(
            result.is_ok(),
            "all-color reset should succeed for a [done] merged branch, got: {:?}",
            result
        );

        // After reset, the repo should be on the color branch
        let output = Cmd::new("git")
            .args(["rev-parse", "--abbrev-ref", "HEAD"])
            .current_dir(&repo_path)
            .output()
            .unwrap();
        let current_branch = String::from_utf8_lossy(&output.stdout).trim().to_string();
        assert_eq!(
            current_branch, "red",
            "worktree with [done] status should be reset and checked out to color branch"
        );

        // The GBIV.md entry for red should have been removed
        let content_after = std::fs::read_to_string(&gbiv_md_path).unwrap();
        assert!(
            !content_after.contains("[red]"),
            "GBIV.md entry for red should be removed after reset with [done] status, got: {}",
            content_after
        );
    }

    // single-color reset ignores status tag
    #[test]
    fn single_color_reset_ignores_status_tag() {
        // Test with [in-progress] status: single-color should still reset
        let (_source_dir, root, repo_path, _main_repo, gbiv_md_path) =
            setup_worktree_with_gbiv_entry(Some("in-progress"));

        let content = std::fs::read_to_string(&gbiv_md_path).unwrap();
        assert!(
            content.contains("[in-progress]"),
            "setup should have [in-progress] entry"
        );

        let result = reset_one(root.path(), "red");
        assert!(
            result.is_ok(),
            "single-color reset should succeed regardless of status tag, got: {:?}",
            result
        );

        let output = Cmd::new("git")
            .args(["rev-parse", "--abbrev-ref", "HEAD"])
            .current_dir(&repo_path)
            .output()
            .unwrap();
        let current_branch = String::from_utf8_lossy(&output.stdout).trim().to_string();
        assert_eq!(
            current_branch, "red",
            "single-color reset should check out to color branch regardless of status"
        );

        // Test with no status tag: single-color should also reset
        let (_source_dir2, root2, repo_path2, _main_repo2, _gbiv_md_path2) =
            setup_worktree_with_gbiv_entry(None);

        let result2 = reset_one(root2.path(), "red");
        assert!(
            result2.is_ok(),
            "single-color reset should succeed with no status tag, got: {:?}",
            result2
        );

        let output2 = Cmd::new("git")
            .args(["rev-parse", "--abbrev-ref", "HEAD"])
            .current_dir(&repo_path2)
            .output()
            .unwrap();
        let branch2 = String::from_utf8_lossy(&output2.stdout).trim().to_string();
        assert_eq!(
            branch2, "red",
            "single-color reset should check out to color branch when there is no status tag"
        );
    }

    // all-color reset prints summary with skip reasons
    #[test]
    fn all_color_reset_prints_summary_with_skip_reasons() {
        let (_source_dir, root, _repo_path, _main_repo, _gbiv_md_path) =
            setup_worktree_with_gbiv_entry(Some("in-progress"));

        let messages = reset_all_to_vec(root.path());

        let has_summary = messages.iter().any(|msg| {
            msg.contains("reset") && (msg.contains("without [done]") || msg.contains("not merged") || msg.contains("already reset"))
        });

        assert!(
            has_summary,
            "all-color reset should print a summary line (e.g., '0 cleaned (1 without [done] status)'), but no summary found in output: {:?}",
            messages
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

        let result = reset_one(root.path(), "red");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.contains("not merged"),
            "expected 'not merged' in error: {}",
            err
        );
    }
}

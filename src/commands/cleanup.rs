use std::path::Path;

use crate::colors::COLORS;
use crate::gbiv_md::remove_gbiv_features_by_tag;
use crate::git_utils::{
    checkout_branch, find_gbiv_root, find_repo_in_worktree, get_quick_status,
    get_remote_main_branch, is_merged_into, pull_remote,
};

pub fn cleanup_one(gbiv_root: &Path, color: &str) -> Result<(), String> {
    let worktree_dir = gbiv_root.join(color);

    let repo_path = find_repo_in_worktree(&worktree_dir)
        .ok_or_else(|| format!("No git repo found in {} worktree", color))?;

    let status = get_quick_status(&repo_path);
    let branch = status
        .branch
        .ok_or_else(|| format!("Could not determine current branch for {} worktree", color))?;

    if branch == color {
        println!("{} worktree is already on the {} branch, skipping", color, color);
        return Ok(());
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
    pull_remote(&repo_path, "origin", color)?;

    match find_repo_in_worktree(&gbiv_root.join("main")) {
        Some(main_repo) => {
            let gbiv_md_path = main_repo.join("GBIV.md");
            remove_gbiv_features_by_tag(&gbiv_md_path, color)?;
        }
        None => {
            eprintln!("Warning [{}]: could not find main repo to update GBIV.md", color);
        }
    }

    Ok(())
}

pub fn cleanup_command(color: Option<&str>) -> Result<(), String> {
    let cwd = std::env::current_dir()
        .map_err(|e| format!("Failed to get current directory: {}", e))?;
    let gbiv_root = find_gbiv_root(&cwd)
        .ok_or_else(|| "Not in a gbiv-structured repository".to_string())?;

    if let Some(c) = color {
        cleanup_one(&gbiv_root.root, c)
    } else {
        for &c in COLORS.iter() {
            if let Err(e) = cleanup_one(&gbiv_root.root, c) {
                eprintln!("Warning [{}]: {}", c, e);
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

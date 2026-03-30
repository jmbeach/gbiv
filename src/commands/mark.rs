use std::path::Path;

use crate::colors::COLORS;
use crate::gbiv_md::set_gbiv_feature_status;
use crate::git_utils::{find_gbiv_root, find_repo_in_worktree};

/// Infer the color from a path by checking if any path component matches a COLORS entry.
fn infer_color_from_path(path: &Path) -> Option<&'static str> {
    for component in path.components() {
        if let Some(name) = component.as_os_str().to_str() {
            for &color in COLORS.iter() {
                if name == color {
                    return Some(color);
                }
            }
        }
    }
    None
}

pub fn mark_command(
    status: Option<&str>,
    color: Option<&str>,
    root_path: Option<&Path>,
) -> Result<String, String> {
    // Determine the working directory to use for finding gbiv root and inferring color
    let cwd = match root_path {
        Some(p) => p.to_path_buf(),
        None => std::env::current_dir()
            .map_err(|e| format!("Failed to get current directory: {}", e))?,
    };

    // Resolve color: explicit or inferred from CWD
    let resolved_color = match color {
        Some(c) => c.to_string(),
        None => {
            infer_color_from_path(&cwd)
                .ok_or_else(|| "Could not infer color from current worktree directory".to_string())?
                .to_string()
        }
    };

    // Find gbiv root
    let gbiv_root = find_gbiv_root(&cwd)
        .ok_or_else(|| "Not in a gbiv-structured repository".to_string())?;

    // Locate GBIV.md in main worktree
    let main_repo = find_repo_in_worktree(&gbiv_root.root.join("main"))
        .ok_or_else(|| "Could not find main repo to locate GBIV.md".to_string())?;
    let gbiv_md_path = main_repo.join("GBIV.md");

    // Map status values
    let gbiv_status = match status {
        Some("done") => Some("done"),
        Some("in-progress") => Some("in-progress"),
        Some("unset") => None,
        Some(other) => return Err(format!("Unknown status: {}", other)),
        None => return Err("No status provided".to_string()),
    };

    let is_unset = status == Some("unset");

    // For unset with missing entry: return Ok (no-op)
    // For done/in-progress with missing entry: return Err
    // set_gbiv_feature_status handles the actual file manipulation
    match set_gbiv_feature_status(&gbiv_md_path, &resolved_color, gbiv_status) {
        Ok(()) => {
            let message = if is_unset {
                format!("{}: status cleared", resolved_color)
            } else {
                format!("{}: marked as {}", resolved_color, status.unwrap())
            };
            Ok(message)
        }
        Err(e) => {
            // For unset, if the entry doesn't exist, that's a no-op
            if is_unset {
                Ok(format!("{}: status cleared", resolved_color))
            } else {
                Err(e)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;
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
        fs::create_dir_all(path).unwrap();
        git(&["init"], path);
        git(&["config", "user.email", "test@example.com"], path);
        git(&["config", "user.name", "Test"], path);
        fs::write(path.join("README.md"), "hello").unwrap();
        git(&["add", "."], path);
        git(&["commit", "-m", "initial"], path);
        git(&["branch", "-m", branch], path);
    }

    /// Creates a gbiv-structured root with a main repo containing GBIV.md.
    /// Returns (root_tempdir, main_repo_path, gbiv_md_path).
    fn setup_gbiv_root_with_gbiv_md(content: &str) -> (TempDir, PathBuf, PathBuf) {
        let root = TempDir::new().unwrap();
        let folder_name = root
            .path()
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .to_string();

        // Create main/folder_name as a git repo (required by find_gbiv_root)
        let main_repo = root.path().join("main").join(&folder_name);
        setup_repo_with_commit(&main_repo, "main");

        // Create GBIV.md in the main repo
        let gbiv_md_path = main_repo.join("GBIV.md");
        fs::write(&gbiv_md_path, content).unwrap();

        // Create at least one color dir so find_gbiv_root is satisfied
        fs::create_dir_all(root.path().join("red")).unwrap();

        (root, main_repo, gbiv_md_path)
    }

    // gbi-yw4d: mark --done sets done status in GBIV.md
    #[test]
    fn mark_done_adds_done_tag_to_entry() {
        let (root, _main_repo, gbiv_md_path) =
            setup_gbiv_root_with_gbiv_md("- [red] Fix critical bug\n");

        let result = mark_command(Some("done"), Some("red"), Some(root.path()));
        assert!(result.is_ok(), "expected Ok but got: {:?}", result);

        let content = fs::read_to_string(&gbiv_md_path).unwrap();
        assert!(
            content.contains("- [red] [done] Fix critical bug"),
            "expected '[done]' tag in entry, got: {:?}",
            content
        );
    }

    #[test]
    fn mark_done_replaces_existing_status_with_done() {
        let (root, _main_repo, gbiv_md_path) =
            setup_gbiv_root_with_gbiv_md("- [red] [in-progress] Fix critical bug\n");

        let result = mark_command(Some("done"), Some("red"), Some(root.path()));
        assert!(result.is_ok(), "expected Ok but got: {:?}", result);

        let content = fs::read_to_string(&gbiv_md_path).unwrap();
        assert!(
            content.contains("- [red] [done] Fix critical bug"),
            "expected '[done]' to replace '[in-progress]', got: {:?}",
            content
        );
        assert!(
            !content.contains("[in-progress]"),
            "expected '[in-progress]' to be removed, got: {:?}",
            content
        );
    }

    // gbi-qkn6: mark --in-progress sets in-progress status in GBIV.md
    #[test]
    fn mark_in_progress_adds_in_progress_tag_to_entry() {
        let (root, _main_repo, gbiv_md_path) =
            setup_gbiv_root_with_gbiv_md("- [red] Fix critical bug\n");

        let result = mark_command(Some("in-progress"), Some("red"), Some(root.path()));
        assert!(result.is_ok(), "expected Ok but got: {:?}", result);

        let content = fs::read_to_string(&gbiv_md_path).unwrap();
        assert!(
            content.contains("- [red] [in-progress] Fix critical bug"),
            "expected '[in-progress]' tag in entry, got: {:?}",
            content
        );
    }

    // gbi-xuln: mark --unset removes status from GBIV.md
    #[test]
    fn mark_unset_removes_status_tag_from_entry() {
        let (root, _main_repo, gbiv_md_path) =
            setup_gbiv_root_with_gbiv_md("- [red] [done] Fix critical bug\n");

        let result = mark_command(Some("unset"), Some("red"), Some(root.path()));
        assert!(result.is_ok(), "expected Ok but got: {:?}", result);

        let content = fs::read_to_string(&gbiv_md_path).unwrap();
        assert!(
            content.contains("- [red] Fix critical bug"),
            "expected status tag to be removed, got: {:?}",
            content
        );
        assert!(
            !content.contains("[done]"),
            "expected '[done]' to be removed, got: {:?}",
            content
        );
    }

    #[test]
    fn mark_unset_noop_when_no_status_present() {
        let original = "- [red] Fix critical bug\n";
        let (root, _main_repo, gbiv_md_path) = setup_gbiv_root_with_gbiv_md(original);

        let result = mark_command(Some("unset"), Some("red"), Some(root.path()));
        assert!(result.is_ok(), "expected Ok but got: {:?}", result);

        let content = fs::read_to_string(&gbiv_md_path).unwrap();
        assert!(
            content.contains("- [red] Fix critical bug"),
            "expected entry to be unchanged, got: {:?}",
            content
        );
    }

    // gbi-fa26: mark errors when no GBIV.md entry for color
    #[test]
    fn mark_done_errors_when_no_matching_color_entry() {
        let (root, _main_repo, _gbiv_md_path) =
            setup_gbiv_root_with_gbiv_md("- [blue] Some other feature\n");

        let result = mark_command(Some("done"), Some("red"), Some(root.path()));
        assert!(
            result.is_err(),
            "expected Err when no [red] entry, but got: {:?}",
            result
        );
        let err = result.unwrap_err();
        assert!(
            err.contains("red") || err.contains("no feature") || err.contains("not found"),
            "expected error to mention missing color entry, got: {:?}",
            err
        );
    }

    #[test]
    fn mark_unset_noop_when_no_matching_color_entry() {
        let (root, _main_repo, _gbiv_md_path) =
            setup_gbiv_root_with_gbiv_md("- [blue] Some other feature\n");

        let result = mark_command(Some("unset"), Some("red"), Some(root.path()));
        assert!(
            result.is_ok(),
            "expected Ok (noop) when no [red] entry for unset, got: {:?}",
            result
        );
    }

    // gbi-gohl: mark infers color from current worktree directory
    #[test]
    fn mark_infers_color_from_worktree_directory() {
        let (root, _main_repo, gbiv_md_path) =
            setup_gbiv_root_with_gbiv_md("- [red] Fix critical bug\n");

        // The red worktree dir already exists (created in setup_gbiv_root_with_gbiv_md)
        // Pass None for color so it must be inferred from root_path context.
        // We simulate being in the red worktree by passing the red subdir as cwd hint.
        // Since mark_command takes root_path for testability, we pass the red worktree path
        // as the cwd (root_path here represents the current working directory).
        let red_worktree = root.path().join("red");
        let result = mark_command(Some("done"), None, Some(&red_worktree));
        assert!(result.is_ok(), "expected Ok when color inferred from CWD, got: {:?}", result);

        let content = fs::read_to_string(&gbiv_md_path).unwrap();
        assert!(
            content.contains("- [red] [done] Fix critical bug"),
            "expected '[done]' tag after color inference, got: {:?}",
            content
        );
    }

    // gbi-xpjp: mark errors when color cannot be inferred
    #[test]
    fn mark_errors_when_color_cannot_be_inferred() {
        let (root, _main_repo, _gbiv_md_path) =
            setup_gbiv_root_with_gbiv_md("- [red] Fix critical bug\n");

        // Pass the main worktree dir as CWD — not inside a color worktree
        let main_dir = root.path().join("main");
        let result = mark_command(Some("done"), None, Some(&main_dir));
        assert!(
            result.is_err(),
            "expected Err when color cannot be inferred from CWD, got: {:?}",
            result
        );
        let err = result.unwrap_err();
        assert!(
            err.contains("infer") || err.contains("color") || err.contains("worktree"),
            "expected error about unable to infer color, got: {:?}",
            err
        );
    }
}

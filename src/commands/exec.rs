use std::path::Path;
use std::process::Command as ProcessCommand;
use std::thread;

use crate::colors::{ansi_color, COLORS, RESET};
use crate::git_utils::{find_gbiv_root, find_repo_in_worktree, infer_color_from_path};

// @spec OBS-EXEC-005 through OBS-EXEC-009
pub fn exec_single(root: &Path, color: &str, command: &[String]) -> Result<String, String> {
    // Validate color
    if !COLORS.contains(&color) {
        return Err(format!("'{}' is not a valid color", color));
    }

    let worktree_dir = root.join(color);
    let repo_path = find_repo_in_worktree(&worktree_dir).ok_or_else(|| {
        format!(
            "Worktree for '{}' does not exist or has no repo at {}",
            color,
            worktree_dir.display()
        )
    })?;

    let joined = command.join(" ");
    let output = ProcessCommand::new("sh")
        .args(["-c", &joined])
        .current_dir(&repo_path)
        .output()
        .map_err(|e| format!("Failed to execute command: {}", e))?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        let mut combined = String::from_utf8_lossy(&output.stdout).to_string();
        combined.push_str(&String::from_utf8_lossy(&output.stderr));
        Err(combined)
    }
}

// @spec OBS-EXEC-010 through OBS-EXEC-016
pub fn exec_all(root: &Path, command: &[String]) -> Result<Vec<(String, Result<String, String>)>, String> {
    // Find which colors have repos
    let mut existing: Vec<(&str, std::path::PathBuf)> = Vec::new();
    for &color in &COLORS {
        let worktree_dir = root.join(color);
        if let Some(repo_path) = find_repo_in_worktree(&worktree_dir) {
            existing.push((color, repo_path));
        }
    }

    let command_owned: Vec<String> = command.to_vec();

    // Spawn threads
    let handles: Vec<_> = existing
        .into_iter()
        .map(|(color, repo_path)| {
            let cmd = command_owned.clone();
            let color_str = color.to_string();
            thread::spawn(move || {
                let joined = cmd.join(" ");
                let output = ProcessCommand::new("sh")
                    .args(["-c", &joined])
                    .current_dir(&repo_path)
                    .output();

                let result = match output {
                    Ok(o) if o.status.success() => {
                        Ok(String::from_utf8_lossy(&o.stdout).to_string())
                    }
                    Ok(o) => {
                        let mut combined = String::from_utf8_lossy(&o.stdout).to_string();
                        combined.push_str(&String::from_utf8_lossy(&o.stderr));
                        Err(combined)
                    }
                    Err(e) => Err(format!("Failed to execute: {}", e)),
                };
                (color_str, result)
            })
        })
        .collect();

    // Join in order — handles are already in ROYGBIV order
    let mut ordered: Vec<(String, Result<String, String>)> = Vec::new();
    for handle in handles {
        let (color, result) = handle.join().map_err(|_| "Thread panicked".to_string())?;
        ordered.push((color, result));
    }

    Ok(ordered)
}

// @spec OBS-EXEC-001 through OBS-EXEC-020
pub fn exec_command(
    target: Option<&str>,
    command: &[String],
    cwd: Option<&Path>,
) -> Result<String, String> {
    let cwd_path = match cwd {
        Some(p) => p.to_path_buf(),
        None => std::env::current_dir()
            .map_err(|e| format!("Failed to get current directory: {}", e))?,
    };

    match target {
        None => {
            // Infer color from CWD
            let gbiv_root = find_gbiv_root(&cwd_path)
                .ok_or_else(|| "Could not infer color: not in a gbiv worktree".to_string())?;
            let color = infer_color_from_path(&cwd_path, &gbiv_root.root)
                .ok_or_else(|| "Could not infer color from current worktree directory".to_string())?;
            exec_single(&gbiv_root.root, color, command)
        }
        Some("all") => {
            let gbiv_root = find_gbiv_root(&cwd_path)
                .ok_or_else(|| "Not in a gbiv-structured repository".to_string())?;
            let results = exec_all(&gbiv_root.root, command)?;
            let mut output = String::new();
            let mut any_failed = false;
            for (color, result) in &results {
                let header = format!("{}[{}]{}", ansi_color(color), color, RESET);
                match result {
                    Ok(stdout) => {
                        output.push_str(&format!("{}\n{}", header, stdout));
                    }
                    Err(stderr) => {
                        any_failed = true;
                        output.push_str(&format!("{} (FAILED)\n{}", header, stderr));
                    }
                }
            }
            if any_failed {
                Err(output)
            } else {
                Ok(output)
            }
        }
        Some(color) => {
            let gbiv_root = find_gbiv_root(&cwd_path)
                .ok_or_else(|| "Not in a gbiv-structured repository".to_string())?;
            exec_single(&gbiv_root.root, color, command)
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

    fn git(args: &[&str], dir: &Path) {
        Cmd::new("git")
            .args(args)
            .current_dir(dir)
            .output()
            .expect("git command failed");
    }

    fn setup_repo_with_commit(path: &Path, branch: &str) {
        fs::create_dir_all(path).unwrap();
        git(&["init"], path);
        git(&["config", "user.email", "test@example.com"], path);
        git(&["config", "user.name", "Test"], path);
        // Prevent git from spawning background gc/fsmonitor daemons that outlive the test
        git(&["config", "gc.auto", "0"], path);
        git(&["config", "maintenance.auto", "false"], path);
        git(&["config", "core.fsmonitor", "false"], path);
        fs::write(path.join("README.md"), "hello").unwrap();
        git(&["add", "."], path);
        git(&["commit", "-m", "initial"], path);
        git(&["branch", "-m", branch], path);
    }

    /// Creates a gbiv-structured root with a main repo.
    /// Optionally creates a color worktree with a repo inside it.
    /// Returns (root_tempdir, main_repo_path).
    fn setup_gbiv_root(color_worktree: Option<&str>) -> (TempDir, PathBuf) {
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

        // Create the requested color worktree with a repo inside
        if let Some(color) = color_worktree {
            let color_repo = root.path().join(color).join(&folder_name);
            setup_repo_with_commit(&color_repo, color);
        } else {
            // Create at least one color dir so find_gbiv_root is satisfied
            fs::create_dir_all(root.path().join("red")).unwrap();
        }

        (root, main_repo)
    }

    // @spec OBS-EXEC-007
    #[test]
    fn exec_single_runs_command_in_color_worktree() {
        let (root, _main_repo) = setup_gbiv_root(Some("green"));

        let command = vec!["touch".to_string(), "exec_was_here.txt".to_string()];
        let result = exec_single(root.path(), "green", &command);

        assert!(result.is_ok(), "expected Ok but got: {:?}", result);

        // Verify the command ran in the green worktree directory
        let folder_name = root
            .path()
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .to_string();
        let green_repo = root.path().join("green").join(&folder_name);
        assert!(
            green_repo.join("exec_was_here.txt").exists(),
            "expected command to have created file in green worktree at {}",
            green_repo.display()
        );
    }

    // @spec OBS-EXEC-006
    #[test]
    fn exec_single_errors_when_worktree_does_not_exist() {
        // Setup root without creating the "indigo" worktree directory
        let (root, _main_repo) = setup_gbiv_root(None);

        let command = vec!["ls".to_string()];
        let result = exec_single(root.path(), "indigo", &command);

        assert!(
            result.is_err(),
            "expected Err when worktree does not exist, but got: {:?}",
            result
        );
        let err = result.unwrap_err();
        assert!(
            err.contains("indigo") || err.contains("does not exist") || err.contains("not found"),
            "expected error to mention missing worktree, got: {:?}",
            err
        );
    }

    // @spec OBS-EXEC-005
    #[test]
    fn exec_single_errors_on_invalid_color_name() {
        let (root, _main_repo) = setup_gbiv_root(None);

        let command = vec!["ls".to_string()];
        let result = exec_single(root.path(), "purple", &command);

        assert!(
            result.is_err(),
            "expected Err for invalid color 'purple', but got: {:?}",
            result
        );
        let err = result.unwrap_err();
        assert!(
            err.contains("purple") || err.contains("invalid") || err.contains("color"),
            "expected error about invalid color, got: {:?}",
            err
        );
    }

    fn setup_color_repo(root: &Path, color: &str) -> PathBuf {
        let folder_name = root
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .to_string();
        let repo_dir = root.join(color).join(&folder_name);
        fs::create_dir_all(&repo_dir).unwrap();
        git(&["init"], &repo_dir);
        git(&["config", "user.email", "test@example.com"], &repo_dir);
        git(&["config", "user.name", "Test"], &repo_dir);
        fs::write(repo_dir.join("README.md"), "hello").unwrap();
        git(&["add", "."], &repo_dir);
        git(&["commit", "-m", "initial"], &repo_dir);
        git(&["branch", "-m", color], &repo_dir);
        repo_dir
    }

    // @spec OBS-EXEC-017, OBS-EXEC-020
    #[test]
    fn exec_command_infers_color_from_cwd() {
        let (root, _main_repo) = setup_gbiv_root(Some("blue"));

        let folder_name = root
            .path()
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .to_string();
        let blue_repo = root.path().join("blue").join(&folder_name);

        // Pass the blue worktree repo as CWD — exec_command should infer "blue"
        let command = vec!["touch".to_string(), "inferred_exec.txt".to_string()];
        let result = exec_command(None, &command, Some(&blue_repo));

        assert!(
            result.is_ok(),
            "expected Ok when color inferred from CWD, got: {:?}",
            result
        );
        assert!(
            blue_repo.join("inferred_exec.txt").exists(),
            "expected command to have run in blue worktree at {}",
            blue_repo.display()
        );
    }

    // @spec OBS-EXEC-010, OBS-EXEC-011
    #[test]
    fn exec_all_runs_command_in_every_existing_worktree_and_returns_roygbiv_order() {
        let (root, _main_repo) = setup_gbiv_root(Some("red"));
        setup_color_repo(root.path(), "green");
        setup_color_repo(root.path(), "blue");

        let command: Vec<String> = vec!["echo".to_string(), "hello".to_string()];
        let result = exec_all(root.path(), &command);

        assert!(result.is_ok(), "expected Ok from exec_all, got: {:?}", result);
        let entries = result.unwrap();

        let colors: Vec<&str> = entries.iter().map(|(c, _)| c.as_str()).collect();
        assert!(colors.contains(&"red"), "expected red in results");
        assert!(colors.contains(&"green"), "expected green in results");
        assert!(colors.contains(&"blue"), "expected blue in results");

        // Results must be in ROYGBIV order: red < green < blue
        let red_pos = colors.iter().position(|&c| c == "red").unwrap();
        let green_pos = colors.iter().position(|&c| c == "green").unwrap();
        let blue_pos = colors.iter().position(|&c| c == "blue").unwrap();
        assert!(
            red_pos < green_pos && green_pos < blue_pos,
            "expected ROYGBIV order (red < green < blue), got order: {:?}",
            colors
        );

        for (color, res) in &entries {
            assert!(res.is_ok(), "expected Ok for color {}, got: {:?}", color, res);
        }
    }

    // @spec OBS-EXEC-016
    #[test]
    fn exec_all_skips_missing_worktrees_without_error() {
        let (root, _main_repo) = setup_gbiv_root(Some("red"));
        setup_color_repo(root.path(), "blue");

        let command: Vec<String> = vec!["echo".to_string(), "hello".to_string()];
        let result = exec_all(root.path(), &command);

        assert!(
            result.is_ok(),
            "expected Ok from exec_all even with missing worktrees, got: {:?}",
            result
        );
        let entries = result.unwrap();
        let colors: Vec<&str> = entries.iter().map(|(c, _)| c.as_str()).collect();

        assert!(colors.contains(&"red"), "expected red in results");
        assert!(colors.contains(&"blue"), "expected blue in results");
        assert!(!colors.contains(&"orange"), "orange should be skipped");
        assert!(!colors.contains(&"yellow"), "yellow should be skipped");
        assert!(!colors.contains(&"green"), "green should be skipped");
        assert!(!colors.contains(&"indigo"), "indigo should be skipped");
        assert!(!colors.contains(&"violet"), "violet should be skipped");
    }

    // @spec OBS-EXEC-014
    #[test]
    fn exec_all_returns_failure_when_any_command_fails() {
        let (root, _main_repo) = setup_gbiv_root(Some("red"));
        setup_color_repo(root.path(), "blue");

        let command: Vec<String> = vec!["false".to_string()];
        let result = exec_all(root.path(), &command);

        match result {
            Err(_) => {
                // Outer error: acceptable, overall failure signalled
            }
            Ok(entries) => {
                let any_failed = entries.iter().any(|(_, r)| r.is_err());
                assert!(
                    any_failed,
                    "expected at least one failed entry when running `false`, but all succeeded: {:?}",
                    entries
                );
            }
        }
    }

    // @spec OBS-EXEC-018
    #[test]
    fn exec_command_errors_when_cwd_is_not_in_a_color_worktree() {
        let (root, _main_repo) = setup_gbiv_root(None);
        let main_dir = root.path().join("main");

        let command: Vec<String> = vec!["echo".to_string(), "hello".to_string()];
        let result = exec_command(None, &command, Some(&main_dir));

        assert!(
            result.is_err(),
            "expected Err when CWD is not in a color worktree, but got: {:?}",
            result
        );
        let err = result.unwrap_err();
        assert!(
            err.contains("infer") || err.contains("color") || err.contains("worktree"),
            "expected error about failing to infer color from CWD, got: {:?}",
            err
        );
    }
}

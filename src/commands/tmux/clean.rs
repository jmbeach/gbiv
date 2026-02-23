use clap::Command;
use std::collections::HashSet;
use std::env;
use std::process::Command as ProcessCommand;

use crate::colors::COLORS;
use crate::gbiv_md::parse_gbiv_md;
use crate::git_utils::find_gbiv_root;

pub fn clean_subcommand() -> Command {
    Command::new("clean")
        .about("Close ROYGBIV tmux windows with no tagged feature in GBIV.md")
}

/// Pure filtering predicate — testable without a live tmux process.
pub fn is_orphaned_window(name: &str, active_colors: &HashSet<String>) -> bool {
    COLORS.contains(&name) && !active_colors.contains(name)
}

pub fn clean_command() -> Result<(), String> {
    // Guard 1: tmux must be available
    let tmux_available = ProcessCommand::new("tmux")
        .arg("-V")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    if !tmux_available {
        return Err("tmux not found. Please install tmux.".to_string());
    }

    // Guard 2: must be inside a gbiv project
    let cwd = env::current_dir().map_err(|e| e.to_string())?;
    let gbiv_root = find_gbiv_root(&cwd)
        .ok_or_else(|| "Not inside a gbiv project. Run `gbiv init` to initialize one.".to_string())?;

    let session_name = &gbiv_root.folder_name;

    // Guard 3: session must already exist
    let session_exists = ProcessCommand::new("tmux")
        .args(["has-session", "-t", session_name])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    if !session_exists {
        return Err(format!(
            "No tmux session '{}' found. Run `gbiv tmux new-session` to create one.",
            session_name
        ));
    }

    // List tmux windows
    let output = ProcessCommand::new("tmux")
        .args(["list-windows", "-t", session_name, "-F", "#{window_name}"])
        .output()
        .map_err(|e| e.to_string())?;

    if !output.status.success() {
        return Err(format!(
            "Failed to list windows for session '{}': {}",
            session_name,
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }

    let windows: Vec<String> = String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(|s| s.to_string())
        .collect();

    // Parse GBIV.md and build active colors set
    let gbiv_md_path = gbiv_root
        .root
        .join("main")
        .join(&gbiv_root.folder_name)
        .join("GBIV.md");
    let features = parse_gbiv_md(&gbiv_md_path);
    let active_colors: HashSet<String> = features.into_iter().filter_map(|f| f.tag).collect();

    // Find orphaned windows
    let orphaned: Vec<&String> = windows
        .iter()
        .filter(|name| is_orphaned_window(name, &active_colors))
        .collect();

    if orphaned.is_empty() {
        println!("Nothing to clean.");
        return Ok(());
    }

    let mut had_error = false;
    for name in orphaned {
        let target = format!("{}:{}", session_name, name);
        let kill_result = ProcessCommand::new("tmux")
            .args(["kill-window", "-t", &target])
            .output();

        match kill_result {
            Ok(out) if out.status.success() => {
                println!("Closed: {}", name);
            }
            _ => {
                eprintln!("Warning: failed to kill window '{}'", name);
                had_error = true;
            }
        }
    }

    if had_error {
        Err("Some windows could not be closed.".to_string())
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;
    use std::process::Command as ProcessCommand;

    #[test]
    #[serial]
    fn test_clean_command_tmux_not_found() {
        let tmpdir = std::env::temp_dir().join("gbiv_empty_path_for_clean_test");
        std::fs::create_dir_all(&tmpdir).unwrap();
        let original_path = env::var("PATH").unwrap_or_default();
        // SAFETY: serialized via #[serial]; no concurrent test reads PATH
        unsafe { env::set_var("PATH", &tmpdir) };

        let result = clean_command();

        // SAFETY: restoring PATH after test
        unsafe { env::set_var("PATH", &original_path) };

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.contains("tmux not found"),
            "Expected 'tmux not found' in error, got: {}",
            err
        );
    }

    #[test]
    #[serial]
    fn test_clean_command_session_not_found() {
        // This test requires a live tmux binary; skip gracefully if not present.
        let tmux_available = ProcessCommand::new("tmux")
            .arg("-V")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);
        if !tmux_available {
            eprintln!("Skipping test_clean_command_session_not_found: tmux not available");
            return;
        }

        // Build a minimal fake gbiv project structure in /tmp.
        let base = std::path::PathBuf::from("/tmp/gbiv_test_clean_session_not_found");
        let _ = std::fs::remove_dir_all(&base);
        let project_name = "testcleanproject";
        let main_repo = base.join(project_name).join("main").join(project_name);
        std::fs::create_dir_all(&main_repo).unwrap();
        ProcessCommand::new("git").args(["init"]).current_dir(&main_repo).output().unwrap();
        ProcessCommand::new("git").args(["config", "user.email", "t@t.com"]).current_dir(&main_repo).output().unwrap();
        ProcessCommand::new("git").args(["config", "user.name", "T"]).current_dir(&main_repo).output().unwrap();
        std::fs::write(main_repo.join("README.md"), "test").unwrap();
        ProcessCommand::new("git").args(["add", "."]).current_dir(&main_repo).output().unwrap();
        ProcessCommand::new("git").args(["commit", "-m", "init"]).current_dir(&main_repo).output().unwrap();
        // A color directory is required for find_gbiv_root to recognise the project.
        std::fs::create_dir_all(base.join(project_name).join("red")).unwrap();

        // SAFETY: serialized via #[serial]; set_current_dir mutates process-global CWD so this
        // test must not run concurrently with any other test that calls env::current_dir().
        let original_dir = env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("/tmp"));
        env::set_current_dir(base.join(project_name)).unwrap();

        let result = clean_command();

        // Restore CWD even if the original was /tmp fallback.
        let _ = env::set_current_dir(&original_dir);
        let _ = std::fs::remove_dir_all(&base);

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.contains("gbiv tmux new-session"),
            "Expected error mentioning 'gbiv tmux new-session', got: {}",
            err
        );
    }

    #[test]
    fn test_orphan_filtering_basic() {
        let active: HashSet<String> = ["red", "blue"].iter().map(|s| s.to_string()).collect();

        // ROYGBIV colors not in active → orphaned
        assert!(is_orphaned_window("orange", &active));
        assert!(is_orphaned_window("yellow", &active));
        assert!(is_orphaned_window("green", &active));

        // ROYGBIV colors in active → not orphaned
        assert!(!is_orphaned_window("red", &active));
        assert!(!is_orphaned_window("blue", &active));
    }

    #[test]
    fn test_orphan_filtering_skips_non_roygbiv() {
        let active: HashSet<String> = HashSet::new();

        // Non-ROYGBIV names must never be considered orphaned
        assert!(!is_orphaned_window("main", &active));
        assert!(!is_orphaned_window("bash", &active));
        assert!(!is_orphaned_window("purple", &active));
        assert!(!is_orphaned_window("", &active));
    }

    #[test]
    fn test_orphan_filtering_all_active() {
        let active: HashSet<String> = COLORS.iter().map(|s| s.to_string()).collect();

        for color in COLORS.iter() {
            assert!(!is_orphaned_window(color, &active));
        }
    }
}

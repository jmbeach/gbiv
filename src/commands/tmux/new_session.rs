use clap::{Arg, Command};
use std::env;
use std::process::Command as ProcessCommand;

use crate::colors::COLORS;
use crate::git_utils::find_gbiv_root;

pub fn new_session_subcommand() -> Command {
    Command::new("new-session")
        .about("Create a detached tmux session with one named window per ROYGBIV worktree")
        .arg(
            Arg::new("session-name")
                .long("session-name")
                .help("Name for the tmux session (defaults to the gbiv folder name)")
                .value_name("NAME"),
        )
}

pub fn new_session_command(session_name: Option<&str>) -> Result<(), String> {
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

    // Determine session name
    let name = session_name
        .map(|s| s.to_string())
        .unwrap_or_else(|| gbiv_root.folder_name.clone());

    // Guard 3: session must not already exist
    let session_exists = ProcessCommand::new("tmux")
        .args(["has-session", "-t", &name])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    if session_exists {
        return Err(format!(
            "Session '{}' already exists. Use `tmux attach -t {}` to attach, or pass `--session-name` to use a different name.",
            name, name
        ));
    }

    // Build the list of worktree paths: main first, then all ROYGBIV colors
    let worktree_paths: Vec<(String, std::path::PathBuf)> =
        std::iter::once("main")
            .chain(COLORS.iter().copied())
            .map(|color| {
                let path = gbiv_root.root.join(color).join(&gbiv_root.folder_name);
                (color.to_string(), path)
            })
            .collect();

    // Determine which paths exist (warn for missing ones)
    let existing_paths: Vec<(String, std::path::PathBuf)> = worktree_paths
        .into_iter()
        .filter(|(color, path)| {
            if path.exists() {
                true
            } else {
                eprintln!("Warning: worktree path for '{}' does not exist: {}", color, path.display());
                false
            }
        })
        .collect();

    // Need at least the main path to create the session
    let (first_color, first_path) = existing_paths
        .first()
        .ok_or_else(|| "No worktree paths exist; cannot create tmux session.".to_string())?;

    // Create the detached session with the first window.
    // Use .arg() with the PathBuf directly so non-UTF8 paths are handled correctly.
    let status = ProcessCommand::new("tmux")
        .args(["new-session", "-d", "-s", &name, "-n", first_color])
        .arg("-c")
        .arg(first_path)
        .status()
        .map_err(|e| format!("Failed to run tmux new-session: {}", e))?;

    if !status.success() {
        return Err(format!("tmux new-session failed with status: {}", status));
    }

    // Create additional windows for the remaining existing paths
    for (color, path) in existing_paths.iter().skip(1) {
        let status = ProcessCommand::new("tmux")
            .args(["new-window", "-t", &name, "-n", color])
            .arg("-c")
            .arg(path)
            .status()
            .map_err(|e| format!("Failed to run tmux new-window for '{}': {}", color, e))?;

        if !status.success() {
            return Err(format!("tmux new-window for '{}' failed with status: {}", color, status));
        }
    }

    println!("Created tmux session '{}' with {} window(s).", name, existing_paths.len());

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    #[test]
    #[serial]
    fn test_new_session_command_tmux_not_found() {
        // Point PATH at an empty temp dir so tmux can't be found
        let tmpdir = std::env::temp_dir().join("gbiv_empty_path_for_test");
        std::fs::create_dir_all(&tmpdir).unwrap();
        let original_path = env::var("PATH").unwrap_or_default();
        // SAFETY: serialized via #[serial]; no concurrent test reads PATH
        unsafe { env::set_var("PATH", &tmpdir) };

        let result = new_session_command(None);

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
}

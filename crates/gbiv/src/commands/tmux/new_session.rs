use clap::{Arg, Command};
use std::env;
use std::process::Command as ProcessCommand;

use gbiv_core::palette::Palette;
use gbiv_core::root::find_gbiv_root;
use gbiv_core::tmux::{has_session, session_name_for_root, tmux_available, TmuxError};

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

// @spec TMX-SESSION-001, TMX-SESSION-002, TMX-SESSION-003, TMX-SESSION-004, TMX-SESSION-005, TMX-SESSION-006, TMX-SESSION-007, TMX-SESSION-008, TMX-SESSION-009, TMX-SESSION-010, TMX-SESSION-011, TMX-SESSION-012, TMX-SESSION-013
pub fn new_session_command(session_name: Option<&str>) -> anyhow::Result<()> {
    // Guard 1: tmux must be available
    tmux_available().map_err(|e| match e {
        TmuxError::NotInstalled => anyhow::anyhow!("tmux not found. Please install tmux."),
        other => anyhow::anyhow!("{}", other),
    })?;

    // Guard 2: must be inside a gbiv project
    let cwd = env::current_dir()?;
    let gbiv_root = find_gbiv_root(&cwd).ok_or_else(|| {
        anyhow::anyhow!("Not inside a gbiv project. Run `gbiv init` to initialize one.")
    })?;

    // Determine session name
    let name = session_name
        .map(|s| s.to_string())
        .unwrap_or_else(|| session_name_for_root(&gbiv_root.folder_name));

    // Guard 3: session must not already exist
    if has_session(&name).map_err(|e| anyhow::anyhow!("{}", e))? {
        return Err(anyhow::anyhow!(
            "Session '{}' already exists. Use `tmux attach -t {}` to attach, or pass `--session-name` to use a different name.",
            name, name
        ));
    }

    // Build the list of worktree paths: main first, then the active palette
    // (base colors, then configured extras)
    let palette = Palette::load(&gbiv_root.root)?;
    let mut names: Vec<String> = vec!["main".to_string()];
    names.extend(palette.names().iter().cloned());
    let worktree_paths: Vec<(String, std::path::PathBuf)> = names
        .into_iter()
        .map(|color| {
            let path = gbiv_root.root.join(&color).join(&gbiv_root.folder_name);
            (color, path)
        })
        .collect();

    // Determine which paths exist (warn for missing ones)
    let existing_paths: Vec<(String, std::path::PathBuf)> = worktree_paths
        .into_iter()
        .filter(|(color, path)| {
            if path.exists() {
                true
            } else {
                eprintln!(
                    "Warning: worktree path for '{}' does not exist: {}",
                    color,
                    path.display()
                );
                false
            }
        })
        .collect();

    // Need at least the main path to create the session
    let (first_color, first_path) = existing_paths
        .first()
        .ok_or_else(|| anyhow::anyhow!("No worktree paths exist; cannot create tmux session."))?;

    // Create the detached session with the first window.
    // Use .arg() with the PathBuf directly so non-UTF8 paths are handled correctly.
    let status = ProcessCommand::new("tmux")
        .args(["new-session", "-d", "-s", &name, "-n", first_color])
        .arg("-c")
        .arg(first_path)
        .status()
        .map_err(|e| anyhow::anyhow!("Failed to run tmux new-session: {}", e))?;

    if !status.success() {
        return Err(anyhow::anyhow!(
            "tmux new-session failed with status: {}",
            status
        ));
    }

    // Create additional windows for the remaining existing paths
    for (color, path) in existing_paths.iter().skip(1) {
        let status = ProcessCommand::new("tmux")
            .args(["new-window", "-t", &name, "-n", color])
            .arg("-c")
            .arg(path)
            .status()
            .map_err(|e| anyhow::anyhow!("Failed to run tmux new-window for '{}': {}", color, e))?;

        if !status.success() {
            return Err(anyhow::anyhow!(
                "tmux new-window for '{}' failed with status: {}",
                color,
                status
            ));
        }
    }

    println!(
        "Created tmux session '{}' with {} window(s).",
        name,
        existing_paths.len()
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    // @spec TMX-SESSION-001
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
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("tmux not found"),
            "Expected 'tmux not found' in error, got: {}",
            err
        );
    }
}

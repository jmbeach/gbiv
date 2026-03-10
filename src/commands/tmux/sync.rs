use clap::{Arg, Command};
use std::collections::HashSet;
use std::env;
use std::process::Command as ProcessCommand;

use crate::colors::COLORS;
use crate::gbiv_md::{parse_gbiv_md, GbivFeature};
use crate::git_utils::find_gbiv_root;

pub fn sync_subcommand() -> Command {
    Command::new("sync")
        .about("Create missing tmux windows for active GBIV.md colors and reorder to ROYGBIV order")
        .arg(
            Arg::new("session-name")
                .long("session-name")
                .help("Name of the tmux session (defaults to gbiv folder name)")
                .num_args(1),
        )
}

/// Extract the set of valid ROYGBIV colors that have at least one tagged feature in GBIV.md.
pub fn active_colors_from_features(features: &[GbivFeature]) -> HashSet<String> {
    let valid_colors: HashSet<&str> = COLORS.iter().copied().collect();
    features
        .iter()
        .filter_map(|f| f.tag.as_deref())
        .filter(|tag| valid_colors.contains(tag))
        .map(|s| s.to_string())
        .collect()
}

/// Given the set of active colors and the list of existing window names,
/// return the colors that need new windows created.
pub fn missing_windows(active_colors: &HashSet<String>, existing_windows: &[String]) -> Vec<String> {
    let existing_set: HashSet<&str> = existing_windows.iter().map(|s| s.as_str()).collect();
    COLORS
        .iter()
        .filter(|c| active_colors.contains(**c) && !existing_set.contains(**c))
        .map(|s| s.to_string())
        .collect()
}

/// Sort window names into ROYGBIV order: main first, then colors in ROYGBIV order,
/// then any non-color windows preserving their relative order.
pub fn sort_windows_roygbiv(window_names: &[String]) -> Vec<String> {
    let color_set: HashSet<&str> = COLORS.iter().copied().collect();
    let mut result: Vec<String> = Vec::new();

    // "main" first
    if window_names.iter().any(|w| w == "main") {
        result.push("main".to_string());
    }

    // Colors in ROYGBIV order
    for color in &COLORS {
        if window_names.iter().any(|w| w.as_str() == *color) {
            result.push(color.to_string());
        }
    }

    // Non-color, non-main windows preserving relative order
    for w in window_names {
        if w != "main" && !color_set.contains(w.as_str()) {
            result.push(w.clone());
        }
    }

    result
}

pub fn sync_command(session_name: Option<&str>) -> Result<(), String> {
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
    let session_name = session_name
        .map(|s| s.to_string())
        .unwrap_or_else(|| gbiv_root.folder_name.clone());

    // Guard 3: session must already exist
    let session_exists = ProcessCommand::new("tmux")
        .args(["has-session", "-t", &session_name])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    if !session_exists {
        return Err(format!(
            "No tmux session '{}' found. Run `gbiv tmux new-session` to create one.",
            session_name
        ));
    }

    // List existing windows
    let output = ProcessCommand::new("tmux")
        .args(["list-windows", "-t", &session_name, "-F", "#{window_name}"])
        .output()
        .map_err(|e| e.to_string())?;

    if !output.status.success() {
        return Err(format!(
            "Failed to list windows for session '{}': {}",
            session_name,
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }

    let existing_windows: Vec<String> = String::from_utf8_lossy(&output.stdout)
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
    let active_colors = active_colors_from_features(&features);

    // Find missing windows
    let missing = missing_windows(&active_colors, &existing_windows);

    // Create missing windows
    let mut created: Vec<String> = Vec::new();
    for color in &missing {
        let worktree_path = gbiv_root.root.join(color).join(&gbiv_root.folder_name);
        if !worktree_path.exists() {
            println!("Warning: worktree path '{}' does not exist, skipping window '{}'", worktree_path.display(), color);
            continue;
        }

        let create_result = ProcessCommand::new("tmux")
            .args([
                "new-window",
                "-t", &session_name,
                "-n", color,
                "-c", &worktree_path.to_string_lossy(),
            ])
            .output();

        match create_result {
            Ok(out) if out.status.success() => {
                println!("Created window: {}", color);
                created.push(color.clone());
            }
            _ => {
                eprintln!("Warning: failed to create window '{}'", color);
            }
        }
    }

    // Reorder windows: re-list all windows after creation
    let output = ProcessCommand::new("tmux")
        .args(["list-windows", "-t", &session_name, "-F", "#{window_name}"])
        .output()
        .map_err(|e| e.to_string())?;

    if !output.status.success() {
        return Err(format!(
            "Failed to list windows for session '{}': {}",
            session_name,
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }

    let current_windows: Vec<String> = String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(|s| s.to_string())
        .collect();

    let desired_order = sort_windows_roygbiv(&current_windows);

    // Move all windows to high temporary indices first to avoid index collisions,
    // then move them back to their desired positions.
    let offset = 1000;
    for (i, name) in desired_order.iter().enumerate() {
        let source = format!("{}:{}", session_name, name);
        let target = format!("{}:{}", session_name, offset + i);
        let _ = ProcessCommand::new("tmux")
            .args(["move-window", "-s", &source, "-t", &target])
            .output();
    }
    for (index, _name) in desired_order.iter().enumerate() {
        let source = format!("{}:{}", session_name, offset + index);
        let target = format!("{}:{}", session_name, index);
        let _ = ProcessCommand::new("tmux")
            .args(["move-window", "-s", &source, "-t", &target])
            .output();
    }

    // Summary
    if created.is_empty() {
        println!("No new windows created. Windows reordered to ROYGBIV order.");
    } else {
        println!(
            "Created {} window(s): {}. Windows reordered to ROYGBIV order.",
            created.len(),
            created.join(", ")
        );
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    // --- Test ticket gbi-xcdt: active color extraction from GBIV.md ---

    #[test]
    fn test_active_colors_extracts_valid_roygbiv_tags() {
        let features = vec![
            GbivFeature { tag: Some("red".to_string()), description: "Fix bug".to_string(), notes: vec![] },
            GbivFeature { tag: Some("blue".to_string()), description: "Add feature".to_string(), notes: vec![] },
            GbivFeature { tag: None, description: "Untagged".to_string(), notes: vec![] },
        ];
        let active = active_colors_from_features(&features);
        assert!(active.contains("red"));
        assert!(active.contains("blue"));
        assert_eq!(active.len(), 2);
    }

    #[test]
    fn test_active_colors_excludes_invalid_tags() {
        let features = vec![
            GbivFeature { tag: Some("purple".to_string()), description: "Invalid color".to_string(), notes: vec![] },
            GbivFeature { tag: Some("red".to_string()), description: "Valid".to_string(), notes: vec![] },
        ];
        let active = active_colors_from_features(&features);
        assert!(active.contains("red"));
        assert!(!active.contains("purple"));
        assert_eq!(active.len(), 1);
    }

    #[test]
    fn test_active_colors_deduplicates() {
        let features = vec![
            GbivFeature { tag: Some("red".to_string()), description: "First".to_string(), notes: vec![] },
            GbivFeature { tag: Some("red".to_string()), description: "Second".to_string(), notes: vec![] },
        ];
        let active = active_colors_from_features(&features);
        assert_eq!(active.len(), 1);
        assert!(active.contains("red"));
    }

    #[test]
    fn test_active_colors_empty_features() {
        let features: Vec<GbivFeature> = vec![];
        let active = active_colors_from_features(&features);
        assert!(active.is_empty());
    }

    // --- Test ticket gbi-kjm9: missing window detection ---

    #[test]
    fn test_missing_windows_color_active_but_no_window() {
        let active: HashSet<String> = ["red", "indigo"].iter().map(|s| s.to_string()).collect();
        let existing = vec!["main".to_string(), "red".to_string()];
        let missing = missing_windows(&active, &existing);
        assert_eq!(missing, vec!["indigo".to_string()]);
    }

    #[test]
    fn test_missing_windows_all_present() {
        let active: HashSet<String> = ["red", "blue"].iter().map(|s| s.to_string()).collect();
        let existing = vec!["main".to_string(), "red".to_string(), "blue".to_string()];
        let missing = missing_windows(&active, &existing);
        assert!(missing.is_empty());
    }

    #[test]
    fn test_missing_windows_inactive_color_not_returned() {
        let active: HashSet<String> = ["red"].iter().map(|s| s.to_string()).collect();
        let existing = vec!["main".to_string()];
        let missing = missing_windows(&active, &existing);
        assert_eq!(missing, vec!["red".to_string()]);
        // "blue" is not active, so it should NOT appear in missing
        assert!(!missing.contains(&"blue".to_string()));
    }

    // --- Test ticket gbi-kp8z: window ordering logic ---

    #[test]
    fn test_sort_windows_roygbiv_basic_order() {
        let windows = vec![
            "yellow".to_string(), "red".to_string(), "main".to_string(), "indigo".to_string(),
        ];
        let sorted = sort_windows_roygbiv(&windows);
        assert_eq!(sorted, vec!["main", "red", "yellow", "indigo"]);
    }

    #[test]
    fn test_sort_windows_roygbiv_non_color_windows_at_end() {
        let windows = vec![
            "bash".to_string(), "red".to_string(), "main".to_string(), "htop".to_string(),
        ];
        let sorted = sort_windows_roygbiv(&windows);
        assert_eq!(sorted, vec!["main", "red", "bash", "htop"]);
    }

    #[test]
    fn test_sort_windows_roygbiv_full_set() {
        let windows = vec![
            "violet".to_string(), "indigo".to_string(), "blue".to_string(),
            "green".to_string(), "yellow".to_string(), "orange".to_string(),
            "red".to_string(), "main".to_string(),
        ];
        let sorted = sort_windows_roygbiv(&windows);
        assert_eq!(
            sorted,
            vec!["main", "red", "orange", "yellow", "green", "blue", "indigo", "violet"]
        );
    }

    #[test]
    fn test_sort_windows_roygbiv_subset_of_colors() {
        let windows = vec!["blue".to_string(), "green".to_string()];
        let sorted = sort_windows_roygbiv(&windows);
        assert_eq!(sorted, vec!["green", "blue"]);
    }

    // --- Test ticket gbi-86mk: pre-flight guard - tmux not found ---

    #[test]
    #[serial]
    fn test_sync_command_tmux_not_found() {
        let tmpdir = std::env::temp_dir().join("gbiv_empty_path_for_sync_test");
        std::fs::create_dir_all(&tmpdir).unwrap();
        let original_path = env::var("PATH").unwrap_or_default();
        // SAFETY: serialized via #[serial]; no concurrent test reads PATH
        unsafe { env::set_var("PATH", &tmpdir) };

        let result = sync_command(None);

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

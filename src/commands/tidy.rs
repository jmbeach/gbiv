use crate::commands::rebase_all::rebase_all_command;
use crate::commands::reset::reset_command;
use crate::commands::tmux::clean::clean_command;

pub fn tidy_command() -> Result<(), String> {
    let mut had_error = false;

    // Step 1: Rebase all worktrees
    println!("Rebasing all worktrees...");
    if let Err(e) = rebase_all_command() {
        eprintln!("rebase-all failed: {}", e);
        had_error = true;
    }

    // Step 2: Reset merged branches
    println!("Resetting merged branches...");
    let _ = reset_command(None);

    // Step 3: Clean tmux windows (only if tmux is installed)
    let tmux_available = std::process::Command::new("which")
        .arg("tmux")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    if tmux_available {
        println!("Cleaning tmux windows...");
        if let Err(e) = clean_command() {
            eprintln!("tmux clean failed: {}", e);
            had_error = true;
        }
    }

    if had_error {
        Err("tidy encountered errors".to_string())
    } else {
        Ok(())
    }
}

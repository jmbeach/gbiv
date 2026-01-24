use std::path::Path;
use std::process::Command as ProcessCommand;

pub fn is_git_repo(path: &Path) -> bool {
    let output = ProcessCommand::new("git")
        .args(["rev-parse", "--git-dir"])
        .current_dir(path)
        .output();
    matches!(output, Ok(o) if o.status.success())
}

pub fn has_commits(path: &Path) -> bool {
    let output = ProcessCommand::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(path)
        .output();
    matches!(output, Ok(o) if o.status.success())
}

pub fn get_main_branch(path: &Path) -> Option<String> {
    let output = ProcessCommand::new("git")
        .args(["symbolic-ref", "--short", "HEAD"])
        .current_dir(path)
        .output()
        .ok()?;
    if output.status.success() {
        Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        None
    }
}

pub fn get_existing_branches(path: &Path) -> Vec<String> {
    let output = ProcessCommand::new("git")
        .args(["branch", "--list"])
        .current_dir(path)
        .output();
    match output {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout)
            .lines()
            .map(|l| l.trim().trim_start_matches("* ").to_string())
            .collect(),
        _ => vec![],
    }
}

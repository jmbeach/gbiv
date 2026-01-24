use clap::{Arg, Command};
use std::fs;
use std::path::Path;
use std::process::Command as ProcessCommand;

const COLORS: [&str; 7] = ["red", "orange", "yellow", "green", "blue", "indigo", "violet"];

fn cli() -> Command {
    Command::new("gbiv")
        .about("A tool / framework for managing git worktrees")
        .subcommand_required(true)
        .arg_required_else_help(true)
        .subcommand(
            Command::new("init")
                .about("Initialize a git repository with ROYGBIV worktree structure")
                .arg(
                    Arg::new("folder")
                        .help("The folder name of the git repository to initialize")
                        .required(true)
                        .index(1),
                ),
        )
}

fn is_git_repo(path: &Path) -> bool {
    path.join(".git").exists()
}

fn has_commits(path: &Path) -> bool {
    let output = ProcessCommand::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(path)
        .output();
    matches!(output, Ok(o) if o.status.success())
}

fn get_main_branch(path: &Path) -> Option<String> {
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

fn get_existing_branches(path: &Path) -> Vec<String> {
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

fn check_color_branches(path: &Path) -> Vec<String> {
    let branches = get_existing_branches(path);
    COLORS
        .iter()
        .filter(|c| branches.contains(&c.to_string()))
        .map(|c| c.to_string())
        .collect()
}

fn init_command(folder: &str) -> Result<(), String> {
    let target_path = Path::new(folder);

    if !target_path.exists() {
        return Err(format!("Folder '{}' does not exist", folder));
    }

    if !is_git_repo(target_path) {
        return Err(format!("Folder '{}' is not a git repository", folder));
    }

    if !has_commits(target_path) {
        return Err(format!(
            "Repository '{}' has no commits. At least one commit is required for worktrees.",
            folder
        ));
    }

    let conflicting_branches = check_color_branches(target_path);
    if !conflicting_branches.is_empty() {
        return Err(format!(
            "Repository has branches named after ROYGBIV colors: {}. Please delete them first.",
            conflicting_branches.join(", ")
        ));
    }

    let main_branch = get_main_branch(target_path)
        .ok_or_else(|| "Could not determine main branch name".to_string())?;

    println!("Initializing '{}' with ROYGBIV worktree structure...", folder);

    let temp_name = format!("{}_gbiv_temp", folder);
    fs::rename(folder, &temp_name)
        .map_err(|e| format!("Failed to move folder temporarily: {}", e))?;

    fs::create_dir_all(format!("{}/main", folder))
        .map_err(|e| format!("Failed to create main directory: {}", e))?;

    fs::rename(&temp_name, format!("{}/main/{}", folder, folder))
        .map_err(|e| format!("Failed to move repo to main: {}", e))?;

    let main_repo_path = format!("{}/main/{}", folder, folder);

    for color in COLORS {
        let color_project_path = format!("{}/{}/{}", folder, color, folder);
        fs::create_dir_all(&color_project_path)
            .map_err(|e| format!("Failed to create {} directory: {}", color, e))?;

        let worktree_path = format!("../../{}/{}", color, folder);
        let output = ProcessCommand::new("git")
            .args(["worktree", "add", "-b", color, &worktree_path, &main_branch])
            .current_dir(&main_repo_path)
            .output()
            .map_err(|e| format!("Failed to run git worktree add for {}: {}", color, e))?;

        if !output.status.success() {
            return Err(format!(
                "Failed to create worktree for {}: {}",
                color,
                String::from_utf8_lossy(&output.stderr)
            ));
        }
        println!("Created worktree: {}", color);
    }

    println!("Successfully initialized '{}' with ROYGBIV worktrees!", folder);
    Ok(())
}

fn main() {
    let matches = cli().get_matches();

    match matches.subcommand() {
        Some(("init", sub_matches)) => {
            let folder = sub_matches.get_one::<String>("folder").unwrap();
            if let Err(e) = init_command(folder) {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        }
        _ => unreachable!(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::process::Command as ProcessCommand;

    fn setup_test_dir(name: &str) -> String {
        let test_dir = format!("/tmp/gbiv_test_{}", name);
        let _ = fs::remove_dir_all(&test_dir);
        fs::create_dir_all(&test_dir).unwrap();
        test_dir
    }

    fn cleanup_test_dir(path: &str) {
        let _ = fs::remove_dir_all(path);
    }

    fn init_git_repo(path: &str) {
        ProcessCommand::new("git")
            .args(["init"])
            .current_dir(path)
            .output()
            .unwrap();
        ProcessCommand::new("git")
            .args(["config", "user.email", "test@test.com"])
            .current_dir(path)
            .output()
            .unwrap();
        ProcessCommand::new("git")
            .args(["config", "user.name", "Test"])
            .current_dir(path)
            .output()
            .unwrap();
    }

    fn add_commit(path: &str) {
        fs::write(format!("{}/test.txt", path), "test").unwrap();
        ProcessCommand::new("git")
            .args(["add", "."])
            .current_dir(path)
            .output()
            .unwrap();
        ProcessCommand::new("git")
            .args(["commit", "-m", "initial"])
            .current_dir(path)
            .output()
            .unwrap();
    }

    #[test]
    fn test_is_git_repo_true() {
        let test_dir = setup_test_dir("is_git_repo_true");
        init_git_repo(&test_dir);
        assert!(is_git_repo(Path::new(&test_dir)));
        cleanup_test_dir(&test_dir);
    }

    #[test]
    fn test_is_git_repo_false() {
        let test_dir = setup_test_dir("is_git_repo_false");
        assert!(!is_git_repo(Path::new(&test_dir)));
        cleanup_test_dir(&test_dir);
    }

    #[test]
    fn test_has_commits_true() {
        let test_dir = setup_test_dir("has_commits_true");
        init_git_repo(&test_dir);
        add_commit(&test_dir);
        assert!(has_commits(Path::new(&test_dir)));
        cleanup_test_dir(&test_dir);
    }

    #[test]
    fn test_has_commits_false() {
        let test_dir = setup_test_dir("has_commits_false");
        init_git_repo(&test_dir);
        assert!(!has_commits(Path::new(&test_dir)));
        cleanup_test_dir(&test_dir);
    }

    #[test]
    fn test_get_main_branch() {
        let test_dir = setup_test_dir("get_main_branch");
        init_git_repo(&test_dir);
        add_commit(&test_dir);
        let branch = get_main_branch(Path::new(&test_dir));
        assert!(branch.is_some());
        let branch_name = branch.unwrap();
        assert!(branch_name == "main" || branch_name == "master");
        cleanup_test_dir(&test_dir);
    }

    #[test]
    fn test_check_color_branches_none() {
        let test_dir = setup_test_dir("color_branches_none");
        init_git_repo(&test_dir);
        add_commit(&test_dir);
        let conflicts = check_color_branches(Path::new(&test_dir));
        assert!(conflicts.is_empty());
        cleanup_test_dir(&test_dir);
    }

    #[test]
    fn test_check_color_branches_conflict() {
        let test_dir = setup_test_dir("color_branches_conflict");
        init_git_repo(&test_dir);
        add_commit(&test_dir);
        ProcessCommand::new("git")
            .args(["branch", "red"])
            .current_dir(&test_dir)
            .output()
            .unwrap();
        let conflicts = check_color_branches(Path::new(&test_dir));
        assert_eq!(conflicts, vec!["red"]);
        cleanup_test_dir(&test_dir);
    }

    #[test]
    fn test_init_command_folder_not_exist() {
        let result = init_command("nonexistent_folder_xyz");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("does not exist"));
    }

    #[test]
    fn test_init_command_not_git_repo() {
        let test_dir = setup_test_dir("not_git_repo");
        let result = init_command(&test_dir);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not a git repository"));
        cleanup_test_dir(&test_dir);
    }

    #[test]
    fn test_init_command_no_commits() {
        let test_dir = setup_test_dir("no_commits");
        init_git_repo(&test_dir);
        let result = init_command(&test_dir);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("no commits"));
        cleanup_test_dir(&test_dir);
    }

    #[test]
    fn test_init_command_color_branch_conflict() {
        let test_dir = setup_test_dir("branch_conflict");
        init_git_repo(&test_dir);
        add_commit(&test_dir);
        ProcessCommand::new("git")
            .args(["branch", "blue"])
            .current_dir(&test_dir)
            .output()
            .unwrap();
        let result = init_command(&test_dir);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("blue"));
        cleanup_test_dir(&test_dir);
    }

    #[test]
    fn test_init_command_success() {
        let base_dir = setup_test_dir("init_success");
        let project_name = "myproject";
        let project_dir = format!("{}/{}", base_dir, project_name);
        fs::create_dir_all(&project_dir).unwrap();
        init_git_repo(&project_dir);
        add_commit(&project_dir);

        std::env::set_current_dir(&base_dir).unwrap();
        let result = init_command(project_name);
        assert!(result.is_ok(), "init_command failed: {:?}", result);

        assert!(Path::new(&format!("{}/main/{}", project_name, project_name)).exists());
        for color in COLORS {
            assert!(Path::new(&format!("{}/{}/{}", project_name, color, project_name)).exists());
        }

        cleanup_test_dir(&base_dir);
    }
}

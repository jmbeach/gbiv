use std::fs;
use std::path::Path;
use std::process::Command;

use crate::colors::COLORS;
use crate::git_utils::{get_existing_branches, get_main_branch, has_commits, is_git_repo};

// Leading blank lines give users visible writing space above the --- so it's obvious features go there.
const GBIV_MD_TEMPLATE: &str = "\n\n\n\n\n\n\n\n\n\n---\n# GBIV.md\n\nAdd features above the `---` line. Each feature starts with `- ` and an optional `[color]` tag.\n\nExample:\n\n- [red] My urgent feature\n  A note about this feature\n- [green] A less urgent feature\n- An untagged backlog item\n\nSupported tags match ROYGBIV colors: red, orange, yellow, green, blue, indigo, violet.\nUntagged items appear with a dim `backlog` label.\nEverything below `---` is ignored by gbiv.";

fn write_gbiv_md_if_absent(main_repo_path: &Path) -> Result<(), String> {
    let gbiv_md_path = main_repo_path.join("GBIV.md");
    if !gbiv_md_path.exists() {
        fs::write(&gbiv_md_path, GBIV_MD_TEMPLATE)
            .map_err(|e| format!("Failed to write GBIV.md: {}", e))?;
        println!("Created GBIV.md");
    }
    Ok(())
}

// @spec WTL-INIT-011
fn ensure_gbiv_md_in_gitignore(main_repo_path: &Path) -> Result<(), String> {
    let gitignore_path = main_repo_path.join(".gitignore");
    let existing = fs::read_to_string(&gitignore_path).unwrap_or_default();
    let already_listed = existing
        .lines()
        .any(|l| l.trim() == "GBIV.md" || l.trim() == "/GBIV.md");
    if already_listed {
        return Ok(());
    }
    let mut new_contents = existing.clone();
    if !new_contents.is_empty() && !new_contents.ends_with('\n') {
        new_contents.push('\n');
    }
    new_contents.push_str("GBIV.md\n");
    fs::write(&gitignore_path, new_contents)
        .map_err(|e| format!("Failed to update .gitignore: {}", e))?;
    println!("Added GBIV.md to .gitignore");
    Ok(())
}

fn check_color_branches(path: &Path) -> Vec<String> {
    let branches = get_existing_branches(path);
    COLORS
        .iter()
        .filter(|c| branches.contains(&c.to_string()))
        .map(|c| c.to_string())
        .collect()
}

// @spec WTL-INIT-001 through WTL-INIT-011
pub fn init_command(folder: &str) -> Result<(), String> {
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

    println!(
        "Initializing '{}' with ROYGBIV worktree structure...",
        folder
    );

    let temp_name = format!("{}_gbiv_temp", folder);
    fs::rename(folder, &temp_name)
        .map_err(|e| format!("Failed to move folder temporarily: {}", e))?;

    let rollback = |temp: &str, target: &str| {
        let _ = fs::remove_dir_all(target);
        let _ = fs::rename(temp, target);
    };

    if let Err(e) = fs::create_dir_all(format!("{}/main", folder)) {
        rollback(&temp_name, folder);
        return Err(format!("Failed to create main directory: {}", e));
    }

    if let Err(e) = fs::rename(&temp_name, format!("{}/main/{}", folder, folder)) {
        rollback(&temp_name, folder);
        return Err(format!("Failed to move repo to main: {}", e));
    }

    let main_repo_path = format!("{}/main/{}", folder, folder);

    let rollback_after_move = |folder: &str| {
        for color in COLORS {
            let worktree_path = format!("{}/{}/{}", folder, color, folder);
            if Path::new(&worktree_path).exists() {
                let _ = Command::new("git")
                    .args(["worktree", "remove", "--force", &worktree_path])
                    .current_dir(format!("{}/main/{}", folder, folder))
                    .output();
            }
            let _ = fs::remove_dir_all(format!("{}/{}", folder, color));
        }
        let main_repo = format!("{}/main/{}", folder, folder);
        let _ = fs::rename(&main_repo, folder);
        let _ = fs::remove_dir_all(format!("{}/main", folder));
    };

    for color in COLORS {
        let worktree_path = format!("../../{}/{}", color, folder);
        let output = Command::new("git")
            .args(["worktree", "add", "-b", color, &worktree_path, &main_branch])
            .current_dir(&main_repo_path)
            .output();

        match output {
            Ok(o) if o.status.success() => {
                println!("Created worktree: {}", color);
            }
            Ok(o) => {
                let err_msg = String::from_utf8_lossy(&o.stderr);
                rollback_after_move(folder);
                return Err(format!(
                    "Failed to create worktree for {}: {}",
                    color, err_msg
                ));
            }
            Err(e) => {
                rollback_after_move(folder);
                return Err(format!(
                    "Failed to run git worktree add for {}: {}",
                    color, e
                ));
            }
        }
    }

    write_gbiv_md_if_absent(Path::new(&main_repo_path))?;
    ensure_gbiv_md_in_gitignore(Path::new(&main_repo_path))?;

    println!(
        "Successfully initialized '{}' with ROYGBIV worktrees!",
        folder
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;
    use std::fs;
    use std::process::Command;

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
        Command::new("git")
            .args(["init"])
            .current_dir(path)
            .output()
            .unwrap();
        Command::new("git")
            .args(["config", "user.email", "test@test.com"])
            .current_dir(path)
            .output()
            .unwrap();
        Command::new("git")
            .args(["config", "user.name", "Test"])
            .current_dir(path)
            .output()
            .unwrap();
    }

    fn add_commit(path: &str) {
        fs::write(format!("{}/test.txt", path), "test").unwrap();
        Command::new("git")
            .args(["add", "."])
            .current_dir(path)
            .output()
            .unwrap();
        Command::new("git")
            .args(["commit", "-m", "initial"])
            .current_dir(path)
            .output()
            .unwrap();
    }

    // @spec WTL-INIT-002
    #[test]
    #[serial]
    fn test_is_git_repo_true() {
        let test_dir = setup_test_dir("is_git_repo_true");
        init_git_repo(&test_dir);
        assert!(is_git_repo(Path::new(&test_dir)));
        cleanup_test_dir(&test_dir);
    }

    // @spec WTL-INIT-002
    #[test]
    fn test_is_git_repo_false() {
        let test_dir = setup_test_dir("is_git_repo_false");
        assert!(!is_git_repo(Path::new(&test_dir)));
        cleanup_test_dir(&test_dir);
    }

    // @spec WTL-INIT-003
    #[test]
    #[serial]
    fn test_has_commits_true() {
        let test_dir = setup_test_dir("has_commits_true");
        init_git_repo(&test_dir);
        add_commit(&test_dir);
        assert!(has_commits(Path::new(&test_dir)));
        cleanup_test_dir(&test_dir);
    }

    // @spec WTL-INIT-003
    #[test]
    #[serial]
    fn test_has_commits_false() {
        let test_dir = setup_test_dir("has_commits_false");
        init_git_repo(&test_dir);
        assert!(!has_commits(Path::new(&test_dir)));
        cleanup_test_dir(&test_dir);
    }

    // @spec WTL-INIT-006
    #[test]
    #[serial]
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

    // @spec WTL-INIT-004
    #[test]
    #[serial]
    fn test_check_color_branches_none() {
        let test_dir = setup_test_dir("color_branches_none");
        init_git_repo(&test_dir);
        add_commit(&test_dir);
        let conflicts = check_color_branches(Path::new(&test_dir));
        assert!(conflicts.is_empty());
        cleanup_test_dir(&test_dir);
    }

    // @spec WTL-INIT-004
    #[test]
    #[serial]
    fn test_check_color_branches_conflict() {
        let test_dir = setup_test_dir("color_branches_conflict");
        init_git_repo(&test_dir);
        add_commit(&test_dir);
        Command::new("git")
            .args(["branch", "red"])
            .current_dir(&test_dir)
            .output()
            .unwrap();
        let conflicts = check_color_branches(Path::new(&test_dir));
        assert_eq!(conflicts, vec!["red"]);
        cleanup_test_dir(&test_dir);
    }

    // @spec WTL-INIT-001
    #[test]
    fn test_init_command_folder_not_exist() {
        let result = init_command("nonexistent_folder_xyz");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("does not exist"));
    }

    // @spec WTL-INIT-002
    #[test]
    fn test_init_command_not_git_repo() {
        let test_dir = setup_test_dir("not_git_repo");
        let result = init_command(&test_dir);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not a git repository"));
        cleanup_test_dir(&test_dir);
    }

    // @spec WTL-INIT-003
    #[test]
    #[serial]
    fn test_init_command_no_commits() {
        let test_dir = setup_test_dir("no_commits");
        init_git_repo(&test_dir);
        let result = init_command(&test_dir);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("no commits"));
        cleanup_test_dir(&test_dir);
    }

    // @spec WTL-INIT-004
    #[test]
    #[serial]
    fn test_init_command_color_branch_conflict() {
        let test_dir = setup_test_dir("branch_conflict");
        init_git_repo(&test_dir);
        add_commit(&test_dir);
        Command::new("git")
            .args(["branch", "blue"])
            .current_dir(&test_dir)
            .output()
            .unwrap();
        let result = init_command(&test_dir);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("blue"));
        cleanup_test_dir(&test_dir);
    }

    // @spec WTL-INIT-005, WTL-INIT-006
    #[test]
    #[serial]
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

    // @spec WTL-INIT-007
    #[test]
    fn test_write_gbiv_md_creates_file_with_correct_content() {
        let dir = setup_test_dir("write_gbiv_md_create");
        write_gbiv_md_if_absent(Path::new(&dir)).unwrap();

        let gbiv_md = Path::new(&dir).join("GBIV.md");
        assert!(gbiv_md.exists(), "GBIV.md should be created");

        let content = fs::read_to_string(&gbiv_md).unwrap();
        assert!(content.starts_with("\n\n\n\n\n\n\n\n\n\n---\n"), "file should start with 10 newlines then ---");
        assert!(content.contains("# GBIV.md"), "file should contain usage header");
        assert!(content.contains("Add features above the `---` line"), "file should contain instructions");
        assert!(content.contains("- [red] My urgent feature"), "file should contain example");
        assert!(content.contains("Everything below `---` is ignored by gbiv."), "file should end with ignore note");

        cleanup_test_dir(&dir);
    }

    // @spec WTL-INIT-011
    #[test]
    fn test_ensure_gbiv_md_in_gitignore_creates_file() {
        let dir = setup_test_dir("gitignore_create");
        ensure_gbiv_md_in_gitignore(Path::new(&dir)).unwrap();
        let content = fs::read_to_string(Path::new(&dir).join(".gitignore")).unwrap();
        assert_eq!(content, "GBIV.md\n");
        cleanup_test_dir(&dir);
    }

    // @spec WTL-INIT-011
    #[test]
    fn test_ensure_gbiv_md_in_gitignore_appends_when_missing() {
        let dir = setup_test_dir("gitignore_append");
        fs::write(Path::new(&dir).join(".gitignore"), "target\n").unwrap();
        ensure_gbiv_md_in_gitignore(Path::new(&dir)).unwrap();
        let content = fs::read_to_string(Path::new(&dir).join(".gitignore")).unwrap();
        assert_eq!(content, "target\nGBIV.md\n");
        cleanup_test_dir(&dir);
    }

    // @spec WTL-INIT-011
    #[test]
    fn test_ensure_gbiv_md_in_gitignore_appends_newline_when_no_trailing() {
        let dir = setup_test_dir("gitignore_no_trailing");
        fs::write(Path::new(&dir).join(".gitignore"), "target").unwrap();
        ensure_gbiv_md_in_gitignore(Path::new(&dir)).unwrap();
        let content = fs::read_to_string(Path::new(&dir).join(".gitignore")).unwrap();
        assert_eq!(content, "target\nGBIV.md\n");
        cleanup_test_dir(&dir);
    }

    // @spec WTL-INIT-011
    #[test]
    fn test_ensure_gbiv_md_in_gitignore_noop_when_already_listed() {
        let dir = setup_test_dir("gitignore_already_listed");
        fs::write(Path::new(&dir).join(".gitignore"), "target\nGBIV.md\nfoo\n").unwrap();
        ensure_gbiv_md_in_gitignore(Path::new(&dir)).unwrap();
        let content = fs::read_to_string(Path::new(&dir).join(".gitignore")).unwrap();
        assert_eq!(content, "target\nGBIV.md\nfoo\n");
        cleanup_test_dir(&dir);
    }

    // @spec WTL-INIT-008
    #[test]
    fn test_write_gbiv_md_does_not_overwrite_existing() {
        let dir = setup_test_dir("write_gbiv_md_no_overwrite");
        let existing_content = "my existing content";
        fs::write(Path::new(&dir).join("GBIV.md"), existing_content).unwrap();

        write_gbiv_md_if_absent(Path::new(&dir)).unwrap();

        let content = fs::read_to_string(Path::new(&dir).join("GBIV.md")).unwrap();
        assert_eq!(content, existing_content, "existing GBIV.md should not be overwritten");

        cleanup_test_dir(&dir);
    }
}

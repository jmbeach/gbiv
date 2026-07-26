use std::path::Path;
use std::process::Command;

use gbiv_core::palette::Palette;
use gbiv_core::root::{find_gbiv_root, find_repo_in_worktree};

use crate::git_utils::{get_existing_branches, get_main_branch};

enum RepairOutcome {
    Present,
    Created,
    Attached,
    Broken,
    Failed(String),
}

// @spec WTL-REPAIR-001 through WTL-REPAIR-012
pub fn repair_command() -> anyhow::Result<()> {
    let cwd = std::env::current_dir()?;
    repair_from(&cwd)
}

// @spec WTL-REPAIR-001, WTL-REPAIR-002, WTL-REPAIR-003, WTL-REPAIR-004, WTL-REPAIR-005, WTL-REPAIR-006, WTL-REPAIR-007, WTL-REPAIR-008, WTL-REPAIR-009, WTL-REPAIR-010, WTL-REPAIR-011, WTL-REPAIR-012
fn repair_from(cwd: &Path) -> anyhow::Result<()> {
    // WTL-REPAIR-001: locate the gbiv root and the main repo.
    let gbiv_root = find_gbiv_root(cwd)
        .ok_or_else(|| anyhow::anyhow!("Not in a gbiv-structured repository"))?;
    let main_repo = find_repo_in_worktree(&gbiv_root.root.join("main"))
        .ok_or_else(|| anyhow::anyhow!("Could not find git repo in main worktree"))?;

    // WTL-REPAIR-002: load the active palette; a bad config aborts before any creation.
    let palette = Palette::load(&gbiv_root.root)?;

    // WTL-REPAIR-003: detect the local main branch name (as init does).
    let main_branch = get_main_branch(&main_repo)
        .ok_or_else(|| anyhow::anyhow!("Could not determine main branch name"))?;

    let existing_branches = get_existing_branches(&main_repo);
    let folder = &gbiv_root.folder_name;

    let mut created = 0u32;
    let mut failed = 0u32;

    // WTL-REPAIR-008: process names sequentially in canonical palette order.
    for name in palette.names() {
        let worktree_dir = gbiv_root.root.join(name);
        let has_repo = find_repo_in_worktree(&worktree_dir).is_some();
        // A non-empty directory with no git repo is "broken"; an empty or absent
        // directory (e.g. a leftover parent after `git worktree remove`) is
        // treated as missing and (re)created.
        let dir_nonempty = worktree_dir
            .read_dir()
            .map(|mut entries| entries.next().is_some())
            .unwrap_or(false);
        let outcome = if has_repo {
            // WTL-REPAIR-004: already present.
            RepairOutcome::Present
        } else if dir_nonempty {
            // WTL-REPAIR-007: directory exists but has no git repo.
            RepairOutcome::Broken
        } else {
            let worktree_path = format!("../../{}/{}", name, folder);
            let branch_exists = existing_branches.iter().any(|b| b == name);
            let output = if branch_exists {
                // WTL-REPAIR-006: attach the pre-existing branch (no -b).
                Command::new("git")
                    .args(["worktree", "add", &worktree_path, name])
                    .current_dir(&main_repo)
                    .output()
            } else {
                // WTL-REPAIR-005: create a fresh branch from local main.
                Command::new("git")
                    .args(["worktree", "add", "-b", name, &worktree_path, &main_branch])
                    .current_dir(&main_repo)
                    .output()
            };
            match output {
                Ok(o) if o.status.success() => {
                    if branch_exists {
                        RepairOutcome::Attached
                    } else {
                        RepairOutcome::Created
                    }
                }
                Ok(o) => {
                    RepairOutcome::Failed(String::from_utf8_lossy(&o.stderr).trim().to_string())
                }
                Err(e) => RepairOutcome::Failed(e.to_string()),
            }
        };

        // WTL-REPAIR-009: per-name reporting.
        match &outcome {
            RepairOutcome::Present => println!("{:<14} present", name),
            RepairOutcome::Created => {
                created += 1;
                println!("{:<14} created", name);
            }
            RepairOutcome::Attached => {
                created += 1;
                println!("{:<14} created (attached existing branch)", name);
            }
            RepairOutcome::Broken => {
                println!(
                    "{:<14} broken (directory exists but has no git repo — needs attention)",
                    name
                );
            }
            RepairOutcome::Failed(e) => {
                failed += 1;
                println!("{:<14} failed: {}", name, e);
            }
        }
    }

    println!();
    println!("{} created, {} failed", created, failed);

    // WTL-REPAIR-010: non-zero exit if any creation failed.
    if failed > 0 {
        Err(anyhow::anyhow!("{} worktree(s) failed to create", failed))
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gbiv_core::colors::BASE_COLORS;
    use gbiv_core::root::find_repo_in_worktree;
    use serial_test::serial;
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

    /// Build a gbiv layout with a main repo (one commit) and the seven base
    /// worktrees. Returns (root, main_repo).
    fn setup_gbiv(base: &Path, folder: &str) -> (PathBuf, PathBuf) {
        let root = base.join(folder);
        let main_repo = root.join("main").join(folder);
        fs::create_dir_all(&main_repo).unwrap();
        git(&["init"], &main_repo);
        git(&["config", "user.email", "t@t.com"], &main_repo);
        git(&["config", "user.name", "T"], &main_repo);
        git(&["config", "gc.auto", "0"], &main_repo);
        fs::write(main_repo.join("f.txt"), "x").unwrap();
        git(&["add", "."], &main_repo);
        git(&["commit", "-m", "init"], &main_repo);
        for color in BASE_COLORS {
            let wt = format!("../../{}/{}", color, folder);
            git(&["worktree", "add", "-b", color, &wt, "HEAD"], &main_repo);
        }
        (root, main_repo)
    }

    fn write_palette_config(root: &Path, extras: &[&str]) {
        let dir = root.join(".gbiv");
        fs::create_dir_all(&dir).unwrap();
        let list = extras
            .iter()
            .map(|e| format!("\"{}\"", e))
            .collect::<Vec<_>>()
            .join(", ");
        fs::write(
            dir.join("config.toml"),
            format!("[palette]\nextra = [{}]\n", list),
        )
        .unwrap();
    }

    // @spec WTL-REPAIR-005
    #[test]
    #[serial]
    fn repair_recreates_a_deleted_base_worktree() {
        let base = TempDir::new().unwrap();
        let (root, main_repo) = setup_gbiv(base.path(), "proj");

        // Remove the green worktree (both the working dir and git's registration).
        git(
            &["worktree", "remove", "--force", "../../green/proj"],
            &main_repo,
        );
        assert!(find_repo_in_worktree(&root.join("green")).is_none());

        repair_from(&root).unwrap();

        assert!(
            find_repo_in_worktree(&root.join("green")).is_some(),
            "green worktree should be restored"
        );
    }

    // @spec WTL-REPAIR-005
    #[test]
    #[serial]
    fn repair_creates_configured_extra_worktree() {
        let base = TempDir::new().unwrap();
        let (root, _main_repo) = setup_gbiv(base.path(), "proj");
        write_palette_config(&root, &["my-extra"]);

        repair_from(&root).unwrap();

        assert!(
            find_repo_in_worktree(&root.join("my-extra")).is_some(),
            "configured extra worktree should be created"
        );
    }

    // @spec WTL-REPAIR-004
    #[test]
    #[serial]
    fn repair_is_idempotent_when_all_present() {
        let base = TempDir::new().unwrap();
        let (root, _main_repo) = setup_gbiv(base.path(), "proj");

        // All base worktrees already exist; repair should succeed as a no-op.
        repair_from(&root).unwrap();
        for color in BASE_COLORS {
            assert!(find_repo_in_worktree(&root.join(color)).is_some());
        }
    }

    // @spec WTL-REPAIR-006
    #[test]
    #[serial]
    fn repair_attaches_a_preexisting_branch() {
        let base = TempDir::new().unwrap();
        let (root, main_repo) = setup_gbiv(base.path(), "proj");

        // A branch named after the extra already exists, but no worktree for it.
        git(&["branch", "amber"], &main_repo);
        write_palette_config(&root, &["amber"]);

        repair_from(&root).unwrap();

        let amber_repo =
            find_repo_in_worktree(&root.join("amber")).expect("amber worktree should be attached");
        let head = Cmd::new("git")
            .args(["rev-parse", "--abbrev-ref", "HEAD"])
            .current_dir(&amber_repo)
            .output()
            .unwrap();
        assert_eq!(String::from_utf8_lossy(&head.stdout).trim(), "amber");
    }

    // @spec WTL-REPAIR-002
    #[test]
    #[serial]
    fn repair_aborts_on_invalid_config() {
        let base = TempDir::new().unwrap();
        let (root, _main_repo) = setup_gbiv(base.path(), "proj");
        // "main" is a reserved name — config is invalid.
        write_palette_config(&root, &["main"]);

        assert!(
            repair_from(&root).is_err(),
            "repair should abort on bad config"
        );
    }
}

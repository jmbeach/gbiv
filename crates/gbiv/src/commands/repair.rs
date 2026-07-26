use std::path::Path;
use std::process::Command;

use gbiv_core::palette::Palette;
use gbiv_core::root::{classify_worktree, find_gbiv_root, GbivRoot, WorktreePresence};

use crate::git_utils::{get_existing_branches, get_main_branch};

enum RepairOutcome {
    Present,
    Created,
    Attached,
    Broken,
    Failed(String),
}

/// The result of a repair pass: the per-name report lines plus counts. Kept
/// separate from printing so the counts and exit decision are unit-testable.
#[derive(Default)]
struct RepairReport {
    lines: Vec<String>,
    created: u32,
    broken: u32,
    failed: u32,
}

// @spec WTL-REPAIR-001 through WTL-REPAIR-012
pub fn repair_command() -> anyhow::Result<()> {
    let cwd = std::env::current_dir()?;
    repair_from(&cwd)
}

// @spec WTL-REPAIR-001, WTL-REPAIR-009, WTL-REPAIR-010
fn repair_from(cwd: &Path) -> anyhow::Result<()> {
    // WTL-REPAIR-001: locate the gbiv root.
    let gbiv_root = find_gbiv_root(cwd)
        .ok_or_else(|| anyhow::anyhow!("Not in a gbiv-structured repository"))?;

    let report = repair_report(&gbiv_root)?;

    // WTL-REPAIR-009: per-name reporting followed by a summary count.
    for line in &report.lines {
        println!("{}", line);
    }
    println!();
    println!(
        "{} created, {} broken, {} failed",
        report.created, report.broken, report.failed
    );

    // WTL-REPAIR-010: a broken or failed worktree means repair could not make the
    // palette whole, so exit non-zero rather than reporting misleading success.
    if report.failed > 0 || report.broken > 0 {
        Err(anyhow::anyhow!(
            "repair incomplete: {} broken, {} failed",
            report.broken,
            report.failed
        ))
    } else {
        Ok(())
    }
}

// @spec WTL-REPAIR-002 through WTL-REPAIR-012
/// Reconcile the on-disk worktrees to the active palette, returning the report
/// (per-name lines + counts) without printing. Resolve-time problems (no main
/// repo, bad config, no main branch) are hard errors that create nothing.
fn repair_report(gbiv_root: &GbivRoot) -> anyhow::Result<RepairReport> {
    // WTL-REPAIR-001: locate the main repo inside the `main/` worktree.
    let main_repo = classify_present(gbiv_root, "main")
        .ok_or_else(|| anyhow::anyhow!("Could not find git repo in main worktree"))?;

    // WTL-REPAIR-002: load the active palette; a bad config aborts before any creation.
    let palette = Palette::load(&gbiv_root.root)?;

    // WTL-REPAIR-003: detect the local main branch name (as init does).
    let main_branch = get_main_branch(&main_repo)
        .ok_or_else(|| anyhow::anyhow!("Could not determine main branch name"))?;

    let existing_branches = get_existing_branches(&main_repo);
    let folder = &gbiv_root.folder_name;

    let mut report = RepairReport::default();

    // WTL-REPAIR-008: process names sequentially in canonical palette order.
    for name in palette.names() {
        // WTL-REPAIR-004/007: classify the slot with the shared definition used by
        // `status`, so the two never disagree about present/missing/broken.
        let outcome = match classify_worktree(&gbiv_root.root, name) {
            WorktreePresence::Present(_) => RepairOutcome::Present,
            WorktreePresence::Broken => RepairOutcome::Broken,
            WorktreePresence::Missing => {
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
            }
        };

        // WTL-REPAIR-009: per-name reporting.
        match &outcome {
            RepairOutcome::Present => report.lines.push(format!("{:<14} present", name)),
            RepairOutcome::Created => {
                report.created += 1;
                report.lines.push(format!("{:<14} created", name));
            }
            RepairOutcome::Attached => {
                report.created += 1;
                report
                    .lines
                    .push(format!("{:<14} created (attached existing branch)", name));
            }
            RepairOutcome::Broken => {
                report.broken += 1;
                report.lines.push(format!(
                    "{:<14} broken (directory exists but has no git repo — needs attention)",
                    name
                ));
            }
            RepairOutcome::Failed(e) => {
                report.failed += 1;
                report.lines.push(format!("{:<14} failed: {}", name, e));
            }
        }
    }

    Ok(report)
}

/// Helper: the repo path of a Present worktree slot, else None.
fn classify_present(gbiv_root: &GbivRoot, name: &str) -> Option<std::path::PathBuf> {
    match classify_worktree(&gbiv_root.root, name) {
        WorktreePresence::Present(repo) => Some(repo),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gbiv_core::colors::BASE_COLORS;
    use gbiv_core::root::{find_gbiv_root, find_repo_in_worktree};
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
    fn repair_creates_configured_extra_worktree_on_a_fresh_branch() {
        let base = TempDir::new().unwrap();
        let (root, main_repo) = setup_gbiv(base.path(), "proj");
        write_palette_config(&root, &["my-extra"]);

        repair_from(&root).unwrap();

        let extra_repo = find_repo_in_worktree(&root.join("my-extra"))
            .expect("configured extra worktree should be created");

        // WTL-REPAIR-005 is specifically the *fresh branch* path (no pre-existing
        // branch): the new worktree must be on a new branch named after the extra,
        // pointing at the same commit as main.
        let head = Cmd::new("git")
            .args(["rev-parse", "--abbrev-ref", "HEAD"])
            .current_dir(&extra_repo)
            .output()
            .unwrap();
        assert_eq!(String::from_utf8_lossy(&head.stdout).trim(), "my-extra");

        let extra_commit = Cmd::new("git")
            .args(["rev-parse", "my-extra"])
            .current_dir(&main_repo)
            .output()
            .unwrap();
        let main_commit = Cmd::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(&main_repo)
            .output()
            .unwrap();
        assert_eq!(
            String::from_utf8_lossy(&extra_commit.stdout).trim(),
            String::from_utf8_lossy(&main_commit.stdout).trim(),
            "fresh extra branch should start at main's HEAD"
        );
    }

    // @spec WTL-REPAIR-007, WTL-REPAIR-009, WTL-REPAIR-010
    #[test]
    #[serial]
    fn repair_reports_broken_worktree_and_exits_nonzero() {
        let base = TempDir::new().unwrap();
        let (root, main_repo) = setup_gbiv(base.path(), "proj");

        // Remove green's worktree, then leave a stray file in the directory so it
        // is non-empty but has no git repo — the "broken" case.
        git(
            &["worktree", "remove", "--force", "../../green/proj"],
            &main_repo,
        );
        fs::create_dir_all(root.join("green")).unwrap();
        fs::write(root.join("green").join("stray.txt"), "leftover").unwrap();

        // The report must count it broken and NOT recreate a repo over it.
        let report = repair_report(&find_gbiv_root(&root).unwrap()).unwrap();
        assert_eq!(report.broken, 1, "green should be reported broken");
        assert_eq!(report.failed, 0);
        assert!(
            find_repo_in_worktree(&root.join("green")).is_none(),
            "repair must not overwrite a broken directory"
        );

        // A broken worktree makes the whole command exit non-zero.
        assert!(
            repair_from(&root).is_err(),
            "repair should exit non-zero when a worktree is broken"
        );
    }

    // @spec WTL-REPAIR-010
    #[test]
    #[serial]
    fn repair_counts_creation_failure_and_exits_nonzero() {
        let base = TempDir::new().unwrap();
        let (root, main_repo) = setup_gbiv(base.path(), "proj");

        // Create a branch named "amber" and check it out in a *separate* worktree,
        // so the branch is occupied. Repair will then try to attach "amber" for the
        // configured extra and git will refuse (already checked out elsewhere).
        git(&["branch", "amber"], &main_repo);
        git(
            &["worktree", "add", "../../amber-held/proj", "amber"],
            &main_repo,
        );
        write_palette_config(&root, &["amber"]);

        let report = repair_report(&find_gbiv_root(&root).unwrap()).unwrap();
        assert_eq!(report.failed, 1, "amber attach should fail");
        assert!(
            find_repo_in_worktree(&root.join("amber")).is_none(),
            "failed worktree should not exist at the palette path"
        );

        assert!(
            repair_from(&root).is_err(),
            "repair should exit non-zero when a creation fails"
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

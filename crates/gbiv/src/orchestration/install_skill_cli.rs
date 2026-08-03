//! `gbiv install-skill`: the real I/O glue — destination resolution via
//! `HOME` (user scope) or `core::find_gbiv_root` (project scope), reading the
//! existing destination file (if any), writing the bundled `SKILL.md`, and
//! `info`-level logging. Delegates the actual idempotency decision to
//! `install_skill_client` (pure, unit-tested there) so this module only has
//! to wire real dependencies together.
//!
//! Callers (the `gbiv install-skill` clap arm in `main.rs`) get an `Outcome`
//! back — this module never prints or calls `process::exit` itself.

use std::path::{Path, PathBuf};

use gbiv_core::root::find_gbiv_root;

use super::fleet_client::{Outcome, EXIT_OTHER};
use super::install_skill_client::{
    decide_action, Action, InstallResult, Scope, EXIT_NOT_A_GBIV_WORKSPACE,
};

/// The bundled `SKILL.md` content, embedded at compile time from the
/// workspace-root `skills/gbiv-orchestrate/SKILL.md` (INSTALL-CLI-002).
pub const BUNDLED_SKILL_MD: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../skills/gbiv-orchestrate/SKILL.md"
));

/// The version this binary ships, compared against any on-disk `version:`
/// frontmatter field (INSTALL-CLI-024 through INSTALL-CLI-028).
pub const BUNDLED_VERSION: &str = env!("CARGO_PKG_VERSION");

const SKILL_DIR_NAME: &str = "gbiv-orchestrate";
const SKILL_FILE_NAME: &str = "SKILL.md";

// ---- Destination resolution (INSTALL-CLI-010 through INSTALL-CLI-013) -----

/// Resolve the directory `SKILL.md` belongs in for the given scope.
/// INSTALL-CLI-010: user scope is `$HOME/.claude/skills/gbiv-orchestrate/`.
/// INSTALL-CLI-011: project scope is `<gbiv-root>/.claude/skills/gbiv-orchestrate/`.
// @spec INSTALL-CLI-010, INSTALL-CLI-011, INSTALL-CLI-012, INSTALL-CLI-013
pub fn resolve_destination_dir(cwd: &Path, scope: Scope) -> Result<PathBuf, Outcome> {
    match scope {
        Scope::User => {
            let home = std::env::var("HOME").map_err(|_| {
                Outcome::err(EXIT_OTHER, "could not resolve HOME for --scope user")
            })?;
            Ok(PathBuf::from(home)
                .join(".claude")
                .join("skills")
                .join(SKILL_DIR_NAME))
        }
        Scope::Project => {
            let gbiv_root = find_gbiv_root(cwd).ok_or_else(|| {
                Outcome::err(EXIT_NOT_A_GBIV_WORKSPACE, "not inside a gbiv project")
            })?;
            Ok(gbiv_root
                .root
                .join(".claude")
                .join("skills")
                .join(SKILL_DIR_NAME))
        }
    }
}

// ---- gbiv install-skill (INSTALL-CLI-020 through INSTALL-CLI-032) ---------

// @spec INSTALL-CLI-020, INSTALL-CLI-021, INSTALL-CLI-022, INSTALL-CLI-023,
// INSTALL-CLI-024, INSTALL-CLI-025, INSTALL-CLI-026, INSTALL-CLI-027,
// INSTALL-CLI-028, INSTALL-CLI-030, INSTALL-CLI-031, INSTALL-CLI-032,
// INSTALL-CLI-040, INSTALL-CLI-041, INSTALL-CLI-042, INSTALL-CLI-043,
// INSTALL-CLI-050
pub fn run_install_skill(cwd: &Path, scope: Scope, force: bool) -> Outcome {
    let dest_dir = match resolve_destination_dir(cwd, scope) {
        Ok(d) => d,
        Err(outcome) => return log_and_return(outcome),
    };
    let dest_file = dest_dir.join(SKILL_FILE_NAME);
    tracing::info!(destination = %dest_file.display(), "resolved install-skill destination");

    let existing_content = match std::fs::read_to_string(&dest_file) {
        Ok(content) => Some(content),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => {
            return log_and_return(Outcome::err(
                EXIT_OTHER,
                format!("failed to read {}: {e}", dest_file.display()),
            ))
        }
    };

    let (action, reason, previous_version) = decide_action(
        existing_content.as_deref(),
        BUNDLED_SKILL_MD,
        BUNDLED_VERSION,
        force,
    );

    if matches!(action, Action::Installed | Action::Updated) {
        if let Err(e) = std::fs::create_dir_all(&dest_dir) {
            return log_and_return(Outcome::err(
                EXIT_OTHER,
                format!("failed to create {}: {e}", dest_dir.display()),
            ));
        }
        if let Err(e) = std::fs::write(&dest_file, BUNDLED_SKILL_MD) {
            return log_and_return(Outcome::err(
                EXIT_OTHER,
                format!("failed to write {}: {e}", dest_file.display()),
            ));
        }
    }

    let result = InstallResult {
        scope: match scope {
            Scope::User => "user",
            Scope::Project => "project",
        },
        destination: dest_file.display().to_string(),
        action,
        bundled_version: BUNDLED_VERSION.to_string(),
        previous_version,
        reason,
    };
    log_and_return(result.to_outcome())
}

/// INSTALL-CLI-051: log the final exit code at `info` immediately before
/// returning, for any non-zero outcome (mirrors `fleet_cli`'s convention).
// @spec INSTALL-CLI-051
fn log_and_return(outcome: Outcome) -> Outcome {
    if outcome.exit_code != super::fleet_client::EXIT_OK {
        tracing::info!(
            exit_code = outcome.exit_code,
            "gbiv install-skill: exiting non-zero"
        );
    }
    outcome
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::orchestration::install_skill_client::parse_frontmatter_version;
    use std::fs;
    use std::process::Command;
    use tempfile::TempDir;

    fn init_git_repo(path: &Path) {
        Command::new("git")
            .args(["init", "-q"])
            .current_dir(path)
            .output()
            .unwrap();
    }

    /// Build a minimal gbiv-shaped layout under `base/<project>` and return
    /// (project_root, main_repo_path). Mirrors `fleet_cli.rs`'s own helper.
    fn setup_gbiv_layout(base: &Path, project: &str) -> (PathBuf, PathBuf) {
        let project_root = base.join(project);
        let main_repo = project_root.join("main").join(project);
        fs::create_dir_all(&main_repo).unwrap();
        init_git_repo(&main_repo);
        fs::create_dir_all(project_root.join("red")).unwrap();
        (project_root, main_repo)
    }

    // ---- resolve_destination_dir (INSTALL-CLI-010, -011) ---------------------

    #[test]
    // @spec INSTALL-CLI-010
    fn user_scope_resolves_under_home_dot_claude_skills() {
        let home = TempDir::new().unwrap();
        std::env::set_var("HOME", home.path());
        let dir = resolve_destination_dir(home.path(), Scope::User).unwrap();
        assert_eq!(
            dir,
            home.path().join(".claude").join("skills").join("gbiv-orchestrate")
        );
    }

    #[test]
    // @spec INSTALL-CLI-011
    fn project_scope_resolves_under_gbiv_root_dot_claude_skills() {
        let tmp = TempDir::new().unwrap();
        let (project_root, main_repo) = setup_gbiv_layout(tmp.path(), "proj");
        let dir = resolve_destination_dir(&main_repo, Scope::Project).unwrap();
        assert_eq!(
            dir,
            project_root
                .join(".claude")
                .join("skills")
                .join("gbiv-orchestrate")
        );
    }

    #[test]
    // @spec INSTALL-CLI-011, INSTALL-CLI-042
    fn project_scope_fails_outside_a_gbiv_project() {
        let tmp = TempDir::new().unwrap();
        let outcome = resolve_destination_dir(tmp.path(), Scope::Project).unwrap_err();
        assert_eq!(outcome.exit_code, EXIT_NOT_A_GBIV_WORKSPACE);
    }

    #[test]
    // @spec INSTALL-CLI-010, INSTALL-CLI-032, INSTALL-CLI-041
    fn user_scope_fails_when_home_is_unset() {
        let previous = std::env::var("HOME").ok();
        std::env::remove_var("HOME");

        let outcome = resolve_destination_dir(Path::new("/irrelevant"), Scope::User).unwrap_err();

        if let Some(home) = previous {
            std::env::set_var("HOME", home);
        }

        assert_eq!(outcome.exit_code, EXIT_OTHER);
        assert_eq!(outcome.stdout, None, "hard failures print nothing to stdout");
        assert!(outcome.stderr.unwrap().contains("HOME"));
    }

    // ---- run_install_skill end-to-end (INSTALL-CLI-020 through -032) --------

    #[test]
    // @spec INSTALL-CLI-020, INSTALL-CLI-040
    fn fresh_install_writes_file_and_reports_installed() {
        let tmp = TempDir::new().unwrap();
        let (_root, main_repo) = setup_gbiv_layout(tmp.path(), "proj");

        let outcome = run_install_skill(&main_repo, Scope::Project, false);

        assert_eq!(outcome.exit_code, 0);
        let stdout = outcome.stdout.unwrap();
        assert!(stdout.contains("\"action\":\"installed\""));

        let written = fs::read_to_string(
            tmp.path()
                .join("proj")
                .join(".claude")
                .join("skills")
                .join("gbiv-orchestrate")
                .join("SKILL.md"),
        )
        .unwrap();
        assert_eq!(written, BUNDLED_SKILL_MD);
    }

    #[test]
    // @spec INSTALL-CLI-021
    fn second_run_with_identical_content_is_unchanged() {
        let tmp = TempDir::new().unwrap();
        let (_root, main_repo) = setup_gbiv_layout(tmp.path(), "proj");

        run_install_skill(&main_repo, Scope::Project, false);
        let outcome = run_install_skill(&main_repo, Scope::Project, false);

        assert_eq!(outcome.exit_code, 0);
        assert!(outcome.stdout.unwrap().contains("\"action\":\"unchanged\""));
    }

    #[test]
    // @spec INSTALL-CLI-025, INSTALL-CLI-043
    fn hand_edited_same_version_is_refused_without_force() {
        let tmp = TempDir::new().unwrap();
        let (project_root, main_repo) = setup_gbiv_layout(tmp.path(), "proj");
        let dest_dir = project_root
            .join(".claude")
            .join("skills")
            .join("gbiv-orchestrate");
        fs::create_dir_all(&dest_dir).unwrap();
        fs::write(
            dest_dir.join("SKILL.md"),
            format!(
                "---\nname: gbiv-orchestrate\nversion: {BUNDLED_VERSION}\n---\nhand-edited\n"
            ),
        )
        .unwrap();

        let outcome = run_install_skill(&main_repo, Scope::Project, false);

        assert_eq!(outcome.exit_code, super::super::install_skill_client::EXIT_REFUSED);
        let stdout = outcome.stdout.unwrap();
        assert!(stdout.contains("\"action\":\"refused\""));
        let on_disk = fs::read_to_string(dest_dir.join("SKILL.md")).unwrap();
        assert!(on_disk.contains("hand-edited"), "refusal must not overwrite");
    }

    #[test]
    // @spec INSTALL-CLI-023
    fn force_overwrites_a_hand_edited_file() {
        let tmp = TempDir::new().unwrap();
        let (project_root, main_repo) = setup_gbiv_layout(tmp.path(), "proj");
        let dest_dir = project_root
            .join(".claude")
            .join("skills")
            .join("gbiv-orchestrate");
        fs::create_dir_all(&dest_dir).unwrap();
        fs::write(dest_dir.join("SKILL.md"), "not even frontmatter").unwrap();

        let outcome = run_install_skill(&main_repo, Scope::Project, true);

        assert_eq!(outcome.exit_code, 0);
        assert!(outcome.stdout.unwrap().contains("\"action\":\"updated\""));
        let on_disk = fs::read_to_string(dest_dir.join("SKILL.md")).unwrap();
        assert_eq!(on_disk, BUNDLED_SKILL_MD);
    }

    #[test]
    // @spec INSTALL-CLI-032, INSTALL-CLI-041
    fn read_error_other_than_missing_reports_exit_1() {
        // A destination that is a directory (not a file) triggers a real
        // read error distinct from NotFound, exercising the generic-failure
        // branch rather than the "no existing content" branch.
        let tmp = TempDir::new().unwrap();
        let (project_root, main_repo) = setup_gbiv_layout(tmp.path(), "proj");
        let dest_dir = project_root
            .join(".claude")
            .join("skills")
            .join("gbiv-orchestrate");
        fs::create_dir_all(dest_dir.join("SKILL.md")).unwrap();

        let outcome = run_install_skill(&main_repo, Scope::Project, false);
        assert_eq!(outcome.exit_code, EXIT_OTHER);
        assert_eq!(outcome.stdout, None, "hard failures print nothing to stdout");
        assert!(outcome.stderr.is_some());
    }

    // ---- Bundled SKILL.md content contract (docs/specs/orchestrate-skill.md) -

    #[test]
    // @spec ORCH-SKILL-001
    fn bundled_skill_has_expected_frontmatter_name() {
        assert!(BUNDLED_SKILL_MD.starts_with("---\nname: gbiv-orchestrate\n"));
    }

    #[test]
    // @spec ORCH-SKILL-002
    fn bundled_skill_description_mentions_trigger_terms() {
        let frontmatter = BUNDLED_SKILL_MD
            .split("---")
            .nth(1)
            .expect("frontmatter block");
        for term in [
            "gbiv",
            "worktree",
            "session status",
            "send input",
            "fleet",
        ] {
            assert!(
                frontmatter.contains(term),
                "description missing required term: {term}"
            );
        }
    }

    #[test]
    // @spec ORCH-SKILL-003
    fn bundled_skill_version_matches_cargo_pkg_version() {
        let version = parse_frontmatter_version(BUNDLED_SKILL_MD)
            .expect("bundled SKILL.md must have a parseable version");
        assert_eq!(version, BUNDLED_VERSION);
    }

    #[test]
    // @spec ORCH-SKILL-010
    fn bundled_skill_documents_the_three_primary_commands() {
        for command in ["gbiv fleet status", "gbiv fleet get", "gbiv fleet send"] {
            assert!(BUNDLED_SKILL_MD.contains(command));
        }
    }

    #[test]
    // @spec ORCH-SKILL-011
    fn bundled_skill_has_a_decision_table() {
        assert!(BUNDLED_SKILL_MD.contains("## Decision table"));
    }

    #[test]
    // @spec ORCH-SKILL-012
    fn bundled_skill_enumerates_things_it_will_not_do() {
        let section = BUNDLED_SKILL_MD
            .split("## What gbiv will not do")
            .nth(1)
            .expect("'What gbiv will not do' section");
        for phrase in [
            "prompt-shaped",
            "Auto-start",
            "Auto-install or auto-update",
            "GBIV.md",
        ] {
            assert!(
                section.contains(phrase),
                "'What gbiv will not do' missing: {phrase}"
            );
        }
    }

    #[test]
    // @spec ORCH-SKILL-013
    fn bundled_skill_instructs_declining_to_answer_prompts_on_users_behalf() {
        assert!(BUNDLED_SKILL_MD.contains("Decline"));
        assert!(BUNDLED_SKILL_MD.contains("push back"));
    }

    #[test]
    // @spec ORCH-SKILL-014
    fn bundled_skill_instructs_not_auto_starting_daemon_on_exit_2() {
        assert!(BUNDLED_SKILL_MD.contains("exit code 2"));
        assert!(BUNDLED_SKILL_MD.contains("gbiv start"));
        assert!(BUNDLED_SKILL_MD.contains("do not auto-start"));
    }

    #[test]
    // @spec ORCH-SKILL-015
    fn bundled_skill_instructs_user_runs_install_skill_themselves() {
        assert!(BUNDLED_SKILL_MD.contains("gbiv install-skill"));
        assert!(BUNDLED_SKILL_MD.contains("do not run it for them"));
    }

    #[test]
    // @spec ORCH-SKILL-016
    fn bundled_skill_instructs_surfacing_refusal_reason_verbatim() {
        assert!(BUNDLED_SKILL_MD.contains("exited 7"));
        assert!(BUNDLED_SKILL_MD.contains("`reason` field verbatim"));
    }

    #[test]
    // @spec ORCH-SKILL-017
    fn bundled_skill_instructs_reading_explanation_on_exit_6() {
        assert!(BUNDLED_SKILL_MD.contains("exits 6"));
        assert!(BUNDLED_SKILL_MD.contains("`explanation` field"));
    }
}

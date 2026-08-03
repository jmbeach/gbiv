//! `gbiv install-skill`: pure idempotency-decision logic.
//!
//! See `docs/llds/orchestrate-cli.md` § `gbiv install-skill` and
//! `docs/specs/orchestrate-cli.md` (`INSTALL-CLI-*`). This module holds the
//! dependency-free decision logic (frontmatter version parsing, version
//! comparison, and the installed/updated/unchanged/refused decision itself)
//! so it is unit-testable without touching the filesystem — the same split
//! `fleet_client`/`fleet_cli` uses for the fleet subcommands. Actual file
//! reads/writes, `HOME`/gbiv-root resolution, and stdout/stderr/logging glue
//! live in `install_skill_cli`.

use serde::Serialize;

use super::fleet_client::Outcome;

// ---- Exit codes (docs/llds/orchestrate-cli.md § gbiv install-skill) -------

pub const EXIT_OK: i32 = 0;
pub const EXIT_NOT_A_GBIV_WORKSPACE: i32 = 2;
pub const EXIT_REFUSED: i32 = 7;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    User,
    Project,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    Installed,
    Updated,
    Unchanged,
    Refused,
}

/// INSTALL-CLI-030, INSTALL-CLI-031: the JSON envelope printed to stdout for
/// every outcome the decision logic actually reaches (i.e. everything except
/// the pre-decision hard failures in INSTALL-CLI-032).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct InstallResult {
    pub scope: &'static str,
    pub destination: String,
    pub action: Action,
    pub bundled_version: String,
    pub previous_version: Option<String>,
    pub reason: Option<String>,
}

impl InstallResult {
    /// INSTALL-CLI-030, INSTALL-CLI-031: both success and refusal render the
    /// same envelope shape to stdout; only the exit code differs.
    // @spec INSTALL-CLI-030, INSTALL-CLI-031
    pub fn to_outcome(&self) -> Outcome {
        let exit_code = match self.action {
            Action::Refused => EXIT_REFUSED,
            _ => EXIT_OK,
        };
        Outcome {
            exit_code,
            stdout: Some(serde_json::to_string(self).expect("InstallResult always serializes")),
            stderr: None,
        }
    }
}

// ---- Frontmatter version parsing -------------------------------------------

/// Parse the `version:` field out of a `SKILL.md`'s YAML frontmatter (the
/// block between the first two `---` lines). Returns `None` if there is no
/// frontmatter block or no `version:` line within it (INSTALL-CLI-028's
/// "no parseable version" case).
pub fn parse_frontmatter_version(content: &str) -> Option<String> {
    let mut lines = content.lines();
    if lines.next()?.trim() != "---" {
        return None;
    }
    for line in lines {
        let trimmed = line.trim();
        if trimmed == "---" {
            break;
        }
        if let Some(rest) = trimmed.strip_prefix("version:") {
            let value = rest.trim();
            if !value.is_empty() {
                return Some(value.to_string());
            }
            return None;
        }
    }
    None
}

// ---- Version comparison -----------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VersionOrder {
    Less,
    Equal,
    Greater,
}

/// Compare two dot-separated numeric version strings (e.g. `"0.1.5"` vs
/// `"0.2.0"`) segment by segment. A missing trailing segment is treated as
/// `0` (so `"1.2"` == `"1.2.0"`). A non-numeric segment is treated as `0` for
/// that position rather than erroring — INSTALL-CLI-024 only calls this when
/// a `version:` value was already found; malformed *values* are rare enough
/// that failing safe (treat as equal-ish) is preferable to a panic.
// @spec INSTALL-CLI-024
pub fn compare_versions(a: &str, b: &str) -> VersionOrder {
    let parse = |s: &str| -> Vec<u64> {
        s.split('.')
            .map(|seg| seg.trim().parse::<u64>().unwrap_or(0))
            .collect()
    };
    let av = parse(a);
    let bv = parse(b);
    let len = av.len().max(bv.len());
    for i in 0..len {
        let x = av.get(i).copied().unwrap_or(0);
        let y = bv.get(i).copied().unwrap_or(0);
        if x < y {
            return VersionOrder::Less;
        }
        if x > y {
            return VersionOrder::Greater;
        }
    }
    VersionOrder::Equal
}

// ---- Idempotency decision (INSTALL-CLI-020 through INSTALL-CLI-028) -------

/// Decide what `gbiv install-skill` should do, given the bundled content, an
/// optional read of the existing destination file, and the `--force` flag.
/// Pure: takes already-read strings, returns the result to write (the caller
/// decides based on `action` whether a write is actually needed).
// @spec INSTALL-CLI-020, INSTALL-CLI-021, INSTALL-CLI-022, INSTALL-CLI-023,
// INSTALL-CLI-024, INSTALL-CLI-025, INSTALL-CLI-026, INSTALL-CLI-027, INSTALL-CLI-028
pub fn decide_action(
    existing_content: Option<&str>,
    bundled_content: &str,
    bundled_version: &str,
    force: bool,
) -> (Action, Option<String>, Option<String>) {
    let Some(existing) = existing_content else {
        // INSTALL-CLI-020: nothing to compare against; --force is moot.
        return (Action::Installed, None, None);
    };
    let previous_version = parse_frontmatter_version(existing);

    if existing == bundled_content {
        // INSTALL-CLI-021, INSTALL-CLI-022
        let action = if force {
            Action::Updated
        } else {
            Action::Unchanged
        };
        return (action, None, previous_version);
    }

    if force {
        // INSTALL-CLI-023: force skips version comparison entirely.
        return (Action::Updated, None, previous_version);
    }

    // INSTALL-CLI-024 through INSTALL-CLI-028
    match &previous_version {
        None => (
            Action::Refused,
            Some("destination has no parseable version; re-run with --force to overwrite".to_string()),
            None,
        ),
        Some(existing_version) => match compare_versions(existing_version, bundled_version) {
            VersionOrder::Equal => (
                Action::Refused,
                Some(
                    "destination differs from bundled content; re-run with --force to overwrite"
                        .to_string(),
                ),
                previous_version,
            ),
            VersionOrder::Less => (Action::Updated, None, previous_version),
            VersionOrder::Greater => (
                Action::Refused,
                Some(format!(
                    "on-disk skill (version {existing_version}) is newer than this binary ships (version {bundled_version}); re-run with --force to overwrite"
                )),
                previous_version,
            ),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BUNDLED: &str = "---\nname: gbiv-orchestrate\nversion: 0.2.0\n---\nbody\n";

    // ---- parse_frontmatter_version ------------------------------------------

    #[test]
    fn parses_version_from_frontmatter() {
        let content = "---\nname: x\nversion: 1.2.3\ndescription: y\n---\nbody";
        assert_eq!(
            parse_frontmatter_version(content),
            Some("1.2.3".to_string())
        );
    }

    #[test]
    fn returns_none_when_no_frontmatter() {
        assert_eq!(
            parse_frontmatter_version("just a body, no frontmatter"),
            None
        );
    }

    #[test]
    fn returns_none_when_frontmatter_has_no_version_line() {
        let content = "---\nname: x\n---\nbody";
        assert_eq!(parse_frontmatter_version(content), None);
    }

    #[test]
    fn returns_none_when_version_line_has_empty_value() {
        let content = "---\nversion:\n---\nbody";
        assert_eq!(parse_frontmatter_version(content), None);
    }

    // ---- compare_versions ----------------------------------------------------

    #[test]
    fn compares_equal_versions() {
        assert_eq!(compare_versions("0.2.0", "0.2.0"), VersionOrder::Equal);
    }

    #[test]
    fn compares_lower_versions() {
        assert_eq!(compare_versions("0.1.5", "0.2.0"), VersionOrder::Less);
    }

    #[test]
    fn compares_higher_versions() {
        assert_eq!(compare_versions("0.3.0", "0.2.0"), VersionOrder::Greater);
    }

    #[test]
    fn treats_missing_trailing_segment_as_zero() {
        assert_eq!(compare_versions("1.2", "1.2.0"), VersionOrder::Equal);
        assert_eq!(compare_versions("1.2.1", "1.2"), VersionOrder::Greater);
    }

    // ---- decide_action: INSTALL-CLI-020 (missing destination) ---------------

    #[test]
    // @spec INSTALL-CLI-020
    fn missing_destination_is_installed_regardless_of_force() {
        let (action, reason, prev) = decide_action(None, BUNDLED, "0.2.0", false);
        assert_eq!(action, Action::Installed);
        assert_eq!(reason, None);
        assert_eq!(prev, None);

        let (action, ..) = decide_action(None, BUNDLED, "0.2.0", true);
        assert_eq!(action, Action::Installed);
    }

    // ---- decide_action: identical content (INSTALL-CLI-021, -022) ----------

    #[test]
    // @spec INSTALL-CLI-021
    fn identical_content_without_force_is_unchanged() {
        let (action, reason, _) = decide_action(Some(BUNDLED), BUNDLED, "0.2.0", false);
        assert_eq!(action, Action::Unchanged);
        assert_eq!(reason, None);
    }

    #[test]
    // @spec INSTALL-CLI-022
    fn identical_content_with_force_is_updated() {
        let (action, ..) = decide_action(Some(BUNDLED), BUNDLED, "0.2.0", true);
        assert_eq!(action, Action::Updated);
    }

    // ---- decide_action: differing content + force (INSTALL-CLI-023) --------

    #[test]
    // @spec INSTALL-CLI-023
    fn differing_content_with_force_is_updated_without_version_check() {
        let existing = "---\nversion: 9.9.9\n---\nstale body";
        let (action, reason, prev) = decide_action(Some(existing), BUNDLED, "0.2.0", true);
        assert_eq!(action, Action::Updated);
        assert_eq!(reason, None);
        assert_eq!(prev, Some("9.9.9".to_string()));
    }

    // ---- decide_action: differing content, no force (INSTALL-CLI-024..028) --

    #[test]
    // @spec INSTALL-CLI-025
    fn same_version_differing_content_is_refused() {
        let existing = "---\nversion: 0.2.0\n---\nhand-edited body";
        let (action, reason, prev) = decide_action(Some(existing), BUNDLED, "0.2.0", false);
        assert_eq!(action, Action::Refused);
        assert!(reason.unwrap().contains("re-run with --force"));
        assert_eq!(prev, Some("0.2.0".to_string()));
    }

    #[test]
    // @spec INSTALL-CLI-026
    fn older_version_differing_content_is_updated() {
        let existing = "---\nversion: 0.1.0\n---\nold body";
        let (action, reason, prev) = decide_action(Some(existing), BUNDLED, "0.2.0", false);
        assert_eq!(action, Action::Updated);
        assert_eq!(reason, None);
        assert_eq!(prev, Some("0.1.0".to_string()));
    }

    #[test]
    // @spec INSTALL-CLI-027
    fn newer_version_differing_content_is_refused() {
        let existing = "---\nversion: 0.3.0\n---\nnewer body";
        let (action, reason, prev) = decide_action(Some(existing), BUNDLED, "0.2.0", false);
        assert_eq!(action, Action::Refused);
        let reason = reason.unwrap();
        assert!(reason.contains("0.3.0"));
        assert!(reason.contains("0.2.0"));
        assert_eq!(prev, Some("0.3.0".to_string()));
    }

    #[test]
    // @spec INSTALL-CLI-028
    fn unparseable_version_differing_content_is_refused() {
        let existing = "not even frontmatter";
        let (action, reason, prev) = decide_action(Some(existing), BUNDLED, "0.2.0", false);
        assert_eq!(action, Action::Refused);
        assert!(reason.unwrap().contains("no parseable version"));
        assert_eq!(prev, None);
    }

    // ---- InstallResult::to_outcome (INSTALL-CLI-030, -031) -------------------

    #[test]
    // @spec INSTALL-CLI-030
    fn success_outcome_exits_zero_and_prints_json_to_stdout() {
        let result = InstallResult {
            scope: "user",
            destination: "/home/x/.claude/skills/gbiv-orchestrate/SKILL.md".to_string(),
            action: Action::Installed,
            bundled_version: "0.2.0".to_string(),
            previous_version: None,
            reason: None,
        };
        let outcome = result.to_outcome();
        assert_eq!(outcome.exit_code, EXIT_OK);
        assert!(outcome.stdout.unwrap().contains("\"action\":\"installed\""));
        assert_eq!(outcome.stderr, None);
    }

    #[test]
    // @spec INSTALL-CLI-031
    fn refused_outcome_exits_seven_and_still_prints_json_to_stdout() {
        let result = InstallResult {
            scope: "user",
            destination: "/home/x/.claude/skills/gbiv-orchestrate/SKILL.md".to_string(),
            action: Action::Refused,
            bundled_version: "0.2.0".to_string(),
            previous_version: Some("0.2.0".to_string()),
            reason: Some(
                "destination differs from bundled content; re-run with --force to overwrite"
                    .to_string(),
            ),
        };
        let outcome = result.to_outcome();
        assert_eq!(outcome.exit_code, EXIT_REFUSED);
        let stdout = outcome.stdout.unwrap();
        assert!(stdout.contains("\"action\":\"refused\""));
        assert!(stdout.contains("re-run with --force"));
        assert_eq!(outcome.stderr, None);
    }
}

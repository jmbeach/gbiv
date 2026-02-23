use std::path::PathBuf;
use std::thread;
use std::time::Duration;

use crate::colors::{ansi_color, COLORS, DIM, GREEN, RED, RESET, YELLOW};
use crate::gbiv_md::parse_gbiv_md;
use crate::git_utils::{
    find_gbiv_root, find_repo_in_worktree, get_ahead_behind_vs, get_last_commit_age,
    get_quick_status, get_remote_main_branch, is_merged_into,
};

struct WorktreeStatus {
    branch: Option<String>,
    is_dirty: bool,
    merged: Option<bool>,
    age: Option<Duration>,
    ahead_behind: Option<(u32, u32)>,
}

fn format_age(duration: Duration) -> String {
    let secs = duration.as_secs();
    if secs < 60 {
        format!("{} secs", secs)
    } else if secs < 3600 {
        format!("{} mins", secs / 60)
    } else if secs < 86400 {
        format!("{} hours", secs / 3600)
    } else {
        format!("{} days", secs / 86400)
    }
}

fn collect_worktree_status(color: &'static str, repo_path: PathBuf) -> WorktreeStatus {
    let quick = get_quick_status(&repo_path);
    let branch = quick.branch;
    let is_dirty = quick.is_dirty;

    let (merged, age, ahead_behind) = if branch.as_deref() != Some(color) {
        let remote_main = get_remote_main_branch(&repo_path);
        let merged = match (&branch, &remote_main) {
            (Some(b), Some(rm)) => Some(is_merged_into(&repo_path, b, rm)),
            _ => None,
        };
        let age = get_last_commit_age(&repo_path);
        let ahead_behind = quick.ahead_behind.or_else(|| {
            remote_main.as_ref().and_then(|rm| get_ahead_behind_vs(&repo_path, rm))
        });
        (merged, age, ahead_behind)
    } else {
        (None, None, None)
    };
    WorktreeStatus { branch, is_dirty, merged, age, ahead_behind }
}

pub fn status_command() -> Result<(), String> {
    let cwd = std::env::current_dir()
        .map_err(|e| format!("Failed to get current directory: {}", e))?;

    let gbiv_root = find_gbiv_root(&cwd)
        .ok_or_else(|| "Not in a gbiv-structured repository".to_string())?;

    let handles: Vec<_> = COLORS
        .iter()
        .map(|&color| {
            let worktree_dir = gbiv_root.root.join(color);
            thread::spawn(move || {
                if !worktree_dir.exists() {
                    return None;
                }
                let repo_path = find_repo_in_worktree(&worktree_dir)?;
                Some(collect_worktree_status(color, repo_path))
            })
        })
        .collect();

    let results: Vec<_> = handles
        .into_iter()
        .map(|h| h.join().unwrap_or(None))
        .collect();

    for (i, result) in results.into_iter().enumerate() {
        let color = COLORS[i];
        let color_code = ansi_color(color);

        match result {
            None => println!("{}{:<8}{}  missing", color_code, color, RESET),
            Some(status) => {
                let branch = status.branch.as_deref().unwrap_or("???");
                let is_dirty = status.is_dirty;

                if branch == color {
                    if is_dirty {
                        println!("{}{:<8}{}  {}{:<24}{} {}dirty{}", color_code, color, RESET, DIM, branch, RESET, YELLOW, RESET);
                    } else {
                        println!("{}{:<8}{}  {}{:<24} clean{}", color_code, color, RESET, DIM, branch, RESET);
                    }
                } else {
                    let dirty_str = if is_dirty {
                        format!("{}dirty{}", YELLOW, RESET)
                    } else {
                        "clean".to_string()
                    };
                    let (merged_str, merged_color) = match status.merged {
                        Some(true) => ("merged", DIM),
                        Some(false) => ("not merged", YELLOW),
                        None => ("no remote", DIM),
                    };
                    let age_str = status.age.map(format_age).unwrap_or_else(|| "???".to_string());
                    let ab_str = match status.ahead_behind {
                        Some((ahead, behind)) => {
                            let ahead_fmt = if ahead > 0 {
                                format!("{}↑{}{}", GREEN, ahead, RESET)
                            } else {
                                format!("{}↑{}{}", DIM, ahead, RESET)
                            };
                            let behind_fmt = if behind > 0 {
                                format!("{}↓{}{}", RED, behind, RESET)
                            } else {
                                format!("{}↓{}{}", DIM, behind, RESET)
                            };
                            format!("{} {}", ahead_fmt, behind_fmt)
                        }
                        None => format!("{}???{}", DIM, RESET),
                    };
                    println!(
                        "{}{:<8}{}  {:<24} {:<5}  {}{}{}  {}{}  {}{}",
                        color_code, color, RESET, branch, dirty_str, merged_color, merged_str, RESET, DIM, age_str, ab_str, RESET
                    );
                }
            }
        }
    }

    let features = find_repo_in_worktree(&gbiv_root.root.join("main"))
        .map(|p| parse_gbiv_md(&p.join("GBIV.md")))
        .unwrap_or_default();
    print!("{}", format_gbiv_features(&features));

    Ok(())
}

fn format_gbiv_features(features: &[crate::gbiv_md::GbivFeature]) -> String {
    if features.is_empty() {
        return String::new();
    }
    let mut out = String::new();
    out.push('\n');
    out.push_str(&format!("{}GBIV.md{}\n", DIM, RESET));
    for feature in features {
        match &feature.tag {
            Some(tag) => {
                let color_code = ansi_color(tag);
                out.push_str(&format!("  {}{:<8}{}  {}\n", color_code, tag, RESET, feature.description));
            }
            None => {
                out.push_str(&format!("  {}{:<8}{}  {}\n", DIM, "backlog", RESET, feature.description));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gbiv_md::GbivFeature;
    use std::fs;
    use tempfile::TempDir;

    fn make_feature(tag: Option<&str>, description: &str) -> GbivFeature {
        GbivFeature { tag: tag.map(|s| s.to_string()), description: description.to_string(), notes: vec![] }
    }

    #[test]
    fn gbiv_md_resolved_from_main_worktree_repo() {
        let gbiv_root = TempDir::new().unwrap();
        let main_dir = gbiv_root.path().join("main");
        let repo_dir = main_dir.join("myrepo");
        fs::create_dir_all(repo_dir.join(".git")).unwrap();
        fs::write(repo_dir.join("GBIV.md"), "- [red] Fix bug\n").unwrap();

        let gbiv_md_path = find_repo_in_worktree(&main_dir)
            .map(|p| p.join("GBIV.md"))
            .unwrap_or_else(|| main_dir.join("GBIV.md"));

        assert_eq!(gbiv_md_path, repo_dir.join("GBIV.md"));
        let features = crate::gbiv_md::parse_gbiv_md(&gbiv_md_path);
        assert_eq!(features.len(), 1);
        assert_eq!(features[0].tag, Some("red".to_string()));
    }

    #[test]
    fn gbiv_md_not_read_from_gbiv_root_directly() {
        let gbiv_root = TempDir::new().unwrap();
        let main_dir = gbiv_root.path().join("main");
        let repo_dir = main_dir.join("myrepo");
        fs::create_dir_all(repo_dir.join(".git")).unwrap();

        // Place GBIV.md at gbiv_root (should be ignored)
        fs::write(gbiv_root.path().join("GBIV.md"), "- [red] Root-level bug\n").unwrap();
        // Place GBIV.md at the correct repo location
        fs::write(repo_dir.join("GBIV.md"), "- [green] Repo-level feature\n").unwrap();

        let gbiv_md_path = find_repo_in_worktree(&main_dir)
            .map(|p| p.join("GBIV.md"))
            .unwrap_or_else(|| main_dir.join("GBIV.md"));

        let features = crate::gbiv_md::parse_gbiv_md(&gbiv_md_path);
        assert_eq!(features.len(), 1);
        // Must find green (repo), not red (root)
        assert_eq!(features[0].tag, Some("green".to_string()));
    }

    #[test]
    fn format_gbiv_features_empty() {
        assert_eq!(format_gbiv_features(&[]), "");
    }

    #[test]
    fn format_gbiv_features_untagged_shows_backlog() {
        let features = vec![make_feature(None, "Do something")];
        let out = format_gbiv_features(&features);
        assert!(out.contains("backlog"));
        assert!(out.contains("Do something"));
        assert!(out.contains("GBIV.md"));
    }

    #[test]
    fn format_gbiv_features_tagged_shows_color_and_tag() {
        let features = vec![make_feature(Some("red"), "Fix critical bug")];
        let out = format_gbiv_features(&features);
        assert!(out.contains("red"));
        assert!(out.contains("Fix critical bug"));
        assert!(out.contains("GBIV.md"));
        // ANSI red code
        assert!(out.contains("\x1b[31m"));
    }

    #[test]
    fn format_gbiv_features_multiple() {
        let features = vec![
            make_feature(Some("green"), "Feature A"),
            make_feature(None, "Feature B"),
        ];
        let out = format_gbiv_features(&features);
        assert!(out.contains("Feature A"));
        assert!(out.contains("Feature B"));
        assert!(out.contains("backlog"));
    }
}

#[derive(Debug, thiserror::Error)]
pub enum GbivMdError {
    #[error("no GBIV.md entry tagged [{0}]")]
    NotFound(String),
    #[error("no GBIV.md entry [{tag}] {description}")]
    EntryNotFound { tag: String, description: String },
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
}

pub struct GbivFeature {
    pub tag: Option<String>,
    pub status: Option<String>,
    pub description: String,
    pub notes: Vec<String>,
}

// @spec FL-PARSE-001 through FL-PARSE-014
pub fn parse_gbiv_md(path: &std::path::Path) -> Vec<GbivFeature> {
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return vec![],
        Err(e) => {
            eprintln!("Warning: failed to read GBIV.md: {}", e);
            return vec![];
        }
    };

    let mut features: Vec<GbivFeature> = vec![];

    for line in content.lines() {
        if line == "---" {
            break;
        }
        if let Some(rest) = line.strip_prefix("- ") {
            let (tag, status, description) = if rest.starts_with('[') {
                if let Some(close) = rest.find(']') {
                    let tag = rest.get(1..close).unwrap_or("").to_string();
                    let after_tag = rest.get(close + 1..).unwrap_or("").trim_start();
                    // Check for optional status bracket
                    if after_tag.starts_with('[') {
                        if let Some(close2) = after_tag.find(']') {
                            let candidate = after_tag.get(1..close2).unwrap_or("").to_string();
                            if candidate == "in-progress" || candidate == "done" {
                                let desc = after_tag
                                    .get(close2 + 1..)
                                    .unwrap_or("")
                                    .trim_start()
                                    .to_string();
                                (Some(tag), Some(candidate), desc)
                            } else {
                                (Some(tag), None, after_tag.to_string())
                            }
                        } else {
                            (Some(tag), None, after_tag.to_string())
                        }
                    } else {
                        (Some(tag), None, after_tag.to_string())
                    }
                } else {
                    (None, None, rest.to_string())
                }
            } else {
                (None, None, rest.to_string())
            };
            features.push(GbivFeature {
                tag,
                status,
                description,
                notes: vec![],
            });
        } else if !line.is_empty() {
            if let Some(last) = features.last_mut() {
                last.notes.push(line.to_string());
            }
        }
    }

    features
}

// @spec FL-MUTATE-007 through FL-MUTATE-011
pub fn remove_gbiv_features_by_tag(path: &std::path::Path, tag: &str) -> Result<(), GbivMdError> {
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(GbivMdError::Io(e)),
    };

    let lines: Vec<&str> = content.lines().collect();
    let mut result_lines: Vec<&str> = vec![];
    let mut skip_current = false;
    let mut past_separator = false;

    for line in &lines {
        if past_separator {
            result_lines.push(line);
            continue;
        }

        if *line == "---" {
            past_separator = true;
            result_lines.push(line);
            continue;
        }

        if let Some(rest) = line.strip_prefix("- ") {
            if rest.starts_with('[') {
                if let Some(close) = rest.find(']') {
                    let feature_tag = &rest[1..close];
                    if feature_tag == tag {
                        skip_current = true;
                        continue;
                    }
                }
            }
            skip_current = false;
            result_lines.push(line);
        } else if line.is_empty() {
            if !skip_current {
                result_lines.push(line);
            }
            skip_current = false;
        } else {
            // Note line belonging to preceding feature
            if !skip_current {
                result_lines.push(line);
            }
        }
    }

    let mut result = result_lines.join("\n");
    if content.ends_with('\n') {
        result.push('\n');
    }

    if result == content {
        return Ok(());
    }

    std::fs::write(path, result)?;
    Ok(())
}

// @spec FL-MUTATE-001 through FL-MUTATE-006
pub fn set_gbiv_feature_status(
    path: &std::path::Path,
    color: &str,
    status: Option<&str>,
) -> Result<(), GbivMdError> {
    let content = std::fs::read_to_string(path)?;

    let mut found = false;
    let mut past_separator = false;
    let mut result_lines: Vec<String> = vec![];
    let tag_prefix = format!("- [{}] ", color);

    for line in content.lines() {
        if line == "---" {
            past_separator = true;
        }
        if !past_separator && line.starts_with(&tag_prefix) {
            found = true;
            let after_tag = &line[tag_prefix.len()..];
            // Strip existing status bracket if present
            let description = if after_tag.starts_with('[') {
                if let Some(close) = after_tag.find(']') {
                    let candidate = &after_tag[1..close];
                    if candidate == "in-progress" || candidate == "done" {
                        after_tag[close + 1..].trim_start()
                    } else {
                        after_tag
                    }
                } else {
                    after_tag
                }
            } else {
                after_tag
            };
            // Rebuild line with or without status
            match status {
                Some(s) => result_lines.push(format!("- [{}] [{}] {}", color, s, description)),
                None => result_lines.push(format!("- [{}] {}", color, description)),
            }
        } else {
            result_lines.push(line.to_string());
        }
    }

    if !found {
        return Err(GbivMdError::NotFound(color.to_string()));
    }

    let mut result = result_lines.join("\n");
    if content.ends_with('\n') {
        result.push('\n');
    }

    std::fs::write(path, result)?;
    Ok(())
}

/// Whether `feature`'s text carries the text tag `[tag]` (case-insensitive,
/// anywhere in the text).
// @spec FL-TAG-001
pub fn carries_text_tag(feature: &GbivFeature, tag: &str) -> bool {
    let _ = (feature, tag);
    todo!("gbv-1ht: carries_text_tag")
}

/// Append `- [<color>] <text>` as a new entry.
// @spec FL-MUTATE-012, FL-MUTATE-013, FL-MUTATE-014, FL-MUTATE-015
pub fn append_gbiv_entry(path: &std::path::Path, color: &str, text: &str) -> Result<(), GbivMdError> {
    let _ = (path, color, text);
    todo!("gbv-1ht: append_gbiv_entry")
}

/// Re-read GBIV.md and remove the one entry whose color tag and text match.
// @spec FL-MUTATE-016, FL-MUTATE-017
pub fn remove_gbiv_entry(
    path: &std::path::Path,
    tag: &str,
    description: &str,
) -> Result<(), GbivMdError> {
    let _ = (path, tag, description);
    todo!("gbv-1ht: remove_gbiv_entry")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn write_temp(content: &str) -> NamedTempFile {
        let mut f = NamedTempFile::new().unwrap();
        write!(f, "{}", content).unwrap();
        f
    }

    // @spec FL-PARSE-010
    #[test]
    fn returns_empty_when_file_missing() {
        let result = parse_gbiv_md(std::path::Path::new("/nonexistent/GBIV.md"));
        assert!(result.is_empty());
    }

    // @spec FL-PARSE-008
    #[test]
    fn returns_empty_when_no_feature_lines() {
        let f = write_temp("Just some text\nno features here\n");
        let result = parse_gbiv_md(f.path());
        assert!(result.is_empty());
    }

    // @spec FL-PARSE-006
    #[test]
    fn parses_simple_feature_without_tag() {
        let f = write_temp("- Add login page\n");
        let result = parse_gbiv_md(f.path());
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].tag, None);
        assert_eq!(result[0].description, "Add login page");
        assert!(result[0].notes.is_empty());
    }

    // @spec FL-PARSE-002
    #[test]
    fn parses_feature_with_tag() {
        let f = write_temp("- [red] Fix critical bug\n");
        let result = parse_gbiv_md(f.path());
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].tag, Some("red".to_string()));
        assert_eq!(result[0].description, "Fix critical bug");
    }

    // @spec FL-PARSE-007
    #[test]
    fn parses_notes_on_preceding_feature() {
        let f = write_temp("- [blue] Add feature\n  This is a note\n  Another note\n");
        let result = parse_gbiv_md(f.path());
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].notes, vec!["  This is a note", "  Another note"]);
    }

    // @spec FL-PARSE-009
    #[test]
    fn stops_at_separator() {
        let f = write_temp("- First feature\n---\n- Second feature\n");
        let result = parse_gbiv_md(f.path());
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].description, "First feature");
    }

    // @spec FL-PARSE-001, FL-PARSE-002
    #[test]
    fn parses_multiple_features() {
        let f = write_temp("- [green] Feature A\n- Feature B\n- [red] Feature C\n");
        let result = parse_gbiv_md(f.path());
        assert_eq!(result.len(), 3);
        assert_eq!(result[0].tag, Some("green".to_string()));
        assert_eq!(result[1].tag, None);
        assert_eq!(result[1].description, "Feature B");
        assert_eq!(result[2].tag, Some("red".to_string()));
    }

    // @spec FL-PARSE-002
    #[test]
    fn parses_feature_with_unrecognized_tag() {
        let f = write_temp("- [purple] Some feature\n");
        let result = parse_gbiv_md(f.path());
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].tag, Some("purple".to_string()));
        assert_eq!(result[0].description, "Some feature");
    }

    // @spec FL-PARSE-008
    #[test]
    fn notes_not_attached_before_first_feature() {
        let f = write_temp("Some header text\n- Feature one\n");
        let result = parse_gbiv_md(f.path());
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].description, "Feature one");
        assert!(result[0].notes.is_empty());
    }

    // Tests for remove_gbiv_features_by_tag

    // @spec FL-MUTATE-007
    #[test]
    fn remove_by_tag_removes_matching_entry() {
        let f = write_temp("- [red] Fix critical bug\n- [blue] Add feature\n");
        remove_gbiv_features_by_tag(f.path(), "red").unwrap();
        let result = parse_gbiv_md(f.path());
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].tag, Some("blue".to_string()));
    }

    // @spec FL-MUTATE-010
    #[test]
    fn remove_by_tag_noop_when_no_match() {
        let original = "- [blue] Add feature\n";
        let f = write_temp(original);
        remove_gbiv_features_by_tag(f.path(), "red").unwrap();
        let on_disk = std::fs::read_to_string(f.path()).unwrap();
        assert_eq!(on_disk, original);
    }

    // @spec FL-MUTATE-007
    #[test]
    fn remove_by_tag_removes_multiple_matching_entries() {
        let f = write_temp("- [red] Bug fix\n- [blue] Feature\n- [red] Another red\n");
        remove_gbiv_features_by_tag(f.path(), "red").unwrap();
        let result = parse_gbiv_md(f.path());
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].tag, Some("blue".to_string()));
    }

    // @spec FL-MUTATE-008
    #[test]
    fn remove_by_tag_preserves_content_after_separator() {
        let f = write_temp("- [red] Bug fix\n---\nSome footer content\n");
        remove_gbiv_features_by_tag(f.path(), "red").unwrap();
        let on_disk = std::fs::read_to_string(f.path()).unwrap();
        assert!(on_disk.contains("---\nSome footer content\n"));
        assert!(!on_disk.contains("[red]"));
    }

    // @spec FL-MUTATE-007
    #[test]
    fn remove_by_tag_also_removes_attached_notes() {
        let f = write_temp("- [red] Bug fix\n  Note line\n  Another note\n- [blue] Feature\n");
        remove_gbiv_features_by_tag(f.path(), "red").unwrap();
        let result = parse_gbiv_md(f.path());
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].tag, Some("blue".to_string()));
        let on_disk = std::fs::read_to_string(f.path()).unwrap();
        assert!(!on_disk.contains("Note line"));
        assert!(!on_disk.contains("Another note"));
    }

    // @spec FL-MUTATE-009
    #[test]
    fn remove_by_tag_no_stray_blank_lines_when_features_separated_by_blank() {
        let f = write_temp("- [red] Bug fix\n\n- [blue] Feature\n");
        remove_gbiv_features_by_tag(f.path(), "red").unwrap();
        let on_disk = std::fs::read_to_string(f.path()).unwrap();
        assert_eq!(on_disk, "- [blue] Feature\n");
    }

    // @spec FL-PARSE-003
    #[test]
    fn parse_entry_with_done_status_tag() {
        let f = write_temp("- [red] [done] Fix critical bug\n");
        let result = parse_gbiv_md(f.path());
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].tag, Some("red".to_string()));
        assert_eq!(result[0].status, Some("done".to_string()));
        assert_eq!(result[0].description, "Fix critical bug");
    }

    // @spec FL-PARSE-004
    #[test]
    fn parse_entry_with_in_progress_status_tag() {
        let f = write_temp("- [red] [in-progress] Fix critical bug\n");
        let result = parse_gbiv_md(f.path());
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].tag, Some("red".to_string()));
        assert_eq!(result[0].status, Some("in-progress".to_string()));
        assert_eq!(result[0].description, "Fix critical bug");
    }

    // @spec FL-PARSE-002
    #[test]
    fn parse_entry_without_status_tag_returns_none() {
        let f = write_temp("- [red] Fix critical bug\n");
        let result = parse_gbiv_md(f.path());
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].tag, Some("red".to_string()));
        assert_eq!(result[0].status, None);
        assert_eq!(result[0].description, "Fix critical bug");
    }

    // @spec FL-PARSE-005
    #[test]
    fn parse_unrecognized_second_bracket_is_not_status() {
        let f = write_temp("- [red] [wip] Fix critical bug\n");
        let result = parse_gbiv_md(f.path());
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].tag, Some("red".to_string()));
        assert_eq!(result[0].status, None);
        assert_eq!(result[0].description, "[wip] Fix critical bug");
    }

    // @spec FL-MUTATE-001
    #[test]
    fn set_status_adds_done_to_entry_with_no_status() {
        let f = write_temp("- [red] Fix bug\n");
        set_gbiv_feature_status(f.path(), "red", Some("done")).unwrap();
        let on_disk = std::fs::read_to_string(f.path()).unwrap();
        assert_eq!(on_disk, "- [red] [done] Fix bug\n");
    }

    // @spec FL-MUTATE-001
    #[test]
    fn set_status_replaces_existing_status() {
        let f = write_temp("- [red] [in-progress] Fix bug\n");
        set_gbiv_feature_status(f.path(), "red", Some("done")).unwrap();
        let on_disk = std::fs::read_to_string(f.path()).unwrap();
        assert_eq!(on_disk, "- [red] [done] Fix bug\n");
    }

    // @spec FL-MUTATE-002
    #[test]
    fn set_status_with_none_removes_status_tag() {
        let f = write_temp("- [red] [done] Fix bug\n");
        set_gbiv_feature_status(f.path(), "red", None).unwrap();
        let on_disk = std::fs::read_to_string(f.path()).unwrap();
        assert_eq!(on_disk, "- [red] Fix bug\n");
    }

    // @spec FL-MUTATE-003
    #[test]
    fn set_status_errors_when_no_matching_color_entry() {
        let f = write_temp("- [blue] Fix bug\n");
        let result = set_gbiv_feature_status(f.path(), "red", Some("done"));
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), GbivMdError::NotFound(_)));
    }

    // @spec FL-MUTATE-004
    #[test]
    fn set_status_preserves_notes() {
        let f = write_temp("- [red] Fix bug\n  Note line one\n  Note line two\n");
        set_gbiv_feature_status(f.path(), "red", Some("done")).unwrap();
        let on_disk = std::fs::read_to_string(f.path()).unwrap();
        assert_eq!(
            on_disk,
            "- [red] [done] Fix bug\n  Note line one\n  Note line two\n"
        );
    }

    // ---- gbv-1ht: exact legacy status tokens ----------------------------------

    // @spec FL-PARSE-003, FL-PARSE-004, FL-PARSE-005
    #[test]
    fn parse_status_requires_exact_done_or_in_progress_token() {
        for second in [
            "[DONE-12]",
            "[undone]",
            "[Done]",
            "[done-ish]",
            "[in-progress-2]",
            "[In-Progress]",
        ] {
            let f = write_temp(&format!("- [red] {second} Fix bug\n"));
            let result = parse_gbiv_md(f.path());
            assert_eq!(result.len(), 1);
            assert_eq!(result[0].tag, Some("red".to_string()));
            assert_eq!(result[0].status, None, "{second} must not become a status");
            assert_eq!(result[0].description, format!("{second} Fix bug"));
        }
    }

    // ---- gbv-1ht: carries_text_tag ---------------------------------------------

    fn feature(tag: Option<&str>, status: Option<&str>, description: &str) -> GbivFeature {
        GbivFeature {
            tag: tag.map(str::to_string),
            status: status.map(str::to_string),
            description: description.to_string(),
            notes: vec![],
        }
    }

    // @spec FL-TAG-001
    #[test]
    fn carries_text_tag_matches_leading_tag() {
        let f = feature(Some("red"), None, "[gbv-1ht] Pluggable integrations");
        assert!(carries_text_tag(&f, "gbv-1ht"));
    }

    // @spec FL-TAG-001
    #[test]
    fn carries_text_tag_is_case_insensitive() {
        let f = feature(Some("red"), None, "[GBV-1HT] Pluggable integrations");
        assert!(carries_text_tag(&f, "gbv-1ht"));
        let f = feature(Some("red"), None, "[gbv-1ht] Pluggable integrations");
        assert!(carries_text_tag(&f, "GBV-1ht"));
    }

    // @spec FL-TAG-001
    #[test]
    fn carries_text_tag_matches_at_any_position() {
        let f = feature(Some("red"), None, "[gbv-1ht] [blocked-by:gbv-x2v] Title");
        assert!(carries_text_tag(&f, "blocked-by:gbv-x2v"));
        let f = feature(Some("red"), None, "Title mentioning [later] at the end [x]");
        assert!(carries_text_tag(&f, "later"));
        assert!(carries_text_tag(&f, "x"));
        let f = feature(None, None, "untagged [abc-1] backlog item");
        assert!(carries_text_tag(&f, "abc-1"));
    }

    // @spec FL-TAG-001
    #[test]
    fn carries_text_tag_requires_the_exact_bracketed_token() {
        let f = feature(
            Some("red"),
            None,
            "[gbv-1ht2] other and gbv-1ht unbracketed",
        );
        assert!(!carries_text_tag(&f, "gbv-1ht"));
        let f = feature(Some("red"), None, "[xgbv-1ht] [gbv-1ht-b]");
        assert!(!carries_text_tag(&f, "gbv-1ht"));
        let f = feature(Some("red"), None, "[gbv 1ht]");
        assert!(!carries_text_tag(&f, "gbv-1ht"));
    }

    // @spec FL-TAG-001
    #[test]
    fn carries_text_tag_ignores_color_and_status_tags() {
        let f = feature(Some("red"), Some("done"), "Fix bug");
        assert!(!carries_text_tag(&f, "red"));
        assert!(!carries_text_tag(&f, "done"));
        let f = feature(Some("red"), Some("in-progress"), "Fix bug");
        assert!(!carries_text_tag(&f, "in-progress"));
    }

    // @spec FL-TAG-001
    #[test]
    fn carries_text_tag_on_parsed_entries() {
        let f = write_temp("- [red] [done] [gbv-7] Old\n- [blue] Plain\n");
        let result = parse_gbiv_md(f.path());
        assert!(carries_text_tag(&result[0], "GBV-7"));
        assert!(!carries_text_tag(&result[0], "done"));
        assert!(!carries_text_tag(&result[1], "blue"));
    }

    // ---- gbv-1ht: append_gbiv_entry --------------------------------------------

    // @spec FL-MUTATE-012
    #[test]
    fn append_inserts_after_last_entry_and_its_notes() {
        let f = write_temp("- [red] A\n  note a\n- [blue] B\n  note b\n\n---\nfooter\n");
        append_gbiv_entry(f.path(), "orange", "C").unwrap();
        let on_disk = std::fs::read_to_string(f.path()).unwrap();
        assert_eq!(
            on_disk,
            "- [red] A\n  note a\n- [blue] B\n  note b\n- [orange] C\n\n---\nfooter\n"
        );
    }

    // @spec FL-MUTATE-012
    #[test]
    fn append_ignores_entries_below_separator() {
        let f = write_temp("- [red] A\n---\n- [blue] example below\n");
        append_gbiv_entry(f.path(), "orange", "C").unwrap();
        let on_disk = std::fs::read_to_string(f.path()).unwrap();
        assert_eq!(
            on_disk,
            "- [red] A\n- [orange] C\n---\n- [blue] example below\n"
        );
    }

    // @spec FL-MUTATE-012
    #[test]
    fn append_goes_before_trailing_blank_lines() {
        let f = write_temp("- [red] A\n\n- [blue] B\n\n\n---\n");
        append_gbiv_entry(f.path(), "orange", "C").unwrap();
        let on_disk = std::fs::read_to_string(f.path()).unwrap();
        assert_eq!(on_disk, "- [red] A\n\n- [blue] B\n- [orange] C\n\n\n---\n");
    }

    // @spec FL-MUTATE-012
    #[test]
    fn append_goes_after_a_note_separated_by_a_blank_line() {
        // The parser attaches "  late note" to A (FL-PARSE-007, FL-PARSE-014).
        let f = write_temp("- [red] A\n\n  late note\n");
        append_gbiv_entry(f.path(), "orange", "C").unwrap();
        let on_disk = std::fs::read_to_string(f.path()).unwrap();
        assert_eq!(on_disk, "- [red] A\n\n  late note\n- [orange] C\n");
    }

    // @spec FL-MUTATE-012
    #[test]
    fn append_without_separator_goes_at_end() {
        let f = write_temp("- [red] A\n- backlog\n");
        append_gbiv_entry(f.path(), "orange", "[gbv-1] Title").unwrap();
        let on_disk = std::fs::read_to_string(f.path()).unwrap();
        assert_eq!(on_disk, "- [red] A\n- backlog\n- [orange] [gbv-1] Title\n");
        let parsed = parse_gbiv_md(f.path());
        assert_eq!(parsed.len(), 3);
        assert_eq!(parsed[2].tag.as_deref(), Some("orange"));
        assert_eq!(parsed[2].description, "[gbv-1] Title");
    }

    // @spec FL-MUTATE-013
    #[test]
    fn append_with_no_entries_goes_after_preamble() {
        let f = write_temp("# Tasks\nSome intro\n\n---\nfooter\n");
        append_gbiv_entry(f.path(), "red", "C").unwrap();
        let on_disk = std::fs::read_to_string(f.path()).unwrap();
        assert_eq!(on_disk, "# Tasks\nSome intro\n- [red] C\n\n---\nfooter\n");
    }

    // @spec FL-MUTATE-013
    #[test]
    fn append_with_no_entries_goes_after_last_non_blank_preamble_line() {
        let f = write_temp("# Tasks\n\nIntro paragraph\n\n---\n");
        append_gbiv_entry(f.path(), "red", "C").unwrap();
        let on_disk = std::fs::read_to_string(f.path()).unwrap();
        assert_eq!(on_disk, "# Tasks\n\nIntro paragraph\n- [red] C\n\n---\n");
    }

    // @spec FL-MUTATE-013
    #[test]
    fn append_with_no_entries_and_no_preamble_goes_at_top() {
        let f = write_temp("\n\n---\nfooter\n");
        append_gbiv_entry(f.path(), "red", "C").unwrap();
        let on_disk = std::fs::read_to_string(f.path()).unwrap();
        assert_eq!(on_disk, "- [red] C\n\n\n---\nfooter\n");

        let f = write_temp("---\n- [blue] example\n");
        append_gbiv_entry(f.path(), "red", "C").unwrap();
        let on_disk = std::fs::read_to_string(f.path()).unwrap();
        assert_eq!(on_disk, "- [red] C\n---\n- [blue] example\n");
    }

    // @spec FL-MUTATE-014
    #[test]
    fn append_creates_missing_file_with_just_the_entry() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("GBIV.md");
        append_gbiv_entry(&path, "red", "Fix login bug").unwrap();
        let on_disk = std::fs::read_to_string(&path).unwrap();
        assert_eq!(on_disk, "- [red] Fix login bug\n");
    }

    // @spec FL-MUTATE-015
    #[test]
    fn append_preserves_other_lines_byte_for_byte() {
        let original = "# Tasks  \n- [red] A  \t\n\t  odd note\t\n---\n  footer with  spaces \n\n";
        let f = write_temp(original);
        append_gbiv_entry(f.path(), "blue", "B").unwrap();
        let on_disk = std::fs::read_to_string(f.path()).unwrap();
        assert_eq!(
            on_disk,
            "# Tasks  \n- [red] A  \t\n\t  odd note\t\n- [blue] B\n---\n  footer with  spaces \n\n"
        );
    }

    // @spec FL-MUTATE-015
    #[test]
    fn append_uses_crlf_when_file_does() {
        let f = write_temp("- [red] A\r\n  note\r\n---\r\nfooter\r\n");
        append_gbiv_entry(f.path(), "blue", "B").unwrap();
        let on_disk = std::fs::read_to_string(f.path()).unwrap();
        assert_eq!(
            on_disk,
            "- [red] A\r\n  note\r\n- [blue] B\r\n---\r\nfooter\r\n"
        );
    }

    // @spec FL-MUTATE-015
    #[test]
    fn append_keeps_missing_trailing_newline() {
        let f = write_temp("- [red] A");
        append_gbiv_entry(f.path(), "blue", "B").unwrap();
        let on_disk = std::fs::read_to_string(f.path()).unwrap();
        assert_eq!(on_disk, "- [red] A\n- [blue] B");
    }

    // @spec FL-MUTATE-015
    #[test]
    fn append_crlf_without_trailing_newline() {
        let f = write_temp("- [red] A\r\n- [green] G");
        append_gbiv_entry(f.path(), "blue", "B").unwrap();
        let on_disk = std::fs::read_to_string(f.path()).unwrap();
        assert_eq!(on_disk, "- [red] A\r\n- [green] G\r\n- [blue] B");
    }

    // @spec FL-MUTATE-015
    #[test]
    fn append_mid_file_keeps_missing_trailing_newline() {
        let f = write_temp("- [red] A\n---\nfooter");
        append_gbiv_entry(f.path(), "blue", "B").unwrap();
        let on_disk = std::fs::read_to_string(f.path()).unwrap();
        assert_eq!(on_disk, "- [red] A\n- [blue] B\n---\nfooter");
    }

    // ---- gbv-1ht: remove_gbiv_entry --------------------------------------------

    // @spec FL-MUTATE-016
    #[test]
    fn remove_entry_removes_matching_entry_notes_and_following_blank() {
        let f = write_temp("- [red] A\n  note\n\n- [blue] B\n---\nfooter\n");
        remove_gbiv_entry(f.path(), "red", "A").unwrap();
        let on_disk = std::fs::read_to_string(f.path()).unwrap();
        assert_eq!(on_disk, "- [blue] B\n---\nfooter\n");
    }

    // @spec FL-MUTATE-016
    #[test]
    fn remove_entry_removes_only_the_entry_whose_text_matches() {
        let f = write_temp("- [red] A\n  note a\n- [red] B\n  note b\n- [blue] B\n");
        remove_gbiv_entry(f.path(), "red", "B").unwrap();
        let on_disk = std::fs::read_to_string(f.path()).unwrap();
        assert_eq!(on_disk, "- [red] A\n  note a\n- [blue] B\n");
    }

    // @spec FL-MUTATE-016
    #[test]
    fn remove_entry_matches_text_with_text_tags() {
        let f = write_temp("# Tasks\n- [green] [gbv-1ht] [blocked-by:x] Title\n- [blue] B\n");
        remove_gbiv_entry(f.path(), "green", "[gbv-1ht] [blocked-by:x] Title").unwrap();
        let on_disk = std::fs::read_to_string(f.path()).unwrap();
        assert_eq!(on_disk, "# Tasks\n- [blue] B\n");
    }

    // @spec FL-MUTATE-016
    #[test]
    fn remove_entry_reads_current_file_contents() {
        let f = write_temp("- [red] Old\n");
        // The file changed after it was first written; removal works from what is on disk now.
        std::fs::write(f.path(), "- [blue] B\n- [red] New\n").unwrap();
        remove_gbiv_entry(f.path(), "red", "New").unwrap();
        let on_disk = std::fs::read_to_string(f.path()).unwrap();
        assert_eq!(on_disk, "- [blue] B\n");
    }

    // @spec FL-MUTATE-017
    #[test]
    fn remove_entry_errors_and_leaves_file_when_text_differs() {
        let original = "- [red] A\n- [blue] B\n";
        let f = write_temp(original);
        let result = remove_gbiv_entry(f.path(), "red", "B");
        assert!(matches!(result, Err(GbivMdError::EntryNotFound { .. })));
        assert_eq!(std::fs::read_to_string(f.path()).unwrap(), original);
    }

    // @spec FL-MUTATE-017
    #[test]
    fn remove_entry_errors_and_leaves_file_when_color_differs() {
        let original = "- [red] A\n";
        let f = write_temp(original);
        let result = remove_gbiv_entry(f.path(), "blue", "A");
        assert!(matches!(result, Err(GbivMdError::EntryNotFound { .. })));
        assert_eq!(std::fs::read_to_string(f.path()).unwrap(), original);
    }

    // @spec FL-MUTATE-017
    #[test]
    fn remove_entry_does_not_match_below_separator() {
        let original = "- [blue] B\n---\n- [red] A\n";
        let f = write_temp(original);
        let result = remove_gbiv_entry(f.path(), "red", "A");
        assert!(result.is_err());
        assert_eq!(std::fs::read_to_string(f.path()).unwrap(), original);
    }

    // @spec FL-MUTATE-017
    #[test]
    fn remove_entry_errors_when_file_missing() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("GBIV.md");
        assert!(remove_gbiv_entry(&path, "red", "A").is_err());
        assert!(!path.exists());
    }
}

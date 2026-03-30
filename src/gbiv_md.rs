pub struct GbivFeature {
    pub tag: Option<String>,
    pub status: Option<String>,
    pub description: String,
    pub notes: Vec<String>,
}

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
                                let desc = after_tag.get(close2 + 1..).unwrap_or("").trim_start().to_string();
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
            features.push(GbivFeature { tag, status, description, notes: vec![] });
        } else if !line.is_empty() {
            if let Some(last) = features.last_mut() {
                last.notes.push(line.to_string());
            }
        }
    }

    features
}

pub fn remove_gbiv_features_by_tag(path: &std::path::Path, tag: &str) -> Result<(), String> {
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(format!("Failed to read GBIV.md: {}", e)),
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

    std::fs::write(path, result).map_err(|e| format!("Failed to write GBIV.md: {}", e))
}

pub fn set_gbiv_feature_status(path: &std::path::Path, color: &str, status: Option<&str>) -> Result<(), String> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("Failed to read GBIV.md: {}", e))?;

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
        return Err(format!("No entry found with color tag [{}]", color));
    }

    let mut result = result_lines.join("\n");
    if content.ends_with('\n') {
        result.push('\n');
    }

    std::fs::write(path, result).map_err(|e| format!("Failed to write GBIV.md: {}", e))
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

    #[test]
    fn returns_empty_when_file_missing() {
        let result = parse_gbiv_md(std::path::Path::new("/nonexistent/GBIV.md"));
        assert!(result.is_empty());
    }

    #[test]
    fn returns_empty_when_no_feature_lines() {
        let f = write_temp("Just some text\nno features here\n");
        let result = parse_gbiv_md(f.path());
        assert!(result.is_empty());
    }

    #[test]
    fn parses_simple_feature_without_tag() {
        let f = write_temp("- Add login page\n");
        let result = parse_gbiv_md(f.path());
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].tag, None);
        assert_eq!(result[0].description, "Add login page");
        assert!(result[0].notes.is_empty());
    }

    #[test]
    fn parses_feature_with_tag() {
        let f = write_temp("- [red] Fix critical bug\n");
        let result = parse_gbiv_md(f.path());
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].tag, Some("red".to_string()));
        assert_eq!(result[0].description, "Fix critical bug");
    }

    #[test]
    fn parses_notes_on_preceding_feature() {
        let f = write_temp("- [blue] Add feature\n  This is a note\n  Another note\n");
        let result = parse_gbiv_md(f.path());
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].notes, vec!["  This is a note", "  Another note"]);
    }

    #[test]
    fn stops_at_separator() {
        let f = write_temp("- First feature\n---\n- Second feature\n");
        let result = parse_gbiv_md(f.path());
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].description, "First feature");
    }

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

    #[test]
    fn parses_feature_with_unrecognized_tag() {
        let f = write_temp("- [purple] Some feature\n");
        let result = parse_gbiv_md(f.path());
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].tag, Some("purple".to_string()));
        assert_eq!(result[0].description, "Some feature");
    }

    #[test]
    fn notes_not_attached_before_first_feature() {
        let f = write_temp("Some header text\n- Feature one\n");
        let result = parse_gbiv_md(f.path());
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].description, "Feature one");
        assert!(result[0].notes.is_empty());
    }

    // Tests for remove_gbiv_features_by_tag

    #[test]
    fn remove_by_tag_removes_matching_entry() {
        let f = write_temp("- [red] Fix critical bug\n- [blue] Add feature\n");
        remove_gbiv_features_by_tag(f.path(), "red").unwrap();
        let result = parse_gbiv_md(f.path());
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].tag, Some("blue".to_string()));
    }

    #[test]
    fn remove_by_tag_noop_when_no_match() {
        let original = "- [blue] Add feature\n";
        let f = write_temp(original);
        remove_gbiv_features_by_tag(f.path(), "red").unwrap();
        let on_disk = std::fs::read_to_string(f.path()).unwrap();
        assert_eq!(on_disk, original);
    }

    #[test]
    fn remove_by_tag_removes_multiple_matching_entries() {
        let f = write_temp("- [red] Bug fix\n- [blue] Feature\n- [red] Another red\n");
        remove_gbiv_features_by_tag(f.path(), "red").unwrap();
        let result = parse_gbiv_md(f.path());
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].tag, Some("blue".to_string()));
    }

    #[test]
    fn remove_by_tag_preserves_content_after_separator() {
        let f = write_temp("- [red] Bug fix\n---\nSome footer content\n");
        remove_gbiv_features_by_tag(f.path(), "red").unwrap();
        let on_disk = std::fs::read_to_string(f.path()).unwrap();
        assert!(on_disk.contains("---\nSome footer content\n"));
        assert!(!on_disk.contains("[red]"));
    }

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

    #[test]
    fn remove_by_tag_no_stray_blank_lines_when_features_separated_by_blank() {
        let f = write_temp("- [red] Bug fix\n\n- [blue] Feature\n");
        remove_gbiv_features_by_tag(f.path(), "red").unwrap();
        let on_disk = std::fs::read_to_string(f.path()).unwrap();
        assert_eq!(on_disk, "- [blue] Feature\n");
    }

    // gbi-4sjo: Test parse entry with [done] status tag
    #[test]
    fn parse_entry_with_done_status_tag() {
        let f = write_temp("- [red] [done] Fix critical bug\n");
        let result = parse_gbiv_md(f.path());
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].tag, Some("red".to_string()));
        assert_eq!(result[0].status, Some("done".to_string()));
        assert_eq!(result[0].description, "Fix critical bug");
    }

    // gbi-luvt: Test parse entry with [in-progress] status tag
    #[test]
    fn parse_entry_with_in_progress_status_tag() {
        let f = write_temp("- [red] [in-progress] Fix critical bug\n");
        let result = parse_gbiv_md(f.path());
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].tag, Some("red".to_string()));
        assert_eq!(result[0].status, Some("in-progress".to_string()));
        assert_eq!(result[0].description, "Fix critical bug");
    }

    // gbi-u6co: Test parse entry without status tag returns status None
    #[test]
    fn parse_entry_without_status_tag_returns_none() {
        let f = write_temp("- [red] Fix critical bug\n");
        let result = parse_gbiv_md(f.path());
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].tag, Some("red".to_string()));
        assert_eq!(result[0].status, None);
        assert_eq!(result[0].description, "Fix critical bug");
    }

    // gbi-wxvs: Test unrecognized second bracket is not a status
    #[test]
    fn parse_unrecognized_second_bracket_is_not_status() {
        let f = write_temp("- [red] [wip] Fix critical bug\n");
        let result = parse_gbiv_md(f.path());
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].tag, Some("red".to_string()));
        assert_eq!(result[0].status, None);
        assert_eq!(result[0].description, "[wip] Fix critical bug");
    }

    // gbi-r3m3: Test set_gbiv_feature_status adds done to entry with no status
    #[test]
    fn set_status_adds_done_to_entry_with_no_status() {
        let f = write_temp("- [red] Fix bug\n");
        set_gbiv_feature_status(f.path(), "red", Some("done")).unwrap();
        let on_disk = std::fs::read_to_string(f.path()).unwrap();
        assert_eq!(on_disk, "- [red] [done] Fix bug\n");
    }

    // gbi-ly6e: Test set_gbiv_feature_status replaces existing status
    #[test]
    fn set_status_replaces_existing_status() {
        let f = write_temp("- [red] [in-progress] Fix bug\n");
        set_gbiv_feature_status(f.path(), "red", Some("done")).unwrap();
        let on_disk = std::fs::read_to_string(f.path()).unwrap();
        assert_eq!(on_disk, "- [red] [done] Fix bug\n");
    }

    // gbi-d7p2: Test set_gbiv_feature_status with None removes status tag (unset)
    #[test]
    fn set_status_with_none_removes_status_tag() {
        let f = write_temp("- [red] [done] Fix bug\n");
        set_gbiv_feature_status(f.path(), "red", None).unwrap();
        let on_disk = std::fs::read_to_string(f.path()).unwrap();
        assert_eq!(on_disk, "- [red] Fix bug\n");
    }

    // gbi-4yh7: Test set_gbiv_feature_status errors when no matching color entry
    #[test]
    fn set_status_errors_when_no_matching_color_entry() {
        let f = write_temp("- [blue] Fix bug\n");
        let result = set_gbiv_feature_status(f.path(), "red", Some("done"));
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_ne!(err, "not implemented".to_string(), "stub should be replaced with real error");
    }

    // gbi-tknr: Test set_gbiv_feature_status preserves notes
    #[test]
    fn set_status_preserves_notes() {
        let f = write_temp("- [red] Fix bug\n  Note line one\n  Note line two\n");
        set_gbiv_feature_status(f.path(), "red", Some("done")).unwrap();
        let on_disk = std::fs::read_to_string(f.path()).unwrap();
        assert_eq!(on_disk, "- [red] [done] Fix bug\n  Note line one\n  Note line two\n");
    }
}

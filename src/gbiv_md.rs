pub struct GbivFeature {
    pub tag: Option<String>,
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
            let (tag, description) = if rest.starts_with('[') {
                if let Some(close) = rest.find(']') {
                    let tag = rest[1..close].to_string();
                    let desc = rest[close + 1..].trim_start().to_string();
                    (Some(tag), desc)
                } else {
                    (None, rest.to_string())
                }
            } else {
                (None, rest.to_string())
            };
            features.push(GbivFeature { tag, description, notes: vec![] });
        } else if !line.is_empty() {
            if let Some(last) = features.last_mut() {
                last.notes.push(line.to_string());
            }
        }
    }

    features
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
}

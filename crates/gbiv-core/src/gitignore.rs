use std::fs;
use std::path::Path;

use crate::error::CoreError;

// @spec WTL-UTIL-016, WTL-UTIL-017, WTL-UTIL-018
pub fn ensure_gitignore_entry(git_dir: &Path, entry: &str) -> Result<(), CoreError> {
    let info_dir = git_dir.join("info");
    fs::create_dir_all(&info_dir)?;
    let exclude_path = info_dir.join("exclude");
    let existing = if exclude_path.exists() {
        fs::read_to_string(&exclude_path)?
    } else {
        String::new()
    };
    if existing.lines().any(|l| l.trim() == entry) {
        return Ok(());
    }
    let mut content = existing;
    if !content.ends_with('\n') && !content.is_empty() {
        content.push('\n');
    }
    content.push_str(entry);
    content.push('\n');
    fs::write(&exclude_path, content)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    // @spec WTL-UTIL-016
    #[test]
    fn creates_info_dir_when_missing() {
        let git_dir = TempDir::new().unwrap();
        assert!(!git_dir.path().join("info").exists());

        ensure_gitignore_entry(git_dir.path(), ".last-branch").unwrap();

        assert!(git_dir.path().join("info").is_dir());
        let exclude = fs::read_to_string(git_dir.path().join("info/exclude")).unwrap();
        assert_eq!(exclude, ".last-branch\n");
    }

    // @spec WTL-UTIL-017
    #[test]
    fn appends_entry_with_leading_newline_when_existing_has_no_trailing_newline() {
        let git_dir = TempDir::new().unwrap();
        let info = git_dir.path().join("info");
        fs::create_dir_all(&info).unwrap();
        fs::write(info.join("exclude"), "existing-entry").unwrap();

        ensure_gitignore_entry(git_dir.path(), ".last-branch").unwrap();

        let exclude = fs::read_to_string(info.join("exclude")).unwrap();
        assert_eq!(exclude, "existing-entry\n.last-branch\n");
    }

    // @spec WTL-UTIL-018
    #[test]
    fn idempotent_when_entry_already_present() {
        let git_dir = TempDir::new().unwrap();
        let info = git_dir.path().join("info");
        fs::create_dir_all(&info).unwrap();
        let original = ".last-branch\n";
        fs::write(info.join("exclude"), original).unwrap();

        ensure_gitignore_entry(git_dir.path(), ".last-branch").unwrap();

        let exclude = fs::read_to_string(info.join("exclude")).unwrap();
        assert_eq!(exclude, original);
    }
}

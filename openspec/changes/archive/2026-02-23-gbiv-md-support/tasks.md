## 1. Parser Module

- [x] 1.1 Create `src/gbiv_md.rs` with a `GbivFeature` struct (fields: `tag: Option<String>`, `description: String`, `notes: Vec<String>`)
- [x] 1.2 Implement `parse_gbiv_md(path: &Path) -> Vec<GbivFeature>` with line-by-line parsing
- [x] 1.3 Handle feature lines starting with `"- "` and extract optional `[tag]` prefix
- [x] 1.4 Attach subsequent non-feature lines as notes to the preceding feature
- [x] 1.5 Stop parsing on a `"---"` line
- [x] 1.6 Return empty vec when file is absent or contains no features

## 2. Module Registration

- [x] 2.1 Add `mod gbiv_md;` to `src/main.rs`

## 3. Status Integration

- [x] 3.1 In `status_command`, call `parse_gbiv_md` with `gbiv_root.join("GBIV.md")`
- [x] 3.2 After the worktree loop, if features are non-empty, print a blank line and dim `"GBIV.md"` header
- [x] 3.3 For each feature, print tag in its ANSI color (or dim `"backlog"` if untagged) followed by description

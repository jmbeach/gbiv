# Rewrite GBIV.md Parser for Flexible Tags

Rewrite the bracket-tag parser in `src/gbiv_md.rs` to support the new tag types with flexible ordering.

## GbivFeature Changes
Add fields: `id: Option<String>`, `by: Option<String>`

## New Parser Logic
Loop through brackets left-to-right, identify each by content pattern:
- `id:xxx` -> sets `id`
- COLORS member (`red`, `orange`, etc.) -> sets `tag` (color)
- `assigned` / `in-progress` / `done` -> sets `status`
- `by:xxx` -> sets `by`
- Anything else -> stop, treat this bracket and everything after as description

## New Helpers
- `find_feature_by_id(features: &[GbivFeature], id: &str) -> Option<&GbivFeature>`
- `set_feature_id(path: &Path, entry_index: usize, id: &str)` — writes `[id:xxx]` onto an entry
- `remove_color_tag_from_entry(path: &Path, color: &str)` — strips `[color]` tag, keeps entry
- `set_feature_by(path: &Path, id: &str, by: Option<&str>)` — sets/clears `[by:name]`
- `remove_feature_by_id(path: &Path, id: &str)` — removes entry + notes by ID

## Modified Functions
- `set_gbiv_feature_status` — add ID-based variant (find by `[id:xxx]` not just `[color]`)
- `remove_gbiv_features_by_tag` — also support removal by ID

## Backward Compat
Old-style entries like `- [red] Fix bug` still parse correctly (color detected as `tag`).

# Rewrite GBIV.md Parser for Flexible Tags

Rewrite the bracket-tag parser in `src/gbiv_md.rs` to support flexible tag ordering
and a new `[by:name]` tag.

## GbivFeature Changes
Add field: `by: Option<String>`

## New Parser Logic
Loop through brackets left-to-right, identify each by content pattern:
- COLORS member (`red`, `orange`, etc.) -> sets `tag` (color)
- `assigned` / `in-progress` / `done` -> sets `status`
- `by:xxx` -> sets `by`
- Anything else -> stop, treat this bracket and everything after as description

Tags may appear in any order, e.g. `- [red] [assigned] [by:jared] feature`.

## New Helpers
- `set_feature_by(path: &Path, color: &str, by: Option<&str>)` — sets/clears `[by:name]`
  on the `[color]` entry

## Backward Compat
Old-style entries like `- [red] Fix bug` still parse correctly.

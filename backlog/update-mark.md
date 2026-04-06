# Update `mark` for Multi-User

Modify `src/commands/mark.rs` to resolve entries via state file and support ID-based marking.

## New Resolution Chain
1. If `--id <id>` provided: find entry directly by ID in GBIV.md
2. If color provided (or inferred from CWD): look up state file (color -> ID) -> find entry by ID
3. If no state entry for that color: error with message

## New Flags
- `--id <id>`: direct ID-based marking, bypasses color->state lookup
- `--assigned`: mark the feature as `[assigned]` (new status value)
- `--id` and `color` are mutually exclusive

## Modified Functions
- `set_gbiv_feature_status`: needs to support finding entries by `[id:xxx]` prefix, not just `[color]` prefix
- Add `--assigned` to the status arg group alongside `--done`, `--in-progress`, `--unset`

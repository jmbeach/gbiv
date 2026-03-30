## Why

Currently there is no way to mark a worktree's task as "in-progress" or "done" within gbiv. The `cleanup` command uses merge status to decide which worktrees to clean, but users may want to mark work as done before or independently of merging. Adding a `mark` command lets users explicitly signal worktree lifecycle state, and filtering `cleanup` to only act on "done" worktrees prevents accidental cleanup of in-progress work.

## What Changes

- Add a new `gbiv mark <--in-progress|--done|--unset> [color]` subcommand that updates the status tag on a feature's GBIV.md entry. Color is optional — when omitted, it is inferred from the current worktree directory (parent folder name matched against COLORS array; error if no match)
- Store worktree status inline in the GBIV.md description — no special parsing. The status tag is simply prepended to the description text (e.g., `- [red] [done] Fix critical bug`). Future enhancement may formalize tag parsing.
- **BREAKING**: Change `gbiv cleanup` (all-color mode) to only clean worktrees whose GBIV.md entry contains `[done]` in the description, skipping all other entries (no status, `[in-progress]`, etc.) even if their branch is merged
- Single-color `gbiv cleanup <color>` retains current behavior (explicit cleanup of a named color regardless of status)
- `gbiv status` output includes the worktree status after the color name when set (e.g., `red [done]  my-feature-branch ...`)
- `gbiv cleanup` removes the GBIV.md entry (including its status tag) after successful cleanup, as it already does today

## Design Decisions

1. **Status tag storage**: Inline in the description string, no separate field or parser changes. Keeps it simple. The status tag shows naturally wherever the description is displayed.
2. **Color branch guard**: `mark` errors if the worktree is on its color branch (no active feature) and no explicit color was passed. Marking only makes sense when there's a feature checked out.
3. **No merge validation**: `--done` does not check merge status. It is pure user intent — the user may have squash-merged via GitHub, or may want to abandon work.
4. **`--unset` returns to no-status state**: Removes the status tag from the description, restoring the entry to how it looked before any `mark` command. From cleanup's perspective, no-status and in-progress are treated identically (both skipped).
5. **Flag exclusivity**: `--in-progress`, `--done`, and `--unset` enforced as mutually exclusive via clap `ArgGroup` (required, single selection).
6. **Missing GBIV.md entry**: `mark` errors with "no GBIV.md entry found for <color>" if the entry doesn't exist. No phantom entries created.
7. **Cleanup rule**: All-color `gbiv cleanup` only acts on entries whose description contains `[done]`. Everything else is skipped.

## Capabilities

### New Capabilities
- `mark`: CLI subcommand to set a worktree's lifecycle status (in-progress, done, or unset)

### Modified Capabilities
- `gbiv-cleanup`: All-color cleanup filters to only worktrees with `[done]` in their description
- `status`: Display worktree status tag after color name in `gbiv status` output
- `gbiv-md-support`: No structural parsing changes needed — status tag lives in the description string

## Impact

- `src/gbiv_md.rs` — add function to update/remove a status tag in a feature's description by color tag
- `src/commands/cleanup.rs` — add `[done]` description filter for all-color cleanup
- `src/commands/status.rs` — display status tag after color name in output
- `src/main.rs` — add `mark` subcommand definition with clap ArgGroup for flag exclusivity
- `src/commands/mod.rs` — add new command module
- New file: `src/commands/mark.rs` — implementation of mark command (color inference, color-branch guard, GBIV.md update)

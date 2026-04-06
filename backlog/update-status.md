# Update `status` for Multi-User

Modify `src/commands/status.rs` to show state file assignments and dangling warnings.

## Default Display (no flags)
For each color worktree, show:
- Color name + current branch + git ahead/behind
- If state file has an assignment: show feature description from GBIV.md
- If state file points to an ID not found in GBIV.md: show dangling warning

## `--backlog` Flag
When passed, also show unassigned GBIV.md entries (entries with IDs that have no color assignment in the state file) in a separate section after the worktree list.

## Dangling State Warnings
If state file references an ID that doesn't exist in GBIV.md, show:
`"Warning: [color] assigned to [id] but entry not found in GBIV.md"`

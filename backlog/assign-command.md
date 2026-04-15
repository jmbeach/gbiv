# `assign` Command (`src/commands/assign.rs`)

New command: `gbiv assign [color]`

Claims the `[color]` entry in GBIV.md for the current user.

## Behavior
1. Load user config; error if `user.name` is unset
   (suggest `gbiv config user.name "<name>"`)
2. Find gbiv root, parse GBIV.md
3. Resolve color:
   - If `[color]` provided: use it
   - Otherwise: infer from CWD (current worktree's color), same as `mark`
4. Find the `[color]` entry in GBIV.md; error if none
5. Add `[assigned]` status and `[by:<user.name>]` to the entry
6. Print: `"Assigned [color] to <user.name>"`

## CLI Definition (clap)
- `color`: optional positional arg, validated against COLORS

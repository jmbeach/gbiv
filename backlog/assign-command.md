# `assign` Command (`src/commands/assign.rs`)

New command: `gbiv assign <id> [color] [--by name]`

## Behavior
1. Find gbiv root, run sync, parse GBIV.md, load state
2. Validate `<id>` exists in GBIV.md (error if not found)
3. If `[color]` omitted: pick first free color (no state assignment AND worktree is on color branch)
4. If no free colors: error listing what each color is doing
5. Write assignment to `.gbiv-state.json`
6. Set `[assigned]` status on the GBIV.md entry
7. If `--by` provided: set `[by:name]` tag on the entry
8. Print confirmation: `"Assigned [id] to [color] worktree"`

## CLI Definition (clap)
- `id`: required positional arg
- `color`: optional positional arg, validated against COLORS
- `--by`: optional string flag

## Context

`gbiv` is a Rust CLI (clap-based) for managing ROYGBIV git worktree structures. Each color worktree lives at `<gbiv_root>/<color>/<repo>/`. The tool already has `find_gbiv_root`, `is_merged_into`, `get_remote_main_branch`, and `parse_gbiv_md` utilities. `GBIV.md` lives in the main worktree at `<gbiv_root>/main/<repo>/GBIV.md` and features are tagged `[<color>]` to associate them with a worktree.

## Goals / Non-Goals

**Goals:**
- Add `gbiv cleanup [<color>]` subcommand
- Detect whether the feature branch in the given worktree is merged into remote main
- If merged: checkout the color branch and pull latest in that worktree
- Remove the GBIV.md entry tagged with that color from the main repo
- Run for all color worktrees when no color is specified

**Non-Goals:**
- Performing any merges
- Deleting worktree directories
- Removing untagged or differently-tagged GBIV.md entries
- Handling worktrees with no remote configured (skip with warning)

## Decisions

### 1. Merge detection: reuse `is_merged_into`
`git_utils::is_merged_into(path, branch, target)` already runs `git merge-base --is-ancestor`. Use it with the feature branch and remote main branch (found via `get_remote_main_branch`).

**Alternative**: `git branch --merged` — rejected, requires more parsing and doesn't handle the remote-tracking branch case as cleanly.

### 2. Worktree already on color branch → skip silently
If the worktree's current branch equals the color name, there is no feature branch in progress. Print a dim notice ("already clean") and skip.

### 3. Git checkout + pull: new helpers in `git_utils`
Add two small functions:
- `checkout_branch(path, branch) -> Result<(), String>`
- `pull(path, remote, branch) -> Result<(), String>`

Consistent with the existing `ProcessCommand`-based pattern; avoids ad-hoc git invocations scattered in the command handler.

### 4. GBIV.md removal: new `remove_gbiv_features_by_tag` in `gbiv_md`
Add a function that reads the file, filters out entries whose tag matches the given color, and writes the file back. Preserve any content after the `---` separator. This keeps all GBIV.md mutation logic in one module.

**Alternative**: Inline string manipulation in the command — rejected, harder to test.

### 5. Multi-color mode: continue on non-fatal errors
When no color is specified, process all colors. If a worktree is missing or its branch is not merged, print a warning and continue to the next color. Only fatal errors (e.g., can't find gbiv root) abort the entire run.

### 6. GBIV.md write format
Serialize features back as `- [<tag>] <description>\n` lines with notes indented as they appear, then re-append the `---` separator and anything after it if present.

## Risks / Trade-offs

- **Race condition on GBIV.md**: If two parallel cleanup runs target different colors, the last writer wins and may lose the other's removal. Acceptable for now; single-user tool.
- **No confirmation prompt**: The command is destructive (removes GBIV.md entries, switches branches). Document this clearly. A `--dry-run` flag can be added later.
- **Checkout fails if worktree is dirty**: `git checkout` will refuse if there are uncommitted changes. Surface the git error message and tell the user to stash or commit first.

## Open Questions

- Should cleanup also delete the feature branch locally/remotely? (Out of scope for this change — can be a follow-up.)

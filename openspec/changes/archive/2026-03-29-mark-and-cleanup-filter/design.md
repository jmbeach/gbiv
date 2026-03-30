## Context

gbiv manages 7 ROYGBIV worktrees for parallel development. Currently, `gbiv cleanup` decides what to clean based solely on git merge status — if a branch is merged into remote main, it gets cleaned up. There is no explicit lifecycle signal from the user.

GBIV.md already tracks features with color tags (e.g., `- [red] Fix critical bug`). Extending this format with a status tag is a natural fit since GBIV.md is already the central place users manage their task assignments.

## Goals / Non-Goals

**Goals:**
- Let users explicitly mark worktree status as `in-progress` or `done` via GBIV.md tags
- Allow unsetting status with `--unset`
- Filter all-color `gbiv cleanup` to only act on entries with `[done]` status
- Display worktree status in `gbiv status` output
- Keep GBIV.md as the single source of truth for worktree assignments and status

**Non-Goals:**
- Custom/arbitrary status values beyond `in-progress` and `done`
- Per-worktree status files (status lives in GBIV.md)
- Changing single-color `gbiv cleanup <color>` behavior (explicit cleanup should always work)
- Status history or timestamps

## Decisions

### 1. Store status as a second bracket tag in GBIV.md

Format: `- [red] [done] Fix critical bug`

The status tag is optional and appears after the color tag. When absent, the feature has no status (idle/backlog). This extends the existing tag syntax naturally.

**Why GBIV.md over per-worktree files**: GBIV.md is already the user-facing file for managing worktree assignments. Keeping status there makes it visible and editable without special commands. Users can also manually edit status by opening the file.

**Alternatives considered:**
- `.gbiv-status` file per worktree: Splits state across many files, not user-visible without commands
- Combined tag like `[red:done]`: Less readable, harder to parse, breaks existing tag format

### 2. Status values are a closed set: `in-progress`, `done`, or unset

Two explicit values plus the ability to unset. No status tag = no status set (feature is idle/backlog). `--unset` removes the status bracket tag, returning the entry to idle.

### 3. All-color cleanup requires `[done]` tag AND merged branch

For safety, `gbiv cleanup` (no color argument) now requires BOTH conditions:
1. GBIV.md entry has `[done]` status tag
2. Branch is merged into remote main (existing check)

This prevents accidental cleanup. Single-color `gbiv cleanup <color>` bypasses the status check since it's an explicit user action.

All-color cleanup SHALL print a summary line with a breakdown of skip reasons (e.g., "1 cleaned (1 not merged, 2 without [done] status)") so the user understands what happened and why.

### 4. Parser extension is backward-compatible

The existing parser reads `[color]` tags. The extended parser will optionally read a second `[status]` bracket after the color. Existing GBIV.md files without status tags parse identically to today.

### 5. `mark` command uses mutually exclusive flags

Signature: `gbiv mark <--in-progress|--done|--unset> [color]`

Status is expressed as mutually exclusive boolean flags (`--in-progress`, `--done`, `--unset`). Exactly one must be provided. Color is an optional positional argument — when omitted, inferred from the current worktree directory.

This avoids positional ambiguity (flags are self-documenting) and follows the pattern of `git remote set-url --push`.

Examples:
- `gbiv mark --done` (from red worktree → marks red as done)
- `gbiv mark --done red` (explicit color)
- `gbiv mark --in-progress blue`
- `gbiv mark --unset red` (removes status tag)

Clap enforces mutual exclusivity via an argument group. If no flag or multiple flags are provided, clap prints usage.

### 6. Color inference uses path components

To detect the current color, walk up from CWD to the gbiv root and check if any path component matches a ROYGBIV color that is a direct child of the root. This is the same structural assumption used by `find_gbiv_root`.

## Risks / Trade-offs

- **[Risk] Users forget to mark before cleanup** → `gbiv cleanup` (all) will skip entries without `[done]` and print a summary indicating how many were skipped and why. Single-color cleanup still works as escape hatch.
- **[Risk] User manually edits GBIV.md with invalid status** → Parser treats unrecognized status tags as part of the description (no crash, just no status recognized).
- **[Trade-off] Breaking change to `gbiv cleanup` behavior** → Users who relied on all-color cleanup acting purely on merge status will need to `gbiv mark --done` first. This is intentional — it adds a safety gate.
- **[Trade-off] GBIV.md lines get longer** → The `[done]` tag adds ~7 characters. Acceptable for the visibility benefit.

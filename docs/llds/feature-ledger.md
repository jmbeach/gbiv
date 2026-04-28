# Feature Ledger

**Created**: 2026-04-23
**Status**: Complete (brownfield mapping)

## Context and Current State

The feature ledger is GBIV.md — a human-editable, git-tracked text file that lives in the main worktree's repository. It serves as the single source of truth for what work is assigned to which color worktree and what lifecycle stage that work is in.

The key design principle is that GBIV.md is a plain text file that a developer edits by hand. gbiv reads and modifies it programmatically, but the format is intentionally simple enough to edit in any text editor. There is no database, no JSON state file, no external tracking — just a text file committed alongside the code.

## GBIV.md Format

```
- [red] [in-progress] Fix the auth bug
  Notes about the auth bug go here
- [green] Add dark mode
- An untagged backlog item

---
# Anything below the separator is ignored by gbiv
```

### Entry Structure

Each feature entry is a line starting with `- `. After the dash:
- Zero or one **color tag**: `[red]`, `[orange]`, etc. (must be a valid ROYGBIV color)
- Zero or one **status tag**: `[done]`, `[in-progress]`, or `[assigned]`
- **Description text**: everything after the tags

Lines that follow a feature entry and don't start with `- ` are **notes** — they belong to the preceding feature.

The `---` separator terminates parsing. Everything below it is preserved but ignored by gbiv. The template uses this area for format documentation.

### Parsing Rules

The parser in `gbiv_md.rs` processes lines top-to-bottom:

1. Lines starting with `- ` begin a new feature
2. For each feature line, extract `[bracketed]` content at the start:
   - If it matches a ROYGBIV color → `tag`
   - If it matches `done` / `in-progress` → `status`
   - Anything else in brackets → treated as part of the description (parsing stops)
3. Non-feature lines after a feature are collected as `notes: Vec<String>`
4. Lines before the first `- ` that aren't features are ignored
5. `---` on its own line stops parsing entirely
6. Missing or empty file returns an empty feature list (no error)

### Data Shape

```rust
pub struct GbivFeature {
    pub tag: Option<String>,        // ROYGBIV color name
    pub status: Option<String>,     // "done" or "in-progress"
    pub description: String,        // feature text after tags
    pub notes: Vec<String>,         // indented lines below
}
```

## Mutations

### `set_gbiv_feature_status(path, color, status)`

Finds the first entry tagged with `[color]` and sets/replaces/removes its status bracket.

- `status = Some("done")` → adds or replaces `[done]`
- `status = Some("in-progress")` → adds or replaces `[in-progress]`
- `status = None` → removes the status bracket entirely

Errors if no entry with that color tag exists (unless unsetting, which is a no-op for missing entries).

### `remove_gbiv_features_by_tag(path, tag)`

Removes all entries with the given color tag, including their note lines. Preserves everything else including the `---` separator and content below it. Cleans up stray blank lines left by removal.

## Mark Command

`gbiv mark (--done | --in-progress | --unset) [<color>]`

### Resolution
1. If `<color>` provided, use it directly
2. Otherwise, infer color from CWD via `infer_color_from_path()`
3. Error if color can't be determined

### Behavior
- Maps `--done` → `set_gbiv_feature_status(path, color, Some("done"))`
- Maps `--in-progress` → `set_gbiv_feature_status(path, color, Some("in-progress"))`
- Maps `--unset` → `set_gbiv_feature_status(path, color, None)`
- Returns a confirmation message on success

### Cross-Component Interactions
- **Reset reads what mark writes**: `reset` (all-color, soft mode) filters by `[done]` status to decide which worktrees to reclaim
- **Status displays what mark writes**: `status` shows the status tag next to each feature's color label
- **Tmux sync/clean reads tags**: tmux commands check which colors have entries to decide which windows to create/remove

## Ledger Display in Status

The `status` command includes a GBIV.md section at the bottom of its output:

```
GBIV.md
  red [done]        Fix the auth bug
  green             Add dark mode
  backlog           An untagged item
```

- Section only appears if GBIV.md exists and has features
- Preceded by a blank line and dim `GBIV.md` header
- Tagged features show color name (in ANSI color) + optional status + description
- Untagged features show dim `backlog` label + description

## Observed Design Decisions

| Decision | Chosen | Alternatives Considered | Rationale |
|---|---|---|---|
| Plain text file | Human-editable `GBIV.md` | JSON, TOML, SQLite | Developers already live in text editors. Git diffs are readable. No tooling required to inspect state. |
| One file in main worktree | Single source of truth | Per-worktree files, shared config | Avoids merge conflicts between worktrees. Main is the canonical location. |
| `---` separator | Content below is ignored | Separate config file, YAML frontmatter | Lets users put documentation in the same file without it being parsed as features. |
| Color tag = worktree assignment | `[red]` means "this is red's work" | Separate assignment mapping | Direct and visible. No indirection layer needed. |
| Status as bracket tag | `[done]`, `[in-progress]` inline | Separate status file, checkboxes | Keeps all feature metadata in one place, one line. |
| Unrecognized brackets stop parsing | `[unknown]` becomes part of description | Error on unknown, ignore and continue | Allows brackets in descriptions (e.g., `[WIP]`) without breaking. |

## Technical Debt & Inconsistencies

1. **First-match semantics for color**: `set_gbiv_feature_status` finds the *first* entry with a given color tag. If multiple entries share a color (which is valid but unusual), only the first is affected. `remove_gbiv_features_by_tag` removes *all* matching entries.

2. **No validation of tag/description boundary**: The parser greedily consumes brackets as tags until it hits one it doesn't recognize. `- [red] [not-a-status] description` would parse `[not-a-status] description` as the description, which is correct but could surprise users who misspell a status.

3. **Mark errors on missing entry for done/in-progress but not unset**: `mark --unset red` succeeds even if there's no `[red]` entry (no-op), but `mark --done red` errors. This asymmetry is intentional (unsetting something absent is fine) but not documented.

## Behavioral Quirks

1. **Notes belong to the preceding feature**: Any non-`- ` line after a feature is a note for that feature. Lines before the first feature are silently discarded. This means a comment at the top of GBIV.md (before any `- ` line) is invisible to gbiv.

2. **Blank line preservation**: The parser/writer is careful about blank lines when removing entries — it collapses adjacent blank lines but preserves the separator and footer.

3. **No duplicate color enforcement**: Two features can both be tagged `[red]`. This isn't an error, but only the first one is affected by `mark` and displayed by `status`. `reset` removes all of them.

4. **GBIV.md is always read from main worktree**: Even if you run `gbiv status` from a color worktree, the ledger is sourced from `main/<repo>/GBIV.md`. This is enforced by `find_repo_in_worktree()` on the main directory.

## References

- `src/gbiv_md.rs` — parser and mutation functions
- `src/commands/mark.rs` — mark command
- `src/commands/status.rs` (lines ~185-220) — ledger display section

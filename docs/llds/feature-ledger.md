# Feature Ledger

**Created**: 2026-04-23
**Status**: Draft — `gbiv start` / `gbiv done` (gbv-1ht) replace `gbiv mark`; parsing and `remove_gbiv_features_by_tag` are brownfield-mapped

## Context and Current State

The feature ledger is GBIV.md — a human-editable, git-tracked text file that lives in the main worktree's repository. It is the single source of truth for which task is on which color worktree.

The key design principle is that GBIV.md is a plain text file a developer can edit by hand. gbiv reads and modifies it programmatically, but the format is intentionally simple enough to edit in any text editor. There is no database or JSON state file. A task's own state (claimed, done, assignee) lives in whatever task system an extension talks to (`docs/llds/extension-runtime.md`); GBIV.md only records the assignment.

Two commands manage an entry's life on a color: `gbiv start` puts a task on the next free color and `gbiv done` takes it off. Both fire extension events around their GBIV.md write. They replace `gbiv mark`: an entry on a color *is* in progress, and a finished task has no entry, so lifecycle status tags are no longer written.

## GBIV.md Format

```
- [red] [gbv-1ht] Pluggable task-system integrations
  Notes about the task go here
- [green] Fix login bug
- An untagged backlog item

---
# Anything below the separator is ignored by gbiv
```

### Entry Structure

Each entry is a line starting with `- `. After the dash, in this order:
- Zero or one **color tag**: `[red]`, `[orange]`, etc.
- Zero or one **status tag**: `[done]` or `[in-progress]`. Legacy: still parsed (so existing files and `gbiv reset`'s `[done]` filter keep working until gbv-tkn retires reset), never written.
- Zero or one **task ID**: a bracketed token matching `[A-Za-z0-9][A-Za-z0-9._-]*` that follows a color tag (and the status tag, if any). Written by `gbiv start` when an extension named the entry.
- **Title** (stored as `description`): everything after the tags.

Lines that follow an entry and don't start with `- ` are **notes** — they belong to the preceding entry.

The `---` separator terminates parsing. Everything below it is preserved but ignored by gbiv. The template uses this area for format documentation.

### Parsing Rules

The parser in `gbiv_md.rs` processes lines top-to-bottom:

1. Lines starting with `- ` begin a new entry
2. Extract `[bracketed]` tokens at the start, left to right:
   - The first bracket → `tag`
   - Next, if it is `done` / `in-progress` → `status`
   - Next, if `tag` is set and the token matches the task-ID pattern → `task_id`
   - The first bracket that matches none of these ends tag parsing; it and the rest are the description
3. Non-feature lines after an entry are collected as `notes: Vec<String>`
4. Lines before the first `- ` that aren't entries are ignored
5. `---` on its own line stops parsing entirely
6. Missing or empty file returns an empty list (no error)

### Data Shape

```rust
pub struct GbivFeature {
    pub tag: Option<String>,        // color name
    pub status: Option<String>,     // legacy "done" / "in-progress"; never written
    pub task_id: Option<String>,    // e.g. "gbv-1ht"
    pub description: String,        // title text after the tags
    pub notes: Vec<String>,         // indented lines below
}
```

## Mutations

### `append_gbiv_entry(path, color, task_id, title)`

Writes `- [<color>] [<task_id>] <title>` (or `- [<color>] <title>` without an ID) as a new entry:

- After the last entry (and its notes) in the parsed region (above `---`), so new work lands at the end of the list.
- If there are no entries, after any preamble (non-blank lines before the first entry, such as a `# Tasks` heading), else at the top of the file, before blank lines and the separator.
- If the file doesn't exist, creates it containing just the entry line.

Everything else in the file, including the separator and the content below it, is preserved byte for byte. The inserted line uses the file's existing line ending (`\r\n` if the file uses it), and the trailing-newline state is kept.

### `remove_gbiv_entry(path, tag, task_id, description)`

Re-reads the file and removes the one entry whose tag, task ID and title all match, with its note lines and the same blank-line cleanup as `remove_gbiv_features_by_tag`. Matching on content rather than position means a hand edit made while extensions ran can't cause the wrong entry to be removed; if the entry is no longer there, it returns an error and changes nothing. Used by `gbiv done`.

### `remove_gbiv_features_by_tag(path, tag)` (unchanged)

Removes all entries with the given color tag, including their note lines. Preserves everything else including the `---` separator and content below it. Cleans up stray blank lines left by removal. Used by `gbiv reset`.

### Removed

`set_gbiv_feature_status` is deleted with `gbiv mark`; nothing else calls it.

### Error Type

`gbiv_md` is library-style and returns `Result<T, GbivMdError>` (a `thiserror`-derived enum):

```rust
#[derive(Debug, thiserror::Error)]
enum GbivMdError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
}
```

`NotFound` goes away with `set_gbiv_feature_status`; "no entry" is now decided by the command handlers, which have the parsed list. Handlers convert errors into `anyhow::Error` with `.context("editing GBIV.md")` at the boundary. See HLD § "Error handling" for the project-wide rule.

## Start Command

`gbiv start <task> [--color <color>]`

`<task>` is either a task ID an extension recognizes (`gbv-1ht`) or free text (`"Fix login bug"`). gbiv trims it for its own checks and passes it to extensions exactly as typed. After trimming it must be non-empty and a single line, and must not start with `[` (it would read back as a tag or task ID).

### Flow

1. Find the gbiv root, the main repo and its GBIV.md; load the palette and config.
2. **Pick a color.** A color is *free* when no GBIV.md entry carries its tag (whatever its worktree holds).
   - With `--color`: it must be in the active palette and free, else error.
   - Without: the first free color in palette order (ROYGBIV, then extras).
   - None free → error `no free color (all N colors have GBIV.md entries)`. No extension loads.
3. **Duplicate check.** If an entry already has `task_id` equal to `<task>` (case-insensitive), error `<task> is already on <color>`. No extension loads.
   Steps 2–6 run while holding `<main>/.gbiv/ledger.lock` (created exclusively; `.gbiv/` is already git-excluded). If the lock exists, the command fails at once with `another gbiv start/done is running (remove <path> if not)`. `gbiv done` takes the same lock.
4. Load extensions and fire `before_start(arg, color)`. A veto stops here: nothing is written, exit 1.
5. **Name the entry.** If a handler returned `{id, title}`, use them; re-run the duplicate check against the returned `id` (error if it's already on a color). Otherwise the title is `<task>` trimmed and there is no ID; if `<task>` matches the task-ID pattern (a single token), print a warning that no extension recognized it and it was added as free text.
6. `append_gbiv_entry`. If the write fails, exit 1 with an error that says the extensions already ran (so a claim may need undoing in the task system); no `after_*` event fires.
7. Fire `after_start`; print any warnings.
8. Print `started <id> on <color>: <title>` (or `started on <color>: <title>` without an ID).

The worktree is never touched: no branch is created or checked out.

## Done Command

`gbiv done [<task|color>]`

### Resolution
1. If the argument is a name in the active palette (exact, case-sensitive match, like every other color argument) → that color.
2. Otherwise, if it is given → a task ID: the entry whose `task_id` matches it (case-insensitive). None → error `no GBIV.md entry for <task>`; more than one → error listing them.
3. If omitted → infer the color from CWD via `infer_color_from_path()`; error if that fails.

A color resolves to the entry tagged with it. No entry → error `no GBIV.md entry for <color>`. More than one entry with that tag (only possible through hand edits) → error listing them and asking for the task ID instead.

### Flow
1. Resolve the entry as above.
2. Load extensions and fire `before_done(color, id, title)`. A veto stops here: the entry stays, exit 1.
3. `remove_gbiv_entry`.
4. Fire `after_done`; print any warnings.
5. Print `done <id> (<color>)` (or `done <color>: <title>` without an ID).

The worktree is never touched.

## Cross-Component Interactions

- **Extension Runtime** — `start` and `done` are the only callers of its events (`docs/llds/extension-runtime.md`).
- **Status displays what start writes** — `status` shows each entry's color, task ID if any, and title (Observation segment).
- **Tmux sync/clean reads tags** — tmux commands check which colors have entries to decide which windows to create/remove. Unchanged.
- **Reset reads the legacy `[done]` tag** — `reset` (all-color, soft mode) still filters on `[done]`. Since nothing writes it any more, all-color soft reset finds nothing to reset; `reset <color>` still works. gbv-tkn removes reset.

## Ledger Display in Status

The `status` command includes a GBIV.md section at the bottom of its output:

```
GBIV.md
  red               [gbv-1ht] Pluggable task-system integrations
  green             Fix login bug
  backlog           An untagged item
```

- Section only appears if GBIV.md exists and has features
- Preceded by a blank line and dim `GBIV.md` header
- Tagged entries show the color name (in ANSI color), then the task ID in brackets if present, then the title; a legacy status tag still displays as today
- Untagged entries show a dim `backlog` label + description

## Design Decisions

| Decision | Chosen | Alternatives Considered | Rationale |
|---|---|---|---|
| Plain text file | Human-editable `GBIV.md` | JSON, TOML, SQLite | Developers already live in text editors. Git diffs are readable. No tooling required to inspect state. |
| One file in main worktree | Single source of truth | Per-worktree files, shared config | Avoids merge conflicts between worktrees. Main is the canonical location. |
| `---` separator | Content below is ignored | Separate config file, YAML frontmatter | Lets users put documentation in the same file without it being parsed as features. |
| Color tag = worktree assignment | `[red]` means "this is red's work" | Separate assignment mapping | Direct and visible. No indirection layer needed. |
| Lifecycle commands | `gbiv start` / `gbiv done` | Keep `gbiv mark` with status tags | Being on a color already means in progress; done means off the color. Two verbs cover what mark did, and they are where extension events fire. |
| Status tags | Parsed (legacy), never written | Drop parsing too | Existing files keep reading correctly, and `reset`'s filter keeps compiling until gbv-tkn. |
| Task ID in GBIV.md | Bracketed token after the color tag | `[task:<id>]` prefix; ID in the title text | Reads naturally (`- [red] [gbv-1ht] …`) and matches how vat bullets look. |
| Free color | No GBIV.md entry for it | Also require a clean / merged worktree | Simple and predictable; settled in the HLD. |
| `--color` override | Allowed, must be free | Always automatic | Lets a user keep a task on a color they prefer. |
| Unrecognized brackets stop tag parsing | Becomes part of the title | Error on unknown, ignore and continue | Allows brackets in titles without breaking. |

## Technical Debt & Inconsistencies

1. **Task-ID pattern is broad**: any `[A-Za-z0-9…]` token after the color is read as a task ID, so a hand-written `- [red] [WIP] thing` now parses `WIP` as the ID rather than as title text. gbiv itself never writes such a line (`start` refuses a title starting with `[`).

2. **No validation of tag/description boundary**: a misspelled status (`- [red] [don] x`) is read as task ID `don`.

## Behavioral Quirks

1. **Notes belong to the preceding entry**: Any non-`- ` line after an entry is a note for it. Lines before the first entry are silently discarded, so a comment at the top of GBIV.md is invisible to gbiv.

2. **Blank line preservation**: The writer is careful about blank lines when removing entries — it collapses adjacent blank lines but preserves the separator and footer.

3. **GBIV.md is always read from the main worktree**: Even when run from a color worktree, the ledger is `main/<repo>/GBIV.md`, located via `find_repo_in_worktree()` on the main directory.

4. **GBIV.md changes are not committed**: `start` and `done` edit the file in the main worktree and leave committing to the user, like every other gbiv write.

## References

- `src/gbiv_md.rs` — parser and mutation functions
- `src/commands/start.rs`, `src/commands/done.rs` — the lifecycle commands (replacing `src/commands/mark.rs`)
- `src/commands/status.rs` — ledger display section
- `docs/llds/extension-runtime.md` — the events these commands fire

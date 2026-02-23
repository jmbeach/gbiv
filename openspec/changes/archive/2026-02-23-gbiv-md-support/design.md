## Context

`gbiv status` currently shows only git worktree state for ROYGBIV color directories. Users may maintain a `GBIV.md` file in the repo root to track features and backlog items, but this file is completely invisible to the tool today.

The codebase is Rust. The status command lives in `src/commands/status.rs` and calls `find_gbiv_root` to locate the repo root. COLORS is a static slice of worktree names.

## Goals / Non-Goals

**Goals:**
- Parse `GBIV.md` from the gbiv root directory
- Display parsed features (tagged and untagged) as a section in `gbiv status` output
- Handle gracefully when the file is absent or empty

**Non-Goals:**
- Writing to or modifying `GBIV.md`
- Acting on GBIV.md content (e.g., auto-creating worktrees)
- Fuzzy search or export/import of items
- Validating or linting GBIV.md format

## Decisions

### 1. New module `src/gbiv_md.rs`
A dedicated parser module keeps GBIV.md logic separate from git worktree logic. The module exposes a `parse_gbiv_md(path: &Path) -> Vec<GbivFeature>` function.

**Alternatives considered:**
- Inline parsing in `status.rs` — rejected; mixes concerns and makes the parser hard to test independently.

### 2. Line-by-line parser (no external crate)
The format is simple enough to parse line-by-line without a markdown parser:
- Lines starting with `"- "` are feature lines; optionally followed by a `[tag]` prefix
- Subsequent lines that don't start with `"- "` and aren't `"---"` are notes for the previous feature
- `"---"` on its own line terminates parsing

**Alternatives considered:**
- Using a markdown parser crate — overkill; the format is a subset of markdown lists, not full markdown.

### 3. Display after worktree rows in `gbiv status`
GBIV.md features are shown as a separate section below the existing worktree table, separated by a blank line and a dim header like `GBIV.md`. Tagged features show their color; untagged items are displayed as backlog.

**Alternatives considered:**
- Interleaving GBIV.md rows with worktree rows — rejected; they represent different things and mixing them would be confusing.

### 4. Silent no-op when file absent
If `GBIV.md` does not exist, `status` output is unchanged. No warning is printed. This keeps the file truly optional.

## Risks / Trade-offs

- **Format ambiguity**: Notes that accidentally start with `- ` will be treated as new features. Mitigation: document the format clearly; the behavior is consistent with the spec.
- **Large files**: No line limit is imposed. Very large GBIV.md files will be read fully. Acceptable for now given typical use.
- **Color tag validation**: Tags like `[purple]` that aren't valid GBIV colors will be displayed as-is without error. Validation is out of scope.

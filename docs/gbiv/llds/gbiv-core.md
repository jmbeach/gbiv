# gbiv-core

**Created**: 2026-05-07
**Status**: Design — extraction from existing `gbiv` crate

## Context

Both `gbiv` (the worktree CLI) and `roy` (the on-demand orchestration daemon) need to agree on a small set of foundational facts about a gbiv project: what the seven canonical colors are, how to locate the gbiv root from any CWD, how to find the user's repo inside a color worktree, and how to register entries in `.git/info/exclude` so gbiv-managed state files don't show up as untracked.

Today these utilities live inside the `gbiv` binary crate (`src/colors.rs`, `src/git_utils.rs`). Roy cannot depend on a binary crate, and even if it could, several gbiv-specific concerns (ANSI color codes, worktree-creation error variants, rebase logic) don't belong in roy. The HLD has long named `gbiv-core` as the shared library crate that resolves this; multiple existing LLDs (`worktree-lifecycle`, `cli-and-palette`, `tmux-mirror`, plus all of roy's LLDs) already reference it as the home for these primitives. This LLD describes that crate as a coherent component for the first time.

This is a refactor: the current behavior is preserved exactly, the public API on the `gbiv-core` boundary is the existing internal API of the affected functions, and no new functionality is introduced.

## Workspace Layout (After Extraction)

```
gbiv/                                  ← repo root
├── Cargo.toml                         ← [workspace] only
├── crates/
│   ├── gbiv-core/                     ← NEW library crate
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs                 ← re-exports module surfaces
│   │       ├── colors.rs              ← COLORS, is_valid_color, infer_color_from_path
│   │       ├── root.rs                ← GbivRoot, find_gbiv_root, find_repo_in_worktree, is_git_repo
│   │       ├── gitignore.rs           ← ensure_gitignore_entry
│   │       └── error.rs               ← CoreError
│   └── gbiv/                          ← MOVED from src/ at repo root
│       ├── Cargo.toml                 ← depends on gbiv-core via path
│       └── src/                       ← unchanged structure (main.rs, commands/, …)
```

The repo-root `Cargo.toml` becomes a workspace manifest only:

```toml
[workspace]
members = ["crates/gbiv-core", "crates/gbiv"]
resolver = "2"
```

`crates/gbiv/Cargo.toml` keeps the existing `[package]` block (so `cargo install --path crates/gbiv` continues to work) and adds `gbiv-core = { path = "../gbiv-core" }`. The published binary name stays `gbiv`.

Roy's future crate will land as a third workspace member (`crates/roy`) under the gbv-7r4 / gbv-x2v tickets — out of scope here, but the layout is chosen to make that drop-in.

## Public API

`gbiv-core` exposes four module surfaces. Everything else stays private.

### `gbiv_core::colors`

```rust
pub const COLORS: [&str; 7];       // ["red","orange","yellow","green","blue","indigo","violet"]
pub fn is_valid_color(name: &str) -> bool;
pub fn infer_color_from_path(cwd: &Path, gbiv_root: &Path) -> Option<&'static str>;
```

`COLORS` is the single canonical ordering. `is_valid_color` is a pure membership check; roy uses it to validate `:color` URL params, gbiv uses it for handler-level argument validation. `infer_color_from_path` matches the first path component below the gbiv root against `COLORS` and returns the `&'static str` from the slice (so callers can compare by pointer or use the string in `info/exclude` paths).

### `gbiv_core::root`

```rust
pub struct GbivRoot {
    pub root: PathBuf,
    pub folder_name: String,
}

pub fn find_gbiv_root(start: &Path) -> Option<GbivRoot>;
pub fn find_repo_in_worktree(worktree_dir: &Path) -> Option<PathBuf>;
pub fn is_git_repo(path: &Path) -> bool;
```

`find_gbiv_root` walks up from `start` looking for the canonical layout (a `main/<folder>` git repo plus at least one ROYGBIV color subdirectory). `find_repo_in_worktree` returns the first child directory that contains a `.git` entry. `is_git_repo` shells out to `git rev-parse --git-dir` and reports success.

These three travel together because `find_gbiv_root` calls `is_git_repo` internally, and roy's startup sequence calls `find_gbiv_root` → `find_repo_in_worktree` back-to-back.

### `gbiv_core::gitignore`

```rust
pub fn ensure_gitignore_entry(git_dir: &Path, entry: &str) -> Result<(), CoreError>;
```

Idempotent append to `<git_dir>/info/exclude`. Creates the `info/` directory if missing, reads the file (or treats absent as empty), checks each line trimmed against `entry`, and appends (with leading newline if needed) only if not already present. Caller is responsible for resolving the *common* git dir (linked worktrees share a single `info/exclude`); roy and gbiv both already do this via `git rev-parse --git-common-dir`.

`git_dir` here means the directory that *contains* `info/exclude` — i.e., the git common dir. The function does not itself resolve gitlinks; that resolution stays in gbiv's `git_utils` since it requires shelling out to git and roy already has its own path-resolution code for this.

### `gbiv_core::error`

```rust
#[derive(Debug, thiserror::Error)]
pub enum CoreError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
}
```

Deliberately minimal. The only fallible function in core today is `ensure_gitignore_entry`, and its only failure mode is I/O. Adding variants is cheap when new fallible helpers are added later (e.g., the future `tmux::tmux_available()` shared primitive).

`gbiv`'s existing `GitError` keeps its current variants (`NotInGbivProject`, `GitFailed`, `RebaseConflict`, `WorktreeAlreadyExists`, `Io`, `Other`) and gains a `#[from] CoreError` arm so call sites that bubble up an `ensure_gitignore_entry` error compose cleanly:

```rust
#[derive(Debug, thiserror::Error)]
pub enum GitError {
    // existing variants…
    #[error(transparent)]
    Core(#[from] gbiv_core::CoreError),
}
```

This keeps gbiv-specific error variants out of core and lets core grow its own error surface independently.

## Migration Mechanics

The migration is mostly a `git mv` plus import-path rewrites. Concretely:

1. **Create the workspace skeleton.** Move `src/` to `crates/gbiv/src/`, move `Cargo.toml`'s `[package]` block + dependencies to `crates/gbiv/Cargo.toml`, replace the root `Cargo.toml` with a `[workspace]` manifest. Move tests likewise (`tests/` if any → `crates/gbiv/tests/`). Verify the bin still builds and runs before touching gbiv-core.

2. **Create `crates/gbiv-core` with empty modules.** Add `gbiv-core` as a path dep in `crates/gbiv/Cargo.toml`. Verify the workspace compiles.

3. **Move `colors.rs` content into `gbiv_core::colors`.** Keep `ansi_color`, `RESET`, `YELLOW`, `GREEN`, `RED`, `DIM` in `crates/gbiv/src/colors.rs` (now a thin gbiv-only module). Move `COLORS` and add `is_valid_color`. Re-export from `crates/gbiv/src/colors.rs` only if needed for terseness; otherwise call sites import from `gbiv_core::colors` directly.

4. **Move root-discovery helpers.** Cut `GbivRoot`, `find_gbiv_root`, `find_repo_in_worktree`, `is_git_repo`, and `infer_color_from_path` (plus their tests) out of `git_utils.rs` into `gbiv_core::root` and `gbiv_core::colors` respectively. Tests move with them.

5. **Move `ensure_gitignore_entry` into `gbiv_core::gitignore`.** Update the gbiv `GitError` enum to add `#[from] CoreError`. Verify call sites in `commands/init.rs`, `commands/rebase_all.rs`, etc., still compile (the `?` operator will compose through the new `From` impl).

6. **Sweep imports.** Replace `crate::colors::COLORS` with `gbiv_core::colors::COLORS`, `crate::git_utils::find_gbiv_root` with `gbiv_core::root::find_gbiv_root`, etc. Same for the other moved items.

Each step is a checkpoint: the workspace must compile and `gbiv` watch tests must pass before moving to the next.

## Testing

Existing tests for `find_gbiv_root` (currently in `git_utils.rs`'s `#[cfg(test)] mod tests`) move with the function into `crates/gbiv-core/src/root.rs`. They already use `tempfile`-pattern scratch directories under `/tmp` that init their own git repos — no live-repo dependency, no changes needed beyond the import update.

`ensure_gitignore_entry` does not currently have a unit test. As part of this migration we add one in `crates/gbiv-core/src/gitignore.rs` covering: (a) creates `info/` if missing and writes the entry; (b) is idempotent — second call with the same entry does not duplicate the line; (c) appends to an existing exclude file without trailing newline.

`is_valid_color` and `infer_color_from_path` get small unit tests in `crates/gbiv-core/src/colors.rs` covering the obvious membership and path-component cases.

End-to-end behavior is unchanged, so the existing `gbiv` integration tests (init, rebase-all, status, etc.) act as the regression backstop. Per testing memory: `cargo nextest run` from the workspace root, watch logs in `watch.log` / `watch-tests.log`.

## Decisions & Alternatives

**Workspace split with both `gbiv` and `gbiv-core` under `crates/`** rather than `gbiv-core` alongside the bin at the root. Cleaner long-term: roy lands as another `crates/` member, no special-cased binary at the workspace root. Trade-off: bigger one-time diff (every path under `src/` moves), and `cargo install --path .` users have to switch to `cargo install --path crates/gbiv`. Acceptable — this is a pre-1.0 tool and the change is documented in the PR.

**Narrow `CoreError` (Io only) rather than moving `GitError` whole into core.** `GitError` carries gbiv-specific variants (`WorktreeAlreadyExists`, `RebaseConflict`) that have no place in a library shared with roy. Core gets its own minimal error type and gbiv composes via `#[from] CoreError`. Matches the HLD's "narrow typed errors per module" guidance and lets core grow its error surface independently as more shared primitives land (`TmuxError`, etc.).

**`is_git_repo` and `infer_color_from_path` moved alongside the named items** in the ticket. `find_gbiv_root` calls `is_git_repo`, so leaving it behind would force core to either duplicate it or expose its private dependency. `infer_color_from_path` depends only on `COLORS` and `Path` and is a natural fit in `gbiv_core::colors`. Roy doesn't need it today, but the cohesion argument (it belongs with the other root-relative path helpers) is strong.

**ANSI codes (`ansi_color`, `RESET`, `DIM`, etc.) stay in gbiv.** Roy is JSON-only and emits no terminal escapes; the `cli-and-palette` LLD already calls this out. These constants stay in `crates/gbiv/src/colors.rs`.

**`gbiv-core` is `publish = false`.** It exists only as a workspace-internal library shared between the `gbiv` and (future) `roy` binaries. Marking it unpublishable in its `Cargo.toml` prevents accidental release to crates.io and signals the API stability contract: gbiv-core's surface can change in lockstep with its consumers without semver concerns.

**Concurrent writes to `info/exclude` are not made atomic in this extraction.** The current `ensure_gitignore_entry` is read-modify-write and can race if two writers target the same `info/exclude` file. Today this is benign — `rebase-all` does write per-color excludes from threads, but each thread targets a distinct git common dir (one per worktree). When `roy` lands, `roy start` may run concurrently with gbiv invocations against the *same* common dir; that's the right time to add file-locking or a verified read-modify-write loop. Tracked outside this ticket; current behavior preserved verbatim.

**`ensure_gitignore_entry` does not resolve gitlinks.** Both gbiv and roy already resolve the common git dir via `git rev-parse --git-common-dir` before calling this function, so pushing that responsibility into core would just duplicate logic. Core takes a `git_dir` and writes; resolution stays at the call site.

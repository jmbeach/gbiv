# gbiv-core

**Created**: 2026-05-07
**Status**: Design

## Context

`gbiv-core` is the workspace-internal library crate that holds the small set of foundational primitives both `gbiv` (the worktree CLI) and `roy` (the on-demand orchestration daemon) depend on: the canonical color list, root-discovery utilities, and the helper that registers entries in `.git/info/exclude` for gbiv-managed state files.

It exists because the two binaries must agree on these facts. If they disagreed about the color list, roy could accept `:purple` while gbiv rejected it; if they disagreed about how to find the gbiv root, the daemon could resolve a different `main/<repo>/` than the CLI; if they disagreed about gitignore registration, one could leave state files showing as untracked while the other hid them. The crate is the single source of truth.

`gbiv-core` is deliberately small. It owns *only* the primitives that need to be shared. Anything binary-specific — ANSI color codes, worktree-creation logic, rebase orchestration, HTTP routing, tmux session mutation — lives in the binary that owns it. The HLD and roy's HLD both name `gbiv-core` as this shared library; this LLD describes what's inside.

## Workspace Position

```
gbiv/                                  ← repo root
├── Cargo.toml                         ← [workspace] manifest
├── crates/
│   ├── gbiv-core/                     ← this crate
│   │   ├── Cargo.toml                 ← publish = false
│   │   └── src/
│   │       ├── lib.rs                 ← module declarations
│   │       ├── colors.rs
│   │       ├── root.rs
│   │       ├── gitignore.rs
│   │       └── error.rs
│   ├── gbiv/                          ← worktree CLI bin
│   └── roy/                           ← on-demand orchestration daemon (future)
```

Both binary crates depend on `gbiv-core` via path. The crate is `publish = false` — it has no independent release cycle and no semver contract beyond its workspace. Its API is allowed to change in lockstep with its consumers.

## Module Surfaces

### `gbiv_core::colors`

```rust
pub const COLORS: [&str; 7];       // ["red","orange","yellow","green","blue","indigo","violet"]
pub fn is_valid_color(name: &str) -> bool;
pub fn infer_color_from_path(cwd: &Path, gbiv_root: &Path) -> Option<&'static str>;
```

`COLORS` is the single canonical ordering of ROYGBIV. Every iteration over the colors — gbiv's clap `PossibleValuesParser`, roy's `/sessions` listing — reads from this slice; nobody hard-codes the seven names.

`is_valid_color` is a pure membership check. Roy uses it to validate `:color` URL params at the routing layer; gbiv uses it for handler-level argument validation.

`infer_color_from_path` matches the first path component below the gbiv root against `COLORS` and returns the `&'static str` from the slice (so callers may compare by pointer or use the string in `info/exclude` paths). Returns `None` if `cwd` is outside the gbiv root or the first component isn't a valid color.

ANSI color codes (`ansi_color`, `RESET`, `DIM`, …) are *not* in this module — they live in the gbiv binary's own `colors` module. Roy emits JSON only and has no use for terminal escapes.

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

`find_gbiv_root` walks up from `start` looking for the canonical layout: a `main/<folder>` directory that is a git repository, plus at least one ROYGBIV color subdirectory. Returns `Some(GbivRoot { root, folder_name })` where `root` is the directory containing `main/` and the color dirs, and `folder_name` is the repo directory name inside `main/`.

`find_repo_in_worktree` returns the first child directory of `worktree_dir` that contains a `.git` entry (file or directory). Used to resolve `<color>/<folder>/` from a `<color>/` worktree dir.

`is_git_repo` shells out to `git rev-parse --git-dir` and reports whether the command succeeded. Internal callers (`find_gbiv_root`) use it to confirm a candidate is a real repo.

These three travel together: `find_gbiv_root` calls `is_git_repo`, and roy's startup path calls `find_gbiv_root` and `find_repo_in_worktree` back-to-back to resolve `<gbiv-root>/main/<repo>/`.

### `gbiv_core::gitignore`

```rust
pub fn ensure_gitignore_entry(git_dir: &Path, entry: &str) -> Result<(), CoreError>;
```

Idempotent append to `<git_dir>/info/exclude`. Creates the `info/` directory if missing. Reads the file (treating absent as empty), and appends `entry` on its own line only if no existing line trims to that exact string. Inserts a leading newline if the existing content lacks a trailing one.

`git_dir` is the directory that contains `info/exclude` — the git *common* dir. Linked worktrees share a single `info/exclude`; resolving the common dir (via `git rev-parse --git-common-dir`, gitlink files, etc.) is the caller's responsibility, not `gbiv-core`'s. Both binaries already do that resolution today.

### `gbiv_core::error`

```rust
#[derive(Debug, thiserror::Error)]
pub enum CoreError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
}
```

Deliberately minimal. The only fallible operation in core is `ensure_gitignore_entry`, and its only failure mode is I/O. The enum exists as a distinct type — rather than `std::io::Error` directly — so new fallible primitives can extend it without churn at every call site.

The gbiv binary's `GitError` carries a `#[from] CoreError` arm so call sites that bubble up an `ensure_gitignore_entry` error compose through `?` cleanly. gbiv-specific failure variants (`WorktreeAlreadyExists`, `RebaseConflict`, `GitFailed`, …) live with `GitError` in the gbiv binary; they have no place in a library shared with roy.

## What `gbiv-core` Is Not

- Not a place for shell-out wrappers around git in general. The dozens of `git checkout` / `git rebase` / `git stash` / `git fetch` helpers in gbiv are gbiv-specific orchestration and stay in the gbiv binary's `git_utils`. Only the primitives roy genuinely shares (root discovery, color validation, ignore registration) cross the boundary.
- Not a place for ANSI codes, terminal formatting, or any code that assumes a TTY consumer.
- Not a place for tmux logic *yet*. A future `gbiv_core::tmux` module is documented in `tmux-mirror.md` and `roy/llds/tmux-driver.md` for primitives both binaries need (`tmux_available`, `has_session`, folder-derived session naming). It lands when roy lands.
- Not an independently published crate. `publish = false`. Its public API exists for the workspace; consumers are versioned together.

## Decisions & Alternatives

**Workspace split with both `gbiv` and `gbiv-core` under `crates/`** rather than `gbiv-core` alongside the gbiv bin at the repo root. Cleaner long-term: roy lands as a third `crates/` member with no special-cased binary at the workspace root. Trade-off accepted: every path under the gbiv binary moves, and `cargo install --path .` users switch to `cargo install --path crates/gbiv`.

**Narrow `CoreError` (Io only) rather than moving `GitError` whole into core.** `GitError` carries gbiv-specific variants that have no place in a library shared with roy. Core has its own minimal error type and gbiv composes via `#[from] CoreError`. Matches the HLD's "narrow typed errors per module" guidance and lets each binary's error surface evolve independently as more shared primitives land (`TmuxError`, etc.).

**`is_git_repo` and `infer_color_from_path` are part of core.** `find_gbiv_root` calls `is_git_repo` internally; leaving it in the gbiv binary would force core to either duplicate it or expose a private dependency. `infer_color_from_path` depends only on `COLORS` and `Path` and belongs with the other root-relative path helpers. Roy doesn't call `infer_color_from_path` today, but cohesion outweighs minimalism here.

**`ensure_gitignore_entry` is not made atomic across concurrent writers.** The current read-modify-write implementation can race if two writers target the same `info/exclude`. Today this is benign — `gbiv rebase-all` does write per-color excludes from threads, but each thread targets a distinct git common dir (one per worktree). When roy lands, `roy start` may run concurrently with gbiv invocations against the *same* common dir; that's the right time to add file-locking or a verified read-modify-write loop. Out of scope here.

**`gbiv-core` is `publish = false`.** It exists only to share primitives between binaries in this workspace. Marking it unpublishable prevents accidental release to crates.io and signals the API stability contract: gbiv-core's surface is allowed to change in lockstep with its consumers.

**`GbivRoot` exposes its fields as `pub`** rather than via accessors. Plain data, no invariants beyond "constructed only by `find_gbiv_root`," no callers that would benefit from encapsulation. Accessors would be ceremony.

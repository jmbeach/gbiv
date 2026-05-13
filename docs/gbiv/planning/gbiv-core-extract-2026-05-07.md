# gbiv-core extraction — implementation plan

**Ticket**: gbv-c8h
**Created**: 2026-05-07
**Status**: Planned

## Purpose

Move the foundational primitives shared between `gbiv` (the worktree CLI) and `roy` (the on-demand orchestration daemon, gbv-9p1+) out of the gbiv binary into a workspace-internal library crate so both binaries can depend on a single source of truth.

This is a refactor: no behavior changes, no new functionality. Every function being moved keeps its current signature; the only externally visible artifact is a workspace layout change.

## Surface of the crate

`gbiv-core` owns four module surfaces. This is the full inventory; the existing component LLDs annotate each function inline at its conceptual home.

| Module | Items | Owner LLD |
|---|---|---|
| `gbiv_core::colors` | `COLORS: [&str; 7]`, `is_valid_color`, `infer_color_from_path` | `cli-and-palette.md` (COLORS, is_valid_color), `worktree-lifecycle.md` (infer_color_from_path) |
| `gbiv_core::root` | `GbivRoot`, `find_gbiv_root`, `find_repo_in_worktree`, `is_git_repo` | `worktree-lifecycle.md` |
| `gbiv_core::gitignore` | `ensure_gitignore_entry` | `worktree-lifecycle.md` |
| `gbiv_core::error` | `CoreError` | (n/a — implementation detail) |

`is_valid_color` is the only item that does not exist in the current source — it's a small new helper introduced as part of this migration so `roy` has a non-iterator way to validate `:color` URL params and gbiv command handlers have a one-line replacement for the ad-hoc `COLORS.contains(&color)` checks.

Everything else moves with its current signature, current tests, and current behavior.

## What does not cross the boundary

- ANSI escape codes (`ansi_color`, `RESET`, `DIM`, `YELLOW`, `GREEN`, `RED`) — terminal output is gbiv-only; `roy` emits JSON.
- The git command wrappers in `git_utils.rs` (`checkout_branch`, `reset_hard`, `stash_push`, `rebase_onto`, `fetch_remote`, `pull`, `get_quick_status`, `get_remote_main_branch`, `get_existing_branches`, `is_merged_into`, `get_last_commit_age`, `resolve_git_dir`, `get_git_dir`) — `roy` does not touch git.
- gbiv-specific error variants (`WorktreeAlreadyExists`, `RebaseConflict`, `GitFailed`, `NotInGbivProject`, `Other`) — these stay with `GitError` in the gbiv binary.
- `GBIV.md` parsing, worktree creation, rebase orchestration, tmux logic — all gbiv-binary concerns.

A future `gbiv_core::tmux` module is documented in `tmux-mirror.md` and `roy/llds/tmux-driver.md`; it lands with gbv-x2v, not here.

## Target workspace layout

```
gbiv/                                  ← repo root
├── Cargo.toml                         ← [workspace] manifest only
├── crates/
│   ├── gbiv-core/                     ← new library crate
│   │   ├── Cargo.toml                 ← publish = false
│   │   └── src/
│   │       ├── lib.rs                 ← module declarations
│   │       ├── colors.rs
│   │       ├── root.rs
│   │       ├── gitignore.rs
│   │       └── error.rs
│   └── gbiv/                          ← moved from src/ at repo root
│       ├── Cargo.toml                 ← [package] + path dep on gbiv-core
│       └── src/                       ← unchanged internal structure
```

The repo-root `Cargo.toml` becomes a workspace manifest. `crates/gbiv/Cargo.toml` keeps the existing `[package]` block so `cargo install gbiv` from crates.io continues to work; local installs switch from `cargo install --path .` to `cargo install --path crates/gbiv`. The published binary name stays `gbiv`.

`roy` lands as a third workspace member (`crates/roy`) under gbv-9p1+ — out of scope here, but this layout is chosen so it drops in cleanly.

## Error model

`gbiv-core` owns a narrow `CoreError`:

```rust
#[derive(Debug, thiserror::Error)]
pub enum CoreError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
}
```

Only `ensure_gitignore_entry` is fallible today and its only failure mode is I/O. The enum exists as a distinct type (rather than `std::io::Error` directly) so new fallible primitives can extend it without churn at every call site.

The gbiv binary's `GitError` gains a `#[from] CoreError` arm so call sites bubbling up an `ensure_gitignore_entry` error compose through `?` cleanly without an explicit conversion.

## Crate metadata

`crates/gbiv-core/Cargo.toml`:

```toml
[package]
name = "gbiv-core"
version = "0.0.0"
edition = "2021"
publish = false
```

`publish = false` because:
- The crate exists only to share primitives between binaries in this workspace.
- It has no independent semver contract — its public API may change in lockstep with its consumers.
- It would be confusing to publish a crate named `gbiv-core` whose only purpose is to be a private dependency of the actual published `gbiv` (and later `roy`) binaries.

## Migration phases

Each phase is a checkpoint: the workspace must compile and the existing test suite must pass before moving to the next. Phases land as separate commits on this branch.

### Phase 1 — workspace skeleton

- Move `src/` → `crates/gbiv/src/`.
- Move `Cargo.toml`'s `[package]` block and `[dependencies]` to `crates/gbiv/Cargo.toml`.
- Replace the repo-root `Cargo.toml` with a `[workspace]` manifest listing `crates/gbiv` as the only member for now.
- Move `Makefile.toml`, `progress.txt`, and any other root-level Cargo-relative files. Update `cargo make` task paths if needed.
- Verify: `cargo build` succeeds, `cargo nextest run` green, `gbiv --help` runs from the resulting binary.

No `gbiv-core` content yet — this phase only establishes the workspace.

### Phase 2 — empty gbiv-core

- Create `crates/gbiv-core/{Cargo.toml, src/lib.rs}` with `publish = false` and empty module declarations.
- Add `gbiv-core = { path = "../gbiv-core" }` to `crates/gbiv/Cargo.toml`.
- Add `gbiv-core` to the workspace `members` list.
- Add `thiserror` as a `[dependencies]` entry on `gbiv-core`.
- Verify: workspace builds, tests still green.

### Phase 3 — colors

- Move `COLORS` from `crates/gbiv/src/colors.rs` to `crates/gbiv-core/src/colors.rs`. Leave the ANSI codes (`ansi_color`, `RESET`, `DIM`, `YELLOW`, `GREEN`, `RED`) in `crates/gbiv/src/colors.rs`.
- Add the new `is_valid_color(name: &str) -> bool` (this is the only behavioral addition in the whole migration).
- Move `infer_color_from_path` from `crates/gbiv/src/git_utils.rs` into `gbiv_core::colors`. Tests for it move with it.
- Update import sites: `crate::colors::COLORS` → `gbiv_core::colors::COLORS`; `crate::git_utils::infer_color_from_path` → `gbiv_core::colors::infer_color_from_path`.
- Re-anchor `@spec` markers: `CLI-COLOR-001` and `WTL-UTIL-004/5/6` move with their functions. Flip `CLI-COLOR-015/016` from `[ ]` to `[x]` after wiring the new `is_valid_color` body and its `@spec` marker.
- Verify: workspace builds, tests green, clap argument parsing still accepts ROYGBIV colors.

### Phase 4 — root

- Move `GbivRoot`, `find_gbiv_root`, `find_repo_in_worktree`, `is_git_repo` from `crates/gbiv/src/git_utils.rs` into `crates/gbiv-core/src/root.rs`. Tests move with them.
- Update import sites.
- Re-anchor `@spec` markers: `WTL-UTIL-001/2/3`, `WTL-UTIL-014/15`, `WTL-UTIL-019`.
- Verify: workspace builds, tests green, `gbiv status` and similar root-walking commands still locate the project from a deep subdirectory.

### Phase 5 — gitignore

- Move `ensure_gitignore_entry` from `crates/gbiv/src/git_utils.rs` into `crates/gbiv-core/src/gitignore.rs`.
- Define `CoreError` in `crates/gbiv-core/src/error.rs`; `ensure_gitignore_entry` returns `Result<(), CoreError>`.
- Add `#[from] CoreError` arm to gbiv's `GitError` in `crates/gbiv/src/git_utils.rs`.
- Add the three new unit tests for `ensure_gitignore_entry` (currently it has none): (a) creates `info/` if missing, (b) idempotent on repeat call, (c) appends with leading newline to a file lacking a trailing one. These cover WTL-UTIL-016/17/18.
- Re-anchor `@spec` markers: `WTL-UTIL-016/17/18`.
- Verify: workspace builds, tests green, `gbiv init` still adds `GBIV.md` and `.last-branch` to `info/exclude`.

### Phase 6 — import sweep and cleanup

- Grep for remaining `crate::colors::*` and `crate::git_utils::{find_gbiv_root, find_repo_in_worktree, is_git_repo, infer_color_from_path, ensure_gitignore_entry}` references and rewrite to `gbiv_core::*` paths. (Most of these were updated in their owning phase; this is a backstop sweep.)
- Update `docs/arrows/worktree-lifecycle.md` and `docs/arrows/cli-and-palette.md` source-file lists to reflect the new paths (e.g., `crates/gbiv/src/git_utils.rs` instead of `src/git_utils.rs`).
- Update `docs/arrows/index.yaml` source-path entries.
- Update `README.md` install instructions (`cargo install --path crates/gbiv` for local builds; `cargo install gbiv` for crates.io is unaffected).
- Verify: `cargo build --workspace`, `cargo nextest run --workspace`, all integration tests pass, `gbiv --help` works from a fresh `cargo install --path crates/gbiv`.

## Testing strategy

- **Existing tests** are the regression backstop. Every test moves with its function; no behavioral changes mean no test changes beyond import paths.
- **New tests** added only for the new `is_valid_color` (CLI-COLOR-015/016) and the previously-untested `ensure_gitignore_entry` (WTL-UTIL-016/17/18). Both are small unit tests inside the new crate.
- **Manual smoke test** after Phase 6 in a real gbiv project: `gbiv status`, `gbiv rebase-all`, `gbiv tidy`, `gbiv init` (against a throwaway repo). End-to-end behavior should be byte-identical.
- Per the project's testing convention, run via `cargo nextest run` (CLAUDE.md). Logs land in `watch.log` / `watch-tests.log` when watch mode is active.

## Risks and rollback

- **Cargo workspace migration is mechanical but disruptive**: every relative path under `src/` changes. Phase 1 isolates this risk to a single commit that does nothing else, so a regression caught later can `git revert` it cleanly.
- **`cargo install --path .` users** lose their install path. Mitigated by README update in Phase 6 and called out in the PR description.
- **No new dependencies**: `gbiv-core` only takes `thiserror` (already in use by gbiv). No version-bump pressure.
- **Concurrent `info/exclude` writes**: `ensure_gitignore_entry` is not atomic across concurrent writers. Today this is benign — each gbiv command targets a distinct git common dir per thread. When `roy` lands (gbv-9p1), `roy start` may run concurrently with gbiv against the same common dir; the right time to add file-locking is then, not now. Out of scope for this ticket.

## Decisions and alternatives considered

**Workspace split with both `gbiv` and `gbiv-core` under `crates/`**, rather than `gbiv-core` alongside the gbiv bin at the repo root.
- Chosen: cleaner long-term — `roy` lands as a third `crates/` member with no special-cased binary at the workspace root.
- Trade-off accepted: every path under the gbiv binary moves; `cargo install --path .` users have to switch to `cargo install --path crates/gbiv`. Documented in the README.

**Narrow `CoreError` (Io only) rather than moving `GitError` whole into core.**
- Chosen: `GitError` carries gbiv-specific variants (`WorktreeAlreadyExists`, `RebaseConflict`) that have no place in a library shared with `roy`. `CoreError` stays narrow and each binary's error surface evolves independently.
- Matches the HLD's "narrow typed errors per module" guidance.

**`is_git_repo` and `infer_color_from_path` move with their cohort** (not strictly required by `roy`).
- Chosen: `find_gbiv_root` calls `is_git_repo`; leaving the predicate in gbiv would force core to either duplicate it or expose a private dependency. `infer_color_from_path` depends only on `COLORS` and `Path` and belongs with the other root-relative helpers.
- Trade-off: minor over-scoping versus the literal ticket text; cohesion wins.

**ANSI codes stay in gbiv.**
- Chosen: `roy` is JSON-only; terminal escape codes are an output-format concern of the gbiv binary, not a shared primitive.
- Mirrors the LLD split between `gbiv-core::colors` (data) and `crates/gbiv/src/colors.rs` (presentation).

**`publish = false` on `gbiv-core`.**
- Chosen: no independent release cycle; no semver contract beyond the workspace; would be confusing to publish a crate whose only purpose is to be a private dependency.

**No standalone LLD for `gbiv-core`.**
- Considered and rejected during this ticket — see commit `33354db`. `gbiv-core` is a packaging artifact, not a component. Each primitive belongs to an existing component's LLD; the crate-boundary decisions live in this plan instead.

## References

- High-level design: `docs/gbiv/high-level-design.md`
- Component LLDs (own the moving primitives):
  - `docs/gbiv/llds/cli-and-palette.md` (Palette section)
  - `docs/gbiv/llds/worktree-lifecycle.md` (Git Utilities section)
- Affected EARS specs:
  - `docs/gbiv/specs/cli-and-palette.md` — CLI-COLOR-001 (existing), CLI-COLOR-015/016 (new)
  - `docs/gbiv/specs/worktree-lifecycle.md` — WTL-UTIL-001/2/3, WTL-UTIL-004/5/6, WTL-UTIL-014/15, WTL-UTIL-016/17/18, WTL-UTIL-019
- Source files (pre-migration):
  - `src/colors.rs`
  - `src/git_utils.rs`
- Downstream tickets blocked on this work: gbv-x2v (gbiv-core::tmux), gbv-9p1 (roy crate skeleton)

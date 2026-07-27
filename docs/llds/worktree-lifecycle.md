# Worktree Lifecycle

**Created**: 2026-04-23
**Status**: Complete (brownfield mapping)

## Context and Current State

gbiv manages a project by restructuring a single git repository into parallel worktrees named after ROYGBIV colors (red, orange, yellow, green, blue, indigo, violet) plus a canonical `main` worktree. The seven colors are the default; a project may extend the set with extra named worktrees declared in `.gbiv/config.toml` (see the CLI & Palette LLD, *Active palette*). This component owns the creation, synchronization, reset, repair, and maintenance of that structure.

The core insight is that git worktrees provide real, independent working directories that share a single object store. A developer can have seven features in flight simultaneously — one per color — and switch between them by changing directories (or tmux windows) rather than stashing and switching branches. The rare project that needs more than seven adds extra slots via config and materializes them with `gbiv repair`.

## Worktree Layout

The canonical directory structure after `gbiv init`:

```
project/
├── main/
│   └── repo/          ← original repo, on main/master branch
├── red/
│   └── repo/          ← git worktree, on 'red' branch
├── orange/
│   └── repo/          ← git worktree, on 'orange' branch
├── yellow/
│   └── repo/
├── green/
│   └── repo/
├── blue/
│   └── repo/
├── indigo/
│   └── repo/
└── violet/
    └── repo/
```

Each color directory contains a subdirectory with the same name as the original repository folder. The `main/repo/` directory is the primary repo; color directories are `git worktree add` targets.

### Root Discovery

`core::root::find_gbiv_root()` walks up from any CWD to find the gbiv root by checking for:
1. A `main/` subdirectory exists
2. At least one `BASE_COLORS` (ROYGBIV) subdirectory exists
3. A git repo exists somewhere under `main/`

Returns `GbivRoot { root: PathBuf, folder_name: String }` where `folder_name` is the repo directory name inside `main/`. Lives in the `core` module because the orchestration daemon calls it from `gbiv start` and from every CLI subcommand to locate `main/<repo>/.gbiv/port`. Root discovery keys off the immutable `BASE_COLORS`, never the active palette: the active palette is loaded *from* the root, so root discovery must not depend on it. Base worktrees always exist after init (and `gbiv repair` restores any that are deleted), so checking for a base color is sufficient.

### Color Inference

`core::colors::infer_color_from_path()` extracts which worktree the CWD is inside by matching the first path component after the gbiv root against the active palette. It takes the loaded palette (the names) and returns an owned `Option<String>` — the palette is runtime data, not `&'static`. Lives in the `core` module alongside the other root-relative helpers; the orchestration daemon does not currently call it (it always runs from `main/`).

## Init (Project Bootstrap)

`gbiv init <folder>` converts an existing git repository into the gbiv layout.

### Preconditions
- `folder` exists and is a directory
- `folder` is a git repository (has `.git`)
- At least one commit exists (git worktrees require this)
- No existing branches named after ROYGBIV colors

### Steps
1. Detect the main branch name (`main`, `master`, etc.)
2. Rename `folder` to `{folder}_gbiv_temp` (temporary backup)
3. Create `folder/main/` and move the repo into `folder/main/{folder}`
4. For each color: `git worktree add -b {color} ../../{color}/{folder} {main_branch}`
5. Write `GBIV.md` template to main repo if absent
6. Ensure `GBIV.md` is listed in the main repo's `.gitignore` (treated as a per-developer working file, not committed)

Init creates only the seven `BASE_COLORS` worktrees. Extra slots are not created at init time — the config file does not exist when a project is first bootstrapped — so the color-branch conflict check (WTL-INIT-004) is concerned only with the base names. Extra worktrees are materialized by `gbiv repair` reading `.gbiv/config.toml`.

### Rollback
If any worktree creation fails, init reverses all changes: removes created worktrees, deletes color directories, restores the original folder name and location.

## Repair (Palette Reconciliation)

`gbiv repair` makes the on-disk worktree layout match the active palette. It is idempotent and append-only: it creates worktrees that should exist but don't, and never removes or renames anything. It is the single way to materialize configured extra worktrees, and it doubles as recovery for a base worktree that was deleted.

### Steps
1. Find the gbiv root and the main repo inside `main/`.
2. Load the active palette (`Palette::load`) — base seven plus any validated extras from `.gbiv/config.toml`. A malformed config aborts here with a `ConfigError` (nothing is created).
3. Detect the local main branch name (as init does).
4. For each name in the active palette, in canonical order, classify the slot with `classify_worktree` (the same helper `status` uses) and act on the result:
   - **Present** — a git repo exists within `<root>/<name>`: skip it (report "present").
   - **Broken** — the directory exists and is non-empty but has no git repo: report "broken" and leave it untouched (repair never overwrites it).
   - **Missing** — the directory is absent or empty:
     - if a branch named `<name>` already exists, attach it: `git worktree add ../../<name>/<folder> <name>` (report "created (attached existing branch)");
     - otherwise create a fresh branch: `git worktree add -b <name> ../../<name>/<folder> <main_branch>` from the main repo, exactly as init does per color.
5. Print a per-name line (created / present / broken / attached / failed) and a summary line counting created, broken, and failed.
6. If any creation failed *or* any worktree is broken, return a non-zero status — repair could not make the palette whole. Successfully created worktrees are kept (append-only, no rollback of the others).

### What repair does NOT do
- It never deletes or renames a worktree, even if a name was removed from the config. Reclaiming a worktree stays the job of `reset` / `tidy`.
- It does not modify `GBIV.md`.
- It does not touch worktrees that already exist (no reset, no checkout, no rebase), and it never overwrites a broken directory.

### Drift detection (warn, never auto-fix)
The shared helper `classify_worktree(root, name) -> WorktreePresence` returns `Present` / `Missing` / `Broken` for a slot. `status` classifies every active-palette name in one pass and derives two sets from it: **missing** (repairable) names get a one-line hint suggesting `gbiv repair`; **broken** names (directory present, no repo) get a separate "needs attention" hint, because `repair` deliberately will not touch them. Only `gbiv repair` mutates worktrees; observation commands never create anything implicitly.

## Rebase-All (Upstream Sync)

`gbiv rebase-all` pulls the main worktree, then rebases all color worktrees onto the remote main branch in parallel.

### Steps
1. Find gbiv root and main repo
2. Determine remote main branch (tries `origin/main`, `origin/master`, `origin/develop` in order)
3. Pull main worktree: `git pull origin {remote_main}`
4. Register `.last-branch` in each worktree's `info/exclude` (gbiv state file, avoid dirty detection)
5. Spawn one thread per color; each thread:
   - Skips if worktree dir missing
   - Skips if rebase already in progress (`rebase-merge` or `rebase-apply` dir exists)
   - Skips if already up-to-date (0 commits behind)
   - Runs `git fetch origin` then `git rebase origin/{remote_main}`
6. Join threads, print per-color status (rebased / up-to-date / skipped / failed)
7. Exit non-zero if any rebase failed

### Conflict Handling
On rebase conflict, `rebase_onto()` automatically runs `git rebase --abort` to leave the worktree in a clean state. The error output (including conflict details) is captured and reported. The command continues with remaining colors.

## Reset (Worktree Reclamation)

`gbiv reset [<color>] [--hard] [--yes]` returns a color worktree to its trunk branch after the feature is merged upstream.

### Single-Color Reset Flow
1. Find repo in color worktree
2. Get current branch and dirty status
3. **Soft mode** (default): if already on color branch, skip (no-op)
4. Determine remote main branch
5. **Soft mode**: verify current branch is merged into remote main; error if not
6. **Hard mode**: if worktree is dirty, stash with descriptive message
7. Checkout color branch
8. `git reset --hard origin/{remote_main}`
9. Remove the color's entries from GBIV.md in main repo

### All-Color Reset Flow (no color arg)
- **Soft mode**: parse GBIV.md, only reset colors with `[done]` status
- **Hard mode**: reset all colors regardless of status; prompt for confirmation unless `--yes`

### Reset Decision Table

| Condition | Soft | Hard |
|---|---|---|
| On color branch already | Skip | Proceed |
| Branch not merged | Error | Stash + reset |
| No `[done]` in GBIV.md | Skip (all-color) | Reset anyway |
| Dirty worktree | N/A (requires merged) | Stash first |

## Tidy (Maintenance Composite)

`gbiv tidy` runs three steps in sequence:

1. `rebase-all` — sync all worktrees with upstream
2. `reset` (soft, all colors) — reclaim `[done]` worktrees
3. `tmux clean` — remove orphaned tmux windows (skipped if tmux not installed)

Errors from individual steps are collected but don't short-circuit — all three steps are attempted. Returns error if any step failed.

## Git Utilities

This component's git helpers split between two homes:

- **The `core` module** owns the primitives that both the worktree commands and the orchestration daemon depend on: root discovery, the `.git/info/exclude` registration helper, the `is_git_repo` predicate, and color inference. See the `core` annotations below.
- **`src/git_utils.rs`** (worktree-only) owns the dozens of `git`-shell-out wrappers used by gbiv commands (`checkout`, `rebase`, `stash`, `fetch`, `pull`, status queries, …). the orchestration daemon does not touch git, so none of these cross the boundary.

### State Queries (worktree-only)
- `get_quick_status()` — parses `git status --porcelain=v2 --branch` into `QuickStatus { branch, is_dirty, ahead_behind }`
- `get_ahead_behind_vs()` — commit count comparison via `git rev-list --left-right --count`
- `is_merged_into()` — ancestry check via `git merge-base --is-ancestor`
- `get_last_commit_age()` — seconds since last commit via `git log -1 --format=%ct`
- `get_remote_main_branch()` — probes for `origin/main`, `origin/master`, `origin/develop`
- `get_existing_branches()` — lists all local branches

### Mutating Operations (worktree-only)
- `checkout_branch()` — `git checkout`
- `reset_hard()` — `git reset --hard <ref>`
- `stash_push()` — `git stash push -m <msg>`
- `rebase_onto()` — `git rebase <upstream>`, aborts on conflict and returns error
- `fetch_remote()` — `git fetch origin`
- `pull()` — `git pull`

### Worktree Navigation
- `find_gbiv_root()` — walk-up root discovery (described above) — **lives in `core::root`**; the orchestration daemon calls it from `gbiv start` and from every CLI subcommand to locate `main/<repo>/.gbiv/port`
- `find_repo_in_worktree()` — find the `.git`-containing subdirectory inside a color dir — **lives in `core::root`**; the orchestration daemon uses it to resolve `main/<repo>/` from the gbiv root
- `is_git_repo()` — `git rev-parse --git-dir` predicate — **lives in `core::root`**; internal to `find_gbiv_root` (moves with its caller)
- `infer_color_from_path()` — CWD → color name (described above) — **lives in `core::colors`**; the orchestration daemon does not currently call it (the orchestration daemon always runs from `main/`) but it belongs with the other root-relative helpers
- `resolve_git_dir()` — handle normal `.git` dir vs worktree gitlink file — worktree-only
- `get_git_dir()` — `git rev-parse --git-common-dir` — worktree-only

### Housekeeping
- `ensure_gitignore_entry()` — appends an entry to `<git_dir>/info/exclude` if not already present (idempotent), creating `info/` if missing — **lives in `core::gitignore`**; the orchestration daemon uses it on `gbiv start` to register `.gbiv/` without making the user edit anything

### Error Type

`git_utils` is library-style code consumed by every command. Functions return `Result<T, GitError>` where `GitError` is a `thiserror`-derived enum:

```rust
#[derive(Debug, thiserror::Error)]
enum GitError {
    #[error("not inside a gbiv project (no main/<repo> found walking up from {0})")]
    NotInGbivProject(PathBuf),
    #[error("git command failed: {cmd}\nstderr: {stderr}")]
    GitFailed { cmd: String, stderr: String },
    #[error("rebase conflict in {0}")]
    RebaseConflict(String),
    #[error("worktree {0} already exists")]
    WorktreeExists(String),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Other(String),
}
```

Callers `?` these into `anyhow::Error` at the command-handler boundary. New variants are added when a command needs to branch on a specific failure (e.g., reset wants to distinguish `WorktreeExists` from a generic git failure); until then `Other` is the catch-all.

`GitError` carries a `#[from] CoreError` arm so that call sites bubbling up an `ensure_gitignore_entry` error (or any future `core` module fallible primitive) compose through `?` without an explicit conversion. `CoreError` is deliberately minimal — Io-only today — and lives in `core::error`. Worktree-specific failure variants (`WorktreeAlreadyExists`, `RebaseConflict`, `GitFailed`, …) stay with `GitError` in the worktree command modules; they have no place in the shared `core` module the orchestration daemon also depends on.

## Observed Design Decisions

| Decision | Chosen | Alternatives Considered | Rationale |
|---|---|---|---|
| ROYGBIV default, config-extensible | Immutable base seven + optional extras from `.gbiv/config.toml` | Fixed 7; always-N; derive-from-disk | Seven memorable colors cover the common case with zero config; a rare project that needs more adds named slots without losing the default simplicity. The base names stay fixed so ROYGBIV remains the product's identity. |
| Repair is append-only | Create-if-missing, never remove/rename | Two-way sync that prunes removed names | Destroying a worktree (possibly holding unmerged work) on a config edit is unsafe. Removal stays an explicit `reset`/`tidy` action. |
| `git worktree add` per color | One worktree per color branch | Sparse checkouts, multiple clones | Worktrees share objects (disk-efficient), each gets full working tree. |
| Parallel rebase | One thread per color | Sequential, async | Worktrees are independent; parallel is safe and faster. |
| Auto-abort on conflict | Abort rebase, report error, leave worktree clean | Leave in conflicted state, auto-resolve | Clean worktree is safer — developer sees the error output and can manually retry. Avoids leaving worktrees in a half-rebased state. |
| Remote main detection order | main → master → develop | Config option, parse HEAD | Covers most conventions; develop is less common but used in gitflow. |
| Rollback on init failure | Restore original folder | Leave partial state | Partial gbiv layout is confusing; clean rollback is safer. |

## Technical Debt & Inconsistencies

1. **`git_utils.rs` is large (~457 lines)** and mixes repo discovery with git command wrappers. Could be split into `discovery.rs` and `git_ops.rs` but the coupling is tight enough that it works.

2. **Remote branch detection** tries 3 hardcoded candidates. Repos with non-standard remote names (not `origin`) or branch names won't work.

3. **No fetch before merge check** in soft reset — relies on cached remote refs. If the remote was updated since last fetch, the merge check may give a stale answer. (Rebase-all fetches, so this is usually fine if tidy is used.)

4. **GBIV state files** (`.last-branch`) are written by some commands but the `ensure_gitignore_entry` mechanism is ad-hoc — only rebase-all calls it.

## Behavioral Quirks

1. **Soft reset skips when on color branch**: If you're already on the `red` branch, `gbiv reset red` prints a notice and exits. This means you can't use soft reset to "clean up GBIV.md" without being on a feature branch first.

2. **Hard reset always proceeds**: Even if already on color branch, hard mode still resets to remote main. This is intentional — it's the "force clean" escape hatch.

3. **Tidy swallows reset errors**: Reset failures during tidy don't affect the exit code (only rebase and tmux-clean failures do). This means silently-skipped resets won't fail CI if tidy is scripted.

4. **Init requires at least one commit**: Git worktrees can't be added to an empty repo. Init checks this early and errors with a clear message.

## References

- `src/git_utils.rs` — worktree-only git command wrappers and state queries
- `src/core/root.rs` — `find_gbiv_root`, `find_repo_in_worktree`, `is_git_repo`, `classify_worktree` (shared with the orchestration daemon)
- `core::colors` — `BASE_COLORS`, `infer_color_from_path` (shared with the orchestration daemon)
- `core::palette` — `Palette` runtime active palette
- `core::config` — `.gbiv/config.toml` loading and `ConfigError`
- `src/core/gitignore.rs` — `ensure_gitignore_entry` (shared with the orchestration daemon)
- `src/commands/init.rs` — project bootstrap
- `src/commands/repair.rs` — palette reconciliation (`gbiv repair`)
- `src/commands/rebase_all.rs` — upstream sync
- `src/commands/reset.rs` — worktree reclamation
- `src/commands/tidy.rs` — maintenance composite

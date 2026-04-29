# Worktree Lifecycle

**Created**: 2026-04-23
**Status**: Complete (brownfield mapping)

## Context and Current State

gbiv manages a project by restructuring a single git repository into a fixed set of parallel worktrees named after ROYGBIV colors (red, orange, yellow, green, blue, indigo, violet) plus a canonical `main` worktree. This component owns the creation, synchronization, reset, and maintenance of that structure.

The core insight is that git worktrees provide real, independent working directories that share a single object store. A developer can have 7 features in flight simultaneously — one per color — and switch between them by changing directories (or tmux windows) rather than stashing and switching branches.

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

`git_utils::find_gbiv_root()` walks up from any CWD to find the gbiv root by checking for:
1. A `main/` subdirectory exists
2. At least one ROYGBIV color subdirectory exists
3. A git repo exists somewhere under `main/`

Returns `GbivRoot { root: PathBuf, folder_name: String }` where `folder_name` is the repo directory name inside `main/`.

### Color Inference

`git_utils::infer_color_from_path()` extracts which color worktree the CWD is inside by matching the first path component after the gbiv root against the COLORS constant. Returns `Option<&'static str>`.

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

### Rollback
If any worktree creation fails, init reverses all changes: removes created worktrees, deletes color directories, restores the original folder name and location.

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

## Git Utilities (`git_utils.rs`)

The shared module providing git abstractions used across all commands.

### State Queries
- `get_quick_status()` — parses `git status --porcelain=v2 --branch` into `QuickStatus { branch, is_dirty, ahead_behind }`
- `get_ahead_behind_vs()` — commit count comparison via `git rev-list --left-right --count`
- `is_merged_into()` — ancestry check via `git merge-base --is-ancestor`
- `get_last_commit_age()` — seconds since last commit via `git log -1 --format=%ct`
- `get_remote_main_branch()` — probes for `origin/main`, `origin/master`, `origin/develop`
- `get_existing_branches()` — lists all local branches

### Mutating Operations
- `checkout_branch()` — `git checkout`
- `reset_hard()` — `git reset --hard <ref>`
- `stash_push()` — `git stash push -m <msg>`
- `rebase_onto()` — `git rebase <upstream>`, aborts on conflict and returns error
- `fetch_remote()` — `git fetch origin`
- `pull()` — `git pull`

### Worktree Navigation
- `find_gbiv_root()` — walk-up root discovery (described above)
- `find_repo_in_worktree()` — find the `.git`-containing subdirectory inside a color dir
- `resolve_git_dir()` — handle normal `.git` dir vs worktree gitlink file
- `get_git_dir()` — `git rev-parse --git-common-dir`
- `infer_color_from_path()` — CWD → color name (described above)

### Housekeeping
- `ensure_gitignore_entry()` — adds entries to `.git/info/exclude` (used for gbiv state files)

## Observed Design Decisions

| Decision | Chosen | Alternatives Considered | Rationale |
|---|---|---|---|
| Fixed 7 colors | ROYGBIV constant array | Configurable count | Simplicity; 7 parallel features is a reasonable ceiling. Colors provide memorable names. |
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

- `src/git_utils.rs` — shared git abstractions
- `src/commands/init.rs` — project bootstrap
- `src/commands/rebase_all.rs` — upstream sync
- `src/commands/reset.rs` — worktree reclamation
- `src/commands/tidy.rs` — maintenance composite

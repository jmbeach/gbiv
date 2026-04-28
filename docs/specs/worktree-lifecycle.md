# Worktree Lifecycle

Specs for worktree creation, upstream sync, reset, and maintenance.

**Component LLD**: `docs/llds/worktree-lifecycle.md`

## Init

- [x] WTL-INIT-001: When the user runs `gbiv init <folder>`, the system shall validate that the folder exists.
- [x] WTL-INIT-002: When the user runs `gbiv init <folder>`, the system shall validate that the folder is a git repository.
- [x] WTL-INIT-003: When the user runs `gbiv init <folder>`, the system shall validate that the repository has at least one commit.
- [x] WTL-INIT-004: When the user runs `gbiv init <folder>`, the system shall reject the operation if any ROYGBIV color branch (red, orange, yellow, green, blue, indigo, violet) already exists in the repository.
- [x] WTL-INIT-005: When all validations pass, the system shall rename the folder to a temporary name (`<folder>_gbiv_temp`) and create a `<folder>/main/<folder>` directory structure containing the original repository.
- [x] WTL-INIT-006: When the directory structure is created, the system shall create seven worktrees via `git worktree add -b <color> <path> <main-branch>` for each ROYGBIV color.
- [x] WTL-INIT-007: When all worktrees are created successfully, the system shall write a `GBIV.md` template file in the main repo if one does not already exist.
- [x] WTL-INIT-008: If a `GBIV.md` file already exists in the main repo, the system shall not overwrite it.
- [x] WTL-INIT-009: If any worktree creation fails, the system shall roll back by removing all created worktrees and restoring the original folder to its pre-init state.
- [x] WTL-INIT-010: If the directory structure creation fails, the system shall roll back by restoring the original folder from the temporary name.
- [ ] WTL-INIT-011: When `GBIV.md` is written, the system shall ensure `GBIV.md` is listed in the main repo's `.gitignore` (creating the file if needed, appending if missing, and leaving it untouched if already present).

## Rebase

- [x] WTL-REBASE-001: When the user runs `gbiv rebase-all`, the system shall locate the gbiv root by walking up from the current directory.
- [x] WTL-REBASE-002: When the gbiv root is found, the system shall locate the main repo inside the `main/` worktree directory.
- [x] WTL-REBASE-003: When the main repo is found, the system shall detect the remote main branch by trying `origin/main`, `origin/master`, and `origin/develop` in order.
- [x] WTL-REBASE-004: When the remote main branch is detected, the system shall pull the main worktree first before rebasing color worktrees.
- [x] WTL-REBASE-005: If the pull on the main worktree fails, the system shall abort the entire rebase-all operation and return an error.
- [x] WTL-REBASE-006: When the main worktree pull succeeds, the system shall register `.last-branch` in `info/exclude` of the common git directory for each color worktree, before spawning rebase threads.
- [x] WTL-REBASE-007: When gitignore registration is complete, the system shall spawn one thread per color worktree to perform rebases in parallel.
- [x] WTL-REBASE-008: While rebasing a color worktree, if the worktree directory does not exist, the system shall skip it with a "not found" message.
- [x] WTL-REBASE-009: While rebasing a color worktree, if no git repo is found inside the worktree directory, the system shall skip it with a "no repo in worktree" message.
- [x] WTL-REBASE-010: While rebasing a color worktree, if a `rebase-merge` or `rebase-apply` directory exists in the git dir, the system shall skip it with a "rebase in progress" warning and count it as a failure.
- [x] WTL-REBASE-011: While rebasing a color worktree, if the worktree is 0 commits behind the remote main (already up-to-date), the system shall skip it with an "already up to date" success message.
- [x] WTL-REBASE-012: While rebasing a color worktree, if the worktree is behind remote main, the system shall fetch from origin and then rebase onto the remote main branch.
- [x] WTL-REBASE-013: If a fetch fails for a color worktree, the system shall record the failure and continue rebasing other worktrees.
- [x] WTL-REBASE-014: If a rebase fails due to conflicts, the system shall abort the in-progress rebase (`git rebase --abort`), record the failure, and continue rebasing other worktrees.
- [x] WTL-REBASE-015: When all rebase threads complete, the system shall print a per-color status line showing the outcome (rebased, skipped, failed, already up to date).
- [x] WTL-REBASE-016: When all rebase threads complete, the system shall print a summary line with the count of successes and failures.
- [x] WTL-REBASE-017: If any color worktree failed to rebase, the system shall exit with a non-zero status (return an error).

## Reset

### Single-color soft reset

- [x] WTL-RESET-001: When the user runs `gbiv reset <color>` (without `--hard`), if the worktree is already on the color branch, the system shall skip the reset and print a message.
- [x] WTL-RESET-002: When the user runs `gbiv reset <color>` (without `--hard`), if no remote is configured, the system shall return an error.
- [x] WTL-RESET-003: When the user runs `gbiv reset <color>` (without `--hard`), the system shall verify the current branch is merged into the remote main branch; if not, it shall return an error.
- [x] WTL-RESET-004: When the merge check passes, the system shall check out the color branch and `git reset --hard` to the remote main ref.
- [x] WTL-RESET-005: When the reset succeeds, the system shall remove entries tagged with the color from `GBIV.md` in the main repo.
- [x] WTL-RESET-006: If the main repo cannot be found for GBIV.md cleanup, the system shall print a warning but still succeed.

### Single-color hard reset

- [x] WTL-RESET-007: When the user runs `gbiv reset <color> --hard`, the system shall not check whether the current branch is merged into remote main.
- [x] WTL-RESET-008: When the user runs `gbiv reset <color> --hard` and the worktree has dirty changes, the system shall stash the dirty changes with a descriptive message before resetting.
- [x] WTL-RESET-009: When the user runs `gbiv reset <color> --hard`, if the worktree is not on the color branch, the system shall check out the color branch and then `git reset --hard` to the remote main ref.
- [x] WTL-RESET-010: When the user runs `gbiv reset <color> --hard`, the system shall remove entries tagged with the color from `GBIV.md` in the main repo.

### All-color soft reset

- [x] WTL-RESET-011: When the user runs `gbiv reset` (no color, no `--hard`), the system shall parse `GBIV.md` to determine feature statuses for each color.
- [x] WTL-RESET-012: When performing an all-color soft reset, the system shall only reset worktrees whose GBIV.md entry has `[done]` status.
- [x] WTL-RESET-013: When performing an all-color soft reset, the system shall skip colors that have no GBIV.md entry (silently).
- [x] WTL-RESET-014: When performing an all-color soft reset, the system shall skip colors whose GBIV.md entry has a status other than `[done]` and report them as "without [done] status".
- [x] WTL-RESET-015: When an all-color reset completes, the system shall print a summary line showing counts of resets, skips (with reasons: not merged, without [done] status, already reset, missing worktree, errors).

### All-color hard reset

- [x] WTL-RESET-016: When the user runs `gbiv reset --hard`, the system shall attempt to reset all color worktrees regardless of GBIV.md status.
- [x] WTL-RESET-017: When the user runs `gbiv reset --hard` without `--yes`, the system shall display each worktree's current branch and prompt for confirmation before proceeding.
- [x] WTL-RESET-018: If the user declines the confirmation prompt, the system shall abort the reset without modifying any worktrees.
- [x] WTL-RESET-019: When the user runs `gbiv reset --hard --yes`, the system shall skip the confirmation prompt and proceed immediately.

### Shared reset behavior

- [x] WTL-RESET-020: While resetting any color, if the worktree directory is missing, the system shall skip it with a warning and continue processing other colors.

## Tidy

- [x] WTL-TIDY-001: When the user runs `gbiv tidy`, the system shall first run `rebase-all`.
- [x] WTL-TIDY-002: When rebase-all completes (regardless of success or failure), the system shall run a soft all-color reset.
- [x] WTL-TIDY-003: When reset completes, the system shall check if tmux is installed by running `which tmux`.
- [x] WTL-TIDY-004: If tmux is installed, the system shall run `tmux clean` to remove stale tmux windows.
- [x] WTL-TIDY-005: If tmux is not installed, the system shall skip the tmux clean step without error.
- [x] WTL-TIDY-006: If any step (rebase-all or tmux clean) fails, the system shall continue executing subsequent steps and collect errors.
- [x] WTL-TIDY-007: If any step failed during tidy, the system shall return an error after all steps have been attempted.

## Utility Helpers

### find_gbiv_root

- [x] WTL-UTIL-001: When `find_gbiv_root` is called, the system shall walk up from the given directory looking for a directory that contains a `main/<folder-name>` subdirectory with a git repo and at least one ROYGBIV color subdirectory.
- [x] WTL-UTIL-002: When a matching directory is found, the system shall return a `GbivRoot` containing the root path and folder name.
- [x] WTL-UTIL-003: If no matching directory is found after walking to the filesystem root, the system shall return `None`.

### infer_color_from_path

- [x] WTL-UTIL-004: When `infer_color_from_path` is called, the system shall extract the first path component relative to the gbiv root and match it against ROYGBIV color names.
- [x] WTL-UTIL-005: If the first path component matches a ROYGBIV color, the system shall return that color name.
- [x] WTL-UTIL-006: If the first path component does not match any ROYGBIV color, the system shall return `None`.

### QuickStatus

- [x] WTL-UTIL-007: When `get_quick_status` is called, the system shall run `git status --porcelain=v2 --branch` and parse the output to extract the current branch name, dirty status, and ahead/behind counts.
- [x] WTL-UTIL-008: The `is_dirty` field shall be true if any non-header line is present in the porcelain output.
- [x] WTL-UTIL-009: The `ahead_behind` field shall be parsed from the `# branch.ab` header line, stripping the `+` and `-` prefixes.

### get_remote_main_branch

- [x] WTL-UTIL-010: When `get_remote_main_branch` is called, the system shall check for `origin/main`, `origin/master`, and `origin/develop` in that order, returning the first that exists.
- [x] WTL-UTIL-011: If none of the candidate remote branches exist, the system shall return `None`.

### resolve_git_dir

- [x] WTL-UTIL-012: When `resolve_git_dir` is called on a repo where `.git` is a directory, the system shall return the `.git` directory path.
- [x] WTL-UTIL-013: When `resolve_git_dir` is called on a repo where `.git` is a gitlink file (as produced by `git worktree add`), the system shall parse the `gitdir:` line and resolve the path to the actual git directory.

### find_repo_in_worktree

- [x] WTL-UTIL-014: When `find_repo_in_worktree` is called, the system shall scan the given worktree directory for a subdirectory containing a `.git` entry, and return its path.
- [x] WTL-UTIL-015: If no subdirectory contains a `.git` entry, the system shall return `None`.

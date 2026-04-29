# Observation

Specs for the status dashboard and cross-worktree command execution.

**Component LLD**: `docs/gbiv/llds/observation.md`

## Status

### Root Discovery

- [x] OBS-STATUS-001: When the user runs `gbiv status`, the system shall determine the gbiv root by traversing upward from CWD looking for a directory that contains `main/<folder_name>` as a git repo and at least one ROYGBIV color directory.
- [x] OBS-STATUS-002: When the gbiv root cannot be found from CWD, the system shall return the error "Not in a gbiv-structured repository".

### Parallel Collection

- [x] OBS-STATUS-003: When collecting worktree state, the system shall spawn one thread per color (7 total, one for each of red, orange, yellow, green, blue, indigo, violet) to gather git status in parallel.
- [x] OBS-STATUS-004: For each color thread, where the worktree directory exists, the system shall locate the git repo within it via `find_repo_in_worktree` (first subdirectory containing `.git`), then call `get_quick_status` to obtain branch name, dirty flag, and ahead/behind counts.
- [x] OBS-STATUS-005: For each color thread, where the worktree directory does not exist or contains no git repo, the system shall return `None`.

### Feature Branch Detection

- [x] OBS-STATUS-006: When the current branch name does not equal the color name (i.e., the worktree is on a feature branch), the system shall additionally retrieve: remote main branch (`origin/main`, `origin/master`, or `origin/develop` in that order), merge status via `merge-base --is-ancestor`, last commit age via `git log -1 --format=%ct`, and ahead/behind counts vs remote main via `rev-list --left-right --count`.
- [x] OBS-STATUS-007: When the current branch name equals the color name, the system shall skip remote main lookup, merge check, commit age, and ahead/behind-vs-remote calculation.

### Output Ordering

- [x] OBS-STATUS-008: When joining thread results, the system shall output worktree statuses in ROYGBIV order: red, orange, yellow, green, blue, indigo, violet.

### Output Format - Missing Worktree

- [x] OBS-STATUS-009: When a worktree has no result (directory missing or no repo found), the system shall print `{color} missing` with the color name ANSI-colored and left-padded to 8 characters.

### Output Format - On Color Branch

- [x] OBS-STATUS-010: When a worktree is on its own color branch and is dirty, the system shall print `{color}  {branch}  dirty` where the branch is DIM, the dirty label is YELLOW.
- [x] OBS-STATUS-011: When a worktree is on its own color branch and is clean, the system shall print `{color}  {branch}  clean` where the branch and "clean" are DIM.

### Output Format - On Feature Branch

- [x] OBS-STATUS-012: When a worktree is on a feature branch and is dirty, the system shall display the dirty label in YELLOW; when clean, the label shall be plain text "clean".
- [x] OBS-STATUS-013: When a worktree is on a feature branch and `is_merged_into` returns true, the system shall display "merged" in DIM.
- [x] OBS-STATUS-014: When a worktree is on a feature branch and `is_merged_into` returns false, the system shall display "not merged" in YELLOW.
- [x] OBS-STATUS-015: When a worktree is on a feature branch and no remote main branch is found, the system shall display "no remote" in DIM.
- [x] OBS-STATUS-016: When a worktree is on a feature branch, the system shall display the last commit age formatted as: `N secs` (<60s), `N mins` (<3600s), `N hours` (<86400s), or `N days` (>=86400s). If age is unavailable, display "???".
- [x] OBS-STATUS-017: When a worktree is on a feature branch with ahead > 0, the system shall display the ahead count prefixed with an up-arrow in GREEN.
- [x] OBS-STATUS-018: When a worktree is on a feature branch with ahead == 0, the system shall display the ahead count prefixed with an up-arrow in DIM.
- [x] OBS-STATUS-019: When a worktree is on a feature branch with behind > 0, the system shall display the behind count prefixed with a down-arrow in RED.
- [x] OBS-STATUS-020: When a worktree is on a feature branch with behind == 0, the system shall display the behind count prefixed with a down-arrow in DIM.
- [x] OBS-STATUS-021: When a worktree is on a feature branch and ahead/behind data is unavailable, the system shall display "???" in DIM.

### GBIV.md Feature List

- [x] OBS-STATUS-022: After printing all worktree statuses, the system shall locate GBIV.md by finding the repo inside the `main` worktree directory (via `find_repo_in_worktree`) and reading `GBIV.md` from that repo root.
- [x] OBS-STATUS-023: When GBIV.md contains features, the system shall print a blank line followed by a DIM "GBIV.md" header, then each feature on its own line.
- [x] OBS-STATUS-024: When GBIV.md contains no features (or the file is missing), the system shall print nothing after the worktree statuses.
- [x] OBS-STATUS-025: For each tagged feature, the system shall display the tag in its ANSI color, followed by an optional lifecycle status in brackets (e.g., `[done]`, `[in-progress]`), followed by the description.
- [x] OBS-STATUS-026: For each untagged feature, the system shall display "backlog" in DIM, left-padded to 8 characters, followed by the description.

## Exec

### Target Parsing

- [x] OBS-EXEC-001: When the user runs `gbiv exec <args>`, the system shall check if the first argument matches a valid ROYGBIV color or "all"; if so, that becomes the target and the remaining arguments become the command tokens.
- [x] OBS-EXEC-002: When the first argument does not match a color or "all", the system shall set target to None (infer from CWD) and treat all arguments as command tokens.
- [x] OBS-EXEC-003: When command tokens contain `--` separators, the system shall strip them from the command token list.
- [x] OBS-EXEC-004: When no command tokens remain after parsing, the system shall print an error "no command specified" with usage hint and exit with code 1.

### Single Color Execution

- [x] OBS-EXEC-005: When the target is a specific color, the system shall validate the color against the COLORS array and return an error "'<color>' is not a valid color" for invalid values.
- [x] OBS-EXEC-006: When the target is a valid color, the system shall locate the repo within the worktree directory via `find_repo_in_worktree` and return an error if the worktree does not exist or has no repo.
- [x] OBS-EXEC-007: When the repo is found, the system shall join command tokens with spaces and execute `sh -c "<joined>"` with the working directory set to the repo path.
- [x] OBS-EXEC-008: When the shell command exits with code 0, the system shall return Ok containing stdout.
- [x] OBS-EXEC-009: When the shell command exits with a non-zero code, the system shall return Err containing stdout concatenated with stderr.

### All-Color Execution

- [x] OBS-EXEC-010: When the target is "all", the system shall spawn one thread per existing color worktree (skipping colors whose worktree directory has no repo) and run the command in each.
- [x] OBS-EXEC-011: When joining results from all-color execution, the system shall preserve ROYGBIV order (threads are spawned and joined in color array order).
- [x] OBS-EXEC-012: When formatting all-color output, the system shall prefix each color's output with an ANSI-colored header `[<color>]` on its own line.
- [x] OBS-EXEC-013: When a command fails for a color in all-color mode, the system shall append `(FAILED)` to that color's header line.
- [x] OBS-EXEC-014: When any command fails in all-color mode, the system shall return Err with the combined output; the process shall exit with code 1.
- [x] OBS-EXEC-015: When all commands succeed in all-color mode, the system shall return Ok with the combined output.
- [x] OBS-EXEC-016: When a color worktree is missing (no repo found) in all-color mode, the system shall silently skip that color without error.

### Inferred Color Execution

- [x] OBS-EXEC-017: When no target is specified, the system shall find the gbiv root from CWD and infer the color by matching the first path component under the root against the COLORS array.
- [x] OBS-EXEC-018: When the color cannot be inferred (CWD is not under a color directory, e.g., under `main`), the system shall return the error "Could not infer color from current worktree directory".
- [x] OBS-EXEC-019: When the gbiv root cannot be found from CWD during inference, the system shall return the error "Could not infer color: not in a gbiv worktree".
- [x] OBS-EXEC-020: When the color is successfully inferred, the system shall delegate to single-color execution with the inferred color and the gbiv root.

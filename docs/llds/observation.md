# Observation

**Created**: 2026-04-23
**Status**: Complete (brownfield mapping)

## Context and Current State

The observation component groups the two read-heavy commands that surface state without mutating it (from gbiv's perspective). `status` presents a dashboard of worktree health and feature assignments. `exec` runs arbitrary shell commands inside worktrees, giving developers access to any diagnostic or operation gbiv doesn't natively provide.

Both commands follow the same pattern: find gbiv root, enumerate worktrees, gather information in parallel, present results in ROYGBIV order.

## Status Command

`gbiv status` — no arguments or flags.

### Output Structure

The output has two sections:

**Section 1: Worktree Health (always shown)**

One line per ROYGBIV color, showing git state:

```
red        feature-auth   dirty  not merged  3 hours  ↑2 ↓0
orange     orange         clean
yellow     missing
green      feature-dark   clean  merged      2 days   ↑0 ↓0
blue       blue           clean
indigo     indigo         clean
violet     violet         dirty
```

**Section 2: Feature Ledger (shown if GBIV.md has entries)**

```
GBIV.md
  red [in-progress]  Fix the auth bug
  green              Add dark mode
  backlog            Untagged item
```

(See Feature Ledger LLD for format details.)

### Worktree Line Format

Each line adapts based on worktree state:

| State | Format |
|---|---|
| Missing worktree dir | `{color}  missing` |
| On color branch | `{color}  {branch}  {dirty\|clean}` |
| On feature branch | `{color}  {branch}  {dirty\|clean}  {merged\|not merged\|no remote}  {age}  ↑{ahead} ↓{behind}` |

"On color branch" means the branch name equals the color name — the worktree is idle, sitting on its trunk. Feature branch detection is simply: branch name != color name.

### Parallel Execution

Status spawns 7 threads (one per color) to collect git state concurrently. Each thread:

1. Checks if `root/{color}/` directory exists
2. Finds the repo inside via `find_repo_in_worktree()`
3. Calls `get_quick_status()` for branch, dirty flag, ahead/behind
4. If on a feature branch (branch != color):
   - `get_remote_main_branch()` — find remote main ref
   - `is_merged_into()` — check if branch is ancestor of remote main
   - `get_last_commit_age()` — time since last commit
   - `get_ahead_behind_vs()` — commits ahead/behind remote main

Threads are joined in ROYGBIV order so output is deterministic regardless of completion order.

### Color Coding

| Element | ANSI Code |
|---|---|
| Color label | Per-color ANSI (e.g., red = `\x1b[31m`, orange = `\x1b[38;5;208m`) |
| Branch name | DIM |
| `clean` | DIM |
| `dirty` | YELLOW |
| `merged` | DIM |
| `not merged` | YELLOW |
| `↑N` (ahead > 0) | GREEN |
| `↓N` (behind > 0) | RED |
| `↑0` or `↓0` | DIM |
| `missing` | DIM |
| `backlog` label | DIM |

### Duration Formatting

Last commit age is formatted as: `N secs`, `N mins`, `N hours`, `N days`. No plural handling (always `N hours` not `N hour`).

## Exec Command

`gbiv exec [<color>|all] -- <command...>`

### Target Resolution

| Invocation | Target |
|---|---|
| `gbiv exec red -- git status` | Single color: `red` |
| `gbiv exec all -- cargo test` | All existing color worktrees |
| `gbiv exec -- git log` | Inferred from CWD |

Target parsing happens in `main.rs`: the first arg is checked against COLORS and `"all"`. If it matches, it's the target; otherwise target is None (infer from CWD). The `--` separator is stripped from command tokens.

### Single-Color Execution

1. Validate color is in COLORS
2. Find worktree dir at `root/{color}/`
3. Find repo inside via `find_repo_in_worktree()`
4. Join command tokens with spaces
5. Run `sh -c "{joined_command}"` with cwd set to repo dir
6. Capture stdout + stderr
7. Return Ok(stdout) on exit 0, Err(stdout + stderr) otherwise

### All-Color Execution

1. Enumerate COLORS, find which have repos
2. Spawn one thread per existing color
3. Each thread runs the command via `sh -c` in its repo dir
4. Join threads in ROYGBIV order
5. Format output with color-coded headers:

```
[red]
<stdout from red>
[green]
<stdout from green>
[blue] (FAILED)
<stderr from blue>
```

6. Return Ok if all succeeded, Err if any failed

### CWD Inference

When no target is specified:
1. Get CWD
2. Find gbiv root
3. `infer_color_from_path(cwd, root)` — extract color from path
4. Error if CWD is not inside a color worktree (e.g., in `main/`)

## Observed Design Decisions

| Decision | Chosen | Alternatives Considered | Rationale |
|---|---|---|---|
| Parallel status collection | 7 threads, join in order | Sequential, async runtime | Fast for I/O-bound git commands. Join-in-order gives deterministic output without sorting. |
| `sh -c` for exec | Shell interpretation of command string | Direct exec without shell | Allows pipes, redirects, `&&` chains. Developers expect shell semantics. |
| All-or-nothing exec result | Err if any color fails | Partial success with per-color exit codes | Simpler for scripting: `gbiv exec all -- cargo test && echo ok`. Per-color details are in the output string. |
| Status has no flags | Fixed output format | `--json`, `--short`, `--color=auto` | YAGNI. Single format covers the primary use case. |
| Merged check only on feature branches | Skip if on color branch | Always show merge status | On color branch = idle worktree. Merge status is meaningless there. |
| Exec skips missing worktrees silently | No output for absent colors | Warn, error | "all" means "all that exist." Missing worktrees are a normal state, not an error. |

## Technical Debt & Inconsistencies

1. **Command token joining is naive**: `exec` joins tokens with spaces and passes to `sh -c`. Tokens containing spaces aren't quoted, so `gbiv exec red -- echo "hello world"` works (shell handles quotes) but passing tokens programmatically could break. In practice this is fine because the shell re-parses the string.

2. **Status hardcodes format**: No `--json` or machine-readable output. If gbiv is ever scripted, status output would need to be parsed with regex.

3. **Duration formatting lacks singular**: `"1 hours"` instead of `"1 hour"`. Minor cosmetic issue.

4. **Exec stdout/stderr merge on failure**: When a command fails, stdout and stderr are concatenated. If both have content, the boundary between them is invisible.

## Behavioral Quirks

1. **Exec is read-only from gbiv's perspective but not from git's**: `gbiv exec all -- git commit -am "msg"` will commit in every worktree. gbiv doesn't guard against this — exec is intentionally a transparent shell.

2. **Status conditional computation**: Merged status, commit age, and ahead/behind are only computed when the worktree is on a feature branch (branch != color). This means these fields are always None for idle worktrees, which keeps the output clean but means you can't see "how far behind is my idle red branch."

3. **Exec "all" output order is ROYGBIV**: Even if violet finishes before red, output is printed red → orange → yellow → green → blue → indigo → violet. This is because threads are joined in order, not as they complete.

4. **GBIV.md is read from main worktree only**: Even when `status` is run from inside a color worktree, the ledger section always sources from `main/<repo>/GBIV.md`. A stale copy of GBIV.md in a color worktree is ignored.

## References

- `src/commands/status.rs` — status command
- `src/commands/exec.rs` — exec command
- `src/main.rs` (lines ~162-174) — exec target parsing

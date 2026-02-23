## Context

`gbiv` is a Rust CLI tool (using `clap`) that manages ROYGBIV git worktrees. After `gbiv init <folder>`, the directory layout becomes:

```
<folder>/
  main/<folder>/    ← main branch
  red/<folder>/
  orange/<folder>/
  yellow/<folder>/
  green/<folder>/
  blue/<folder>/
  indigo/<folder>/
  violet/<folder>/
```

Currently there is one top-level command: `init`. All external process calls (git) are made via `std::process::Command`. There are no new Cargo dependencies to introduce; the same pattern extends naturally to tmux.

The `tmux new-session` command needs to:
1. Locate the gbiv root from the current working directory.
2. Enumerate the 8 worktree paths (main + 7 ROYGBIV colors).
3. Shell out to `tmux` to create a session and windows.

## Goals / Non-Goals

**Goals:**
- Add a `gbiv tmux new-session` subcommand that creates a tmux session with one named window per gbiv worktree.
- Each window's working directory is set to the corresponding worktree path.
- Session name defaults to the project folder name; overridable with `--session-name`.
- Fail clearly if: `tmux` is not installed, the current directory is not a gbiv root, or a session with that name already exists.

**Non-Goals:**
- Attaching to the session automatically (user attaches with `tmux attach`).
- Managing tmux layouts, splits, or running startup commands in windows.
- Supporting partial worktree sets (all 8 are always created, matching `gbiv init`).
- Any other `tmux` subcommands beyond `new-session`.

## Decisions

### 1. Detecting the gbiv root

**Decision**: Walk upward from CWD looking for a directory whose children include a `main/<folder>/` path that is a git worktree. Specifically: look for a parent directory `P` where `P/main/<P's name>/` exists and is a git repo.

**Alternatives considered**:
- Require the user to pass the path explicitly — simpler but worse UX than auto-detection.
- Store a `.gbiv` marker file during `init` — cleaner long-term, but out of scope for this change and requires modifying `init`.

**Rationale**: The directory layout created by `gbiv init` is deterministic and detectable without a marker file. Walking up from CWD is the standard convention (similar to how git detects `.git`).

### 2. Enumerating worktrees

**Decision**: Hardcode the ROYGBIV color list (same constant already in `init.rs`) plus `main`. Do not call `git worktree list`.

**Rationale**: The worktree structure is always exactly these 8 paths after `gbiv init`. Calling `git worktree list` adds complexity and fragility (parsing output, handling detached worktrees). The hardcoded list is consistent with the existing `COLORS` constant.

### 3. tmux invocation strategy

**Decision**: Use a sequence of `std::process::Command` calls:
1. Check `tmux -V` to confirm tmux is installed.
2. `tmux new-session -d -s <name> -c <main-worktree-path> -n main` — create the session detached, first window = main.
3. For each color in ROYGBIV order: `tmux new-window -t <name> -n <color> -c <color-worktree-path>`.

**Alternatives considered**:
- Generate and exec a shell script — harder to handle errors per-step.
- Use a tmux control-mode socket — significant complexity with no benefit here.

**Rationale**: Direct process invocations match the existing codebase pattern (git calls in `init.rs`). Each step is independently checkable for success/failure. The `-d` flag lets the command complete without requiring an attached terminal.

### 4. Module structure

```
src/
  commands/
    mod.rs          ← add `pub mod tmux`
    tmux/
      mod.rs        ← defines `tmux_command()` returning a clap::Command, dispatches sub-subcommands
      new_session.rs ← implements new_session_command(session_name: Option<&str>) -> Result<(), String>
  git_utils.rs      ← add find_gbiv_root(start: &Path) -> Option<GbivRoot>
  main.rs           ← wire up `tmux` subcommand
```

`GbivRoot` is a small struct `{ root: PathBuf, folder_name: String }` used to derive all 8 worktree paths.

### 5. Error on duplicate session

**Decision**: Check for existing session with `tmux has-session -t <name>` before creating. Return an error if it exists, rather than silently attaching or replacing.

**Rationale**: Destructive/surprising behavior should require explicit user action. The error message should suggest `tmux attach -t <name>` or using `--session-name`.

## Risks / Trade-offs

- **tmux not in PATH** → Mitigation: check `tmux -V` first and emit a clear "tmux not found" error.
- **Partially initialized gbiv repo** (some color dirs missing) → Mitigation: warn per missing worktree and skip that window, but still create the session. Log which paths were skipped.
- **CWD not inside a gbiv structure** → Mitigation: walk up to filesystem root; if not found, emit "Not inside a gbiv project. Run `gbiv init` first."
- **Session name collision** → Mitigation: check with `tmux has-session` before creating; error with actionable message.
- **Windows only / non-tmux environments** → No mitigation in scope; this command is Unix/tmux-specific by design.

## Migration Plan

No migration needed. This is a purely additive new subcommand. Existing `gbiv init` behavior is unchanged.

## Open Questions

- Should `gbiv tmux new-session` also accept an explicit `<path>` argument to target a gbiv root other than the CWD? (Deferred — auto-detection covers the common case.)
- Should the `main` window be renamed to match the folder name instead of `"main"`? (Use `"main"` for clarity; matches the directory name.)

## Context

GBIV manages git worktrees organized by ROYGBIV colors. Several commands already operate across worktrees (`rebase-all`, `cleanup`), but there's no general-purpose way to run an arbitrary command in a specific worktree or across all of them. Users must manually navigate to each worktree directory.

The codebase already has patterns for:
- Resolving worktree paths via `find_gbiv_root()` + `find_repo_in_worktree()`
- Parallel execution across colors using `std::thread::spawn` (see `rebase-all`)
- Color-labeled output with ANSI codes

## Goals / Non-Goals

**Goals:**
- Allow running any shell command in a single color worktree's repo directory
- Allow running a command across all color worktrees with labeled output
- Parallel execution when targeting all worktrees
- Clear, color-labeled output so users can distinguish results per worktree
- Non-zero exit code if any execution fails

**Non-Goals:**
- Running commands in the main worktree (users are likely already there)
- Interactive command support (stdin piping to subprocesses)
- Custom ordering or filtering of which colors to target
- Streaming/interleaved output (collect and print per-color to avoid garbled output)

## Decisions

### 1. Command syntax: `gbiv exec [<color>|all] -- <command...>`

Use `--` separator to clearly delineate gbiv args from the user's command. The target is a single color name or the literal `all`. If no target is specified, infer the color from the current working directory (consistent with `mark` command pattern).

**Alternatives considered:**
- Positional-only parsing (ambiguous when the user command starts with a color name)
- Quoting the command as a single string (`gbiv exec all "cargo build"`) — less ergonomic for commands with their own flags

### 2. Execution model: `std::process::Command` with collected output

For `all` mode, spawn each command in a thread (same pattern as `rebase-all`), collect stdout/stderr, then print results sequentially with color labels. This avoids interleaved output.

For single-color mode, inherit stdio directly so the user gets real-time output.

**Alternatives considered:**
- Always collecting output — loses real-time feedback for single-color use
- Streaming with line-prefix — complex and still risks interleaving

### 3. Shell invocation: pass command through `sh -c`

Join the trailing arguments into a single string and run via `sh -c "<command>"`. This enables shell features like pipes, redirects, and glob expansion.

**Alternatives considered:**
- Direct exec without shell — breaks pipes/redirects, less ergonomic
- Requiring the command as a single quoted argument — worse UX

### 4. Color inference from CWD

When no target is given, infer the color from the current directory using `infer_color_from_path()` (already exists in `mark` command). This makes `gbiv exec -- cargo build` work naturally when you're already in a color worktree.

## Risks / Trade-offs

- **[Shell injection]** → The user is intentionally running their own commands; this is equivalent to typing them manually. No untrusted input is involved.
- **[Parallel output ordering]** → Output order may vary between runs when using `all`. Mitigation: collect all output, then print in ROYGBIV order.
- **[Missing worktrees]** → Some color directories may not exist. Mitigation: skip missing worktrees with a note, don't treat as error.
- **[Long-running commands]** → No timeout mechanism. Acceptable for v1; users can Ctrl+C.

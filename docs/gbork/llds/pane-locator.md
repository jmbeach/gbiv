# Pane Locator

**Created**: 2026-04-28
**Status**: Draft

## Context

For a given color (e.g., `red`), the Pane Locator answers two questions:

1. Does the gbiv tmux session have a window for this color?
2. If so, which pane in that window is running Claude Code?

Both answers feed every HTTP endpoint. `/sessions` runs the locator across all seven colors; `/session/:color` and `/session/:color/send` run it once. The locator's output is a `Resolution` value that tells the caller exactly what to do — including how to fail.

The locator is the only place in gbork that reasons about Claude Code's process identity. Higher layers see only `Resolution`.

## Why a Custom Locator

`tmux capture-pane` and `send-keys` need a specific pane ID. The naive approach — "first pane in the window" — breaks the moment a user splits the window for an editor or watcher. The natural tmux signal — `#{pane_current_command}` — is also unreliable: Claude Code calls `process.title = "<version>"`, so its foreground process appears in tmux as `2.1.122` (or whatever version is running). The reported "command" is a number that changes between releases.

The locator therefore identifies Claude Code by its **executable path**, not its process name, by walking the process tree under each pane's PID and inspecting each descendant's executable.

## Resolution

```rust
enum Resolution {
    Ok { pane_id: String },
    NoWindow,            // no tmux window for this color
    NoClaudePane,        // window exists, no pane is running claude
    MultipleClaudePanes  // window exists, more than one claude pane found
        { pane_ids: Vec<String> },
}
```

`MultipleClaudePanes` is surfaced rather than guessed because it usually means the user has nested sessions or two claude processes — the commander should be told, not silently sent to a random one.

## Locating a Pane

Inputs: `session: &str`, `color: &str`.

1. **Find the window.** `tmux_driver::list_windows(session)`. Match a window where `name == color`. If none, return `NoWindow`. (The session itself missing produces `TmuxError::SessionNotFound` from the driver, which the caller maps to a 5xx — that's a daemon-misconfiguration case, not a locator case.)

2. **List panes.** `tmux_driver::list_panes(format!("{session}:{color}"))`. Returns one or more `PaneInfo { id, pid, current_command, current_path }`.

3. **Identify claude panes.** For each pane, run `is_claude_process_tree(pane.pid)` (see below). Collect panes for which it returns `true`.

4. **Resolve.**
   - 0 claude panes → `NoClaudePane`
   - 1 claude pane → `Ok { pane_id }`
   - >1 → `MultipleClaudePanes { pane_ids }`

## Process Tree Walk

```
is_claude_process_tree(root_pid: u32) → bool
```

Walks `root_pid` and its descendants and returns `true` if any of them is a Claude Code process. The root PID itself is included in the check — when `claude` is invoked directly with no intermediate shell, the pane PID *is* the claude process.

A process is "Claude Code" if its **executable path** ends in a known basename:

- `claude` (the standard Claude Code binary name)
- `claude-code` (alternative install name)

Basename matching is **case-sensitive** on both macOS and Linux. The Claude Code distribution uses a lowercase binary name; an install with a differently-cased name (`Claude`) is not detected. Symlinks in the executable path are not canonicalized — whatever path the OS reports (`/proc/<pid>/exe` resolves symlinks; macOS `ps` does not always) is the path we match against. In practice both produce a basename ending in `claude`.

Self-reported process names, argv\[0\], and tmux's `pane_current_command` are all ignored — Claude Code rewrites its own title to the version string, and argv\[0\] is often `node` because Claude Code is a Node CLI.

The walk does **not short-circuit on first match**: it visits every descendant up to the bounds below. This is cheap (≤64 visits) and means a wrapper script or shell that happens to be named `claude` doesn't mask a real claude further down the tree. The boolean result is "did we see at least one claude," but the walk is exhaustive so future variants can return counts or pids without redesign.

### Walk Mechanism

The walk is OS-specific because there is no portable cross-platform process API in std. v1 supports macOS and Linux:

- **macOS**: `ps -A -o pid=,ppid=,comm=` once at the start of the walk; build a child map; DFS from `root_pid`. For each descendant, resolve the executable via `ps -p <pid> -o comm=` (which on macOS returns the full path of the executable, not the renamed title).
- **Linux**: read `/proc/<pid>/exe` (a symlink to the executable path) for each descendant. Children are listed via `/proc/<pid>/task/<tid>/children` or, more portably, by scanning `/proc/*/stat` for matching `ppid`.

The walk is bounded: depth ≤ 8, total descendants visited ≤ 64. A pane shell rarely has more than a handful of descendants; these caps prevent runaway in pathological cases.

Both platforms surface failures as `Locator returns conservative result: pane treated as non-claude.` The locator never errors on a single pane's walk failing — it just means that pane doesn't count as claude. This is the right default: if we can't prove a pane is claude, we should not send keystrokes to it.

## Concurrency

`/sessions` runs the locator for all seven colors. The natural implementation is to walk colors sequentially in ROYGBIV order — list_windows is one call, then per-color list_panes + process walks. v1 does not parallelize: the cost is a handful of tmux invocations and a `ps` per color, well under 100ms in practice. If profiling shows otherwise, parallelizing per color is a single-thread-per-color spawn (matching the gbiv `parallel-by-color` pattern).

Within a single resolution, pane info from `list_panes` is read once. The walk uses that snapshot. Processes can come and go between snapshot and walk, including PID reuse: a pane PID could in principle be reassigned to an unrelated process between `list_panes` and `is_claude_process_tree`. v1 accepts this race because the consequences are mild: at worst the response says `NoClaudePane` when claude was just starting, or `Ok` for a pane that just exited (the subsequent `capture_pane` then returns `PaneNotFound`). PID reuse mid-resolution would require a process to die and a new one to be spawned with the exact same PID inside the few milliseconds of one HTTP request — vanishingly rare on the timescales involved.

## Decisions & Alternatives

| Decision | Chosen | Alternatives | Rationale |
|---|---|---|---|
| Identification signal | Executable path of any descendant | tmux `pane_current_command`, argv\[0\], pane title, claude lockfile | Claude Code's self-reported name is the version number; the executable path is the only stable signal |
| Walk depth | DFS, bounded depth and count | Single-level (direct children only) | Users sometimes wrap claude in a shell script or `direnv exec`; depth-1 would miss those |
| Match semantics | Path basename in `{claude, claude-code}` | Full-path match against known install dirs | Install paths vary per-user and per-OS; basenames are stable |
| Multiple claude panes | Surface as distinct status | Pick the first / pick the most recent | Commander should be told. Auto-picking would silently misroute keystrokes |
| OS support | macOS + Linux native APIs | Use a `sysinfo`-style crate | One small dep avoided; the walk is ~30 lines per platform; future port targets (Windows? unlikely) can add modules |
| Error policy | Walk failure → "not claude" | Walk failure → propagate | Refusing to send keystrokes is safer than guessing |
| Caching | None (re-walk per request) | Cache resolutions for N seconds | Daemon is on-demand; pane state can change between requests; caching adds invalidation complexity for no measurable benefit |

## Edge Cases

| Case | Behavior |
|---|---|
| Window exists but is empty (impossible in tmux) | `list_panes` returns empty → `NoClaudePane` |
| Pane shell forks a non-claude process and claude is gone | Walk finds no claude executable → `NoClaudePane` |
| User has two terminal multiplexers and runs claude inside a nested tmux | Only the outer pane's process tree is walked. If claude is reachable as a descendant of the outer pane's PID, it is found |
| Claude crashed; only the shell is left | Walk finds shell + maybe some defunct child → `NoClaudePane` |
| Pane has multiple claude children (unusual; e.g., user explicitly `claude & claude &`) | Locator returns `MultipleClaudePanes` for the *window*, not for the *pane*; the pane is counted once. Two such panes in one window → `MultipleClaudePanes` |
| `claude` binary is a wrapper script that execs the real claude | Walk follows the exec; the real claude shows up as a descendant. If the wrapper is itself named `claude`, the wrapper is matched too — either way returns true |
| User aliased `claude` to a script with a different basename (e.g., `cc`) | Not detected. HLD already calls this out as a non-goal |
| Process tree walk hits a permission error | Treated as walk failure for that pane → pane not counted as claude |
| Color is not a valid ROYGBIV color | Caller's responsibility; the locator does not validate. HTTP layer rejects unknown colors before calling |

## Technical Debt & Future Work

1. **No tie-breaking when multiple claude panes are found.** v1 surfaces the ambiguity. A future version could prefer the pane whose process started most recently, or expose pane metadata (cwd, ppid chain) so the commander can disambiguate.
2. **No Windows support.** Out of scope for v1.
3. **Walk re-runs on every request.** Caching with a short TTL (e.g., 1s) is the obvious optimization if `/sessions` becomes a hot path.
4. **Configurable matchers.** The set of accepted basenames (`{claude, claude-code}`) is hard-coded. A flag like `--claude-binary <name>` could let users with custom installs opt in.

## References

- HLD: `docs/gbork/high-level-design.md` § Components > Pane Locator
- Companion: `docs/gbork/llds/tmux-driver.md` (`list_windows`, `list_panes`)

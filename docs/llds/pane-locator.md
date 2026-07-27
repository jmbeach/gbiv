# Pane Locator

**Created**: 2026-04-28
**Status**: Draft

## Context

For a given color (e.g., `red`), the Pane Locator answers two questions:

1. Does the gbiv tmux session have a window for this color?
2. If so, which pane in that window is running Claude Code?

Both answers feed every HTTP endpoint. `/sessions` runs the locator across all seven colors; `/session/:color` and `/session/:color/send` run it once. The locator's output is a `Resolution` value that tells the caller exactly what to do — including how to fail.

The locator is the only place in gbiv that reasons about Claude Code's process identity. Higher layers see only `Resolution`.

## Why a Custom Locator

`tmux capture-pane` and `send-keys` need a specific pane ID. The naive approach — "first pane in the window" — breaks the moment a user splits the window for an editor or watcher. The natural tmux signal — `#{pane_current_command}` — is also unreliable: Claude Code calls `process.title = "<version>"`, so its foreground process appears in tmux as `2.1.122` (or whatever version is running). The reported "command" is a number that changes between releases.

The locator therefore identifies Claude Code by its **executable path**, not its process name, by walking the process tree under each pane's PID and inspecting each descendant's executable.

## Resolution

```rust
enum Resolution {
    Ok {
        pane_id: String,
        // When >1 claude pane was present, the also-rans (oldest-first
        // ordering put `pane_id` at index 0; the rest live here so the
        // HTTP layer can include them in the response for transparency).
        other_pane_ids: Vec<String>,
    },
    NoWindow,            // no tmux window for this color
    NoClaudePane,        // window exists, no pane is running claude
}
```

When more than one claude pane is found, the locator picks the **oldest** by process start time and returns `Ok`. The other claude pane IDs are returned alongside in `other_pane_ids` (empty in the common single-pane case) so the HTTP layer can surface the situation without changing status. Rationale: the commander almost always wants the long-running claude session for the worktree; a newer claude in the same window is typically a transient (a `claude --print` invocation, a nested session a user spun up to ask a quick question). Picking the oldest matches that intent and keeps `/send` usable instead of forcing a 409.

## Locating a Pane

Inputs: `session: &str`, `color: &str`.

1. **Find the window.** `tmux_driver::list_windows(session)`. Match a window where `name == color`. If none, return `NoWindow`. (The session itself missing produces `TmuxError::SessionNotFound` from the driver, which the caller maps to a 5xx — that's a daemon-misconfiguration case, not a locator case.)

2. **List panes.** `tmux_driver::list_panes(format!("{session}:{color}"))`. Returns one or more `PaneInfo { id, pid, current_command, current_path }`.

3. **Identify claude panes.** For each pane, run `is_claude_process_tree(pane.pid)` (see below). Collect panes for which it returns `true`.

4. **Resolve.**
   - 0 claude panes → `NoClaudePane`
   - 1 claude pane → `Ok { pane_id, other_pane_ids: [] }`
   - >1 claude panes → sort by the start time of the matching claude process within each pane (ascending — oldest first); return `Ok { pane_id: <oldest>, other_pane_ids: <rest in same order> }`. Ties on start time (same second/jiffy) break by lower PID, then by lexicographic pane ID — fully deterministic.

### Reading process start time

- **macOS**: `ps -p <pid> -o lstart=` returns a parseable timestamp; or `-o etime=` for elapsed time (lower etime = younger). v1 uses `lstart=` parsed to a unix timestamp.
- **Linux**: field 22 of `/proc/<pid>/stat` is the process start time in clock ticks since boot. Lower value = older.

When a pane has multiple claude descendants (rare; see Edge Cases), the *earliest* claude descendant's start time represents that pane in the sort.

If reading start time fails for a candidate (process exited mid-resolution, permission error), that pane drops to the back of the sort — we'd rather pick a known-old pane than a pane whose age we can't confirm.

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

- **macOS**: `ps -A -o pid=,ppid=` once at the start of the walk to build a child map; DFS from `root_pid`. For each descendant, resolve the executable via `ps -p <pid> -o comm=` (which on macOS returns the full path of the executable, not the renamed title). The bulk listing omits `comm` because macOS truncates it there; the per-pid query returns the full path.
- **Linux**: read `/proc/<pid>/exe` (a symlink to the executable path) for each descendant. Children are listed via `/proc/<pid>/task/<tid>/children` or, more portably, by scanning `/proc/*/stat` for matching `ppid`.

The walk is bounded: depth ≤ 8, total descendants visited ≤ 64. A pane shell rarely has more than a handful of descendants; these caps prevent runaway in pathological cases.

Both platforms surface failures as `Locator returns conservative result: pane treated as non-claude.` The locator never errors on a single pane's walk failing — it just means that pane doesn't count as claude. This is the right default: if we can't prove a pane is claude, we should not send keystrokes to it.

## Errors

Resolution itself is total — the four `Resolution` variants are not error states; they're outcomes the HTTP layer maps to status codes. The locator does propagate one true error, though: if `tmux_driver::list_windows` returns `TmuxError::SessionNotFound` (the gbiv tmux session itself is missing), the locator surfaces that as `Err(LocatorError::TmuxSession(TmuxError))`. Process-tree walk failures stay internal — they degrade individual panes to "not claude," not the whole call.

```rust
#[derive(Debug, thiserror::Error)]
enum LocatorError {
    #[error("tmux session missing: {0}")]
    TmuxSession(#[from] TmuxError),
}
```

`#[from]` lets callers `?` a `TmuxError` straight into a `LocatorError`. The locator never uses `anyhow`.

## Concurrency

`/sessions` runs the locator for all seven colors. The batch entry point `locate_panes(session, colors)` walks colors sequentially in ROYGBIV order but builds the expensive shared state — the host-wide process snapshot (the `ps -A` / `/proc` child map) **and** the `list_windows` call — exactly once, then resolves every color against it. This avoids the full-host scan being repeated per color that calling the single-color `locate_pane` seven times would incur. Only the per-pane work (per-pid executable/start-time reads and the tree walk) is inherently per-color. v1 does not parallelize the per-color work: the remaining cost is a handful of tmux invocations plus bounded `ps`/`/proc` reads per color, well under 100ms in practice. If profiling shows otherwise, parallelizing per color is a single-thread-per-color spawn (matching the gbiv `parallel-by-color` pattern).

Per-color error handling in the batch is isolated: a color whose `list_panes` fails yields an `Err` for that color alone, while the others still resolve. A failure of the shared `list_windows` (the session itself is missing) fails the whole batch — there are no windows to resolve against. Neither entry point caches across calls; the shared snapshot lives only for the duration of one `locate_panes` call, preserving per-request freshness.

Within a single resolution, pane info from `list_panes` is read once. The walk uses that snapshot. Processes can come and go between snapshot and walk, including PID reuse: a pane PID could in principle be reassigned to an unrelated process between `list_panes` and `is_claude_process_tree`. v1 accepts this race because the consequences are mild: at worst the response says `NoClaudePane` when claude was just starting, or `Ok` for a pane that just exited (the subsequent `capture_pane` then returns `PaneNotFound`). PID reuse mid-resolution would require a process to die and a new one to be spawned with the exact same PID inside the few milliseconds of one HTTP request — vanishingly rare on the timescales involved.

## Decisions & Alternatives

| Decision | Chosen | Alternatives | Rationale |
|---|---|---|---|
| Identification signal | Executable path of any descendant | tmux `pane_current_command`, argv\[0\], pane title, claude lockfile | Claude Code's self-reported name is the version number; the executable path is the only stable signal |
| Walk depth | DFS, bounded depth and count | Single-level (direct children only) | Users sometimes wrap claude in a shell script or `direnv exec`; depth-1 would miss those |
| Match semantics | Path basename in `{claude, claude-code}` | Full-path match against known install dirs | Install paths vary per-user and per-OS; basenames are stable |
| Multiple claude panes | Pick the oldest by process start time; expose the others in `other_pane_ids` | Surface as distinct status (no auto-pick); pick most recent; pick lowest PID | The long-running session is almost always what the commander wants. Newer claudes are typically transient (`claude --print`, ad-hoc nested session). Picking the oldest keeps `/send` usable while still surfacing the situation |
| Tie-break for same start time | Lower PID, then lexicographic pane ID | Random | Deterministic ordering means the commander gets the same pane on repeated calls without surprise |
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
| Pane has multiple claude children (unusual; e.g., user explicitly `claude & claude &`) | The pane is counted once; the *earliest* claude descendant's start time represents the pane in the sort. Two panes each with their own claude → both compete on age, oldest wins |
| `claude` binary is a wrapper script that execs the real claude | Walk follows the exec; the real claude shows up as a descendant. If the wrapper is itself named `claude`, the wrapper is matched too — either way returns true |
| User aliased `claude` to a script with a different basename (e.g., `cc`) | Not detected. HLD already calls this out as a non-goal |
| Process tree walk hits a permission error | Treated as walk failure for that pane → pane not counted as claude |
| Color is not a valid ROYGBIV color | Caller's responsibility; the locator does not validate. HTTP layer rejects unknown colors before calling |

## Technical Debt & Future Work

1. **Tie-breaker is a heuristic.** Oldest-pane-wins is right for the common case (long-running worktree session vs. transient nested claude) but wrong if the user genuinely intended the newer pane. A future version could expose pane metadata (cwd, ppid chain, claude session id once it's available via tmux env or a sidecar file) so the commander can disambiguate explicitly, or accept a `?prefer=newest` query param.
2. **No Windows support.** Out of scope for v1.
3. **Walk re-runs on every request.** Caching with a short TTL (e.g., 1s) is the obvious optimization if `/sessions` becomes a hot path.
4. **Configurable matchers.** The set of accepted basenames (`{claude, claude-code}`) is hard-coded. A flag like `--claude-binary <name>` could let users with custom installs opt in.

## References

- HLD: `docs/high-level-design.md` § Components > Pane Locator
- Companion: `docs/llds/tmux-driver.md` (`list_windows`, `list_panes`)

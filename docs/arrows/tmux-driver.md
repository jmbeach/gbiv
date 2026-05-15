# Arrow: tmux Driver

The only roy component that touches the tmux CLI: list_windows, list_panes, capture_pane, send_keys.

**Status**: UNMAPPED (sampled 2026-05-15) — HLD + LLD authored; EARS specs not yet written; no code.

## References

| Artifact | Location |
|---|---|
| HLD sections | `docs/roy/high-level-design.md` |
| LLD | `docs/roy/llds/tmux-driver.md` |
| EARS specs | (none yet — pending `docs/roy/specs/tmux-driver.md`) |
| Source | (none yet — pending `crates/roy/` or equivalent) |
| Tests | (none yet) |

## Spec Coverage

| Category | Spec IDs | Implemented | Deferred | Gaps |
|---|---|---|---|---|
| (pending) | — | 0 | 0 | 0 |

## Architecture

**Purpose:** Centralize tmux subprocess invocation, argument escaping, and exit-code handling so the rest of roy can be tested against a fake driver.

**Key Operations:**
1. `list_windows` — enumerate windows in a session
2. `list_panes` — enumerate panes in a window with `pane_id`, `pane_pid`, `pane_current_command`, `pane_current_path`
3. `capture_pane` — capture recent textual contents of a pane
4. `send_keys` — type text into a pane and press Enter

## Work Required

- Author EARS specs in `docs/roy/specs/tmux-driver.md`
- Generate implementation plan
- Implement driver + tests

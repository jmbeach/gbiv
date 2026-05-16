# Arrow: Pane Locator

For a given color, answers (a) does the gbiv tmux session have a window for it, and (b) which pane in that window is running Claude Code.

**Status**: UNMAPPED (sampled 2026-05-15) — HLD + LLD authored; EARS specs not yet written; no code.

## References

| Artifact | Location |
|---|---|
| HLD sections | `docs/roy/high-level-design.md` |
| LLD | `docs/roy/llds/pane-locator.md` |
| EARS specs | (none yet — pending `docs/roy/specs/pane-locator.md`) |
| Source | (none yet) |
| Tests | (none yet) |

## Spec Coverage

| Category | Spec IDs | Implemented | Deferred | Gaps |
|---|---|---|---|---|
| (pending) | — | 0 | 0 | 0 |

## Architecture

**Purpose:** The only place in roy that reasons about Claude Code's process identity. Outputs a `Resolution` value that tells the caller exactly how to proceed (or how to fail).

**Why custom:** Naive "first pane" breaks when users split. `#{pane_current_command}` is unreliable because Claude Code sets `process.title` to its version string. The locator walks process trees instead.

## Work Required

- Author EARS specs in `docs/roy/specs/pane-locator.md`
- Implement against tmux-driver

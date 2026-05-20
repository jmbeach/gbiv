# Arrow: Tmux Primitives

Shared tmux lookup primitives in `gbiv-core` consumed by the `gbiv` binary today and by `roy` once its tmux driver lands.

**Status**: OK (sampled 2026-05-19) — All 26 TMX-CORE-* specs implemented; gbiv's tmux subcommands migrated to the primitives.

## References

| Artifact | Location |
|---|---|
| HLD | `docs/gbiv-core/high-level-design.md` |
| LLD | `docs/gbiv-core/llds/tmux-primitives.md` |
| EARS specs | `docs/gbiv-core/specs/tmux-primitives.md` |
| Source | `crates/gbiv-core/src/tmux.rs` |
| Tests | `crates/gbiv-core/tests/tmux.rs`, inline `#[cfg(test)] mod tests` in `crates/gbiv-core/src/tmux.rs` |

## Spec Coverage

| Category | Spec IDs | Implemented | Deferred | Gaps |
|---|---|---|---|---|
| Public surface | TMX-CORE-001..003 | 3 | 0 | 0 |
| tmux_available | TMX-CORE-010..016 | 6 | 0 | 0 |
| has_session | TMX-CORE-020..024 | 5 | 0 | 0 |
| list_windows | TMX-CORE-030..036 | 7 | 0 | 0 |
| session_name_for_root | TMX-CORE-040..042 | 3 | 0 | 0 |
| Subprocess conventions | TMX-CORE-060..061 | 2 | 0 | 0 |
| **Total** | | **26** | **0** | **0** |

## Key Findings

1. `tmux_available()`, `has_session()`, `list_windows()`, and `session_name_for_root()` are the only shared tmux operations; window mutation stays in gbiv, pane operations stay in roy.
2. One `TmuxError` enum in `gbiv-core` covers both binaries; roy populates the pane-variants without an additional wrapping type.
3. `tmux_available` distinguishes only "installed vs. not"; no minimum tmux version is gated.
4. All parsers operate on UTF-8 lossy decoding; malformed `list-windows` lines abort the call.
5. `Other` message format: `stderr.trim()` if non-empty, else `exit status: {code}` (or `"signal"`).

## Dependencies

| This arrow depends on | For |
|---|---|
| (none) | — |

| Depended on by | For |
|---|---|
| Tmux Mirror (gbiv) | `tmux_available`, `has_session`, `list_windows`, `session_name_for_root` used by `new-session`, `sync`, `clean` |
| Tmux Driver (roy) | `tmux_available`, `has_session`, `list_windows` reused; pane ops layered on top |

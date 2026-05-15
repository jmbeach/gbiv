# Arrow: CLI & Palette

Command dispatch, argument parsing, and terminal color formatting.

**Status**: AUDITED (audited 2026-05-15, git `1638fe0`) — All 33 specs implemented.

## References

| Artifact | Location |
|---|---|
| HLD sections | `docs/gbiv/high-level-design.md` § Component Architecture > CLI & Palette |
| LLD | `docs/gbiv/llds/cli-and-palette.md` |
| EARS specs | `docs/gbiv/specs/cli-and-palette.md` |
| Source | `crates/gbiv-core/src/colors.rs`, `crates/gbiv/src/main.rs`, `crates/gbiv/src/colors.rs` |
| Tests | `crates/gbiv-core/src/colors.rs` (palette unit tests), `crates/gbiv/src/main.rs` (inline tests for exec parsing) |

## Spec Coverage

| Category | Spec IDs | Implemented | Deferred | Gaps |
|---|---|---|---|---|
| Dispatch | CLI-DISPATCH-001..010 | 10 | 0 | 0 |
| Exec Argument Parsing | CLI-EXEC-PARSE-001..007 | 7 | 0 | 0 |
| Color Palette | CLI-COLOR-001..016 | 16 | 0 | 0 |
| **Total** | | **33** | **0** | **0** |

## Key Findings

1. COLORS constant is the single source of truth for valid colors — used by every component.
2. clap builder API (not derive) — gives explicit control, especially for exec's freeform args.
3. Color validation happens in command handlers, not clap — enables custom error messages.
4. ANSI codes always emitted (no `--color=auto`). Fine for interactive use, problematic if piped.
5. Unknown color → RESET fallback in ansi_color() — defensive, can't happen through normal CLI paths.
6. Exit code is always 0 (success) or 1 (any error). No per-error codes.

## Dependencies

| This arrow depends on | For |
|---|---|
| (none) | Root of the dependency tree |

| Depended on by | For |
|---|---|
| Worktree Lifecycle | COLORS constant, command routing |
| Feature Ledger | COLORS constant, command routing |
| Observation | COLORS constant, ANSI codes, exec arg parsing, command routing |
| Tmux Mirror | COLORS constant, command routing |

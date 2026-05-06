# Arrow: CLI & Palette

Command dispatch, argument parsing, and terminal color formatting.

**Status**: MAPPED (2026-04-23) — Brownfield inventory complete. All current behavior documented.

## References

| Artifact | Location |
|---|---|
| HLD sections | `docs/gbiv/high-level-design.md` § Component Architecture > CLI & Palette |
| LLD | `docs/gbiv/llds/cli-and-palette.md` |
| EARS specs | `docs/gbiv/specs/cli-and-palette.md` |
| Source | `src/main.rs`, `src/colors.rs` |
| Tests | `src/main.rs` (inline tests for exec parsing) |

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

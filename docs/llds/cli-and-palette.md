# CLI & Palette

**Created**: 2026-04-23
**Status**: Complete (brownfield mapping)

## Context and Current State

This component owns two cross-cutting concerns: the command-line interface dispatch layer (`main.rs`) and the color/formatting constants (`colors.rs`). Every other component depends on these — CLI for routing user input, palette for terminal output.

This is the thinnest component by line count (~290 lines total) but it defines the contracts that all other components conform to.

## CLI Dispatch (`main.rs`)

### Command Tree

```
gbiv
├── init <folder>
├── status
├── mark (--done | --in-progress | --unset) [<color>]
├── reset [<color>] [--hard/--force] [--yes/-y]
├── rebase-all
├── tidy
├── exec [<color>|all] -- <command...>
└── tmux
    ├── new-session [--session-name <NAME>]
    ├── sync [--session-name <NAME>]
    └── clean
```

### Argument Parsing

All parsing uses `clap` (v4.5.54) with the builder API (not derive). The `cli()` function constructs the full `Command` tree and is `pub(crate)` so tests can validate subcommand registration.

### Dispatch Flow

`main()` calls `cli().get_matches()`, then pattern-matches on the subcommand name to call the appropriate handler. Each handler returns `anyhow::Result<()>` (migrating from the historical `Result<_, String>` — see HLD § "Error Propagation"). On `Err`, `main` prints the chain to stderr (top-level message by default; full `anyhow` cause chain when debug-level logging is enabled, as reported by `core::observability::debug_enabled()` — so any `RUST_LOG` form that enables debug for gbiv, e.g. `debug` or `gbiv=debug`, gets the full chain) and exits with code 1.

### Exec Argument Parsing

Exec has special parsing because its arguments are positional + freeform:

```rust
// Simplified from main.rs lines ~162-174
fn parse_exec(argv) -> (Option<target>, Vec<command>)
```

1. Collect all raw args into a vector
2. If first arg matches a ROYGBIV color or `"all"`, treat it as target; shift rest to command
3. Otherwise, target is None (infer from CWD)
4. Strip `--` separator from command tokens
5. Error if command is empty after stripping

This logic lives in `main.rs` rather than `exec.rs` because clap's `num_args(0..)` with `allow_hyphen_values(true)` captures everything as a flat list — the semantic split happens post-parse.

### Color Validation

Several subcommands accept an optional `<color>` argument. Validation happens at the command handler level (checking against `COLORS`), not at the clap level. This means clap accepts any string and the handler returns a descriptive error like `"'purple' is not a valid color"`.

## Palette

The palette splits along a binary boundary: the canonical color list is shared with the orchestration daemon via the `core` module; ANSI escape codes and terminal formatting stay worktree-only.

### ROYGBIV Constant — lives in `core::colors`

```rust
pub const COLORS: [&str; 7] = [
    "red", "orange", "yellow", "green", "blue", "indigo", "violet"
];
pub fn is_valid_color(name: &str) -> bool;
```

This array is the single source of truth for:
- Which worktrees exist (init creates one per color)
- Valid color arguments (mark, reset, exec)
- Iteration order (status, exec-all, rebase-all use ROYGBIV order)
- Tmux window names and sort order

`COLORS` and `is_valid_color` live in the `core` module so the orchestration daemon can validate `:color` URL params and iterate `/sessions` in the same canonical order without re-declaring the list. Both binaries must agree about which seven names are colors, so there can only be one home for this.

### ANSI Codes — worktree-only (`src/colors.rs`)

The ANSI escape codes and formatting constants below stay in the gbiv binary. the orchestration daemon emits JSON and has no use for terminal escapes.

### ANSI Codes

```rust
pub fn ansi_color(color: &str) -> &'static str
```

Maps color names to ANSI escape sequences:

| Color | ANSI Code | Type |
|---|---|---|
| red | `\x1b[31m` | Standard 8-color |
| orange | `\x1b[38;5;208m` | 256-color extended |
| yellow | `\x1b[33m` | Standard |
| green | `\x1b[32m` | Standard |
| blue | `\x1b[34m` | Standard |
| indigo | `\x1b[38;5;54m` | 256-color extended |
| violet | `\x1b[35m` | Standard (magenta) |
| unknown | `\x1b[0m` | Reset (fallback) |

### Formatting Constants

| Constant | Value | Used For |
|---|---|---|
| `RESET` | `\x1b[0m` | End any ANSI sequence |
| `DIM` | `\x1b[2m` | Muted text (branch names, clean status, backlog label, zero counts) |
| `YELLOW` | `\x1b[33m` | Warnings (dirty, not merged) |
| `GREEN` | `\x1b[32m` | Positive indicators (ahead count > 0) |
| `RED` | `\x1b[31m` | Negative indicators (behind count > 0) |

### Color Semantics

The palette encodes a consistent visual language across all commands:

- **Color name in its own color**: identity (e.g., `red` printed in red ANSI)
- **DIM**: neutral/inactive state
- **YELLOW**: attention needed (dirty worktree, unmerged branch)
- **GREEN**: positive (commits ahead)
- **RED**: negative (commits behind)

## Observed Design Decisions

| Decision | Chosen | Alternatives Considered | Rationale |
|---|---|---|---|
| clap builder API | `Command::new()` chain | clap derive macros | Builder gives explicit control over arg parsing, especially for exec's freeform args. |
| Color validation in handlers | Handler checks `COLORS` | clap `PossibleValue` | Allows custom error messages. Also enables "infer from CWD" when color is omitted. |
| Hardcoded ANSI codes | Direct escape sequences | `colored` crate, `termcolor` | Zero dependencies for terminal output. gbiv targets modern terminals where ANSI is universal. |
| 256-color for orange/indigo | Extended ANSI codes | Nearest 8-color approximation | Orange and indigo don't have standard 8-color equivalents. 256-color support is widespread. |
| Unknown color → reset | Fallback to `\x1b[0m]` | Panic, return error | Defensive — unknown color silently renders as plain text rather than crashing. |
| Exit code 1 for all errors | Single non-zero code | Per-error exit codes | Simple. gbiv is interactive, not heavily scripted. One non-zero code is sufficient. |

## Technical Debt & Inconsistencies

1. **No `--color` flag**: ANSI codes are always emitted, even when stdout is piped to a file or another program. No `--color=auto/always/never` support. In practice this hasn't been an issue because gbiv is used interactively.

2. **No `--help` customization**: clap's default help formatting is used. The about strings are minimal (e.g., `"Show status of all ROYGBIV worktrees"`). No examples or extended help.

3. **Exec parsing in main.rs**: The `parse_exec()` logic is in `main.rs` rather than `exec.rs`, which splits the exec command's concerns across two files. This happened because clap delivers all args as a flat list and the semantic parsing needs to happen before calling the handler.

## Behavioral Quirks

1. **No global flags**: There are no flags that apply to all subcommands (e.g., `--verbose`, `--quiet`). Each subcommand defines its own flags independently.

2. **`gbiv` with no subcommand**: Prints clap's auto-generated help and exits. No default action.

3. **`gbiv tmux` with no sub-subcommand**: Also prints help, but exits with code 1 (non-zero) rather than 0. This is because the tmux handler explicitly returns an error when no subcommand is provided.

4. **ANSI fallback for unknown colors**: `ansi_color("purple")` returns the reset code, which means the text renders unstyled rather than crashing. This can only happen if a color string bypasses validation — currently not possible through normal CLI paths.

## References

- `src/main.rs` — CLI definition and dispatch
- `src/colors.rs` — ANSI codes and formatting constants (worktree-only)
- `src/core/colors.rs` — `COLORS` slice, `is_valid_color`, `infer_color_from_path` (shared with the orchestration daemon)

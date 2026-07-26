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
2. If first arg matches a name in the active palette or `"all"`, treat it as target; shift rest to command
3. Otherwise, target is None (infer from CWD)
4. Strip `--` separator from command tokens
5. Error if command is empty after stripping

This logic lives in `main.rs` rather than `exec.rs` because clap's `num_args(0..)` with `allow_hyphen_values(true)` captures everything as a flat list — the semantic split happens post-parse. Because step 2 tests membership in the *active palette* (not a compile-time constant), the split is performed after root discovery has loaded the palette; the raw token list is carried until then.

### Color Validation

Several subcommands accept an optional `<color>` argument. Validation happens at the command handler level (checking the argument against the loaded active palette via `Palette::contains`), not at the clap level. clap accepts any string and the handler returns a descriptive error like `"'purple' is not a valid color"`. This applies uniformly to `mark`, `reset`, and `exec`: color validity depends on the active palette, which is only known after root discovery, so it cannot be enforced by a clap `PossibleValuesParser` (which runs during argument parsing, before the root is found). The `all` keyword accepted by `exec` is checked alongside palette membership.

## Palette

The palette splits along a binary boundary: the canonical worktree-name list is shared with the orchestration daemon via the `core` module; ANSI escape codes and terminal formatting stay worktree-only.

The palette has two layers: an immutable **base** of seven ROYGBIV names known at compile time, and an **active palette** loaded at runtime that equals the base plus any extra names declared in the project's optional `.gbiv/config.toml`. The base is the default; absent a config file the active palette is exactly the seven ROYGBIV names, so behaviour is unchanged for the common case.

### Base constant — lives in `core::colors`

```rust
pub const BASE_COLORS: [&str; 7] = [
    "red", "orange", "yellow", "green", "blue", "indigo", "violet"
];
```

`BASE_COLORS` is the immutable ROYGBIV prefix of every active palette and the seed `gbiv init` creates. It is also what **root discovery** keys off (`find_gbiv_root` checks for at least one `BASE_COLORS` directory) — root discovery cannot depend on the active palette, because the active palette is loaded *from* the root it would be trying to find. The base names are fixed: the config appends, it never renames or removes a base color.

### Active palette — `core::palette::Palette`

```rust
pub struct Palette { names: Vec<String> }   // BASE_COLORS first, then config extras, in order

impl Palette {
    pub fn load(gbiv_root: &Path) -> Result<Palette, ConfigError>; // reads .gbiv/config.toml
    pub fn from_extras(extras: Vec<String>) -> Palette;  // BASE_COLORS then extras (tests / known callers)
    pub fn names(&self) -> &[String];       // full active list, in canonical order
    pub fn extras(&self) -> &[String];      // names beyond the base seven
    pub fn contains(&self, name: &str) -> bool;
    pub fn is_base(name: &str) -> bool;     // BASE_COLORS membership, no load needed
}
// Palette also implements Default (BASE_COLORS only).
```

`Palette` is the single source of truth, at runtime, for:
- Which worktrees the active project has (base seven + configured extras)
- Valid color arguments (mark, reset, exec) — via `contains`
- Iteration order (status, exec-all, rebase-all, repair iterate `names()` in order)
- Tmux window names and sort order

Each command loads the palette once, after root discovery, and passes it (or its `names()`) to the helpers that need it. `infer_color_from_path` takes the active palette and returns an owned `Option<String>` (the palette is runtime data, not `&'static`). `Palette` lives in `core` so the orchestration daemon validates `:color` URL params and iterates `/sessions` over the same active palette without re-declaring it. Both binaries load the same `.gbiv/config.toml` from the same root, so they always agree.

### Config loading — `core::config`

The palette's extras are declared in an optional `.gbiv/config.toml` at the gbiv root. It is a *general* config file (sectioned so other config domains can live alongside the palette), not a feature-state store:

```toml
[palette]
extra = ["my-lingering-feature", "another-slot"]
```

`Palette::load` resolves `<root>/.gbiv/config.toml`. A missing file, a missing `[palette]` table, or an empty `extra` list all yield the default palette (base seven). An unrecognized *top-level* table is ignored (so the file can host other config domains), but an unrecognized key *inside* `[palette]` (a mistyped `extra`) is a hard parse error rather than a silent base-only palette. When `extra` is present each name is validated; the active palette is `BASE_COLORS` followed by the validated extras in declared order.

**Validation (all enforced at load; any violation is a hard error).** Each extra name must be:
- non-empty;
- unique, case-insensitively — no duplicate among the extras, and not equal to any base color (e.g. `Red` collides with `red`);
- not a reserved word — `main` (the canonical main worktree) or `all` (the exec/target keyword);
- a valid git branch name and a single safe path component — conservatively `[A-Za-z0-9._-]+`, not beginning with `-` or `.`.

On a TOML parse error or any validation failure, `load` returns `ConfigError` and the invoking command fails with a message naming the file and the offending value, rather than silently falling back to ROYGBIV — a malformed power-user config should be loud, and the remedy is to fix or delete the file.

```rust
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("reading {path}: {source}")] Io { path: PathBuf, source: std::io::Error },
    #[error("parsing {path}: {source}")] Parse { path: PathBuf, source: toml::de::Error },
    #[error("invalid palette name {name:?} in {path}: {reason}")] InvalidName { path: PathBuf, name: String, reason: String },
}
```

TOML parsing uses the `toml` + `serde` crates (added to `gbiv-core`). This is the project's only structured-config dependency; it is justified by a sectioned config that other domains can extend, while the default, no-config experience carries no runtime cost.

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

`ansi_color` maps only the seven base ROYGBIV names. Extra (non-base) palette names fall through to the reset fallback and therefore **render with no color** — base colors are painted in their hue, extras render plain. This is intentional: extras are user-chosen labels, not hues, so the palette carries no color for them.

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
| Unknown color → reset | Fallback to `\x1b[0m]` | Panic, return error | Defensive — unknown color silently renders as plain text rather than crashing. Doubles as the rendering for extra (non-base) palette names, which carry no hue. |
| Runtime active palette | `Palette` loaded per-command from the root | Compile-time `COLORS` const | Lets a project extend the worktree set without recompiling. ROYGBIV remains the compile-time `BASE_COLORS` default and prefix, so the no-config path is unchanged. |
| Config as `.gbiv/config.toml` | Sectioned TOML (`toml`+`serde`) | Plain-text name list; hand-rolled parser | A general, sectioned config file other domains can extend. Cost is a structured-config dependency; users without the file are unaffected. |
| Hard-fail on bad config | `ConfigError` aborts the command | Silent fallback to ROYGBIV | A malformed power-user config should be loud; the fix is to correct or delete one file. |
| Palette-dependent validation in handlers | `mark`/`reset`/`exec` validate post-load | clap `PossibleValuesParser` | The valid set is only known after root discovery loads the palette; clap parses earlier. |
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
- `core::colors` — `BASE_COLORS` const and `infer_color_from_path` (shared with the orchestration daemon)
- `core::palette` — `Palette` (runtime active palette, `load`/`contains`/`names`/`extras`)
- `core::config` — `.gbiv/config.toml` loading and `ConfigError`

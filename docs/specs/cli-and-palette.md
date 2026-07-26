# CLI & Palette

Specs for command dispatch, argument parsing, and terminal color formatting.

**Component LLD**: `docs/llds/cli-and-palette.md`

## Dispatch

- [x] CLI-DISPATCH-001: When gbiv is invoked with no subcommand, the system shall print help text and exit with code 0.
- [x] CLI-DISPATCH-002: When gbiv is invoked with a recognized subcommand, the system shall route to the corresponding handler function.
- [x] CLI-DISPATCH-003: When a handler returns Err, the system shall print the error message to stderr prefixed with "Error: " and exit with code 1.
- [x] CLI-DISPATCH-004: While registering subcommands, the system shall use the clap builder API (not derive macros).
- [x] CLI-DISPATCH-005: When gbiv is invoked with an unrecognized subcommand, the system shall print an error message and exit non-zero (clap default behavior).
- [x] CLI-DISPATCH-006: When `gbiv tmux` is invoked with no sub-subcommand, the system shall print help text and exit non-zero.
- [x] CLI-DISPATCH-007: When the `exec` handler receives Ok with non-empty output, the system shall print the output to stdout without a trailing newline added.
- [x] CLI-DISPATCH-008: When the `exec` handler receives Ok with empty output, the system shall produce no stdout.
- [x] CLI-DISPATCH-009: When the `exec` handler receives Err, the system shall print the error to stderr without the "Error: " prefix and exit with code 1.
- [x] CLI-DISPATCH-010: When the `mark` handler receives Ok, the system shall print the success message to stdout via println.
- [x] CLI-DISPATCH-011: When the top-level error handler prints a handler's Err to stderr, it shall print the full `anyhow` cause chain (the `{:#}` format) when `core::observability::debug_enabled()` is true, and only the top-level message otherwise.

## Exec Argument Parsing

- [x] CLI-EXEC-PARSE-001: When parsing exec arguments, the system shall collect all trailing arguments into a vector.
- [x] CLI-EXEC-PARSE-002: When the first argument matches a name in the active palette, the system shall treat it as the target and the remaining arguments as the command. The active palette must therefore be loaded (via root discovery) before the target/command split is decided.
- [x] CLI-EXEC-PARSE-003: When the first argument is "all", the system shall treat it as the target and the remaining arguments as the command.
- [x] CLI-EXEC-PARSE-004: When the first argument does not match a color or "all", the system shall set target to None and treat all arguments as the command.
- [x] CLI-EXEC-PARSE-005: When processing command tokens, the system shall strip any "--" separator tokens from the command vector.
- [x] CLI-EXEC-PARSE-006: If the command vector is empty after stripping, the system shall print a usage error to stderr and exit with code 1.
- [x] CLI-EXEC-PARSE-007: When exec arguments contain flags (e.g., "-la") after the "--" separator, the system shall preserve them as command tokens.

## Color Palette

- [x] CLI-COLOR-001: The BASE_COLORS constant shall contain exactly seven entries: "red", "orange", "yellow", "green", "blue", "indigo", "violet", in that order. BASE_COLORS is the immutable default palette and the prefix of every active palette.
- [x] CLI-COLOR-002: When ansi_color is called with "red", the system shall return the standard ANSI red escape sequence (\x1b[31m).
- [x] CLI-COLOR-003: When ansi_color is called with "orange", the system shall return the 256-color extended escape sequence (\x1b[38;5;208m).
- [x] CLI-COLOR-004: When ansi_color is called with "yellow", the system shall return the standard ANSI yellow escape sequence (\x1b[33m).
- [x] CLI-COLOR-005: When ansi_color is called with "green", the system shall return the standard ANSI green escape sequence (\x1b[32m).
- [x] CLI-COLOR-006: When ansi_color is called with "blue", the system shall return the standard ANSI blue escape sequence (\x1b[34m).
- [x] CLI-COLOR-007: When ansi_color is called with "indigo", the system shall return the 256-color extended escape sequence (\x1b[38;5;54m).
- [x] CLI-COLOR-008: When ansi_color is called with "violet", the system shall return the standard ANSI magenta escape sequence (\x1b[35m).
- [x] CLI-COLOR-009: When ansi_color is called with an unknown color name, the system shall return the RESET escape sequence (\x1b[0m) without crashing.
- [x] CLI-COLOR-017: When ansi_color is called with an extra (non-base) palette name, the system shall return the RESET escape sequence, so extra worktrees render with no color (extras are labels, not hues).
- [x] CLI-COLOR-010: The module shall export a RESET constant set to \x1b[0m.
- [x] CLI-COLOR-011: The module shall export a DIM constant set to \x1b[2m for neutral visual semantics.
- [x] CLI-COLOR-012: The module shall export a YELLOW constant set to \x1b[33m for attention visual semantics.
- [x] CLI-COLOR-013: The module shall export a GREEN constant set to \x1b[32m for positive visual semantics.
- [x] CLI-COLOR-014: The module shall export a RED constant set to \x1b[31m for negative visual semantics.
- [x] CLI-COLOR-015: When `Palette::contains` is called with a name that is in the active palette (base colors or a configured extra), the system shall return true.
- [x] CLI-COLOR-016: When `Palette::contains` is called with a name that is not in the active palette, the system shall return false.

## Active Palette & Config

- [x] CLI-COLOR-018: The active palette shall be loaded at runtime from the gbiv root; the base ROYGBIV names are fixed, and the config may append extra names but shall never rename or remove a base color.
- [x] CLI-COLOR-019: When `Palette::load` is called, the system shall read the optional config file at `<gbiv_root>/.gbiv/config.toml`.
- [x] CLI-COLOR-020: When `.gbiv/config.toml` is absent, has no `[palette]` table, or its `[palette].extra` list is empty, the active palette shall equal BASE_COLORS.
- [x] CLI-COLOR-021: When `[palette].extra` is present and all entries are valid, the active palette shall be BASE_COLORS followed by those extra names in their declared order.
- [x] CLI-COLOR-022: Each extra name shall be non-empty and shall match `[A-Za-z0-9._-]+` without beginning with `.` or `-`; an entry that does not shall cause `Palette::load` to fail with a `ConfigError` naming the file and the offending value.
- [x] CLI-COLOR-023: Each extra name shall be unique case-insensitively among the extras and against the base colors; a case-insensitive collision shall cause `Palette::load` to fail with a `ConfigError`.
- [x] CLI-COLOR-024: An extra name equal case-insensitively to a reserved word ("main" or "all") shall cause `Palette::load` to fail with a `ConfigError`.
- [x] CLI-COLOR-025: When `.gbiv/config.toml` exists but cannot be parsed as TOML, `Palette::load` shall fail with a `ConfigError` naming the file.
- [x] CLI-COLOR-026: When palette loading fails, the invoking command shall abort with that error rather than falling back to BASE_COLORS.
- [x] CLI-COLOR-027: Color-argument validation for `mark`, `reset`, and `exec` shall check the argument against the active palette at the command-handler level (not via a clap `PossibleValuesParser`), because the valid set is known only after root discovery loads the palette.
- [x] CLI-COLOR-028: When the `[palette]` table contains an unrecognized key (e.g. a misspelled `extra`), `Palette::load` shall fail with a `ConfigError::Parse` naming the file, so a typo fails loudly instead of silently yielding a base-only palette. Unrecognized *top-level* tables remain ignored (per CLI-COLOR-020) so the shared config file may host other sections.

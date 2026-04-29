# CLI & Palette

Specs for command dispatch, argument parsing, and terminal color formatting.

**Component LLD**: `docs/gbiv/llds/cli-and-palette.md`

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

## Exec Argument Parsing

- [x] CLI-EXEC-PARSE-001: When parsing exec arguments, the system shall collect all trailing arguments into a vector.
- [x] CLI-EXEC-PARSE-002: When the first argument matches a ROYGBIV color name, the system shall treat it as the target and the remaining arguments as the command.
- [x] CLI-EXEC-PARSE-003: When the first argument is "all", the system shall treat it as the target and the remaining arguments as the command.
- [x] CLI-EXEC-PARSE-004: When the first argument does not match a color or "all", the system shall set target to None and treat all arguments as the command.
- [x] CLI-EXEC-PARSE-005: When processing command tokens, the system shall strip any "--" separator tokens from the command vector.
- [x] CLI-EXEC-PARSE-006: If the command vector is empty after stripping, the system shall print a usage error to stderr and exit with code 1.
- [x] CLI-EXEC-PARSE-007: When exec arguments contain flags (e.g., "-la") after the "--" separator, the system shall preserve them as command tokens.

## Color Palette

- [x] CLI-COLOR-001: The COLORS constant shall contain exactly seven entries: "red", "orange", "yellow", "green", "blue", "indigo", "violet", in that order.
- [x] CLI-COLOR-002: When ansi_color is called with "red", the system shall return the standard ANSI red escape sequence (\x1b[31m).
- [x] CLI-COLOR-003: When ansi_color is called with "orange", the system shall return the 256-color extended escape sequence (\x1b[38;5;208m).
- [x] CLI-COLOR-004: When ansi_color is called with "yellow", the system shall return the standard ANSI yellow escape sequence (\x1b[33m).
- [x] CLI-COLOR-005: When ansi_color is called with "green", the system shall return the standard ANSI green escape sequence (\x1b[32m).
- [x] CLI-COLOR-006: When ansi_color is called with "blue", the system shall return the standard ANSI blue escape sequence (\x1b[34m).
- [x] CLI-COLOR-007: When ansi_color is called with "indigo", the system shall return the 256-color extended escape sequence (\x1b[38;5;54m).
- [x] CLI-COLOR-008: When ansi_color is called with "violet", the system shall return the standard ANSI magenta escape sequence (\x1b[35m).
- [x] CLI-COLOR-009: When ansi_color is called with an unknown color name, the system shall return the RESET escape sequence (\x1b[0m) without crashing.
- [x] CLI-COLOR-010: The module shall export a RESET constant set to \x1b[0m.
- [x] CLI-COLOR-011: The module shall export a DIM constant set to \x1b[2m for neutral visual semantics.
- [x] CLI-COLOR-012: The module shall export a YELLOW constant set to \x1b[33m for attention visual semantics.
- [x] CLI-COLOR-013: The module shall export a GREEN constant set to \x1b[32m for positive visual semantics.
- [x] CLI-COLOR-014: The module shall export a RED constant set to \x1b[31m for negative visual semantics.

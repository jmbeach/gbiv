// @spec TIDY-001
// Verify the tidy subcommand is registered in the CLI.
#[test]
fn cli_registers_tidy_subcommand() {
    let m = crate::cli().try_get_matches_from(["gbiv", "tidy"]);
    assert!(m.is_ok(), "tidy subcommand should be registered");
}

// @spec TIDY-003
// Verify tidy_command exists, is callable, and returns a Result
// (not a panic). The actual sub-commands may succeed or fail depending
// on the environment, so we only verify it runs to completion.
#[test]
fn tidy_command_runs_without_panic() {
    use crate::commands::tidy::tidy_command;
    let _result = tidy_command();
}

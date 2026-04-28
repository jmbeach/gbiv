// @spec CLI-DISPATCH-004
#[test]
fn cli_registers_tidy_subcommand() {
    let m = crate::cli().try_get_matches_from(["gbiv", "tidy"]);
    assert!(m.is_ok(), "tidy subcommand should be registered");
}

// @spec WTL-TIDY-001
#[test]
fn tidy_command_runs_without_panic() {
    use crate::commands::tidy::tidy_command;
    let _result = tidy_command();
}

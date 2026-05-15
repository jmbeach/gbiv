// @spec CLI-DISPATCH-004
#[test]
fn cli_registers_tidy_subcommand() {
    let m = crate::cli().try_get_matches_from(["gbiv", "tidy"]);
    assert!(m.is_ok(), "tidy subcommand should be registered");
}

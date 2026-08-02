use anyhow::Result;
use clap::{Arg, ArgGroup, Command};
use commands::init::init_command;
use commands::mark::mark_command;
use commands::rebase_all::rebase_all_command;
use commands::repair::repair_command;
use commands::reset::reset_command;
use commands::status::status_command;
use commands::tidy::tidy_command;
use commands::tmux;
use gbiv_core::palette::Palette;
use gbiv_core::root::find_gbiv_root;

mod colors;
mod commands;
mod gbiv_md;
mod git_utils;
mod orchestration;

// @spec CLI-DISPATCH-001, CLI-DISPATCH-002, CLI-DISPATCH-004, CLI-DISPATCH-005, CLI-DISPATCH-006
pub(crate) fn cli() -> Command {
    Command::new("gbiv")
        .about("A tool / framework for managing git worktrees")
        .subcommand_required(true)
        .arg_required_else_help(true)
        .subcommand(
            Command::new("init")
                .about("Initialize a git repository with ROYGBIV worktree structure")
                .arg(
                    Arg::new("folder")
                        .help("The folder name of the git repository to initialize")
                        .required(true)
                        .index(1),
                ),
        )
        .subcommand(
            Command::new("status")
                .about("Show status of all ROYGBIV worktrees"),
        )
        .subcommand(
            Command::new("start")
                .about("Run the fleet orchestration HTTP daemon in the foreground")
                .arg(
                    Arg::new("session-name")
                        .long("session-name")
                        .help("Override the inferred tmux session name"),
                )
                .arg(
                    Arg::new("bind")
                        .long("bind")
                        .help("Reserved; parsed but ignored in v1 (always binds 127.0.0.1)"),
                ),
        )
        .subcommand(fleet_command())
        .subcommand(tmux::tmux_command())
        .subcommand(
            Command::new("rebase-all")
                .about("Pull the remote main branch into the main worktree then rebase all color worktrees onto it"),
        )
        .subcommand(
            Command::new("repair")
                .about("Create any active-palette worktrees (base colors + configured extras) missing on disk"),
        )
        .subcommand(
            Command::new("reset")
                .about("Check out color branch and remove GBIV.md entry after feature branch is merged")
                .arg(
                    Arg::new("color")
                        .help("The color worktree to reset (omit to reset all)")
                        .required(false)
                        .index(1),
                )
                .arg(
                    Arg::new("hard")
                        .long("hard")
                        .visible_alias("force")
                        .action(clap::ArgAction::SetTrue)
                        .help("Force-reset, bypassing merge and status checks; stashes uncommitted changes"),
                )
                .arg(
                    Arg::new("yes")
                        .long("yes")
                        .short('y')
                        .action(clap::ArgAction::SetTrue)
                        .help("Skip confirmation prompt for all-color --hard reset"),
                ),
        )
        .subcommand(
            Command::new("exec")
                .about("Execute a command in a color worktree: gbiv exec [<color>|all] -- <command...>")
                .trailing_var_arg(true)
                .arg(
                    Arg::new("args")
                        .num_args(0..)
                        .allow_hyphen_values(true),
                ),
        )
        .subcommand(
            Command::new("tidy")
                .about("Rebase all worktrees, reset merged branches, and clean tmux windows"),
        )
        .subcommand(
            Command::new("mark")
                .about("Set lifecycle status on a GBIV.md feature entry")
                .arg(
                    Arg::new("done")
                        .long("done")
                        .action(clap::ArgAction::SetTrue)
                        .help("Mark the feature as done"),
                )
                .arg(
                    Arg::new("in-progress")
                        .long("in-progress")
                        .action(clap::ArgAction::SetTrue)
                        .help("Mark the feature as in-progress"),
                )
                .arg(
                    Arg::new("unset")
                        .long("unset")
                        .action(clap::ArgAction::SetTrue)
                        .help("Remove the status from the feature"),
                )
                .group(
                    ArgGroup::new("status")
                        .args(["done", "in-progress", "unset"])
                        .required(true),
                )
                .arg(
                    Arg::new("color")
                        .help("The color worktree to mark (inferred from CWD if omitted)")
                        .required(false)
                        .index(1)
                        .value_parser(clap::builder::ValueParser::new(|s: &str| -> Result<String, String> {
                            if s == "done" || s == "in-progress" || s == "unset" {
                                return Err(format!("'{}' is a status flag, not a color. Did you mean: gbiv mark --{}", s, s));
                            }
                            // Color validity is checked in the handler against the
                            // active palette (only known after root discovery).
                            Ok(s.to_string())
                        })),
                ),
        )
}

// @spec FLEET-CLI-001, FLEET-CLI-002, FLEET-CLI-003, FLEET-CLI-004,
// FLEET-CLI-005, FLEET-CLI-006, FLEET-CLI-007
fn fleet_command() -> Command {
    Command::new("fleet")
        .about("Fleet orchestration client commands (talk to a running `gbiv start` daemon)")
        .subcommand_required(true)
        .arg_required_else_help(true)
        .subcommand(
            Command::new("status")
                .about("Survey every color's Claude Code pane")
                .arg(
                    Arg::new("lines")
                        .long("lines")
                        .help("Lines of tail output per color (default 35)"),
                ),
        )
        .subcommand(
            Command::new("get")
                .about("Detail on one color's Claude Code pane")
                .arg(Arg::new("color").required(true).index(1))
                .arg(
                    Arg::new("lines")
                        .long("lines")
                        .conflicts_with_all(["start-line", "end-line"])
                        .help("Tail mode: lines of output (default: server default of 200)"),
                )
                .arg(
                    Arg::new("start-line")
                        .long("start-line")
                        .requires("end-line")
                        .help("Window mode: first line (or \"top\")"),
                )
                .arg(
                    Arg::new("end-line")
                        .long("end-line")
                        .requires("start-line")
                        .help("Window mode: last line"),
                ),
        )
        .subcommand(
            Command::new("send")
                .about("Send text + Enter to one color's Claude Code pane")
                .arg(Arg::new("color").required(true).index(1))
                .arg(Arg::new("text").required(true).index(2)),
        )
}

/// Dispatch a `gbiv fleet` subcommand and exit the process with its resolved
/// exit code (docs/llds/orchestrate-cli.md exit tables) — bypassing the
/// generic anyhow `Err -> 1` path, since fleet subcommands need distinct
/// exit codes 0 through 6, not just success/failure.
fn dispatch_fleet(sub_matches: &clap::ArgMatches) -> Result<()> {
    let cwd = std::env::current_dir()?;
    let outcome = match sub_matches.subcommand() {
        Some(("status", m)) => {
            let lines = m.get_one::<String>("lines").map(String::as_str);
            orchestration::fleet_cli::run_status(&cwd, lines)
        }
        Some(("get", m)) => {
            let color = m.get_one::<String>("color").unwrap();
            let lines = m.get_one::<String>("lines").map(String::as_str);
            let start_line = m.get_one::<String>("start-line").map(String::as_str);
            let end_line = m.get_one::<String>("end-line").map(String::as_str);
            orchestration::fleet_cli::run_get(&cwd, color, lines, start_line, end_line)
        }
        Some(("send", m)) => {
            let color = m.get_one::<String>("color").unwrap();
            let text = m.get_one::<String>("text").unwrap();
            orchestration::fleet_cli::run_send(&cwd, color, text)
        }
        _ => unreachable!(),
    };

    // @spec FLEET-CLI-050
    if let Some(stdout) = &outcome.stdout {
        println!("{stdout}");
    }
    if let Some(stderr) = &outcome.stderr {
        eprintln!("{stderr}");
    }
    std::process::exit(outcome.exit_code);
}

// @spec CLI-EXEC-PARSE-002, CLI-EXEC-PARSE-003, CLI-EXEC-PARSE-004, CLI-EXEC-PARSE-005
/// Split the raw exec argument list into an optional target and the command
/// tokens. The first token is treated as the target when it names an
/// active-palette worktree or "all"; otherwise the target is inferred from CWD.
pub(crate) fn split_exec_args(
    all_args: Vec<String>,
    palette: &Palette,
) -> (Option<String>, Vec<String>) {
    let (target, rest) = if all_args
        .first()
        .map(|s| palette.contains(s) || s == "all")
        .unwrap_or(false)
    {
        (Some(all_args[0].clone()), all_args[1..].to_vec())
    } else {
        (None, all_args)
    };
    let command: Vec<String> = rest.into_iter().filter(|a| a != "--").collect();
    (target, command)
}

/// Extract `gbiv start`'s flags into `StartOptions`. Factored out of the
/// dispatch arm so the extraction itself — which field name maps to which
/// clap arg — is unit-testable without invoking `orchestration::daemon::run`
/// (which binds a real port and blocks forever).
fn start_options_from_matches(
    sub_matches: &clap::ArgMatches,
) -> orchestration::daemon::StartOptions {
    orchestration::daemon::StartOptions {
        session_name: sub_matches.get_one::<String>("session-name").cloned(),
        bind: sub_matches.get_one::<String>("bind").cloned(),
    }
}

// @spec CLI-DISPATCH-003, CLI-DISPATCH-007 through CLI-DISPATCH-010
fn run() -> Result<()> {
    let matches = cli().get_matches();

    match matches.subcommand() {
        Some(("init", sub_matches)) => {
            let folder = sub_matches.get_one::<String>("folder").unwrap();
            init_command(folder)?;
        }
        Some(("status", _)) => {
            status_command()?;
        }
        Some(("start", sub_matches)) => {
            orchestration::daemon::run(start_options_from_matches(sub_matches))?;
        }
        Some(("fleet", sub_matches)) => {
            dispatch_fleet(sub_matches)?;
        }
        Some(("tmux", sub_matches)) => {
            tmux::dispatch(sub_matches)?;
        }
        Some(("rebase-all", _)) => {
            rebase_all_command()?;
        }
        Some(("repair", _)) => {
            repair_command()?;
        }
        Some(("reset", sub_matches)) => {
            let color = sub_matches.get_one::<String>("color").map(|s| s.as_str());
            let hard = sub_matches.get_flag("hard");
            let yes = sub_matches.get_flag("yes");
            reset_command(color, hard, yes)?;
        }
        Some(("exec", sub_matches)) => {
            use commands::exec::exec_command;
            let all_args: Vec<String> = sub_matches
                .get_many::<String>("args")
                .map(|vals| vals.cloned().collect())
                .unwrap_or_default();
            // The target/command split tests the first token against the active
            // palette, so the palette is loaded (via root discovery) first.
            let cwd = std::env::current_dir()?;
            let palette = match find_gbiv_root(&cwd) {
                Some(root) => Palette::load(&root.root)?,
                None => Palette::default(),
            };
            let (target, command) = split_exec_args(all_args, &palette);
            if command.is_empty() {
                return Err(anyhow::anyhow!(
                    "no command specified. Usage: gbiv exec [<color>|all] -- <command...>"
                ));
            }
            let target_ref = target.as_deref();
            // @spec CLI-DISPATCH-009: exec surfaces the command's own output, so its
            // errors print without the "Error: " prefix the generic handler adds.
            // The palette resolved above is threaded in so the config loads once.
            match exec_command(target_ref, &command, None, &palette) {
                Ok(output) => {
                    if !output.is_empty() {
                        print!("{}", output);
                    }
                }
                Err(e) => {
                    eprintln!(
                        "{}",
                        render_handler_error(&e, true, gbiv_core::observability::debug_enabled())
                    );
                    std::process::exit(1);
                }
            }
        }
        Some(("tidy", _)) => {
            tidy_command()?;
        }
        Some(("mark", sub_matches)) => {
            let status = if sub_matches.get_flag("done") {
                Some("done")
            } else if sub_matches.get_flag("in-progress") {
                Some("in-progress")
            } else if sub_matches.get_flag("unset") {
                Some("unset")
            } else {
                None
            };
            let color = sub_matches.get_one::<String>("color").map(|s| s.as_str());
            let msg = mark_command(status, color, None)?;
            println!("{}", msg);
        }
        _ => unreachable!(),
    }

    Ok(())
}

/// Render a handler error for stderr. `exec` errors carry the failed command's own
/// combined output, so they get no `"Error: "` prefix; every other handler error
/// is prefixed. The full `anyhow` cause chain is shown when debug logging is
/// enabled, otherwise only the top-level message.
// @spec CLI-DISPATCH-003, CLI-DISPATCH-009, CLI-DISPATCH-011
fn render_handler_error(err: &anyhow::Error, is_exec: bool, debug: bool) -> String {
    let body = if debug {
        format!("{:#}", err)
    } else {
        format!("{}", err)
    };
    if is_exec {
        body
    } else {
        format!("Error: {}", body)
    }
}

fn main() {
    // @spec LOG-001
    gbiv_core::observability::init(tracing::level_filters::LevelFilter::INFO);
    if let Err(e) = run() {
        eprintln!(
            "{}",
            render_handler_error(&e, false, gbiv_core::observability::debug_enabled())
        );
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- `gbiv fleet` argument parsing (FLEET-CLI-001 through -007) -------

    // @spec FLEET-CLI-001, FLEET-CLI-002
    #[test]
    fn fleet_status_parses_lines_flag() {
        let m = cli().get_matches_from(["gbiv", "fleet", "status", "--lines", "50"]);
        let sub = m.subcommand_matches("fleet").unwrap();
        let status = sub.subcommand_matches("status").unwrap();
        assert_eq!(
            status.get_one::<String>("lines").map(String::as_str),
            Some("50")
        );
    }

    // @spec FLEET-CLI-001, FLEET-CLI-003
    #[test]
    fn fleet_get_parses_color_and_lines() {
        let m = cli().get_matches_from(["gbiv", "fleet", "get", "red", "--lines", "100"]);
        let get = m
            .subcommand_matches("fleet")
            .unwrap()
            .subcommand_matches("get")
            .unwrap();
        assert_eq!(get.get_one::<String>("color").map(String::as_str), Some("red"));
        assert_eq!(get.get_one::<String>("lines").map(String::as_str), Some("100"));
    }

    // @spec FLEET-CLI-004
    #[test]
    fn fleet_get_parses_start_and_end_line() {
        let m = cli().get_matches_from([
            "gbiv", "fleet", "get", "red", "--start-line", "top", "--end-line", "20",
        ]);
        let get = m
            .subcommand_matches("fleet")
            .unwrap()
            .subcommand_matches("get")
            .unwrap();
        assert_eq!(
            get.get_one::<String>("start-line").map(String::as_str),
            Some("top")
        );
        assert_eq!(
            get.get_one::<String>("end-line").map(String::as_str),
            Some("20")
        );
    }

    // @spec FLEET-CLI-005
    #[test]
    fn fleet_get_rejects_lines_with_start_line() {
        let result = cli().try_get_matches_from([
            "gbiv",
            "fleet",
            "get",
            "red",
            "--lines",
            "50",
            "--start-line",
            "top",
            "--end-line",
            "20",
        ]);
        assert!(result.is_err(), "expected a clap usage error");
    }

    // @spec FLEET-CLI-006
    #[test]
    fn fleet_get_rejects_start_line_without_end_line() {
        let result =
            cli().try_get_matches_from(["gbiv", "fleet", "get", "red", "--start-line", "top"]);
        assert!(result.is_err(), "expected a clap usage error");
    }

    // @spec FLEET-CLI-007
    #[test]
    fn fleet_send_parses_color_and_text() {
        let m = cli().get_matches_from(["gbiv", "fleet", "send", "red", "please run the tests"]);
        let send = m
            .subcommand_matches("fleet")
            .unwrap()
            .subcommand_matches("send")
            .unwrap();
        assert_eq!(send.get_one::<String>("color").map(String::as_str), Some("red"));
        assert_eq!(
            send.get_one::<String>("text").map(String::as_str),
            Some("please run the tests")
        );
    }

    // ---- `gbiv start` flag parsing (HTTP-SRV-057, HTTP-SRV-058) -----------

    // @spec HTTP-SRV-057
    #[test]
    fn start_parses_session_name_flag() {
        let m = cli().get_matches_from(["gbiv", "start", "--session-name", "custom"]);
        let sub = m.subcommand_matches("start").unwrap();
        assert_eq!(
            sub.get_one::<String>("session-name").map(String::as_str),
            Some("custom")
        );
    }

    // @spec HTTP-SRV-057
    #[test]
    fn start_session_name_is_optional() {
        let m = cli().get_matches_from(["gbiv", "start"]);
        let sub = m.subcommand_matches("start").unwrap();
        assert_eq!(sub.get_one::<String>("session-name"), None);
    }

    // @spec HTTP-SRV-057, HTTP-SRV-058
    #[test]
    fn start_options_from_matches_extracts_both_flags() {
        let m = cli().get_matches_from([
            "gbiv",
            "start",
            "--session-name",
            "custom",
            "--bind",
            "0.0.0.0",
        ]);
        let sub = m.subcommand_matches("start").unwrap();
        let opts = start_options_from_matches(sub);
        assert_eq!(opts.session_name.as_deref(), Some("custom"));
        assert_eq!(opts.bind.as_deref(), Some("0.0.0.0"));
    }

    // @spec HTTP-SRV-057, HTTP-SRV-058
    #[test]
    fn start_options_from_matches_defaults_to_none() {
        let m = cli().get_matches_from(["gbiv", "start"]);
        let sub = m.subcommand_matches("start").unwrap();
        let opts = start_options_from_matches(sub);
        assert!(opts.session_name.is_none());
        assert!(opts.bind.is_none());
    }

    // @spec HTTP-SRV-058
    #[test]
    fn start_parses_bind_flag_but_it_is_only_stored_not_acted_on() {
        let m = cli().get_matches_from(["gbiv", "start", "--bind", "0.0.0.0"]);
        let sub = m.subcommand_matches("start").unwrap();
        assert_eq!(
            sub.get_one::<String>("bind").map(String::as_str),
            Some("0.0.0.0")
        );
    }

    // @spec CLI-DISPATCH-003
    #[test]
    fn render_error_prefixes_non_exec() {
        let err = anyhow::anyhow!("root cause").context("outer");
        let out = render_handler_error(&err, false, false);
        assert_eq!(out, "Error: outer");
    }

    // @spec CLI-DISPATCH-009
    #[test]
    fn render_error_omits_prefix_for_exec() {
        let err = anyhow::anyhow!("command output").context("outer");
        let out = render_handler_error(&err, true, false);
        assert!(
            !out.starts_with("Error: "),
            "exec error must not be prefixed: {out:?}"
        );
        assert_eq!(out, "outer");
    }

    // @spec CLI-DISPATCH-011
    #[test]
    fn render_error_shows_cause_chain_only_when_debug() {
        let err = anyhow::anyhow!("root cause").context("outer");
        let terse = render_handler_error(&err, false, false);
        let verbose = render_handler_error(&err, false, true);
        assert!(
            !terse.contains("root cause"),
            "terse must hide the chain: {terse:?}"
        );
        assert!(
            verbose.contains("root cause"),
            "verbose must show the chain: {verbose:?}"
        );
    }

    // @spec CLI-EXEC-PARSE-001 through CLI-EXEC-PARSE-007
    /// Helper to parse exec args the same way main() does.
    fn parse_exec(argv: &[&str]) -> (Option<String>, Vec<String>) {
        let m = cli().get_matches_from(argv);
        let sub = m.subcommand_matches("exec").unwrap();
        let all_args: Vec<String> = sub
            .get_many::<String>("args")
            .map(|vals| vals.cloned().collect())
            .unwrap_or_default();
        split_exec_args(all_args, &Palette::default())
    }

    // @spec CLI-EXEC-PARSE-002
    #[test]
    fn exec_parses_extra_palette_target() {
        let palette = Palette::from_extras(vec!["my-extra".to_string()]);
        let (target, cmd) = split_exec_args(
            vec!["my-extra".to_string(), "--".to_string(), "ls".to_string()],
            &palette,
        );
        assert_eq!(target.as_deref(), Some("my-extra"));
        assert_eq!(cmd, vec!["ls"]);
    }

    // @spec CLI-EXEC-PARSE-002, CLI-EXEC-PARSE-005
    #[test]
    fn exec_parses_color_target_and_command() {
        let (target, cmd) = parse_exec(&["gbiv", "exec", "green", "--", "echo", "hello"]);
        assert_eq!(target.as_deref(), Some("green"));
        assert_eq!(cmd, vec!["echo", "hello"]);
    }

    // @spec CLI-EXEC-PARSE-003, CLI-EXEC-PARSE-005
    #[test]
    fn exec_parses_all_target_and_command() {
        let (target, cmd) = parse_exec(&["gbiv", "exec", "all", "--", "git", "status"]);
        assert_eq!(target.as_deref(), Some("all"));
        assert_eq!(cmd, vec!["git", "status"]);
    }

    // @spec CLI-EXEC-PARSE-004, CLI-EXEC-PARSE-005
    #[test]
    fn exec_parses_no_target_with_command() {
        let (target, cmd) = parse_exec(&["gbiv", "exec", "--", "cargo", "build"]);
        assert!(target.is_none(), "target should be None when omitted");
        assert_eq!(cmd, vec!["cargo", "build"]);
    }

    // @spec CLI-EXEC-PARSE-007
    #[test]
    fn exec_parses_command_with_flags_after_separator() {
        let (target, cmd) = parse_exec(&["gbiv", "exec", "red", "--", "ls", "-la"]);
        assert_eq!(target.as_deref(), Some("red"));
        assert_eq!(cmd, vec!["ls", "-la"]);
    }
}

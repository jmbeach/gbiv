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
mod extensions;
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
        .subcommand(install_skill_command())
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

// @spec INSTALL-CLI-001, INSTALL-CLI-002
fn install_skill_command() -> Command {
    Command::new("install-skill")
        .about("Write the bundled gbiv-orchestrate Claude Code skill to disk")
        .arg(
            Arg::new("scope")
                .long("scope")
                .value_parser(["user", "project"])
                .default_value("user")
                .help("Install for the current user (default) or the current gbiv project"),
        )
        .arg(
            Arg::new("force")
                .long("force")
                .action(clap::ArgAction::SetTrue)
                .help("Overwrite an on-disk copy even if it looks hand-edited"),
        )
}

/// Dispatch `gbiv install-skill` and exit the process with its resolved exit
/// code (docs/llds/orchestrate-cli.md exit table for INSTALL-CLI), bypassing
/// the generic anyhow `Err -> 1` path so refusal (exit 7) is distinguishable
/// from a hard failure.
fn dispatch_install_skill(sub_matches: &clap::ArgMatches) -> Result<()> {
    let cwd = std::env::current_dir()?;
    let scope = match sub_matches.get_one::<String>("scope").map(String::as_str) {
        Some("project") => orchestration::install_skill_client::Scope::Project,
        _ => orchestration::install_skill_client::Scope::User,
    };
    let force = sub_matches.get_flag("force");
    let outcome = orchestration::install_skill_cli::run_install_skill(&cwd, scope, force);

    if let Some(stdout) = &outcome.stdout {
        println!("{stdout}");
    }
    if let Some(stderr) = &outcome.stderr {
        eprintln!("{stderr}");
    }
    std::process::exit(outcome.exit_code);
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
        Some(("install-skill", sub_matches)) => {
            dispatch_install_skill(sub_matches)?;
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
        assert_eq!(
            get.get_one::<String>("color").map(String::as_str),
            Some("red")
        );
        assert_eq!(
            get.get_one::<String>("lines").map(String::as_str),
            Some("100")
        );
    }

    // @spec FLEET-CLI-004
    #[test]
    fn fleet_get_parses_start_and_end_line() {
        let m = cli().get_matches_from([
            "gbiv",
            "fleet",
            "get",
            "red",
            "--start-line",
            "top",
            "--end-line",
            "20",
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
        assert_eq!(
            send.get_one::<String>("color").map(String::as_str),
            Some("red")
        );
        assert_eq!(
            send.get_one::<String>("text").map(String::as_str),
            Some("please run the tests")
        );
    }

    // ---- helpers for clap-surface tests (gbv-1ht) ---------------------------

    /// Parse `argv`, panicking (not exiting the process) on a usage error.
    fn parse(argv: &[&str]) -> clap::ArgMatches {
        cli()
            .try_get_matches_from(argv)
            .unwrap_or_else(|e| panic!("{argv:?} should parse, got: {e}"))
    }

    /// The registered definition of the subcommand at `path` (e.g. `["fleet", "start"]`).
    fn subcommand_def(path: &[&str]) -> Command {
        let mut cmd = cli();
        for name in path {
            cmd = cmd
                .find_subcommand(name)
                .unwrap_or_else(|| panic!("subcommand {name:?} (in {path:?}) is not registered"))
                .clone();
        }
        cmd
    }

    /// The sub-matches at `path`.
    fn sub_matches<'a>(m: &'a clap::ArgMatches, path: &[&str]) -> &'a clap::ArgMatches {
        let mut cur = m;
        for name in path {
            cur = cur
                .subcommand_matches(name)
                .unwrap_or_else(|| panic!("expected subcommand {name:?} in {path:?}"));
        }
        cur
    }

    /// The arg id behind `--<long>` on the subcommand at `path`, so tests don't
    /// depend on internal arg ids.
    fn long_arg_id(path: &[&str], long: &str) -> String {
        subcommand_def(path)
            .get_arguments()
            .find(|a| a.get_long() == Some(long))
            .unwrap_or_else(|| panic!("{path:?} has no --{long} flag"))
            .get_id()
            .as_str()
            .to_string()
    }

    /// The ids of the positional args of the subcommand at `path`, in order.
    fn positional_ids(path: &[&str]) -> Vec<String> {
        subcommand_def(path)
            .get_positionals()
            .map(|a| a.get_id().as_str().to_string())
            .collect()
    }

    fn top_level_names() -> Vec<String> {
        let mut names: Vec<String> = cli()
            .get_subcommands()
            .map(|c| c.get_name().to_string())
            .collect();
        names.sort();
        names
    }

    // ---- top-level command surface (CLI-DISPATCH-012) ------------------------

    // @spec CLI-DISPATCH-012
    #[test]
    fn top_level_subcommands_are_exactly_the_specified_set() {
        let mut expected: Vec<String> = [
            "init",
            "status",
            "start",
            "done",
            "reset",
            "rebase-all",
            "repair",
            "tidy",
            "exec",
            "install-skill",
            "install-extension",
            "fleet",
            "tmux",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        expected.sort();
        assert_eq!(top_level_names(), expected);
    }

    // @spec CLI-DISPATCH-012
    #[test]
    fn mark_subcommand_is_gone() {
        assert!(
            !top_level_names().iter().any(|n| n == "mark"),
            "`gbiv mark` must not be registered"
        );
        let result = cli().try_get_matches_from(["gbiv", "mark", "--done", "red"]);
        assert!(result.is_err(), "`gbiv mark` must be a usage error");
    }

    // ---- `gbiv start <task> [--color <color>]` (FL-START-001) ---------------

    // @spec FL-START-001
    #[test]
    fn start_takes_one_required_task_positional() {
        let positionals = positional_ids(&["start"]);
        assert_eq!(positionals.len(), 1, "start takes exactly one positional");
        let def = subcommand_def(&["start"]);
        let task = def.get_positionals().next().unwrap();
        assert!(task.is_required_set(), "<task> is required");
        assert!(
            cli().try_get_matches_from(["gbiv", "start"]).is_err(),
            "`gbiv start` without a task must be a usage error"
        );
    }

    // @spec FL-START-001
    #[test]
    fn start_parses_task_and_color() {
        let m = parse(&["gbiv", "start", "fix the login bug", "--color", "red"]);
        let sub = sub_matches(&m, &["start"]);
        let task_id = positional_ids(&["start"]).remove(0);
        assert_eq!(
            sub.get_one::<String>(&task_id).map(String::as_str),
            Some("fix the login bug")
        );
        let color_id = long_arg_id(&["start"], "color");
        assert_eq!(
            sub.get_one::<String>(&color_id).map(String::as_str),
            Some("red")
        );
    }

    // @spec FL-START-001
    #[test]
    fn start_color_is_optional() {
        let m = parse(&["gbiv", "start", "gbv-1ht"]);
        let sub = sub_matches(&m, &["start"]);
        let color_id = long_arg_id(&["start"], "color");
        assert_eq!(sub.get_one::<String>(&color_id), None);
    }

    // @spec FL-START-001, FLEET-CLI-001
    #[test]
    fn top_level_start_is_not_the_daemon() {
        let def = subcommand_def(&["start"]);
        for daemon_flag in ["session-name", "bind"] {
            assert!(
                def.get_arguments()
                    .all(|a| a.get_long() != Some(daemon_flag)),
                "top-level `start` must not carry the daemon's --{daemon_flag}"
            );
        }
        assert!(cli()
            .try_get_matches_from(["gbiv", "start", "--session-name", "custom"])
            .is_err());
    }

    // ---- `gbiv done [<color|tag>]` (FL-DONE-001) -----------------------------

    // @spec FL-DONE-001
    #[test]
    fn done_argument_is_optional() {
        let m = parse(&["gbiv", "done"]);
        let sub = sub_matches(&m, &["done"]);
        let positionals = positional_ids(&["done"]);
        assert_eq!(positionals.len(), 1, "done takes one optional positional");
        assert_eq!(sub.get_one::<String>(&positionals[0]), None);
    }

    // @spec FL-DONE-001
    #[test]
    fn done_parses_a_color_or_tag() {
        for arg in ["red", "gbv-1ht", "[gbv-1ht]"] {
            let m = parse(&["gbiv", "done", arg]);
            let sub = sub_matches(&m, &["done"]);
            let id = positional_ids(&["done"]).remove(0);
            assert_eq!(sub.get_one::<String>(&id).map(String::as_str), Some(arg));
        }
    }

    // ---- `gbiv install-extension` (EXT-INST-001) ------------------------------

    // @spec EXT-INST-001
    #[test]
    fn install_extension_parses_name_with_defaults() {
        let m = parse(&["gbiv", "install-extension", "vat"]);
        let sub = sub_matches(&m, &["install-extension"]);
        let name_id = positional_ids(&["install-extension"]).remove(0);
        assert_eq!(
            sub.get_one::<String>(&name_id).map(String::as_str),
            Some("vat")
        );
        let scope_id = long_arg_id(&["install-extension"], "scope");
        assert_eq!(
            sub.get_one::<String>(&scope_id).map(String::as_str),
            Some("user"),
            "--scope defaults to user"
        );
        let force_id = long_arg_id(&["install-extension"], "force");
        assert!(!sub.get_flag(&force_id));
    }

    // @spec EXT-INST-001
    #[test]
    fn install_extension_parses_scope_project_and_force() {
        let m = parse(&[
            "gbiv",
            "install-extension",
            "vat",
            "--scope",
            "project",
            "--force",
        ]);
        let sub = sub_matches(&m, &["install-extension"]);
        let scope_id = long_arg_id(&["install-extension"], "scope");
        assert_eq!(
            sub.get_one::<String>(&scope_id).map(String::as_str),
            Some("project")
        );
        let force_id = long_arg_id(&["install-extension"], "force");
        assert!(sub.get_flag(&force_id));
    }

    // @spec EXT-INST-001
    #[test]
    fn install_extension_requires_a_name_and_a_known_scope() {
        assert!(cli()
            .try_get_matches_from(["gbiv", "install-extension"])
            .is_err());
        // Make sure the subcommand exists, so the error above isn't just
        // "unrecognized subcommand".
        let _ = subcommand_def(&["install-extension"]);
        assert!(cli()
            .try_get_matches_from(["gbiv", "install-extension", "vat", "--scope", "global"])
            .is_err());
    }

    // ---- `gbiv fleet` group (FLEET-CLI-001) -----------------------------------

    // @spec FLEET-CLI-001
    #[test]
    fn fleet_group_has_start_status_get_send() {
        let mut names: Vec<String> = subcommand_def(&["fleet"])
            .get_subcommands()
            .map(|c| c.get_name().to_string())
            .collect();
        names.sort();
        assert_eq!(names, vec!["get", "send", "start", "status"]);
    }

    // ---- `gbiv fleet start` flag parsing (HTTP-SRV-057, HTTP-SRV-058) -------

    // @spec HTTP-SRV-057, FLEET-CLI-001
    #[test]
    fn fleet_start_parses_session_name_flag() {
        let m = parse(&["gbiv", "fleet", "start", "--session-name", "custom"]);
        let sub = sub_matches(&m, &["fleet", "start"]);
        assert_eq!(
            sub.get_one::<String>("session-name").map(String::as_str),
            Some("custom")
        );
    }

    // @spec HTTP-SRV-057
    #[test]
    fn fleet_start_session_name_is_optional() {
        let m = parse(&["gbiv", "fleet", "start"]);
        let sub = sub_matches(&m, &["fleet", "start"]);
        assert_eq!(sub.get_one::<String>("session-name"), None);
    }

    // @spec HTTP-SRV-057, HTTP-SRV-058
    #[test]
    fn start_options_from_matches_extracts_both_flags() {
        let m = parse(&[
            "gbiv",
            "fleet",
            "start",
            "--session-name",
            "custom",
            "--bind",
            "0.0.0.0",
        ]);
        let sub = sub_matches(&m, &["fleet", "start"]);
        let opts = start_options_from_matches(sub);
        assert_eq!(opts.session_name.as_deref(), Some("custom"));
        assert_eq!(opts.bind.as_deref(), Some("0.0.0.0"));
    }

    // @spec HTTP-SRV-057, HTTP-SRV-058
    #[test]
    fn start_options_from_matches_defaults_to_none() {
        let m = parse(&["gbiv", "fleet", "start"]);
        let sub = sub_matches(&m, &["fleet", "start"]);
        let opts = start_options_from_matches(sub);
        assert!(opts.session_name.is_none());
        assert!(opts.bind.is_none());
    }

    // @spec HTTP-SRV-058
    #[test]
    fn fleet_start_parses_bind_flag_but_it_is_only_stored_not_acted_on() {
        let m = parse(&["gbiv", "fleet", "start", "--bind", "0.0.0.0"]);
        let sub = sub_matches(&m, &["fleet", "start"]);
        assert_eq!(
            sub.get_one::<String>("bind").map(String::as_str),
            Some("0.0.0.0")
        );
    }

    // ---- palette-dependent color args validate in handlers (CLI-COLOR-027) ---

    // @spec CLI-COLOR-027
    #[test]
    fn color_args_accept_non_base_names_at_parse_time() {
        // `amber` is not a base color; only the handler (after loading the
        // active palette) may decide whether it's valid.
        for argv in [
            &["gbiv", "start", "a task", "--color", "amber"][..],
            &["gbiv", "done", "amber"][..],
            &["gbiv", "reset", "amber"][..],
            &["gbiv", "exec", "amber", "--", "ls"][..],
        ] {
            assert!(
                cli().try_get_matches_from(argv).is_ok(),
                "{argv:?} must parse; color validation belongs to the handler"
            );
        }
    }

    // @spec CLI-COLOR-027
    #[test]
    fn color_args_declare_no_clap_possible_values() {
        let start_color = long_arg_id(&["start"], "color");
        let start = subcommand_def(&["start"]);
        let arg = start
            .get_arguments()
            .find(|a| a.get_id().as_str() == start_color)
            .unwrap();
        assert!(arg.get_possible_values().is_empty());

        for path in [&["done"][..], &["reset"][..]] {
            let def = subcommand_def(path);
            for arg in def.get_positionals() {
                assert!(
                    arg.get_possible_values().is_empty(),
                    "{path:?} positional {:?} must not use a PossibleValuesParser",
                    arg.get_id()
                );
            }
        }
    }

    // ---- layered config wiring (CLI-CFG-004, CLI-CFG-006) ---------------------

    // @spec CLI-CFG-004
    #[test]
    fn layered_extensions_tables_merge_by_extension_runtime_rules() {
        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path().join("proj");
        let main_repo = root.join("main").join("proj");
        std::fs::create_dir_all(&main_repo).unwrap();
        std::fs::create_dir_all(root.join(".gbiv")).unwrap();
        std::fs::write(
            gbiv_core::config::config_path(&root),
            "[extensions]\nenabled = [\"b\", \"c\"]\ndisabled = [\"b\"]\n\n[extensions.a]\ny = [3]\n",
        )
        .unwrap();
        let global = tmp.path().join("xdg").join("gbiv").join("config.toml");
        std::fs::create_dir_all(global.parent().unwrap()).unwrap();
        std::fs::write(
            &global,
            "[extensions]\nenabled = [\"a\", \"b\"]\n\n[extensions.a]\nx = 1\ny = [1, 2]\n",
        )
        .unwrap();

        let layers = gbiv_core::config::read_layers(&root, Some(&global)).unwrap();
        let cfg = extensions::ExtensionsConfig::from_layers(
            layers.global.as_ref(),
            layers.project.as_ref(),
            &main_repo,
            tmp.path(),
        )
        .unwrap();
        assert_eq!(cfg.enabled, vec!["a".to_string(), "c".to_string()]);
        let a = &cfg.settings["a"].options;
        assert_eq!(a["x"].as_integer(), Some(1));
        assert_eq!(
            a["y"].as_array().unwrap(),
            &vec![toml::Value::Integer(3)],
            "project wins per key and lists replace whole"
        );
    }

    /// A gbiv workspace whose global config (via `XDG_CONFIG_HOME`/`HOME`, both
    /// pointed at a temp dir) is not TOML and whose project `[extensions]`
    /// tables are invalid for the extension runtime. Returns (tmp, root, main_repo).
    fn workspace_with_broken_extension_config(
    ) -> (tempfile::TempDir, std::path::PathBuf, std::path::PathBuf) {
        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path().join("proj");
        let main_repo = root.join("main").join("proj");
        let red_repo = root.join("red").join("proj");
        for repo in [&main_repo, &red_repo] {
            std::fs::create_dir_all(repo).unwrap();
            std::process::Command::new("git")
                .args(["init", "-q"])
                .current_dir(repo)
                .output()
                .unwrap();
        }
        std::fs::create_dir_all(root.join(".gbiv")).unwrap();
        std::fs::write(
            gbiv_core::config::config_path(&root),
            "[extensions]\nenabled = 5\n\n[extensions.vat]\npaths = \"not-a-list\"\n",
        )
        .unwrap();
        let cfg_home = tmp.path().join("cfg");
        for global in [
            cfg_home.join("gbiv").join("config.toml"),
            cfg_home.join(".config").join("gbiv").join("config.toml"),
        ] {
            std::fs::create_dir_all(global.parent().unwrap()).unwrap();
            std::fs::write(&global, "this is [[[ not toml").unwrap();
        }
        (tmp, root, main_repo)
    }

    /// Run `f` with `XDG_CONFIG_HOME` and `HOME` pointed at `dir`; restores both.
    fn with_config_home<T>(dir: &std::path::Path, f: impl FnOnce() -> T) -> T {
        let old_xdg = std::env::var_os("XDG_CONFIG_HOME");
        let old_home = std::env::var_os("HOME");
        std::env::set_var("XDG_CONFIG_HOME", dir);
        std::env::set_var("HOME", dir);
        let out = f();
        match old_xdg {
            Some(v) => std::env::set_var("XDG_CONFIG_HOME", v),
            None => std::env::remove_var("XDG_CONFIG_HOME"),
        }
        match old_home {
            Some(v) => std::env::set_var("HOME", v),
            None => std::env::remove_var("HOME"),
        }
        out
    }

    // @spec CLI-CFG-006
    #[test]
    #[serial_test::serial]
    fn exec_ignores_broken_global_config_and_extensions_tables() {
        let (tmp, root, main_repo) = workspace_with_broken_extension_config();
        let out = with_config_home(&tmp.path().join("cfg"), || {
            let palette = Palette::load(&root)?;
            commands::exec::exec_command(
                Some("red"),
                &["echo".to_string(), "hi".to_string()],
                Some(&main_repo),
                &palette,
            )
        });
        assert_eq!(
            out.expect("exec must not read [extensions] or the global file"),
            "hi\n"
        );
    }

    // @spec CLI-CFG-006
    #[test]
    #[serial_test::serial]
    fn fleet_client_ignores_broken_global_config_and_extensions_tables() {
        let (tmp, _root, main_repo) = workspace_with_broken_extension_config();
        let outcome = with_config_home(&tmp.path().join("cfg"), || {
            orchestration::fleet_cli::run_status(&main_repo, None)
        });
        // No daemon is running: the only acceptable failure is "no port file".
        assert_eq!(outcome.exit_code, 2, "{outcome:?}");
        let stderr = outcome.stderr.unwrap_or_default();
        assert!(stderr.contains("port file"), "got: {stderr}");
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

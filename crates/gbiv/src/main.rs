use anyhow::Result;
use clap::{Arg, ArgGroup, Command};
use commands::init::init_command;
use commands::mark::mark_command;
use commands::rebase_all::rebase_all_command;
use commands::reset::reset_command;
use commands::status::status_command;
use commands::tidy::tidy_command;
use commands::tmux;
use gbiv_core::colors::{is_valid_color, COLORS};

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
        .subcommand(tmux::tmux_command())
        .subcommand(
            Command::new("rebase-all")
                .about("Pull the remote main branch into the main worktree then rebase all color worktrees onto it"),
        )
        .subcommand(
            Command::new("reset")
                .about("Check out color branch and remove GBIV.md entry after feature branch is merged")
                .arg(
                    Arg::new("color")
                        .help("The color worktree to reset (omit to reset all)")
                        .required(false)
                        .index(1)
                        .value_parser(clap::builder::PossibleValuesParser::new(COLORS)),
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
                            if is_valid_color(s) {
                                Ok(s.to_string())
                            } else {
                                Err(format!("invalid color '{}'. Possible values: {}", s, COLORS.join(", ")))
                            }
                        })),
                ),
        )
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
        Some(("tmux", sub_matches)) => {
            tmux::dispatch(sub_matches)?;
        }
        Some(("rebase-all", _)) => {
            rebase_all_command()?;
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
            let (target, rest) = if all_args
                .first()
                .map(|s| is_valid_color(s) || s == "all")
                .unwrap_or(false)
            {
                (Some(all_args[0].clone()), all_args[1..].to_vec())
            } else {
                (None, all_args)
            };
            let command: Vec<String> = rest.into_iter().filter(|a| a != "--").collect();
            if command.is_empty() {
                return Err(anyhow::anyhow!(
                    "no command specified. Usage: gbiv exec [<color>|all] -- <command...>"
                ));
            }
            let target_ref = target.as_deref();
            // @spec CLI-DISPATCH-009: exec surfaces the command's own output, so its
            // errors print without the "Error: " prefix the generic handler adds.
            match exec_command(target_ref, &command, None) {
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
        assert!(!out.starts_with("Error: "), "exec error must not be prefixed: {out:?}");
        assert_eq!(out, "outer");
    }

    // @spec CLI-DISPATCH-011
    #[test]
    fn render_error_shows_cause_chain_only_when_debug() {
        let err = anyhow::anyhow!("root cause").context("outer");
        let terse = render_handler_error(&err, false, false);
        let verbose = render_handler_error(&err, false, true);
        assert!(!terse.contains("root cause"), "terse must hide the chain: {terse:?}");
        assert!(verbose.contains("root cause"), "verbose must show the chain: {verbose:?}");
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
        let (target, rest) = if all_args
            .first()
            .map(|s| is_valid_color(s) || s == "all")
            .unwrap_or(false)
        {
            (Some(all_args[0].clone()), all_args[1..].to_vec())
        } else {
            (None, all_args)
        };
        let command: Vec<String> = rest.into_iter().filter(|a| a != "--").collect();
        (target, command)
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

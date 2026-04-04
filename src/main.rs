use clap::{Arg, ArgGroup, Command};
use colors::COLORS;
use commands::init::init_command;
use commands::reset::reset_command;
use commands::mark::mark_command;
use commands::rebase_all::rebase_all_command;
use commands::status::status_command;
use commands::tidy::tidy_command;
use commands::tmux;

mod colors;
mod commands;
mod gbiv_md;
mod git_utils;

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
                            if COLORS.contains(&s) {
                                Ok(s.to_string())
                            } else {
                                Err(format!("invalid color '{}'. Possible values: {}", s, COLORS.join(", ")))
                            }
                        })),
                ),
        )
}

fn main() {
    let matches = cli().get_matches();

    match matches.subcommand() {
        Some(("init", sub_matches)) => {
            let folder = sub_matches.get_one::<String>("folder").unwrap();
            if let Err(e) = init_command(folder) {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        }
        Some(("status", _)) => {
            if let Err(e) = status_command() {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        }
        Some(("tmux", sub_matches)) => {
            if let Err(e) = tmux::dispatch(sub_matches) {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        }
        Some(("rebase-all", _)) => {
            if let Err(e) = rebase_all_command() {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        }
        Some(("reset", sub_matches)) => {
            let color = sub_matches.get_one::<String>("color").map(|s| s.as_str());
            let hard = sub_matches.get_flag("hard");
            let yes = sub_matches.get_flag("yes");
            if let Err(e) = reset_command(color, hard, yes) {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        }
        Some(("exec", sub_matches)) => {
            use commands::exec::exec_command;
            let all_args: Vec<String> = sub_matches
                .get_many::<String>("args")
                .map(|vals| vals.cloned().collect())
                .unwrap_or_default();
            let valid_targets: Vec<&str> = COLORS.iter().copied().chain(std::iter::once("all")).collect();
            let (target, rest) = if all_args.first().map(|s| valid_targets.contains(&s.as_str())).unwrap_or(false) {
                (Some(all_args[0].clone()), all_args[1..].to_vec())
            } else {
                (None, all_args)
            };
            let command: Vec<String> = rest.into_iter().filter(|a| a != "--").collect();
            if command.is_empty() {
                eprintln!("Error: no command specified. Usage: gbiv exec [<color>|all] -- <command...>");
                std::process::exit(1);
            }
            let target_ref = target.as_deref();
            match exec_command(target_ref, &command, None) {
                Ok(output) => {
                    if !output.is_empty() {
                        print!("{}", output);
                    }
                }
                Err(e) => {
                    eprintln!("{}", e);
                    std::process::exit(1);
                }
            }
        }
        Some(("tidy", _)) => {
            if let Err(e) = tidy_command() {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
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
            match mark_command(status, color, None) {
                Ok(msg) => println!("{}", msg),
                Err(e) => {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                }
            }
        }
        _ => unreachable!(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper to parse exec args the same way main() does.
    fn parse_exec(argv: &[&str]) -> (Option<String>, Vec<String>) {
        let m = cli().get_matches_from(argv);
        let sub = m.subcommand_matches("exec").unwrap();
        let all_args: Vec<String> = sub
            .get_many::<String>("args")
            .map(|vals| vals.cloned().collect())
            .unwrap_or_default();
        let valid_targets: Vec<&str> = COLORS.iter().copied().chain(std::iter::once("all")).collect();
        let (target, rest) = if all_args.first().map(|s| valid_targets.contains(&s.as_str())).unwrap_or(false) {
            (Some(all_args[0].clone()), all_args[1..].to_vec())
        } else {
            (None, all_args)
        };
        let command: Vec<String> = rest.into_iter().filter(|a| a != "--").collect();
        (target, command)
    }

    #[test]
    fn exec_parses_color_target_and_command() {
        let (target, cmd) = parse_exec(&["gbiv", "exec", "green", "--", "echo", "hello"]);
        assert_eq!(target.as_deref(), Some("green"));
        assert_eq!(cmd, vec!["echo", "hello"]);
    }

    #[test]
    fn exec_parses_all_target_and_command() {
        let (target, cmd) = parse_exec(&["gbiv", "exec", "all", "--", "git", "status"]);
        assert_eq!(target.as_deref(), Some("all"));
        assert_eq!(cmd, vec!["git", "status"]);
    }

    #[test]
    fn exec_parses_no_target_with_command() {
        let (target, cmd) = parse_exec(&["gbiv", "exec", "--", "cargo", "build"]);
        assert!(target.is_none(), "target should be None when omitted");
        assert_eq!(cmd, vec!["cargo", "build"]);
    }

    #[test]
    fn exec_parses_command_with_flags_after_separator() {
        let (target, cmd) = parse_exec(&["gbiv", "exec", "red", "--", "ls", "-la"]);
        assert_eq!(target.as_deref(), Some("red"));
        assert_eq!(cmd, vec!["ls", "-la"]);
    }
}

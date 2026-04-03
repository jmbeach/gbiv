use clap::{Arg, ArgGroup, Command};
use colors::COLORS;
use commands::cleanup::cleanup_command;
use commands::reset::reset_command;
use commands::init::init_command;
use commands::mark::mark_command;
use commands::rebase_all::rebase_all_command;
use commands::status::status_command;
use commands::tmux;

mod colors;
mod commands;
mod gbiv_md;
mod git_utils;

fn cli() -> Command {
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
            Command::new("cleanup")
                .about("Check out color branch and remove GBIV.md entry after feature branch is merged")
                .arg(
                    Arg::new("color")
                        .help("The color worktree to clean up (omit to clean up all)")
                        .required(false)
                        .index(1)
                        .value_parser(clap::builder::PossibleValuesParser::new(COLORS)),
                ),
        )
        .subcommand(
            Command::new("reset")
                .about("Reset a color worktree to the remote main branch")
                .arg(
                    Arg::new("color")
                        .help("The color worktree to reset (inferred from CWD if omitted)")
                        .required(false)
                        .index(1)
                        .value_parser(clap::builder::PossibleValuesParser::new(COLORS)),
                ),
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
        Some(("cleanup", sub_matches)) => {
            let color = sub_matches.get_one::<String>("color").map(|s| s.as_str());
            if let Err(e) = cleanup_command(color) {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        }
        Some(("reset", sub_matches)) => {
            let color = sub_matches.get_one::<String>("color").map(|s| s.as_str());
            if let Err(e) = reset_command(color) {
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

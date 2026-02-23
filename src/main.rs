use clap::{Arg, Command};
use commands::init::init_command;
use commands::prd::{lock_command, unlock_command};
use commands::status::status_command;

mod colors;
mod commands;
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
        .subcommand(
            Command::new("prd")
                .about("Manage prd.json coordination across worktrees")
                .subcommand_required(true)
                .arg_required_else_help(true)
                .subcommand(
                    Command::new("lock")
                        .about("Acquire exclusive lock on prd.json in the main worktree")
                        .arg(
                            Arg::new("timeout")
                                .long("timeout")
                                .help("Seconds to wait before giving up (default: 30)")
                                .value_parser(clap::value_parser!(u64))
                                .default_value("30"),
                        ),
                )
                .subcommand(
                    Command::new("unlock")
                        .about("Release the prd.json lock held by this worktree")
                        .arg(
                            Arg::new("force")
                                .long("force")
                                .help("Remove lock even if this worktree is not the owner")
                                .action(clap::ArgAction::SetTrue),
                        ),
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
        Some(("prd", prd_matches)) => match prd_matches.subcommand() {
            Some(("lock", lock_matches)) => {
                let timeout = *lock_matches.get_one::<u64>("timeout").unwrap();
                if let Err(e) = lock_command(timeout) {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                }
            }
            Some(("unlock", unlock_matches)) => {
                let force = unlock_matches.get_flag("force");
                if let Err(e) = unlock_command(force) {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                }
            }
            _ => unreachable!(),
        },
        _ => unreachable!(),
    }
}

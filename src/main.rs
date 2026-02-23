use clap::{Arg, Command};
use commands::init::init_command;
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
        _ => unreachable!(),
    }
}

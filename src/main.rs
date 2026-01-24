use clap::Command;

fn cli() -> Command {
    Command::new("gbiv")
        .about("A tool / framework for managing git worktrees")
        .subcommand_required(true)
        .arg_required_else_help(true)
        .subcommand(Command::new("init"))
}
fn main() {
    let matches = cli().get_matches();

    match matches.subcommand() {
        Some(("init", _)) => {
            println!("Initializing...")
        }
        _ => unreachable!(),
    }
}

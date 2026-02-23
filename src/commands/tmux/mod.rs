pub mod new_session;

use clap::Command;

pub fn tmux_command() -> Command {
    Command::new("tmux")
        .about("Manage tmux sessions for gbiv worktrees")
        .subcommand_required(true)
        .arg_required_else_help(true)
        .subcommand(new_session::new_session_subcommand())
}

pub fn dispatch(sub_matches: &clap::ArgMatches) -> Result<(), String> {
    match sub_matches.subcommand() {
        Some(("new-session", args)) => {
            let session_name = args.get_one::<String>("session-name").map(|s| s.as_str());
            new_session::new_session_command(session_name)
        }
        _ => unreachable!(),
    }
}

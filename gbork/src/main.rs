use clap::{Parser, Subcommand};
use std::process::ExitCode;

mod api;
mod cli;
mod gbiv_root;
mod locator;
mod port_file;
mod proc_walk;
mod server;
mod tmux;

pub const COLORS: [&str; 7] = [
    "red", "orange", "yellow", "green", "blue", "indigo", "violet",
];

#[derive(Parser)]
#[command(
    name = "gbork",
    version,
    about = "Orchestrate Claude Code sessions running in gbiv color worktrees"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Run the gbork daemon in the foreground (Ctrl+C to stop)
    Start {
        /// Override the inferred tmux session name
        #[arg(long)]
        session_name: Option<String>,
        /// Reserved; ignored in v1
        #[arg(long)]
        bind: Option<String>,
    },
    /// Print a status summary of all colors
    Status {
        #[arg(long, default_value_t = 50)]
        lines: u32,
        #[arg(long)]
        json: bool,
    },
    /// Print captured pane output for one color
    Get {
        color: String,
        #[arg(long, default_value_t = 200)]
        lines: u32,
        #[arg(long)]
        json: bool,
    },
    /// Send literal text + Enter to one color's claude pane
    Send { color: String, text: String },
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let code = match cli.command {
        Command::Start { session_name, bind } => cli::start::run(session_name, bind),
        Command::Status { lines, json } => cli::status::run(lines, json),
        Command::Get { color, lines, json } => cli::get::run(&color, lines, json),
        Command::Send { color, text } => cli::send::run(&color, &text),
    };
    ExitCode::from(code)
}

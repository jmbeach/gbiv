//! `gbiv done [<color|tag>]` — take a task off its color.
//! See `docs/llds/feature-ledger.md` § Done Command.

use crate::commands::lifecycle::{CmdEnv, Sink};

// @spec FL-DONE-001 through FL-DONE-011
pub fn done_command(arg: Option<&str>, env: &CmdEnv, sink: &mut dyn Sink) -> anyhow::Result<()> {
    let _ = (arg, env, sink);
    todo!("gbv-1ht: done_command")
}

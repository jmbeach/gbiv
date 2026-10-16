//! `gbiv start <task> [--color <color>]` — put a task on the next free color.
//! See `docs/llds/feature-ledger.md` § Start Command.

use crate::commands::lifecycle::{CmdEnv, Sink};

// @spec FL-START-001 through FL-START-014
pub fn start_command(
    task: &str,
    color: Option<&str>,
    env: &CmdEnv,
    sink: &mut dyn Sink,
) -> anyhow::Result<()> {
    let _ = (task, color, env, sink);
    todo!("gbv-1ht: start_command")
}

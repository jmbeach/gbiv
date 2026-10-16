//! Shared plumbing for `gbiv start` / `gbiv done`: the process environment
//! they read and the sink they print through.

use std::path::PathBuf;

use crate::extensions::Limits;

/// Inputs the lifecycle commands take from the process, injected for tests.
#[derive(Debug, Clone)]
pub struct CmdEnv {
    pub cwd: PathBuf,
    pub home: PathBuf,
    pub xdg_config_home: Option<PathBuf>,
    pub limits: Limits,
}

impl CmdEnv {
    pub fn from_process() -> anyhow::Result<CmdEnv> {
        todo!("gbv-1ht: CmdEnv::from_process")
    }
}

/// Where command output goes, in order. `main` prints immediately; tests record.
pub trait Sink {
    fn out(&mut self, line: &str);
    fn err(&mut self, line: &str);
}

/// Prints stdout / stderr lines as they come.
pub struct StdSink;

impl Sink for StdSink {
    fn out(&mut self, line: &str) {
        println!("{line}");
    }
    fn err(&mut self, line: &str) {
        eprintln!("{line}");
    }
}

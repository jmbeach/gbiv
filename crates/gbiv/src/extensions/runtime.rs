//! Loading extensions into sandboxed Lua states and firing the four events.
//! See `docs/llds/extension-runtime.md`.

use std::path::PathBuf;
use std::time::Duration;

use super::{ExtensionError, ExtensionsConfig};

/// Sandbox limits. Defaults: 64 MiB per state, 60 s per handler call.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Limits {
    pub memory_bytes: usize,
    pub handler_timeout: Duration,
}

impl Default for Limits {
    // @spec EXT-SBX-003, EXT-SBX-004
    fn default() -> Self {
        Limits {
            memory_bytes: 64 * 1024 * 1024,
            handler_timeout: Duration::from_secs(60),
        }
    }
}

/// Where extensions are looked up, and what relative / `~` paths resolve against.
#[derive(Debug, Clone)]
pub struct LoadPaths {
    /// `<gbiv-root>/.gbiv/extensions`
    pub project_dir: PathBuf,
    /// `<user-config-dir>/gbiv/extensions`
    pub user_dir: PathBuf,
    /// `<gbiv-root>/main/<repo>`
    pub main_repo: PathBuf,
    pub home: PathBuf,
}

/// The loaded, enabled extensions of one command invocation.
pub struct Runtime {
    _private: (),
}

impl Runtime {
    /// Load every enabled extension, in order, before any event fires.
    // @spec EXT-ENG-001, EXT-ENG-002, EXT-ENG-003, EXT-LOAD-001 through EXT-LOAD-008, EXT-LOAD-010, EXT-CFG-008, EXT-SBX-001, EXT-SBX-002, EXT-API-001, EXT-API-005
    pub fn load(
        paths: &LoadPaths,
        config: &ExtensionsConfig,
        limits: Limits,
    ) -> Result<Runtime, ExtensionError> {
        let _ = (paths, config, limits);
        todo!("gbv-1ht: Runtime::load")
    }

    /// Names of the loaded extensions, in run order.
    pub fn names(&self) -> Vec<String> {
        todo!("gbv-1ht: Runtime::names")
    }

    /// Fire `before_start`. `Ok(Some(text))` when a handler named the entry.
    // @spec EXT-EVT-001, EXT-EVT-002, EXT-EVT-004 through EXT-EVT-010
    pub fn before_start(&mut self, arg: &str, color: &str) -> Result<Option<String>, ExtensionError> {
        let _ = (arg, color);
        todo!("gbv-1ht: Runtime::before_start")
    }

    /// Fire `after_start`; handler errors become warnings in `take_messages`.
    // @spec EXT-EVT-003, EXT-EVT-011
    pub fn after_start(&mut self, color: &str, text: &str) {
        let _ = (color, text);
        todo!("gbv-1ht: Runtime::after_start")
    }

    /// Fire `before_done`.
    // @spec EXT-EVT-003, EXT-EVT-004, EXT-EVT-005
    pub fn before_done(&mut self, color: &str, text: &str) -> Result<(), ExtensionError> {
        let _ = (color, text);
        todo!("gbv-1ht: Runtime::before_done")
    }

    /// Fire `after_done`; handler errors become warnings in `take_messages`.
    // @spec EXT-EVT-003, EXT-EVT-011
    pub fn after_done(&mut self, color: &str, text: &str) {
        let _ = (color, text);
        todo!("gbv-1ht: Runtime::after_done")
    }

    /// Drain the stderr lines produced so far (`gbiv.log` / `gbiv.warn`,
    /// after-event failures, naming and grant warnings), already prefixed
    /// with the extension name, in the order they were produced.
    pub fn take_messages(&mut self) -> Vec<String> {
        todo!("gbv-1ht: Runtime::take_messages")
    }
}

//! `gbiv install-extension`. Mirrors `gbiv install-skill`.
//! See `docs/llds/extension-runtime.md` § `gbiv install-extension`.

use std::path::PathBuf;

use crate::orchestration::fleet_client::Outcome;
pub use crate::orchestration::install_skill_client::Scope;

/// Where the command looks things up; injected so tests stay hermetic.
#[derive(Debug, Clone)]
pub struct InstallEnv {
    pub cwd: PathBuf,
    /// The user config dir (`$XDG_CONFIG_HOME`, else `~/.config`).
    pub user_config_dir: PathBuf,
}

/// Names of the extensions compiled into this binary.
// @spec EXT-INST-002
pub fn bundled_names() -> Vec<&'static str> {
    todo!("gbv-1ht: bundled_names")
}

/// Run `gbiv install-extension <name>` and return the printable outcome.
// @spec EXT-INST-001, EXT-INST-003 through EXT-INST-013
pub fn run_install_extension(name: &str, scope: Scope, force: bool, env: &InstallEnv) -> Outcome {
    let _ = (name, scope, force, env);
    todo!("gbv-1ht: run_install_extension")
}

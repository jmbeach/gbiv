//! The `[extensions]` tables of the layered config: effective enabled list,
//! per-extension grants and options. See `docs/llds/extension-runtime.md`
//! § Configuration.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use super::ExtensionError;

/// One extension's merged settings.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ExtensionSettings {
    /// Granted paths, `~/` expanded and relative paths resolved against the main repo.
    pub paths: Vec<PathBuf>,
    pub http: bool,
    pub env: Vec<String>,
    /// Every non-reserved key, handed to the extension as `ctx.options`.
    pub options: toml::Table,
}

/// The effective `[extensions]` configuration after merging both layers.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ExtensionsConfig {
    /// Effective list, in load / run order.
    pub enabled: Vec<String>,
    /// Settings for enabled extensions only.
    pub settings: BTreeMap<String, ExtensionSettings>,
}

impl ExtensionsConfig {
    /// Merge the `[extensions]` tables of the global and project layers.
    /// `main_repo` resolves relative grant paths; `home` expands `~/`.
    // @spec EXT-CFG-001, EXT-CFG-002, EXT-CFG-003, EXT-CFG-004, EXT-CFG-005, EXT-CFG-006, EXT-CFG-007
    pub fn from_layers(
        global: Option<&toml::Table>,
        project: Option<&toml::Table>,
        main_repo: &Path,
        home: &Path,
    ) -> Result<ExtensionsConfig, ExtensionError> {
        let _ = (global, project, main_repo, home);
        todo!("gbv-1ht: ExtensionsConfig::from_layers")
    }
}

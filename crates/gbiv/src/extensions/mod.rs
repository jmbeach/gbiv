//! Extension runtime — sandboxed Lua extensions that hook `gbiv start` and
//! `gbiv done`. See `docs/llds/extension-runtime.md`.

pub mod config;
pub mod install;
pub mod runtime;

#[cfg(test)]
mod config_tests;
#[cfg(test)]
mod install_tests;
#[cfg(test)]
mod runtime_tests;
#[cfg(test)]
pub(crate) mod test_support;
#[cfg(test)]
mod vat_tests;

use std::path::PathBuf;

pub use config::{ExtensionSettings, ExtensionsConfig};
pub use runtime::{Limits, LoadPaths, Runtime};

/// The `api_version` values this gbiv can load.
pub const SUPPORTED_API_VERSIONS: &[i64] = &[1];

#[derive(Debug, thiserror::Error)]
pub enum ExtensionError {
    #[error("extension {name:?} is enabled but not installed (looked in {searched:?}); run: gbiv install-extension {name}")]
    NotFound { name: String, searched: Vec<PathBuf> },
    #[error("extension {name:?}: bad manifest {path}: {detail}")]
    BadManifest {
        name: String,
        path: PathBuf,
        detail: String,
    },
    #[error("extension {name:?} targets api_version {wanted}; this gbiv supports {supported:?}")]
    ApiVersion {
        name: String,
        wanted: i64,
        supported: Vec<i64>,
    },
    #[error("extension {name:?} failed to load: {detail}")]
    Load { name: String, detail: String },
    #[error("{name}: {message}")]
    Veto { name: String, message: String },
    #[error("[extensions] config: {detail}")]
    Config { detail: String },
}

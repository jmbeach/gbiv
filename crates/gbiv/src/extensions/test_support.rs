//! Shared helpers for extension-runtime and vat-extension tests (gbv-1ht).

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use tempfile::TempDir;

use super::{ExtensionSettings, ExtensionsConfig, Limits, LoadPaths};

/// A throwaway gbiv-shaped directory tree:
/// `<tmp>/root/{main/repo, .gbiv/extensions}` and `<tmp>/home/.config/gbiv/extensions`.
pub struct Layout {
    _tmp: TempDir,
    pub root: PathBuf,
    pub main_repo: PathBuf,
    pub project_ext: PathBuf,
    pub user_ext: PathBuf,
    pub home: PathBuf,
}

pub fn layout() -> Layout {
    let tmp = TempDir::new().expect("tempdir");
    let root = tmp.path().join("root");
    let main_repo = root.join("main").join("repo");
    let project_ext = root.join(".gbiv").join("extensions");
    let home = tmp.path().join("home");
    let user_ext = home.join(".config").join("gbiv").join("extensions");
    for d in [&main_repo, &project_ext, &user_ext] {
        fs::create_dir_all(d).expect("mkdir");
    }
    Layout {
        _tmp: tmp,
        root,
        main_repo,
        project_ext,
        user_ext,
        home,
    }
}

impl Layout {
    pub fn paths(&self) -> LoadPaths {
        LoadPaths {
            project_dir: self.project_ext.clone(),
            user_dir: self.user_ext.clone(),
            main_repo: self.main_repo.clone(),
            home: self.home.clone(),
        }
    }

    /// Write `<dir>/<name>/extension.toml` (from `manifest`) and `init.lua`.
    pub fn write_ext(&self, dir: &Path, name: &str, manifest: &str, init_lua: &str) -> PathBuf {
        let ext = dir.join(name);
        fs::create_dir_all(&ext).expect("mkdir ext");
        fs::write(ext.join("extension.toml"), manifest).expect("write manifest");
        fs::write(ext.join("init.lua"), init_lua).expect("write init.lua");
        ext
    }

    /// Shorthand: a valid extension in the project directory.
    pub fn project_ext(&self, name: &str, caps: &[&str], init_lua: &str) -> PathBuf {
        let dir = self.project_ext.clone();
        self.write_ext(&dir, name, &manifest(name, caps), init_lua)
    }
}

/// A valid manifest for `name` declaring `caps`.
pub fn manifest(name: &str, caps: &[&str]) -> String {
    let caps: Vec<String> = caps.iter().map(|c| format!("{c:?}")).collect();
    format!(
        "name = {name:?}\nversion = \"0.1.0\"\napi_version = 1\ncapabilities = [{}]\n",
        caps.join(", ")
    )
}

/// An effective config enabling `names` with no grants.
pub fn config(names: &[&str]) -> ExtensionsConfig {
    let mut cfg = ExtensionsConfig::default();
    for n in names {
        cfg.enabled.push((*n).to_string());
        cfg.settings.insert((*n).to_string(), ExtensionSettings::default());
    }
    cfg
}

/// An effective config enabling one extension with the given settings.
pub fn config_with(name: &str, settings: ExtensionSettings) -> ExtensionsConfig {
    let mut cfg = ExtensionsConfig::default();
    cfg.enabled.push(name.to_string());
    cfg.settings.insert(name.to_string(), settings);
    cfg
}

/// Default memory limit, short handler timeout so limit tests run fast.
pub fn fast_limits() -> Limits {
    Limits {
        memory_bytes: 64 * 1024 * 1024,
        handler_timeout: Duration::from_secs(2),
    }
}

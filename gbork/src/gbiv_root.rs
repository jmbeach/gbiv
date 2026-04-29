use std::path::{Path, PathBuf};

use crate::COLORS;

pub struct GbivRoot {
    pub root: PathBuf,
    pub folder_name: String,
}

impl GbivRoot {
    pub fn main_repo(&self) -> PathBuf {
        self.root.join("main").join(&self.folder_name)
    }

    pub fn port_file(&self) -> PathBuf {
        self.main_repo().join(".gbork").join("port")
    }

    pub fn default_session_name(&self) -> String {
        self.folder_name.clone()
    }
}

pub fn find(start: &Path) -> Option<GbivRoot> {
    let mut current = start.to_path_buf();
    loop {
        if let Some(folder_name) = current.file_name().and_then(|n| n.to_str()) {
            let candidate = current.join("main").join(folder_name);
            let has_color_dir = COLORS.iter().any(|c| current.join(c).is_dir());
            if candidate.exists() && has_color_dir {
                return Some(GbivRoot {
                    root: current.clone(),
                    folder_name: folder_name.to_string(),
                });
            }
        }
        if !current.pop() {
            break;
        }
    }
    None
}

pub fn find_from_cwd() -> Option<GbivRoot> {
    std::env::current_dir().ok().and_then(|p| find(&p))
}

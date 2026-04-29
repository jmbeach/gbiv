use std::fs;
use std::path::Path;

pub fn write(path: &Path, port: u16) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, format!("{port}\n"))
}

pub fn read(path: &Path) -> Result<u16, String> {
    let raw = fs::read_to_string(path).map_err(|e| {
        format!(
            "daemon not running (no port file at {}): {e}\nstart it with: gbork start",
            path.display()
        )
    })?;
    raw.trim()
        .parse::<u16>()
        .map_err(|_| format!("port file at {} is corrupt", path.display()))
}

pub fn remove(path: &Path) {
    let _ = fs::remove_file(path);
}

pub mod get;
pub mod send;
pub mod start;
pub mod status;

use crate::gbiv_root::{self, GbivRoot};
use crate::port_file;

pub fn require_gbiv_root() -> Result<GbivRoot, u8> {
    match gbiv_root::find_from_cwd() {
        Some(r) => Ok(r),
        None => {
            eprintln!("gbork: not inside a gbiv project");
            Err(2)
        }
    }
}

pub fn require_daemon_url() -> Result<String, u8> {
    let root = require_gbiv_root()?;
    let port_path = root.port_file();
    let port = match port_file::read(&port_path) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("gbork: {e}");
            return Err(2);
        }
    };
    Ok(format!("http://127.0.0.1:{port}"))
}

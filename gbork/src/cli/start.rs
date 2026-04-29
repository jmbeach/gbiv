use crate::cli::require_gbiv_root;
use crate::port_file;
use crate::server::{self, Config};
use crate::tmux;

pub fn run(session_name: Option<String>, _bind: Option<String>) -> u8 {
    let root = match require_gbiv_root() {
        Ok(r) => r,
        Err(code) => return code,
    };
    if let Err(e) = tmux::check_installed() {
        eprintln!("gbork: {e}");
        return 1;
    }
    let session_name = session_name.unwrap_or_else(|| root.default_session_name());
    let cfg = Config {
        session_name: session_name.clone(),
    };
    let handle = match server::serve(cfg) {
        Ok(h) => h,
        Err(e) => {
            eprintln!("gbork: {e}");
            return 1;
        }
    };
    let port_path = root.port_file();
    if let Err(e) = port_file::write(&port_path, handle.port) {
        eprintln!(
            "gbork: failed to write port file at {}: {e}",
            port_path.display()
        );
        return 1;
    }
    println!(
        "gbork listening on http://127.0.0.1:{} (session: {})",
        handle.port, session_name
    );
    println!("port file: {}", port_path.display());
    println!("press Ctrl+C to stop (port file may be left stale; the next `gbork start` overwrites it)");
    handle.join.join().ok();
    port_file::remove(&port_path);
    0
}

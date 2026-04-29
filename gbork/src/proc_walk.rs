use std::collections::{HashMap, HashSet};
use std::path::Path;

const MAX_DEPTH: u32 = 8;
const MAX_VISITS: usize = 64;

const CLAUDE_BASENAMES: &[&str] = &["claude", "claude-code"];

fn basename_matches(path: &str) -> bool {
    let name = Path::new(path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("");
    CLAUDE_BASENAMES.iter().any(|b| name == *b)
}

pub fn is_claude_process_tree(root_pid: u32) -> bool {
    let snap = match snapshot() {
        Ok(s) => s,
        Err(_) => return false,
    };
    if let Some(exe) = snap.exe.get(&root_pid) {
        if basename_matches(exe) {
            return true;
        }
    }
    let mut stack: Vec<(u32, u32)> = vec![(root_pid, 0)];
    let mut seen: HashSet<u32> = HashSet::new();
    let mut visits: usize = 0;
    while let Some((pid, depth)) = stack.pop() {
        if !seen.insert(pid) {
            continue;
        }
        visits += 1;
        if visits > MAX_VISITS {
            break;
        }
        if depth >= MAX_DEPTH {
            continue;
        }
        if let Some(children) = snap.children.get(&pid) {
            for child in children {
                if let Some(exe) = snap.exe.get(child) {
                    if basename_matches(exe) {
                        return true;
                    }
                }
                stack.push((*child, depth + 1));
            }
        }
    }
    false
}

struct Snapshot {
    children: HashMap<u32, Vec<u32>>,
    exe: HashMap<u32, String>,
}

#[cfg(target_os = "linux")]
fn snapshot() -> Result<Snapshot, std::io::Error> {
    use std::fs;
    let mut children: HashMap<u32, Vec<u32>> = HashMap::new();
    let mut exe: HashMap<u32, String> = HashMap::new();
    for entry in fs::read_dir("/proc")? {
        let entry = entry?;
        let name = entry.file_name();
        let name_s = match name.to_str() {
            Some(s) => s,
            None => continue,
        };
        let pid: u32 = match name_s.parse() {
            Ok(p) => p,
            Err(_) => continue,
        };
        let stat_path = entry.path().join("stat");
        let stat = match fs::read_to_string(&stat_path) {
            Ok(s) => s,
            Err(_) => continue,
        };
        if let Some(ppid) = parse_ppid_from_stat(&stat) {
            children.entry(ppid).or_default().push(pid);
        }
        let exe_path = entry.path().join("exe");
        if let Ok(target) = fs::read_link(&exe_path) {
            if let Some(s) = target.to_str() {
                exe.insert(pid, s.to_string());
            }
        }
    }
    Ok(Snapshot { children, exe })
}

#[cfg(target_os = "linux")]
fn parse_ppid_from_stat(stat: &str) -> Option<u32> {
    let close = stat.rfind(')')?;
    let after = &stat[close + 1..];
    let mut parts = after.split_whitespace();
    let _state = parts.next()?;
    let ppid_s = parts.next()?;
    ppid_s.parse().ok()
}

#[cfg(target_os = "macos")]
fn snapshot() -> Result<Snapshot, std::io::Error> {
    use std::process::Command;
    let mut children: HashMap<u32, Vec<u32>> = HashMap::new();
    let mut exe: HashMap<u32, String> = HashMap::new();
    let out = Command::new("ps")
        .args(["-A", "-o", "pid=,ppid="])
        .output()?;
    if !out.status.success() {
        return Err(std::io::Error::new(std::io::ErrorKind::Other, "ps failed"));
    }
    let s = String::from_utf8_lossy(&out.stdout);
    let mut all_pids = Vec::new();
    for line in s.lines() {
        let mut parts = line.split_whitespace();
        let pid: u32 = match parts.next().and_then(|p| p.parse().ok()) {
            Some(p) => p,
            None => continue,
        };
        let ppid: u32 = match parts.next().and_then(|p| p.parse().ok()) {
            Some(p) => p,
            None => continue,
        };
        children.entry(ppid).or_default().push(pid);
        all_pids.push(pid);
    }
    for pid in all_pids {
        if let Some(path) = darwin_proc_pidpath(pid) {
            exe.insert(pid, path);
        }
    }
    Ok(Snapshot { children, exe })
}

#[cfg(target_os = "macos")]
fn darwin_proc_pidpath(pid: u32) -> Option<String> {
    use std::os::raw::{c_char, c_int};
    extern "C" {
        fn proc_pidpath(pid: c_int, buf: *mut c_char, buf_size: u32) -> c_int;
    }
    let mut buf = vec![0u8; 4096];
    let n = unsafe {
        proc_pidpath(
            pid as c_int,
            buf.as_mut_ptr() as *mut c_char,
            buf.len() as u32,
        )
    };
    if n <= 0 {
        return None;
    }
    buf.truncate(n as usize);
    String::from_utf8(buf).ok()
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn snapshot() -> Result<Snapshot, std::io::Error> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "platform not supported",
    ))
}

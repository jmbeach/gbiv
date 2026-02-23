use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::git_utils::find_gbiv_root;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct LockData {
    pub worktree: String,
    // pid is recorded for diagnostic purposes (shown in timeout errors) only,
    // not used for stale detection — locks must be cleared manually.
    pub pid: u32,
}

fn find_lock_path(start: &Path) -> Result<PathBuf, String> {
    let root = find_gbiv_root(start)
        .ok_or_else(|| "Not inside a gbiv worktree structure. Run `gbiv init` first.".to_string())?;

    let main_dir = root.join("main");

    let project_dir = std::fs::read_dir(&main_dir)
        .map_err(|e| format!("Cannot read main/ directory at {}: {}", main_dir.display(), e))?
        .filter_map(|e| e.ok())
        .find(|e| e.path().is_dir() && !e.file_name().to_string_lossy().starts_with('.'))
        .ok_or_else(|| "No project directory found in main/".to_string())?;

    Ok(project_dir.path().join(".prd.lock"))
}

fn get_current_worktree(path: &Path) -> Result<String, String> {
    let output = Command::new("git")
        .args(["symbolic-ref", "--short", "HEAD"])
        .current_dir(path)
        .output()
        .map_err(|e| format!("Failed to run git: {}", e))?;

    if output.status.success() {
        let branch = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if branch.is_empty() {
            Err("Could not determine current branch (detached HEAD?)".to_string())
        } else {
            Ok(branch)
        }
    } else {
        Err(format!(
            "git symbolic-ref failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ))
    }
}

fn read_lock_file(path: &Path) -> Option<LockData> {
    let contents = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&contents).ok()
}

fn lock_with_path(lock_path: &Path, worktree: &str, pid: u32, timeout_secs: u64) -> Result<(), String> {
    let lock_data = LockData { worktree: worktree.to_string(), pid };
    let lock_json = serde_json::to_string(&lock_data)
        .map_err(|e| format!("Failed to serialize lock data: {}", e))?;

    let deadline = Instant::now() + Duration::from_secs(timeout_secs);

    loop {
        match std::fs::OpenOptions::new().write(true).create_new(true).open(lock_path) {
            Ok(mut file) => {
                file.write_all(lock_json.as_bytes())
                    .map_err(|e| format!("Failed to write lock data: {}", e))?;
                eprintln!("Acquired lock: {}", lock_path.display());
                return Ok(());
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                match read_lock_file(lock_path) {
                    None => {
                        if !lock_path.exists() {
                            // File was deleted between AlreadyExists and read — retry acquisition.
                            continue;
                        }
                        return Err(format!(
                            "Lock file exists but is unreadable or corrupt. Delete it manually: {}",
                            lock_path.display()
                        ));
                    }
                    Some(existing) => {
                        if Instant::now() >= deadline {
                            return Err(format!(
                                "Timeout waiting for prd.json lock (held by '{}', pid {})",
                                existing.worktree, existing.pid
                            ));
                        }
                        std::thread::sleep(Duration::from_millis(500));
                    }
                }
            }
            Err(e) => return Err(format!("Failed to create lock file: {}", e)),
        }
    }
}

fn unlock_with_path(lock_path: &Path, worktree: &str, force: bool) -> Result<(), String> {
    if !lock_path.exists() {
        return Ok(());
    }

    match read_lock_file(lock_path) {
        None => {
            // Corrupt lock file — remove it
            std::fs::remove_file(lock_path)
                .map_err(|e| format!("Failed to remove corrupt lock file: {}", e))
        }
        Some(existing) => {
            if existing.worktree == worktree || force {
                std::fs::remove_file(lock_path)
                    .map_err(|e| format!("Failed to remove lock file: {}", e))
            } else {
                Err(format!(
                    "Cannot unlock: lock is owned by '{}' (pid {}). Use --force to override.",
                    existing.worktree, existing.pid
                ))
            }
        }
    }
}

pub fn lock_command(timeout_secs: u64) -> Result<(), String> {
    let cwd = std::env::current_dir()
        .map_err(|e| format!("Cannot get current directory: {}", e))?;
    let lock_path = find_lock_path(&cwd)?;
    let worktree = get_current_worktree(&cwd)?;
    let pid = std::process::id();
    lock_with_path(&lock_path, &worktree, pid, timeout_secs)
}

pub fn unlock_command(force: bool) -> Result<(), String> {
    let cwd = std::env::current_dir()
        .map_err(|e| format!("Cannot get current directory: {}", e))?;
    let lock_path = find_lock_path(&cwd)?;
    let worktree = get_current_worktree(&cwd)?;
    unlock_with_path(&lock_path, &worktree, force)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn temp_lock_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("gbiv_test_lock_{}.lock", name))
    }

    fn cleanup(path: &Path) {
        let _ = fs::remove_file(path);
    }

    #[test]
    fn test_lock_succeeds_when_no_lock_exists() {
        let lock_path = temp_lock_path("no_lock");
        cleanup(&lock_path);

        let result = lock_with_path(&lock_path, "blue", std::process::id(), 5);
        assert!(result.is_ok(), "Expected lock to succeed: {:?}", result);
        assert!(lock_path.exists(), "Lock file should exist after locking");

        cleanup(&lock_path);
    }


    #[test]
    fn test_lock_times_out_with_live_lock() {
        let lock_path = temp_lock_path("live_lock");
        cleanup(&lock_path);

        // Lock with our own PID — we are definitely alive
        let live = LockData { worktree: "red".to_string(), pid: std::process::id() };
        fs::write(&lock_path, serde_json::to_string(&live).unwrap()).unwrap();

        let result = lock_with_path(&lock_path, "blue", std::process::id(), 1);
        assert!(result.is_err(), "Expected timeout error");
        assert!(result.unwrap_err().contains("Timeout"), "Expected timeout message");

        cleanup(&lock_path);
    }

    #[test]
    fn test_unlock_succeeds_for_owner() {
        let lock_path = temp_lock_path("unlock_owner");
        cleanup(&lock_path);

        lock_with_path(&lock_path, "blue", std::process::id(), 5).unwrap();
        let result = unlock_with_path(&lock_path, "blue", false);
        assert!(result.is_ok(), "Expected unlock to succeed: {:?}", result);
        assert!(!lock_path.exists(), "Lock file should be gone after unlock");
    }

    #[test]
    fn test_unlock_fails_for_non_owner_without_force() {
        let lock_path = temp_lock_path("unlock_non_owner");
        cleanup(&lock_path);

        lock_with_path(&lock_path, "red", std::process::id(), 5).unwrap();
        let result = unlock_with_path(&lock_path, "blue", false);
        assert!(result.is_err(), "Expected error for non-owner");
        assert!(lock_path.exists(), "Lock file should still exist");

        cleanup(&lock_path);
    }

    #[test]
    fn test_unlock_succeeds_for_non_owner_with_force() {
        let lock_path = temp_lock_path("unlock_force");
        cleanup(&lock_path);

        lock_with_path(&lock_path, "red", std::process::id(), 5).unwrap();
        let result = unlock_with_path(&lock_path, "blue", true);
        assert!(result.is_ok(), "Expected force unlock to succeed: {:?}", result);
        assert!(!lock_path.exists(), "Lock file should be gone after force unlock");
    }

    #[test]
    fn test_unlock_idempotent_when_no_lock() {
        let lock_path = temp_lock_path("unlock_idempotent");
        cleanup(&lock_path);

        let result = unlock_with_path(&lock_path, "blue", false);
        assert!(result.is_ok(), "Expected idempotent unlock: {:?}", result);
    }

    #[test]
    fn test_lock_fails_on_corrupt_lock_file() {
        let lock_path = temp_lock_path("corrupt_lock");
        cleanup(&lock_path);

        fs::write(&lock_path, b"not valid json at all!!!").unwrap();

        let result = lock_with_path(&lock_path, "blue", std::process::id(), 5);
        assert!(result.is_err(), "Expected error on corrupt lock file");
        assert!(result.unwrap_err().contains("corrupt"), "Expected corruption message");

        cleanup(&lock_path);
    }

    #[test]
    fn test_find_lock_path_fails_outside_gbiv() {
        let result = find_lock_path(Path::new("/tmp"));
        assert!(result.is_err(), "Expected error outside gbiv structure");
    }
}

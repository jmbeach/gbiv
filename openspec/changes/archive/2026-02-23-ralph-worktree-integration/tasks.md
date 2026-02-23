## 1. Project Structure

- [x] 1.1 Create `src/commands/prd.rs` module file
- [x] 1.2 Add `pub mod prd;` to `src/commands/mod.rs`
- [x] 1.3 Add `prd` subcommand with `lock` and `unlock` subcommands to `cli()` in `src/main.rs`
- [x] 1.4 Wire `prd lock` and `prd unlock` match arms in `main()`

## 2. Lock File Utilities

- [x] 2.1 Implement `find_lock_path(start: &Path) -> Result<PathBuf, String>` — uses `find_gbiv_root` to locate main worktree, returns path to `main/<project>/.prd.lock`
- [x] 2.2 Implement `get_current_worktree(path: &Path) -> Result<String, String>` — reads branch name via `git symbolic-ref --short HEAD`
- [x] 2.3 Implement `read_lock_file(path: &Path) -> Option<LockData>` — parses JSON `{ worktree, pid }`, returns `None` on missing or invalid JSON
- [x] 2.4 ~~Implement `is_pid_alive`~~ — removed; stale lock detection is not automatic, users break locks manually
- [x] 2.5 Define `LockData` struct with `worktree: String` and `pid: u32` fields, derive Serialize/Deserialize

## 3. Lock Command

- [x] 3.1 Implement `lock_command(timeout_secs: u64) -> Result<(), String>`
- [x] 3.2 Use `OpenOptions::create_new(true)` for atomic lock file creation
- [x] 3.3 On `AlreadyExists` error: read existing lock for owner info
- [x] 3.4 ~~Auto-break stale lock~~ — removed; any existing lock file blocks acquisition regardless of PID state
- [x] 3.5 Sleep 500ms and retry; fail with owner info after `timeout_secs`
- [x] 3.6 If lock file contains invalid JSON: return immediate error with path to delete manually
- [x] 3.7 On successful lock: print lock file path to stderr

## 4. Unlock Command

- [x] 4.1 Implement `unlock_command(force: bool) -> Result<(), String>`
- [x] 4.2 If no lock file exists: return `Ok(())` (idempotent)
- [x] 4.3 Read lock file; if current worktree matches owner: delete lock file
- [x] 4.4 If not owner and `--force` is false: return error identifying the current owner
- [x] 4.5 If not owner and `--force` is true: delete lock file and return `Ok(())`

## 5. Add serde Dependency

- [x] 5.1 Add `serde = { version = "1", features = ["derive"] }` and `serde_json = "1"` to `Cargo.toml`

## 6. Tests

- [x] 6.1 Test `lock_command` succeeds when no lock file exists
- [x] 6.2 Test `lock_command` fails immediately on corrupt lock file (with actionable error)
- [x] 6.3 Test `lock_command` times out when lock is held by live owner
- [x] 6.4 Test `unlock_command` succeeds for owner
- [x] 6.5 Test `unlock_command` fails for non-owner without `--force`
- [x] 6.6 Test `unlock_command` succeeds for non-owner with `--force`
- [x] 6.7 Test `unlock_command` is idempotent when no lock exists
- [x] 6.8 Test corrupted lock file is treated as stale during acquisition
- [x] 6.9 Test `find_lock_path` fails gracefully outside a gbiv structure

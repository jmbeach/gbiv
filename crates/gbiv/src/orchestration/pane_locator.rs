//! Pane Locator: map a ROYGBIV color to the tmux pane running Claude Code.
//!
//! See `docs/llds/pane-locator.md` and `docs/specs/pane-locator.md`. The locator
//! answers, for a color: is there a window for it, and which pane in that window
//! is running claude? It identifies claude by walking each pane's process tree
//! and matching the executable *basename* (`claude`/`claude-code`) — never the
//! self-reported process name, which Claude Code rewrites to its version string.
//!
//! Built on `gbiv_core::tmux::list_windows` and `orchestration::tmux_driver::list_panes`.

use std::collections::{HashMap, HashSet};

use gbiv_core::tmux::{list_windows, TmuxError, WindowInfo};

use super::tmux_driver::{list_panes, PaneInfo};

/// Maximum depth of the process-tree walk (root is depth 0).
const MAX_DEPTH: usize = 8;
/// Maximum number of processes visited during a single walk.
const MAX_VISITS: usize = 64;

// @spec PANE-LOC-001
/// The outcome of locating a claude pane for one color.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Resolution {
    /// A claude pane was located. `other_pane_ids` holds any additional claude
    /// panes (empty in the common single-pane case), ordered oldest-first behind
    /// the chosen `pane_id`.
    Ok {
        pane_id: String,
        other_pane_ids: Vec<String>,
    },
    /// No tmux window exists for the color.
    NoWindow,
    /// A window exists but no pane in it is running claude.
    NoClaudePane,
}

// @spec PANE-LOC-002
#[derive(Debug, thiserror::Error)]
pub enum LocatorError {
    #[error("tmux session error: {0}")]
    TmuxSession(#[from] TmuxError),
}

/// One color paired with its resolution outcome, as returned by [`locate_panes`].
/// A per-color `Err` isolates that color's failure from the rest of the batch.
pub type ColorResolution = (String, Result<Resolution, LocatorError>);

/// A source of process-tree information, abstracted so the walk logic is testable
/// with an injected in-memory table instead of the live OS.
trait ProcSource {
    /// Direct child PIDs of `pid`.
    fn children(&self, pid: u32) -> Vec<u32>;
    /// Basename of `pid`'s executable path, or `None` if it cannot be read.
    fn exe_basename(&self, pid: u32) -> Option<String>;
    /// Process start time in a platform-specific unit (compared only within one
    /// host), or `None` if it cannot be read.
    fn start_time(&self, pid: u32) -> Option<u64>;
}

/// A pane classified as running claude, carrying the data needed to order it.
#[derive(Debug, Clone, PartialEq, Eq)]
struct ClaudePane {
    pane_id: String,
    pid: u32,
    /// Earliest start time among the claude processes in the pane's tree, or
    /// `None` if no claude start time could be read.
    start: Option<u64>,
}

/// Result of walking one pane's process tree.
#[derive(Debug, Clone, PartialEq, Eq)]
struct PaneClaude {
    /// Whether any process in the tree is claude.
    is_claude: bool,
    /// Earliest readable start time among claude processes, if any.
    start: Option<u64>,
}

// @spec PANE-LOC-012, PANE-LOC-013
/// A process is claude iff its executable basename is exactly `claude` or
/// `claude-code` (case-sensitive).
fn is_claude_basename(basename: &str) -> bool {
    basename == "claude" || basename == "claude-code"
}

// @spec PANE-LOC-012, PANE-LOC-015, PANE-LOC-016, PANE-LOC-017, PANE-LOC-018
/// Walk the tree rooted at `root_pid` (root included), bounded by `MAX_DEPTH` and
/// `MAX_VISITS`. Exhaustive within bounds (does not stop at the first match) so a
/// wrapper named `claude` cannot mask a deeper real claude, and so the *earliest*
/// claude start time is found. A read failure for any process simply omits it.
fn walk_claude<S: ProcSource>(root_pid: u32, src: &S) -> PaneClaude {
    let mut visited = 0usize;
    let mut is_claude = false;
    let mut earliest: Option<u64> = None;
    let mut seen: HashSet<u32> = HashSet::new();
    // DFS with per-node depth. `seen` guards against cycles / PID reuse in the
    // snapshot; the two bounds cap cost in pathological trees.
    let mut stack: Vec<(u32, usize)> = vec![(root_pid, 0)];
    while let Some((pid, depth)) = stack.pop() {
        if visited >= MAX_VISITS {
            break;
        }
        if !seen.insert(pid) {
            continue;
        }
        visited += 1;
        if let Some(base) = src.exe_basename(pid) {
            if is_claude_basename(&base) {
                is_claude = true;
                // Track the earliest claude start so the pane sorts by its oldest
                // claude; a process with no readable start time simply doesn't
                // lower `earliest`.
                if let Some(st) = src.start_time(pid) {
                    earliest = Some(earliest.map_or(st, |e| e.min(st)));
                }
            }
        }
        if depth < MAX_DEPTH {
            for child in src.children(pid) {
                stack.push((child, depth + 1));
            }
        }
    }
    PaneClaude {
        is_claude,
        start: earliest,
    }
}

// @spec PANE-LOC-007, PANE-LOC-008, PANE-LOC-009, PANE-LOC-010, PANE-LOC-011
/// Map the set of claude panes to a `Resolution`. 0 → `NoClaudePane`; otherwise
/// order oldest-first (unreadable start times sink to the back; ties break by
/// lower pid then lexicographic pane id) and return the oldest as `pane_id`.
fn resolve_claude_panes(mut panes: Vec<ClaudePane>) -> Resolution {
    if panes.is_empty() {
        return Resolution::NoClaudePane;
    }
    // Sort key: readable start times first (ascending = oldest first); unreadable
    // starts sink to the back. Ties break by lower pid, then lexicographic pane
    // id — fully deterministic.
    panes.sort_by(|a, b| {
        let a_key = (a.start.is_none(), a.start.unwrap_or(0), a.pid);
        let b_key = (b.start.is_none(), b.start.unwrap_or(0), b.pid);
        a_key.cmp(&b_key).then_with(|| a.pane_id.cmp(&b.pane_id))
    });
    let mut ids = panes.into_iter().map(|p| p.pane_id);
    let pane_id = ids.next().expect("non-empty checked above");
    Resolution::Ok {
        pane_id,
        other_pane_ids: ids.collect(),
    }
}

// @spec PANE-LOC-005, PANE-LOC-006, PANE-LOC-014, PANE-LOC-022, PANE-LOC-023
/// Resolve one color against an already-fetched window list and a shared process
/// source. Factored out so the batch path can amortize the host-wide process
/// scan and the window listing across many colors in a single request.
fn resolve_color<P, S>(
    session: &str,
    color: &str,
    windows: &[WindowInfo],
    list_panes: &P,
    src: &S,
) -> Result<Resolution, LocatorError>
where
    P: Fn(&str) -> Result<Vec<PaneInfo>, TmuxError>,
    S: ProcSource,
{
    // First window whose name matches the color; the daemon creates exactly one
    // per color, so a duplicate name is a tolerated anomaly (PANE-LOC-022).
    if !windows.iter().any(|w| w.name == color) {
        return Ok(Resolution::NoWindow);
    }
    let target = format!("{session}:{color}");
    // A list_panes error (window vanished mid-resolution, or any tmux failure)
    // propagates as LocatorError, not a Resolution (PANE-LOC-023).
    let panes = list_panes(&target)?;
    let claude_panes: Vec<ClaudePane> = panes
        .iter()
        .filter_map(|p| {
            let info = walk_claude(p.pid, src);
            info.is_claude.then(|| ClaudePane {
                pane_id: p.id.clone(),
                pid: p.pid,
                start: info.start,
            })
        })
        .collect();
    Ok(resolve_claude_panes(claude_panes))
}

// @spec PANE-LOC-003, PANE-LOC-004
/// Single-color locator logic with tmux calls and the process source injected.
fn locate_pane_with<W, P, S>(
    session: &str,
    color: &str,
    list_windows: W,
    list_panes: P,
    src: &S,
) -> Result<Resolution, LocatorError>
where
    W: Fn(&str) -> Result<Vec<WindowInfo>, TmuxError>,
    P: Fn(&str) -> Result<Vec<PaneInfo>, TmuxError>,
    S: ProcSource,
{
    let windows = list_windows(session)?;
    resolve_color(session, color, &windows, &list_panes, src)
}

// @spec PANE-LOC-024, PANE-LOC-025
/// Batch locator logic: fetch the window list once and resolve every color
/// against a single shared process source. A per-color pane-listing failure is
/// isolated to that color's inline `Result`; a session-level window-listing
/// failure (session missing) fails the whole batch.
fn locate_panes_with<W, P, S>(
    session: &str,
    colors: &[&str],
    list_windows: W,
    list_panes: P,
    src: &S,
) -> Result<Vec<ColorResolution>, LocatorError>
where
    W: Fn(&str) -> Result<Vec<WindowInfo>, TmuxError>,
    P: Fn(&str) -> Result<Vec<PaneInfo>, TmuxError>,
    S: ProcSource,
{
    let windows = list_windows(session)?;
    Ok(colors
        .iter()
        .map(|color| {
            (
                (*color).to_string(),
                resolve_color(session, color, &windows, &list_panes, src),
            )
        })
        .collect())
}

// @spec PANE-LOC-019, PANE-LOC-020
/// The live OS process source (macOS via `ps`, Linux via `/proc`). The child map
/// is built once at construction; executable and start time are read per-pid
/// during the walk (bounded to `MAX_VISITS`, so a handful of syscalls).
struct RealProcSource {
    children: HashMap<u32, Vec<u32>>,
}

impl RealProcSource {
    fn new() -> Self {
        RealProcSource {
            children: build_children_map(),
        }
    }
}

impl ProcSource for RealProcSource {
    fn children(&self, pid: u32) -> Vec<u32> {
        self.children.get(&pid).cloned().unwrap_or_default()
    }
    fn exe_basename(&self, pid: u32) -> Option<String> {
        read_exe_basename(pid)
    }
    fn start_time(&self, pid: u32) -> Option<u64> {
        read_start_time(pid)
    }
}

/// Basename of an executable path, or `None` for an empty/rootless path.
fn path_basename(path: &str) -> Option<String> {
    let trimmed = path.trim_end_matches('/');
    trimmed
        .rsplit('/')
        .next()
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

// ---- macOS process source (ps) --------------------------------------------

// @spec PANE-LOC-019
#[cfg(target_os = "macos")]
fn build_children_map() -> HashMap<u32, Vec<u32>> {
    use std::process::Command;
    let mut map: HashMap<u32, Vec<u32>> = HashMap::new();
    let out = match Command::new("ps").args(["-A", "-o", "pid=,ppid="]).output() {
        Ok(o) if o.status.success() => o.stdout,
        _ => return map,
    };
    for line in String::from_utf8_lossy(&out).lines() {
        let mut it = line.split_whitespace();
        if let (Some(pid), Some(ppid)) = (it.next(), it.next()) {
            if let (Ok(pid), Ok(ppid)) = (pid.parse::<u32>(), ppid.parse::<u32>()) {
                map.entry(ppid).or_default().push(pid);
            }
        }
    }
    map
}

// @spec PANE-LOC-019
#[cfg(target_os = "macos")]
fn read_exe_basename(pid: u32) -> Option<String> {
    use std::process::Command;
    // `ps -o comm=` returns the executable path on macOS (not the renamed title).
    let out = Command::new("ps")
        .args(["-p", &pid.to_string(), "-o", "comm="])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let path = String::from_utf8_lossy(&out.stdout).trim().to_string();
    path_basename(&path)
}

// @spec PANE-LOC-019
#[cfg(target_os = "macos")]
fn read_start_time(pid: u32) -> Option<u64> {
    use std::process::Command;
    // `ps -o lstart=` yields a C-locale wall-clock timestamp, e.g.
    // "Sat Jul 26 16:54:03 2026", parsed to a unix timestamp (smaller = older).
    let out = Command::new("ps")
        .args(["-p", &pid.to_string(), "-o", "lstart="])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    parse_lstart(String::from_utf8_lossy(&out.stdout).trim())
}

/// Parse a C-locale `ps lstart` string ("Dow Mon DD HH:MM:SS YYYY") into an
/// ordering key (seconds, treating the fields as if UTC). `ps` reports local
/// time, so the value is offset from the true unix epoch by the host's timezone
/// — but that offset is constant across all panes on one host, so it cancels
/// under comparison, which is all the locator uses it for. Returns `None` on any
/// malformed field. Pure (no OS access) so it is unit-tested directly.
fn parse_lstart(s: &str) -> Option<u64> {
    let parts: Vec<&str> = s.split_whitespace().collect();
    if parts.len() != 5 {
        return None;
    }
    let month = match parts[1] {
        "Jan" => 1,
        "Feb" => 2,
        "Mar" => 3,
        "Apr" => 4,
        "May" => 5,
        "Jun" => 6,
        "Jul" => 7,
        "Aug" => 8,
        "Sep" => 9,
        "Oct" => 10,
        "Nov" => 11,
        "Dec" => 12,
        _ => return None,
    };
    let day: i64 = parts[2].parse().ok()?;
    let mut hms = parts[3].split(':');
    let hh: i64 = hms.next()?.parse().ok()?;
    let mm: i64 = hms.next()?.parse().ok()?;
    let ss: i64 = hms.next()?.parse().ok()?;
    let year: i64 = parts[4].parse().ok()?;
    let days = days_from_civil(year, month, day);
    let secs = days * 86_400 + hh * 3_600 + mm * 60 + ss;
    u64::try_from(secs).ok()
}

/// Days since the Unix epoch for a civil (proleptic Gregorian) date.
/// Howard Hinnant's algorithm.
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = (if y >= 0 { y } else { y - 399 }) / 400;
    let yoe = y - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

// ---- Linux process source (/proc) -----------------------------------------

// @spec PANE-LOC-020
#[cfg(target_os = "linux")]
fn build_children_map() -> HashMap<u32, Vec<u32>> {
    let mut map: HashMap<u32, Vec<u32>> = HashMap::new();
    let entries = match std::fs::read_dir("/proc") {
        Ok(e) => e,
        Err(_) => return map,
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        let Ok(pid) = name.parse::<u32>() else {
            continue;
        };
        if let Some(ppid) = read_stat_field(pid, StatField::Ppid) {
            map.entry(ppid as u32).or_default().push(pid);
        }
    }
    map
}

// @spec PANE-LOC-020
#[cfg(target_os = "linux")]
fn read_exe_basename(pid: u32) -> Option<String> {
    let target = std::fs::read_link(format!("/proc/{pid}/exe")).ok()?;
    path_basename(&target.to_string_lossy())
}

// @spec PANE-LOC-020
#[cfg(target_os = "linux")]
fn read_start_time(pid: u32) -> Option<u64> {
    let content = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    parse_stat_field(&content, StatField::StartTime)
}

#[cfg(target_os = "linux")]
fn read_stat_field(pid: u32, field: StatField) -> Option<u64> {
    let content = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    parse_stat_field(&content, field)
}

// Only the Linux path (`read_stat_field` above) calls these in production;
// deliberately not `#[cfg(target_os = "linux")]` so the parsing logic stays
// unit-tested on every CI platform (see `parse_stat_field`'s doc comment).
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
enum StatField {
    Ppid,
    StartTime,
}

/// Parse a field from the contents of `/proc/<pid>/stat`. `comm` (field 2) is
/// parenthesized and may itself contain spaces and parentheses, so fields are
/// indexed from after the *last* ')'. Pure (no `/proc` access) so it is
/// unit-tested directly with synthetic stat lines.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
fn parse_stat_field(content: &str, field: StatField) -> Option<u64> {
    let rest = &content[content.rfind(')')? + 1..];
    let tokens: Vec<&str> = rest.split_whitespace().collect();
    // After the last ')', tokens[0] is field 3 (state); field N maps to N-3.
    let idx = match field {
        StatField::Ppid => 1,       // field 4
        StatField::StartTime => 19, // field 22
    };
    tokens.get(idx)?.parse::<u64>().ok()
}

// ---- Fallback for other platforms ------------------------------------------

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn build_children_map() -> HashMap<u32, Vec<u32>> {
    HashMap::new()
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn read_exe_basename(_pid: u32) -> Option<String> {
    None
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn read_start_time(_pid: u32) -> Option<u64> {
    None
}

// @spec PANE-LOC-003, PANE-LOC-021
/// Locate the claude pane for a single `color` in the gbiv tmux `session`.
/// Re-resolves on every call (no caching), so pane state that changes between
/// calls is observed fresh. To resolve several colors at once, prefer
/// [`locate_panes`], which shares one host process scan across them.
pub fn locate_pane(session: &str, color: &str) -> Result<Resolution, LocatorError> {
    locate_pane_with(
        session,
        color,
        list_windows,
        list_panes,
        &RealProcSource::new(),
    )
}

// @spec PANE-LOC-021, PANE-LOC-024, PANE-LOC-025
/// Locate the claude pane for each of `colors` in one pass, building the host
/// process snapshot and window list **once** and resolving every color against
/// that shared snapshot. This is the batch path for callers resolving the whole
/// fleet (e.g. all seven ROYGBIV colors); it avoids the redundant full-host scan
/// that calling [`locate_pane`] per color would incur. Like the single-color
/// path it does not cache across calls — freshness is preserved per request.
pub fn locate_panes(session: &str, colors: &[&str]) -> Result<Vec<ColorResolution>, LocatorError> {
    locate_panes_with(
        session,
        colors,
        list_windows,
        list_panes,
        &RealProcSource::new(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use std::collections::HashMap;

    // ---- Fake process source -------------------------------------------------

    #[derive(Default)]
    struct FakeNode {
        exe: Option<String>,
        start: Option<u64>,
        children: Vec<u32>,
    }

    #[derive(Default)]
    struct FakeProcSource {
        nodes: HashMap<u32, FakeNode>,
        exe_calls: Cell<usize>,
    }

    impl FakeProcSource {
        fn add(&mut self, pid: u32, exe: Option<&str>, start: Option<u64>, children: &[u32]) {
            self.nodes.insert(
                pid,
                FakeNode {
                    exe: exe.map(|s| s.to_string()),
                    start,
                    children: children.to_vec(),
                },
            );
        }
    }

    impl ProcSource for FakeProcSource {
        fn children(&self, pid: u32) -> Vec<u32> {
            self.nodes
                .get(&pid)
                .map(|n| n.children.clone())
                .unwrap_or_default()
        }
        fn exe_basename(&self, pid: u32) -> Option<String> {
            self.exe_calls.set(self.exe_calls.get() + 1);
            self.nodes.get(&pid).and_then(|n| n.exe.clone())
        }
        fn start_time(&self, pid: u32) -> Option<u64> {
            self.nodes.get(&pid).and_then(|n| n.start)
        }
    }

    fn pane(id: &str, pid: u32, cmd: &str) -> PaneInfo {
        PaneInfo {
            id: id.into(),
            pid,
            current_command: cmd.into(),
            current_path: "/tmp".into(),
        }
    }

    fn win(name: &str) -> WindowInfo {
        WindowInfo {
            id: format!("@{name}"),
            name: name.into(),
        }
    }

    fn cp(pane_id: &str, pid: u32, start: Option<u64>) -> ClaudePane {
        ClaudePane {
            pane_id: pane_id.into(),
            pid,
            start,
        }
    }

    // ---- is_claude_basename (PANE-LOC-012, 013) ------------------------------

    // @spec PANE-LOC-012
    #[test]
    fn basename_matches_known_names() {
        assert!(is_claude_basename("claude"));
        assert!(is_claude_basename("claude-code"));
        assert!(!is_claude_basename("node"));
        assert!(!is_claude_basename("zsh"));
    }

    // @spec PANE-LOC-013
    #[test]
    fn basename_match_is_case_sensitive() {
        assert!(!is_claude_basename("Claude"));
        assert!(!is_claude_basename("CLAUDE"));
    }

    // ---- walk_claude ---------------------------------------------------------

    // @spec PANE-LOC-012
    #[test]
    fn walk_root_is_claude() {
        let mut src = FakeProcSource::default();
        src.add(100, Some("claude"), Some(5), &[]);
        let r = walk_claude(100, &src);
        assert!(r.is_claude);
        assert_eq!(r.start, Some(5));
    }

    // @spec PANE-LOC-012
    #[test]
    fn walk_descendant_is_claude_root_is_not() {
        let mut src = FakeProcSource::default();
        src.add(100, Some("zsh"), Some(1), &[200]);
        src.add(200, Some("claude"), Some(7), &[]);
        let r = walk_claude(100, &src);
        assert!(r.is_claude);
        assert_eq!(r.start, Some(7));
    }

    // @spec PANE-LOC-014
    #[test]
    fn walk_ignores_non_claude_executable() {
        // A pane whose foreground command *looks* like claude but whose executable
        // is node must not be classified as claude — only the exe path matters.
        let mut src = FakeProcSource::default();
        src.add(100, Some("node"), Some(1), &[]);
        let r = walk_claude(100, &src);
        assert!(!r.is_claude);
    }

    // @spec PANE-LOC-015, PANE-LOC-018
    #[test]
    fn walk_is_exhaustive_and_takes_earliest_claude() {
        // Root is a claude wrapper started later; a real claude started earlier
        // lives below it. Exhaustive walk must find the earliest (100), proving it
        // did not short-circuit at the root.
        let mut src = FakeProcSource::default();
        src.add(100, Some("claude"), Some(200), &[200]);
        src.add(200, Some("claude"), Some(100), &[]);
        let r = walk_claude(100, &src);
        assert!(r.is_claude);
        assert_eq!(r.start, Some(100));
    }

    // @spec PANE-LOC-016
    #[test]
    fn walk_respects_depth_bound() {
        // Linear chain with claude past the depth bound: pids 1..=12, claude at 11
        // (depth 10). With MAX_DEPTH = 8 it must not be reached.
        let mut src = FakeProcSource::default();
        for pid in 1u32..=12 {
            let exe = if pid == 11 { "claude" } else { "zsh" };
            let children: &[u32] = if pid < 12 { &[pid + 1] } else { &[] };
            src.add(pid, Some(exe), Some(pid as u64), children);
        }
        let r = walk_claude(1, &src);
        assert!(!r.is_claude, "claude beyond depth bound must not be found");
    }

    // @spec PANE-LOC-016
    #[test]
    fn walk_respects_visit_bound() {
        // Root with 200 non-claude children; the walk must visit at most MAX_VISITS
        // processes and terminate.
        let mut src = FakeProcSource::default();
        let kids: Vec<u32> = (1000..1200).collect();
        src.add(1, Some("zsh"), Some(1), &kids);
        for k in &kids {
            src.add(*k, Some("zsh"), Some(*k as u64), &[]);
        }
        let r = walk_claude(1, &src);
        assert!(!r.is_claude);
        assert!(
            src.exe_calls.get() <= MAX_VISITS,
            "visited {} processes, cap is {}",
            src.exe_calls.get(),
            MAX_VISITS
        );
    }

    // @spec PANE-LOC-017
    #[test]
    fn walk_unreadable_exe_is_not_claude() {
        let mut src = FakeProcSource::default();
        src.add(100, None, None, &[]); // exe unreadable
        let r = walk_claude(100, &src);
        assert!(!r.is_claude);
    }

    // @spec PANE-LOC-011
    #[test]
    fn walk_claude_with_unreadable_start_is_still_claude() {
        // Claude executable but start time unreadable → is_claude true, start None.
        let mut src = FakeProcSource::default();
        src.add(100, Some("claude"), None, &[]);
        let r = walk_claude(100, &src);
        assert!(r.is_claude);
        assert_eq!(r.start, None);
    }

    // ---- resolve_claude_panes ------------------------------------------------

    // @spec PANE-LOC-007, PANE-LOC-001
    #[test]
    fn resolve_zero_is_no_claude_pane() {
        assert_eq!(resolve_claude_panes(vec![]), Resolution::NoClaudePane);
    }

    // @spec PANE-LOC-008
    #[test]
    fn resolve_one_is_ok_with_empty_others() {
        let r = resolve_claude_panes(vec![cp("%1", 10, Some(5))]);
        assert_eq!(
            r,
            Resolution::Ok {
                pane_id: "%1".into(),
                other_pane_ids: vec![],
            }
        );
    }

    // @spec PANE-LOC-009
    #[test]
    fn resolve_many_orders_oldest_first() {
        // Start times: %a=300, %b=100, %c=200 → oldest is %b, then %c, then %a.
        let r = resolve_claude_panes(vec![
            cp("%a", 1, Some(300)),
            cp("%b", 2, Some(100)),
            cp("%c", 3, Some(200)),
        ]);
        assert_eq!(
            r,
            Resolution::Ok {
                pane_id: "%b".into(),
                other_pane_ids: vec!["%c".into(), "%a".into()],
            }
        );
    }

    // @spec PANE-LOC-010
    #[test]
    fn resolve_ties_break_by_lower_pid_then_pane_id() {
        // Equal start times → lower pid wins.
        let r = resolve_claude_panes(vec![cp("%hi", 50, Some(100)), cp("%lo", 20, Some(100))]);
        assert_eq!(
            r,
            Resolution::Ok {
                pane_id: "%lo".into(),
                other_pane_ids: vec!["%hi".into()],
            }
        );
        // Equal start AND equal pid → lexicographically smaller pane id wins.
        let r = resolve_claude_panes(vec![cp("%zzz", 7, Some(100)), cp("%aaa", 7, Some(100))]);
        assert_eq!(
            r,
            Resolution::Ok {
                pane_id: "%aaa".into(),
                other_pane_ids: vec!["%zzz".into()],
            }
        );
    }

    // @spec PANE-LOC-011
    #[test]
    fn resolve_unreadable_start_sorts_to_back() {
        // Known start beats unknown; among unknowns, lower pid first.
        let r = resolve_claude_panes(vec![
            cp("%none1", 30, None),
            cp("%known", 40, Some(999)),
            cp("%none2", 10, None),
        ]);
        assert_eq!(
            r,
            Resolution::Ok {
                pane_id: "%known".into(),
                other_pane_ids: vec!["%none2".into(), "%none1".into()],
            }
        );
    }

    // ---- locate_pane_with ----------------------------------------------------

    fn no_panes(_t: &str) -> Result<Vec<PaneInfo>, TmuxError> {
        Ok(vec![])
    }

    // @spec PANE-LOC-003
    #[test]
    fn locate_no_matching_window_is_no_window() {
        let windows = |_s: &str| Ok(vec![win("blue"), win("green")]);
        let src = FakeProcSource::default();
        let r = locate_pane_with("sess", "red", windows, no_panes, &src).unwrap();
        assert_eq!(r, Resolution::NoWindow);
    }

    // @spec PANE-LOC-004, PANE-LOC-002
    #[test]
    fn locate_session_missing_is_error() {
        let windows = |s: &str| Err(TmuxError::SessionNotFound(s.to_string()));
        let src = FakeProcSource::default();
        let err = locate_pane_with("sess", "red", windows, no_panes, &src).unwrap_err();
        match err {
            LocatorError::TmuxSession(TmuxError::SessionNotFound(s)) => assert_eq!(s, "sess"),
            e => panic!("expected TmuxSession(SessionNotFound), got {e:?}"),
        }
    }

    // @spec PANE-LOC-005
    #[test]
    fn locate_uses_session_colon_color_target() {
        let windows = |_s: &str| Ok(vec![win("red")]);
        let seen = Cell::new(String::new());
        let list = |t: &str| {
            seen.set(t.to_string());
            Ok(vec![])
        };
        let src = FakeProcSource::default();
        let _ = locate_pane_with("mysess", "red", windows, list, &src).unwrap();
        assert_eq!(seen.into_inner(), "mysess:red");
    }

    // @spec PANE-LOC-022
    #[test]
    fn locate_duplicate_windows_resolves_once() {
        // Two windows named "red"; the locator must resolve a single window (one
        // list_panes call), not union or error.
        let windows = |_s: &str| Ok(vec![win("red"), win("red")]);
        let calls = Cell::new(0usize);
        let list = |_t: &str| {
            calls.set(calls.get() + 1);
            Ok(vec![])
        };
        let src = FakeProcSource::default();
        let r = locate_pane_with("sess", "red", windows, list, &src).unwrap();
        assert_eq!(calls.get(), 1, "must list panes for exactly one window");
        assert_eq!(r, Resolution::NoClaudePane);
    }

    // @spec PANE-LOC-023
    #[test]
    fn locate_list_panes_error_is_error() {
        let windows = |_s: &str| Ok(vec![win("red")]);
        let list = |t: &str| Err(TmuxError::PaneNotFound(t.to_string()));
        let src = FakeProcSource::default();
        let err = locate_pane_with("sess", "red", windows, list, &src).unwrap_err();
        match err {
            LocatorError::TmuxSession(TmuxError::PaneNotFound(_)) => {}
            e => panic!("expected TmuxSession(PaneNotFound), got {e:?}"),
        }
    }

    // @spec PANE-LOC-006, PANE-LOC-007
    #[test]
    fn locate_no_claude_pane_when_no_pane_runs_claude() {
        let windows = |_s: &str| Ok(vec![win("red")]);
        let list = |_t: &str| Ok(vec![pane("%1", 10, "zsh"), pane("%2", 20, "vim")]);
        let mut src = FakeProcSource::default();
        src.add(10, Some("zsh"), Some(1), &[]);
        src.add(20, Some("vim"), Some(2), &[]);
        let r = locate_pane_with("sess", "red", windows, list, &src).unwrap();
        assert_eq!(r, Resolution::NoClaudePane);
    }

    // @spec PANE-LOC-006, PANE-LOC-008
    #[test]
    fn locate_single_claude_pane() {
        let windows = |_s: &str| Ok(vec![win("red")]);
        let list = |_t: &str| Ok(vec![pane("%1", 10, "zsh"), pane("%2", 20, "2.1.122")]);
        let mut src = FakeProcSource::default();
        src.add(10, Some("zsh"), Some(1), &[]);
        src.add(20, Some("claude"), Some(2), &[]);
        let r = locate_pane_with("sess", "red", windows, list, &src).unwrap();
        assert_eq!(
            r,
            Resolution::Ok {
                pane_id: "%2".into(),
                other_pane_ids: vec![],
            }
        );
    }

    // @spec PANE-LOC-009, PANE-LOC-014
    #[test]
    fn locate_multiple_claude_panes_picks_oldest() {
        // Both panes run claude; %1 started earlier. Pane %1's current_command is a
        // version string and %2's is "node" — neither is consulted; only exe wins.
        let windows = |_s: &str| Ok(vec![win("red")]);
        let list = |_t: &str| Ok(vec![pane("%1", 10, "2.1.122"), pane("%2", 20, "node")]);
        let mut src = FakeProcSource::default();
        src.add(10, Some("claude"), Some(50), &[]);
        src.add(20, Some("claude"), Some(90), &[]);
        let r = locate_pane_with("sess", "red", windows, list, &src).unwrap();
        assert_eq!(
            r,
            Resolution::Ok {
                pane_id: "%1".into(),
                other_pane_ids: vec!["%2".into()],
            }
        );
    }

    // @spec PANE-LOC-021
    #[test]
    fn locate_re_resolves_each_call_no_cache() {
        let windows = |_s: &str| Ok(vec![win("red")]);
        let list = |_t: &str| Ok(vec![pane("%1", 10, "zsh")]);

        let mut claude_src = FakeProcSource::default();
        claude_src.add(10, Some("claude"), Some(1), &[]);
        let first = locate_pane_with("sess", "red", windows, list, &claude_src).unwrap();
        assert!(matches!(first, Resolution::Ok { .. }));

        // Same target, a fresh source that no longer sees claude → the second call
        // reflects the new state rather than a cached "Ok".
        let mut gone_src = FakeProcSource::default();
        gone_src.add(10, Some("zsh"), Some(1), &[]);
        let second = locate_pane_with("sess", "red", windows, list, &gone_src).unwrap();
        assert_eq!(second, Resolution::NoClaudePane);
    }

    // ---- RealProcSource (PANE-LOC-019, 020) ----------------------------------

    // @spec PANE-LOC-019, PANE-LOC-020
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    #[test]
    fn real_source_reads_self() {
        let src = RealProcSource::new();
        let me = std::process::id();
        assert!(
            src.exe_basename(me).is_some(),
            "should read this test process's executable basename"
        );
        assert!(
            src.start_time(me).is_some(),
            "should read this test process's start time"
        );
    }

    // ---- locate_panes_with (batch, PANE-LOC-024, 025) ------------------------

    // @spec PANE-LOC-024
    #[test]
    fn locate_panes_shares_one_window_list_across_colors() {
        // The window list (and, in production, the host process scan) is fetched
        // once per batch, not once per color.
        let win_calls = Cell::new(0usize);
        let windows = |_s: &str| {
            win_calls.set(win_calls.get() + 1);
            Ok(vec![win("red"), win("blue")])
        };
        let list = |t: &str| {
            if t == "sess:red" {
                Ok(vec![pane("%1", 10, "node")])
            } else {
                Ok(vec![])
            }
        };
        let mut src = FakeProcSource::default();
        src.add(10, Some("claude"), Some(5), &[]);
        let out = locate_panes_with("sess", &["red", "blue"], windows, list, &src).unwrap();
        assert_eq!(
            win_calls.get(),
            1,
            "window list must be fetched once per batch"
        );
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].0, "red");
        assert_eq!(
            out[0].1.as_ref().unwrap(),
            &Resolution::Ok {
                pane_id: "%1".into(),
                other_pane_ids: vec![],
            }
        );
        assert_eq!(out[1].0, "blue");
        assert_eq!(out[1].1.as_ref().unwrap(), &Resolution::NoClaudePane);
    }

    // @spec PANE-LOC-024
    #[test]
    fn locate_panes_isolates_per_color_pane_errors() {
        // One color's list_panes failure must not sink the others.
        let windows = |_s: &str| Ok(vec![win("red"), win("blue")]);
        let list = |t: &str| {
            if t == "sess:red" {
                Err(TmuxError::PaneNotFound(t.to_string()))
            } else {
                Ok(vec![])
            }
        };
        let src = FakeProcSource::default();
        let out = locate_panes_with("sess", &["red", "blue"], windows, list, &src).unwrap();
        assert!(matches!(
            out[0].1,
            Err(LocatorError::TmuxSession(TmuxError::PaneNotFound(_)))
        ));
        assert_eq!(out[1].1.as_ref().unwrap(), &Resolution::NoClaudePane);
    }

    // @spec PANE-LOC-025
    #[test]
    fn locate_panes_session_missing_fails_whole_batch() {
        let windows = |s: &str| Err(TmuxError::SessionNotFound(s.to_string()));
        let src = FakeProcSource::default();
        let err = locate_panes_with("sess", &["red", "blue"], windows, no_panes, &src).unwrap_err();
        assert!(matches!(
            err,
            LocatorError::TmuxSession(TmuxError::SessionNotFound(_))
        ));
    }

    // ---- parse_lstart / days_from_civil (macOS start time, PANE-LOC-019) -----

    // @spec PANE-LOC-019
    #[test]
    fn parse_lstart_known_timestamp() {
        // 2026-07-26 16:54:03; fields treated as if UTC (ordering key), so this is
        // a fixed, concrete value independent of the host timezone.
        assert_eq!(
            parse_lstart("Sun Jul 26 16:54:03 2026"),
            Some(1_785_084_843)
        );
    }

    // @spec PANE-LOC-019
    #[test]
    fn parse_lstart_single_digit_day_double_space() {
        // `ps` pads a single-digit day with an extra space; split_whitespace
        // absorbs it, and an earlier day sorts strictly before a later one.
        assert_eq!(
            parse_lstart("Sun Jul  6 16:54:03 2026"),
            Some(1_783_356_843)
        );
        assert!(
            parse_lstart("Sun Jul  6 16:54:03 2026") < parse_lstart("Sun Jul 26 16:54:03 2026")
        );
    }

    // @spec PANE-LOC-019
    #[test]
    fn parse_lstart_rejects_malformed() {
        assert_eq!(parse_lstart("garbage"), None);
        assert_eq!(parse_lstart("Sun Jul 26 16:54:03"), None); // only 4 fields
        assert_eq!(parse_lstart("Sun Foo 26 16:54:03 2026"), None); // bad month
        assert_eq!(parse_lstart("Sun Jul 26 16:54 2026"), None); // missing seconds
    }

    // @spec PANE-LOC-019
    #[test]
    fn days_from_civil_epoch_anchors() {
        assert_eq!(days_from_civil(1970, 1, 1), 0);
        assert_eq!(days_from_civil(2000, 1, 1), 10_957);
        assert_eq!(days_from_civil(2026, 1, 1), 20_454);
    }

    // ---- parse_stat_field (Linux /proc/stat, PANE-LOC-020) -------------------

    // @spec PANE-LOC-020
    #[test]
    fn parse_stat_field_handles_comm_with_parens_and_spaces() {
        // comm = "sh (foo)" contains a space AND parentheses; field indexing must
        // key off the LAST ')'. Fields after comm: 3=state, 4=ppid, ..., 22=start.
        let mut fields = vec!["R".to_string(), "1000".to_string()]; // f3 state, f4 ppid
        for f in 5..=21 {
            fields.push(f.to_string()); // f5..f21 placeholders
        }
        fields.push("8675309".to_string()); // f22 starttime
        let content = format!("4242 (sh (foo)) {}\n", fields.join(" "));
        assert_eq!(parse_stat_field(&content, StatField::Ppid), Some(1000));
        assert_eq!(
            parse_stat_field(&content, StatField::StartTime),
            Some(8_675_309)
        );
    }

    // @spec PANE-LOC-020
    #[test]
    fn parse_stat_field_rejects_short_or_missing() {
        assert_eq!(parse_stat_field("no parens here", StatField::Ppid), None);
        // Has ')' but too few fields to reach starttime (field 22).
        assert_eq!(parse_stat_field("1 (x) R 2 3", StatField::StartTime), None);
    }
}

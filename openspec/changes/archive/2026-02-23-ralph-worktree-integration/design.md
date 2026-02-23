## Context

gbiv is a Rust CLI tool (using clap) that manages ROYGBIV git worktrees. The directory layout it creates is:

```
<project>/
  main/<project>/     ← main worktree (contains prd.json)
  red/<project>/
  orange/<project>/
  yellow/<project>/
  green/<project>/
  blue/<project>/
  indigo/<project>/
  violet/<project>/
```

Ralph is an autonomous AI agent loop that reads `prd.json`, picks the highest-priority incomplete story (`passes: false`), implements it in a fresh AI context, then marks it `passes: true`. It was designed for single-agent use. Running 7 instances in parallel across ROYGBIV worktrees requires an exclusive lock around the read-modify-write cycle of `prd.json`.

`git_utils::find_gbiv_root` already exists and can locate the project root from any worktree path.

## Goals / Non-Goals

**Goals:**
- Add `gbiv prd lock` — acquire an exclusive advisory lock on `prd.json`
- Add `gbiv prd unlock` — release the lock (only by the owning worktree)
- Lock is advisory: relies on agents cooperating, not OS-level file locking
- Stale lock detection: if the owning PID is no longer alive, the lock can be broken
- Works from any ROYGBIV worktree (uses `find_gbiv_root` to locate main)

**Non-Goals:**
- Kernel-level file locking (flock/fcntl) — cross-platform complexity not worth it
- Automatic unlock on crash — agents should handle cleanup; stale detection covers failures
- Modifying Ralph itself — gbiv is a wrapper, Ralph is unchanged
- Locking anything other than `prd.json`

## Decisions

### Decision 1: Lock file location — `main/<project>/.prd.lock`

**Chosen:** Sibling to `prd.json` in the main worktree.

**Alternatives considered:**
- Project root (`<project>/.prd.lock`): Farther from the resource being locked; feels less cohesive.
- Each worktree's own folder: Makes no sense — the lock is on a shared resource.

**Rationale:** Placing the lock file next to `prd.json` makes the association obvious and keeps both files in the same directory for atomic operations.

### Decision 2: Lock file format — JSON with `worktree` and `pid`

**Chosen:**
```json
{ "worktree": "blue", "pid": 12345 }
```

**Alternatives considered:**
- Plain text (just worktree name): Insufficient for stale detection.
- Include timestamp: Useful for age-based expiry but adds complexity; PID liveness check is simpler and more accurate.

**Rationale:** `worktree` identifies the owner for error messages; `pid` enables stale lock detection by checking if the process is still alive.

### Decision 3: Worktree identity — branch name from `git symbolic-ref`

**Chosen:** Read branch name from `git symbolic-ref --short HEAD` (already in `git_utils::get_quick_status`).

**Alternatives considered:**
- Infer from directory path: Brittle if user renames folders.
- Require `--worktree` flag: Forces callers to know their name; should be automatic.

**Rationale:** The branch name *is* the worktree color in the ROYGBIV model. It's authoritative and already retrieved elsewhere in gbiv.

### Decision 4: Lock acquisition — poll with timeout

**Chosen:** Retry every 500ms, fail after 30s (configurable via `--timeout` flag).

**Alternatives considered:**
- Fail immediately (no retry): Ralph agents would need their own retry loop — push complexity up.
- inotify/fsevents: Cross-platform complexity not justified for this use case.

**Rationale:** 30s is generous for a `prd.json` read-modify-write cycle. Polling at 500ms adds minimal overhead. Agents block naturally without extra orchestration.

### Decision 5: Stale lock handling — manual only via `--force` on unlock

**Chosen:**
- On `lock`: any existing `.prd.lock` blocks acquisition — no automatic stale detection or removal.
- On `unlock`: if not the owner → error by default; `--force` overrides.
- Corrupt lock files (invalid JSON) surface an immediate error with the path to delete manually.

**Rationale:** Auto-breaking stale locks based on PID liveness proved error-prone (PID reuse, cross-OS differences). The simpler model is: the lock file is the source of truth. If it's there, the lock is held. Users clear it explicitly with `gbiv prd unlock --force` when needed. This is predictable and requires no OS-specific process inspection.

## Risks / Trade-offs

- **Race on lock file creation** → Two processes check "no lock" simultaneously then both write. Mitigation: use `O_CREAT | O_EXCL` (atomic create-if-not-exists) via `OpenOptions::create_new(true)` in Rust. Only one writer wins.
- **Agent forgets to unlock** → Lock held until manually cleared. Mitigation: Agents should use shell `trap` or equivalent to call `gbiv prd unlock` on exit. Users can always break a stuck lock with `gbiv prd unlock --force`.
- **Corrupt lock file** → Blocks acquisition with an immediate error. Mitigation: Error message includes the full path so users know exactly what to delete.

## Migration Plan

No migration needed. This adds new subcommands under a new `prd` subcommand group. Existing `init` and `status` commands are unaffected. No data format changes.

## Open Questions

- Should `gbiv prd lock` print the lock path on success (useful for debugging)? Lean: yes, to stderr.
- Should timeout be configurable only via flag, or also via an env var (`GBIV_LOCK_TIMEOUT`)? Lean: flag only for now.

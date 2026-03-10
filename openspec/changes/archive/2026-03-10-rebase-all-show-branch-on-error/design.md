## Context

`gbiv rebase-all` iterates over color worktrees and rebases each onto `origin/main`. When a rebase fails due to a conflict, the `rebase_onto` function in `git_utils.rs` uses `.output()` to capture git's stderr, then returns it as an `Err(String)`. The caller in `rebase_all.rs` prints a formatted line with the color name.

However, `git rebase` also writes to **stdout** (e.g., "Could not apply ...", "Recorded preimage ..."). These stdout lines are not captured or displayed, yet they may leak through to the terminal in some environments. The result is that error context appears without a branch name prefix.

## Goals / Non-Goals

**Goals:**
- Ensure the branch/color name always appears on the error status line when a rebase fails
- Capture both stdout and stderr from the failed `git rebase` so no unprefixed output leaks
- Include the most useful line(s) from the error in the formatted status output

**Non-Goals:**
- Changing the overall output format for successful rebases
- Adding verbose/debug logging modes

## Decisions

1. **Capture both stdout and stderr in `rebase_onto`**: The function already captures stderr. It should also include relevant stdout content in the error string so the caller has full context. Combine stderr and stdout (stderr first) into the returned error string.

2. **Truncate error to first meaningful line in the status output**: The `rebase_all.rs` caller should print a single-line summary with the color name (e.g., `yellow    rebase failed: could not apply 69957f7...`). The full multi-line error can be printed as indented detail lines below.

3. **Abort failed rebases**: The current code already aborts failed rebases (`git rebase --abort`) to leave worktrees clean. This behavior is correct and should be preserved.

## Risks / Trade-offs

- [Risk] Combining stdout+stderr may produce redundant text → Mitigation: Take the first non-empty line from stderr as the summary; include full output only in detail lines.
- [Risk] Output format change could break scripts parsing the output → Mitigation: The primary status line format (`color  rebase failed: ...`) is preserved; only the detail lines below are new.

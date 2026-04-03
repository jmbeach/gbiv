## Context

gbiv manages multiple git worktrees named after ROYGBIV colors. The existing `cleanup` command can reset a worktree, but only if the feature branch has been merged into remote main. Users need a way to forcefully reset a worktree regardless of merge status — for abandoned experiments, stale branches, or quick resets.

The codebase already has all the git utility functions needed: `checkout_branch`, `reset_hard`, `get_remote_main_branch`, and `find_gbiv_root`.

## Goals / Non-Goals

**Goals:**
- Provide a single command to reset any color worktree to a clean state matching remote main
- Infer the current color when no argument is provided
- Reuse existing git utility functions

**Non-Goals:**
- Modifying GBIV.md entries (that's `cleanup`'s responsibility)
- Adding confirmation prompts or `--force` flags (the command is explicitly destructive by design — the user asked for "regardless of state")
- Handling the main worktree (reset only applies to color worktrees)

## Decisions

### 1. Color inference from current directory
When `<color>` is omitted, determine which color worktree the user is in by examining the directory path against `find_gbiv_root()` and matching the path segment to a known color.

**Rationale:** Consistent with how `cleanup` works when called without an argument — it operates on the current worktree's color.

### 2. No merge check
Unlike `cleanup`, `reset` skips the `is_merged_into` check entirely. This is the core differentiator.

**Rationale:** The entire purpose of this command is to reset regardless of merge status. The name "reset" implies returning to a known-good state.

### 3. Reuse cleanup's core git operations
The actual git operations (checkout color branch → reset hard to remote main) are identical to what `cleanup` does after its merge check. Extract or directly call the same sequence: `checkout_branch` then `reset_hard` to `get_remote_main_branch`.

**Rationale:** Avoids duplication and ensures consistent behavior.

### 4. Error when not in a gbiv project or unknown color
If `find_gbiv_root()` fails or the specified color is invalid, return a descriptive error. If the color worktree directory doesn't exist, error with guidance.

**Rationale:** Fail fast with clear messages rather than silently doing nothing.

## Risks / Trade-offs

- **[Data loss]** → This is intentional. The command is explicitly destructive. Uncommitted and unpushed work on the color worktree will be lost. This matches the user's stated requirement ("regardless of state of things"). No mitigation needed beyond the command name being clear about its purpose.
- **[Divergence from cleanup]** → The git operation sequence is the same but without the merge guard. If cleanup's reset logic changes, reset should change too. Mitigation: share the core reset logic or keep both implementations simple enough that divergence is unlikely.

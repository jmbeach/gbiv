## Why

When `gbiv rebase-all` encounters a rebase conflict, the raw git error output is printed without identifying which branch failed. The user has to infer the failing branch by checking which color is missing from the formatted output. The branch name should be clearly shown alongside the error.

## What Changes

- Ensure the branch/color name is printed on the error line when a rebase fails, so the output consistently shows `<color>  rebase failed: <details>` even when git emits multi-line error output
- Capture and prefix or suppress raw git stderr/stdout so it doesn't appear unprefixed between the formatted branch status lines

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `rebase-all`: Error output must include the branch name so the user can identify which worktree had the conflict

## Impact

- `src/commands/rebase_all.rs` — error display formatting
- `src/git_utils.rs` — `rebase_onto` may need to ensure all subprocess output is captured (not inherited)

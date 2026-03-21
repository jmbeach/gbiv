## Context

The `gbiv cleanup` command iterates over all color worktrees and resets ones whose feature branch has been merged. Currently, the only output is for skipped worktrees ("already on the color branch"). Worktrees that are actually cleaned up produce no output, making it unclear what happened.

## Goals / Non-Goals

**Goals:**
- Print a success message when a worktree is cleaned up, including the previous branch name
- Keep existing skip message but ensure it's clear

**Non-Goals:**
- Changing cleanup logic or behavior
- Adding verbose/quiet flags
- Changing error output format

## Decisions

1. **Add println after successful cleanup in `cleanup_one`**: After `reset_hard` succeeds and before the GBIV.md update, print a message like `"red worktree cleaned up (was on branch-name), reset to origin/main"`. This keeps the message close to the action and uses information already available in local variables (`branch` and `remote_main`).

2. **Keep skip message as-is**: The existing skip message `"{color} worktree is already on the {color} branch, skipping"` already shows the branch. The user's example output confirms this is the desired format.

## Risks / Trade-offs

- Minimal risk — adding print statements only, no logic changes.

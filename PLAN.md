Brainstorm a new "status" command for gbiv.

Basic requirements:

For each ROYGBIV worktree, print out the worktree name (the color), the name of the branch that's checked out, the clean/dirty status. If the branch name is the worktree's color (ex: worktree name "blue", branch "blue") then don't include any more information. Otherwise, also include:

- Whether the branch is merged into main/develop (or master) already
- How many days old the last commit was
- Ahead / behind remote

---

Execution context: Status should be able to be ran from any worktree of a repo (not a folder argument)

Main branch detection: Use the same main branch detection logic that init uses

Output format: Plain text with color coding (ANSI)

"Dirty" definition: Should dirty include: all of the above

Remote tracking: Show "no upstream"

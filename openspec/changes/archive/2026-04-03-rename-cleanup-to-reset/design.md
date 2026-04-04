## Context

The `gbiv cleanup` command resets color worktrees back to the main branch after feature branches are merged. The name "cleanup" is vague — "reset" better describes the action and aligns with git terminology. This is a straightforward rename with no behavioral changes.

## Goals / Non-Goals

**Goals:**
- Rename the `cleanup` subcommand to `reset` across CLI, code, tests, and docs
- Update all user-facing strings (help text, error messages, output) to use "reset"

**Non-Goals:**
- Changing any behavior or logic of the command
- Adding backwards compatibility aliases (no `cleanup` → `reset` redirect)
- Modifying the openspec archived changes (they reflect historical state)

## Decisions

### 1. Clean rename with no alias

Rename directly without keeping `cleanup` as a hidden alias. This is a developer tool with a small user base, so a clean break is preferable to maintaining compatibility shims.

**Alternative considered**: Keep `cleanup` as a deprecated alias — rejected because it adds unnecessary complexity for a small-audience CLI tool.

### 2. Rename file and all identifiers

Rename `src/commands/cleanup.rs` → `src/commands/reset.rs` and update all function names (`cleanup_one` → `reset_one`, `cleanup_all_to_vec` → `reset_all_to_vec`, `cleanup_command` → `reset_command`). This keeps the codebase consistent and avoids confusion between the command name and internal identifiers.

### 3. Update existing specs in-place at archive time

The delta specs in this change will be archived into the existing `gbiv-cleanup` and `cleanup-output` spec directories with RENAMED requirements, so the canonical specs reflect the new naming.

## Risks / Trade-offs

- **[Breaking change]** → Users must update muscle memory and any scripts. Mitigated by the small user base and the simplicity of the rename.
- **[Spec rename overhead]** → Multiple requirements reference "cleanup" by name. Mitigated by using RENAMED operations in delta specs to handle this systematically.

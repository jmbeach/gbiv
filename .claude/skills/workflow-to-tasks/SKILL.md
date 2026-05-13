---
name: workflow-to-tasks
description: Materialize a workflow TOML file (like those in `.workflows/`) into a tree of tasks. Reads the workflow definition, prompts for any required variables, substitutes `{{var}}` placeholders in titles and descriptions, calls TaskCreate for each step in dependency order, and wires up blocking relationships from each step's `needs` array. Use when the user says "run the arrow workflow", "kick off the epic-design workflow", "turn .workflows/foo.toml into tasks", or any variation of materializing a workflow file into FleetView tasks.
argument-hint: <workflow-file> [var=value ...]
allowed-tools: Read, Bash, AskUserQuestion, ToolSearch, TaskCreate
---

# workflow-to-tasks skill

Materializes a workflow TOML (the format used in `.workflows/`) into a connected set of FleetView tasks via the `TaskCreate` tool, wiring up dependencies from each step's `needs` array.

## Invocation

The user's request is: **$ARGUMENTS**

The first positional argument is the workflow file path (e.g., `.workflows/arrow.toml`). Any additional `key=value` arguments pre-fill workflow variables. Examples:

- `/workflow-to-tasks .workflows/arrow.toml`
- `/workflow-to-tasks .workflows/arrow.toml feature_name="gbiv-core" plan_file="docs/gbiv/planning/foo.md"`
- `/workflow-to-tasks .workflows/epic-design.toml epic_name="roy" epic_id="abc-123"`

If `$ARGUMENTS` is empty, ask the user which workflow file to materialize. Offer the contents of `.workflows/` as the obvious candidate set.

## Workflow file format

Workflow files are TOML. Schema (see `.workflows/arrow.toml` and `.workflows/epic-design.toml` for live examples):

```toml
workflow = "<name>"              # required, string
description = "..."              # required, string (may contain {{vars}})
version = 1                      # required, int

[vars.<name>]                    # zero or more
description = "..."
required = true                  # if absent, treat as optional

[[steps]]                        # one or more, in declaration order
id = "step-id"                   # required, unique within the file
title = "..."                    # required (may contain {{vars}})
description = """..."""          # optional (may contain {{vars}})
type = "human"                   # optional, defaults to "ai"
needs = ["other-step-id", ...]   # optional, defaults to []
```

`{{var}}` placeholders may appear in `description` (workflow-level), each step's `title`, and each step's `description`. They are substituted with the resolved variable values.

## Procedure

### 1. Load and parse

1. Read the workflow file.
2. Parse TOML. If parsing fails, print a clear error pointing to the offending file and stop.
3. Validate:
   - `workflow`, `description`, `version` exist at the top level
   - `version == 1` (the only schema this skill knows). If higher, refuse with `workflow file is version <N>; this skill supports up to version 1`.
   - Every `[[steps]]` has a unique `id` and a non-empty `title`
   - Every entry in any `needs` array references a known step `id`
   - The `needs` graph has no cycles (depth-first check, error on back-edge)

### 2. Resolve variables

Build the set of variables needed: every `[vars.*]` table plus any `{{name}}` token appearing in `description` or in step `title`/`description` (warn if a token references an undeclared variable — still treat it as a variable to resolve).

For each variable, in declaration order:

1. If it was supplied as a `key=value` argument, use that value (strip surrounding quotes if present).
2. Otherwise, if the variable is marked `required = true` *or* it appears in any string that will be substituted, prompt the user for it via `AskUserQuestion`. Use the variable's `description` as the question text. Accept any non-empty string.
3. Optional variables with no value and no `{{}}` references can be skipped (rare).

Variable substitution is literal: replace every occurrence of `{{name}}` (no surrounding whitespace tolerance — matches the TOML examples) with the resolved value. Re-substitute strings that themselves came from a variable value, in case of nested `{{}}` (unlikely but cheap to support — apply substitution iteratively until no more `{{}}` tokens remain or until a fixed point is reached; abort with an error after 10 iterations to prevent infinite loops).

### 3. Topologically order steps

Order steps so that every step appears after the steps in its `needs` array. Use a stable topological sort: among ready steps, prefer the one declared earliest in the file. This keeps task IDs roughly aligned with file order when no dependencies are involved.

### 4. Discover the TaskCreate schema

Before calling `TaskCreate`, confirm the tool's actual parameters at runtime using `ToolSearch` with `select:TaskCreate`:

```
ToolSearch(query="select:TaskCreate", max_results=1)
```

Read the returned schema to find the actual parameter names for:

- The **title** parameter (likely `title` or `name`)
- The **description** / body parameter (likely `description`, `body`, or `details`)
- The **dependency** parameter (could be `blockers`, `blocked_by`, `depends_on`, `predecessors`, `needs`, or similar — pick the one that matches the workflow's `needs` semantics)
- Any **parent** parameter, if relevant

If the schema does not expose a dependency parameter, fall back to:

1. Create all tasks first (capturing the assigned task ID for each step).
2. After creation, search for a `TaskUpdate` or similar tool that can set dependencies. Use `ToolSearch(query="select:TaskUpdate", max_results=1)`.
3. If still no dependency mechanism exists, emit a clear warning to the user listing the intended dependency edges (step `A` → blocks step `B`) so they can wire them manually. Do not silently drop the relationships.

### 5. Create tasks

Iterate the topologically ordered step list. For each step:

1. Apply variable substitution to `title` and `description`.
2. Resolve the dependency parameter: map each id in `needs` to the task ID returned by the earlier `TaskCreate` call for that step. (You must have a step-id → task-id map; build it as you go.)
3. Build the `TaskCreate` arguments. Include a clearly labelled prefix or suffix indicating the workflow + step id for traceability — append `\n\n(workflow: <workflow-name>, step: <step-id>, type: <human|ai>)` to the description.
4. Call `TaskCreate`.
5. Record the returned task ID under the step's `id`.

When you encounter a `type = "human"` step, still create the task — flag it in the description so a downstream agent or human knows not to attempt the work autonomously.

If a `TaskCreate` call fails, stop immediately. Print:
- The step `id` that failed
- The full title/description that was sent
- The tool error
- The step-id → task-id map for all successful creations so far, so the user can clean up partial state.

Do **not** retry automatically — task creation is not idempotent and a retry could create duplicates.

### 6. Report

After all tasks are created, print a summary listing:

- Workflow name and the resolved variable values
- One line per created task: `<step-id> → <task-id>: <title>`
- The dependency edges that were wired (`<step-id> blocks <step-id>`)
- Any human steps (so the user knows where the workflow has manual gates)
- Whether any dependency edges were left unwired (and why)

## Edge cases

- **Steps with no `needs`**: create them with no dependencies. Multiple steps may have no needs (they become parallel starting points).
- **Variable referenced in a step but missing from `[vars.*]`**: warn and prompt for it as if it were declared with `required = true` and an empty description.
- **Workflow file is in a worktree other than the gbiv root**: the path the user gives is what you use; don't try to "find" it.
- **Substitution leaves stray `{{...}}` tokens**: error out before any `TaskCreate` call. Show the offending text so the user can identify the missing variable.
- **`needs` references a step that comes later in the file**: that's fine — the topological sort handles it. Don't reject based on file order.
- **Multiple workflow files in one invocation**: not supported. Run the skill once per file.

## Non-goals

- This skill does **not** execute the workflow's steps. It only materializes them as tasks. Running them is a separate concern (driven by whatever agent or human picks up the tasks).
- It does not validate that the steps' described actions are sensible — that's the workflow author's responsibility.
- It does not delete or update existing tasks. If a workflow has already been materialized and re-run is needed, the user should clean up the prior tasks first.
- It does not write to the workflow file. The TOML is read-only input.

## Example session

User: `/workflow-to-tasks .workflows/arrow.toml feature_name="gbiv-core-extract" plan_file="docs/gbiv/planning/gbiv-core-extract-2026-05-07.md"`

The skill:
1. Reads `.workflows/arrow.toml` (the file shown in the project).
2. Both required vars are pre-filled — no prompts.
3. Substitutes `{{feature_name}}` and `{{plan_file}}` into all step titles/descriptions.
4. Topologically orders the 17 steps. `plan` comes first; `gbiv-active` depends on it; etc.
5. Looks up the TaskCreate schema.
6. Creates 17 tasks, each blocked by the tasks corresponding to its `needs` entries.
7. Prints a summary mapping step IDs to task IDs.

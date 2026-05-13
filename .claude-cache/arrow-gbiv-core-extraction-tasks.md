# Materialized workflow: arrow

_gbiv-core extraction Feature implementation using /linked-intent-dev to plan and TaskCreate/TaskUpdate to track work_

## Resolved variables

- `feature_name` = `gbiv-core extraction`
- `plan_file` = `docs/gbiv/planning/gbiv-core-extract-2026-05-07.md`

## Tasks (22 total, topologically ordered)

### 1. `plan` — [gbiv-core extraction] Plan the feature and create artifacts using /linked-intent-dev

- **type**: `human`
- **needs**: _(none)_

Once the HLD, LLD(s), SPECs, and a plan file are generated, you're ready to move past this plan.
Technically this is a pre-requisite to running this workflow as the workflow takes plan file path as a parameter.
So really, you should be able to mark this done right away, but this is here more as documentation for how to
use this workflow.

### 2. `gbiv-active` — [gbiv-core extraction] GBIV set task in-progress

- **type**: `ai`
- **needs**: `plan`

Set the GBIV worktree task to active by running 'gbiv mark --in-progress'

### 3. `cbcp` — [gbiv-core extraction] Create a branch, commit, and push (use /cbcp)

- **type**: `ai`
- **needs**: `gbiv-active`

Create a new branch for this work. Then commit and push (use /cbcp)
If the user doesn't have a cbcp skill just do the following:
Your role is to help the user quickly create a new git branch, commit their changes, and push to the remote repository.
Don't prompt the user for any of these values. Use your best judgement to decide for them unless they pass in specific values
Put code backticks around code blocks.

### 4. `create-test-tasks` — [gbiv-core extraction] Create Test Tasks

- **type**: `ai`
- **needs**: `cbcp`

Looking at the plan document that was created for this branch (Plan file: docs/gbiv/planning/gbiv-core-extract-2026-05-07.md),
create tasks (TaskCreate) for TDD (one task per test we need) under this task.
In general, we should have at least one test per spec.
Each task should have Acceptance Criteria that the tests should fail!
That's how TDD works.
DO NOT WRITE THE IMPLEMENTATION TO PASS THE TESTS: We'll do that later.

### 5. `create-tests` — [gbiv-core extraction] Spawn subagents to create the tests

- **type**: `ai`
- **needs**: `create-test-tasks`

Use TaskGet on the "Create Test Tasks" task to see what tests need created.

Write failing tests in parallel by spawning subagents. Follow these rules:

- Count the number of test tasks (subtasks) under the parent task.
- Spawn ceil(task_count / 5) subagents, maximum 10.
- Start each agent with model "sonnet".

WORK PARTITIONING (file ownership):
- Assign each agent a disjoint set of test tasks. No two subagents share a task.
- Each subagent OWNS the test files for their assigned tasks. No other subagent may edit those files.

SPAWN PROMPT (include all of this for every subagent):
- The /linked-intent-dev artifact paths for this branch (list them explicitly - the ones with plan file: docs/gbiv/planning/gbiv-core-extract-2026-05-07.md).
- The test framework and conventions used in this project (detect from existing tests).
- The specific task IDs and descriptions assigned to this subagent.
- The exact file paths this subagent is responsible for creating.
- The instruction: "Write tests that FAIL. Do not write any implementation code."

WORKFLOW:
1. Partition tasks across subagents and spawn them with detailed prompts.
2. Subagents work in parallel, each writing tests only for their assigned tasks/files.
3. After all subagents finish, do a consistency review across all test files
   to check for duplicated setup, inconsistent naming, or missing coverage.
4. Mark each task as done (TaskUpdate) only after verifying the tests exist and fail.
5. Ensure each test has a comment with the spec ID it covers (e.g., "@spec IMPORT-002").
   IMPORTANT: Do NOT include task IDs in code comments — they are
   ephemeral and meaningless outside this session. Only use the spec IDs from the spec files.

DO NOT WRITE THE IMPLEMENTATION TO PASS THE TESTS: We'll do that later.

### 6. `draft-pr-tests` — [gbiv-core extraction] Open a draft PR for test review

- **type**: `ai`
- **needs**: `create-tests`

Commit the test files and push, then open a draft PR so the tests can be reviewed on GitHub.

1. Stage and commit only the test files added/modified in the create-tests step.
   Commit message: "test(wip): failing tests for gbiv-core extraction"
2. Push the branch.
3. Create a draft PR with title "[WIP] gbiv-core extraction — failing tests" and a body that lists:
   - The test files added/modified
   - The spec IDs covered
   - A note: "Draft — tests only. Implementation not started."
4. Print the PR URL clearly so it can be captured.

### 7. `review-tests` — [gbiv-core extraction] Manually review the generated tests

- **type**: `human`
- **needs**: `draft-pr-tests`

This is a human step. The user reviews the draft PR on GitHub (opened in the previous step).

Print the PR URL again clearly, then wait for the user's feedback.

The user will check that the tests:
- Properly reference the spec IDs from the /linked-intent-dev artifacts
- Cover all the necessary requirements and edge cases
- Follow the project's testing conventions
- Actually fail as expected (TDD principle)
- Are well-organized and maintainable

If the user requests changes, make them, push to the same branch, and the draft PR will update automatically.
Once the user approves, proceed to implementation.

### 8. `create-implementation-tasks` — [gbiv-core extraction] Create Implementation Tasks

- **type**: `ai`
- **needs**: `review-tests`

Looking at the tests that were created for this branch (See the create-test-tasks task implemented previously),
create tasks (TaskCreate) that satisfy the implementation. Use this task (the "Create Implementation Tasks" task)
as the parent task for the new tasks.

The descriptions / acceptance criteria for the new implementation tasks should
be based on the /linked-intent-dev artifacts created for this branch (the ones with plan file docs/gbiv/planning/gbiv-core-extract-2026-05-07.md) and
the tests that have already been created.

### 9. `implement` — [gbiv-core extraction] Run subagents to implement the implementation tasks.

- **type**: `ai`
- **needs**: `create-implementation-tasks`

Use TaskGet to find children of the "Create Implementation Tasks" task to see the tasks that need implemented.

Create subagents to implement the feature in parallel. Follow these rules:

- Count the number of implementation tasks under the "Create Implementation Tasks" task.
- If task_count <= 5, spawn 1 subagent. Otherwise spawn ceil(task_count / 5) subagents, maximum 10.
- Start each subagent with model "opus".

WORK PARTITIONING (file ownership):
- Assign each subagent a disjoint set of implementation tasks.
- Each subagent OWNS the source files for their assigned tasks. No two subagents edit the same file.
- If two tasks require changes to the same file, assign them to the same subagent.
- You must state the file ownership mapping in each subagent's spawn prompt.

SPAWN PROMPT (include all of this for every subagent):
- The /linked-intent-dev artifact paths for this branch (list them explicitly - the ones with plan file docs/gbiv/planning/gbiv-core-extract-2026-05-07.md).
- The paths to the failing test files relevant to this subagent's tasks.
- The specific task IDs and descriptions assigned to this subagent.
- The exact source file paths this subagent is responsible for.
- The instruction: "Make the failing tests pass. Do not modify test files."

WORKFLOW:
1. Partition tasks, map file ownership, and spawn subagents with detailed prompts.
2. Subagents implement in parallel, each only touching their own files.
3. After all subagents finish, run the full test suite.
4. If tests fail, identify and fix spawning new subagents if appropriate.
5. Mark each task as done (TaskUpdate) only after its tests pass.
6. Ensure implementation methods have a comment with the spec ID(s) it covers when appropriate (e.g., "@spec IMPORT-002").

### 10. `vet` — [gbiv-core extraction] Fix any lints, warnings, complier errors, etc

- **type**: `ai`
- **needs**: `implement`

Look at watch.log and watch-tests.log. These should have a timestamp at the bottom that indicates
when it was last ran. If not recent, prompt the user to run 'make watch-build' and 'make watch-tests'.
Once those are running, address any lint issues, warnings / errors of any kind, failing tests, etc.
Address them even if they weren't made by us in this session.
If tests are failing, DO NOT fix by modifying the tests! Modify the implementation or alert the user
if it seems that there were gaps in the plan vs the implementation.

### 11. `codereview` — [gbiv-core extraction] Commit any unstaged changes then run the /codereview skill

- **type**: `ai`
- **needs**: `vet`

### 12. `fix-review` — [gbiv-core extraction] Address code review feedback

- **type**: `ai`
- **needs**: `codereview`

Scan the codebase for comments containing "AI_REVIEW".
If there are no findings, skip this step.

If there are findings that need to be addressed:

Use subagents (TaskCreate tool) to fix findings in parallel.

GROUPING:
- Parse AI_REVIEW findings and group them by file (or group of closely related files).
- Spawn one subagent per group. No two subagents should touch the same file.

SUBAGENT PROMPT (include all of this for every subagent):
- The specific AI_REVIEW comments assigned to this subagent (paste the full text).
- The file paths this subagent is responsible for.
- The instruction: "Address these review findings. Do not modify files outside your assignment."

WORKFLOW:
1. Group findings by file.
2. Spawn all subagents in parallel (multiple TaskCreate tool calls in a single message).
3. When all subagents return, review the changes for correctness.

### 13. `vet-again` — [gbiv-core extraction] Fix any lints, warnings, complier errors, etc again now that code is reviewed.

- **type**: `ai`
- **needs**: `fix-review`

Look at watch.log and watch-tests.log. These should have a timestamp at the bottom that indicates
when it was last ran. If not recent, prompt the user to run 'make watch-build' and 'make watch-tests'.
Once those are running, address any lint issues, warnings / errors of any kind, failing tests, etc.
Address them even if they weren't made by us in this session.
If tests are failing, DO NOT fix by modifying the tests! Modify the implementation or alert the user
if it seems that there were gaps in the plan vs the implementation.

### 14. `verify` — [gbiv-core extraction] Verify the entirety of the plan was implemented

- **type**: `ai`
- **needs**: `vet-again`

Look at plan file docs/gbiv/planning/gbiv-core-extract-2026-05-07.md again to ensure that all tasks are complete.
Check off everything that was completed and warn about anything that wasn't.
Also look at the specs created in this effort and see if any are unchecked.
Look to see if they were not implemented.

### 15. `manual-verify` — [gbiv-core extraction] Manually verify that the implementation actually works

- **type**: `human`
- **needs**: `verify`

This is a human step. Output a manual validation plan for how the human in the loop can verify the functionality of
the changes.
Write the manual implementation plan to a file in .claude-cache.
Wait for the user's feedback before proceeding.
If the user's feedback includes significant gaps in the design, advise them that the best solution is actually to go back to the design step.
If we don't go back to the design step, we risk gaps in intent in our design that will persist down the line.
If they agree, walk them through abandoning the implementation work and get them back to where only HLD / LLD modifications have been made.
Help guide them to the pieces of the HLD (if applicable) / LLD(s) that need modified.

### 16. `delete-plan` — [gbiv-core extraction] Get rid of plan file

- **type**: `ai`
- **needs**: `manual-verify`

The plan file (docs/gbiv/planning/gbiv-core-extract-2026-05-07.md) isn't really useful outside of the scope of the implementation work.
Delete it.

### 17. `update-docs` — [gbiv-core extraction] Update docs

- **type**: `ai`
- **needs**: `delete-plan`

Looking at everything that changed since we forked from the main branch, ensure code is well-documented.
For features that are user-facing or client/consumer-facing, update appropriate documentation such as README.md
or more granular documentation as appropriate. Do not over-document internal implementation details.

### 18. `push` — [gbiv-core extraction] Commit (if there's any unstaged changes), push, convert draft PR to ready

- **type**: `ai`
- **needs**: `update-docs`

Stage and commit any remaining unstaged changes, then push.

A draft PR was already opened in the draft-pr-tests step. Convert it to ready for review:
  gh pr ready

Do NOT open a new PR. If for some reason no draft PR exists, open one now.

### 19. `manual-pr-review` — [gbiv-core extraction] Manually review the PR.

- **type**: `human`
- **needs**: `push`

This is a human step. The user needs to actually look at the PR and make sure it's in good shape before proceeding to merge.
Ask the user to do this and then STOP and wait for their instructions.

### 20. `update-backlog` — [gbiv-core extraction] Update the backlog item

- **type**: `ai`
- **needs**: `manual-pr-review`

Remove the backlog item from backlog/backlog.md - along with corresponding entry in backlog/items/ if it exists.
Ammend commit and push this change.

### 21. `merge` — [gbiv-core extraction] Merge

- **type**: `human`
- **needs**: `update-backlog`

Merge the PR.
This is a human step. Use the AskUserQuestion tool to ask the user if they are ready to merge.

### 22. `gbiv-mark-done` — [gbiv-core extraction] mark gbiv item done

- **type**: `ai`
- **needs**: `merge`

Mark the GBIV worktree task as completed by running 'gbiv mark --done'

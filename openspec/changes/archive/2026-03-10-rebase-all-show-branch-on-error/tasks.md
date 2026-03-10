## 1. Capture full git output in rebase_onto

- [x] 1.1 Update `rebase_onto` in `src/git_utils.rs` to combine both stdout and stderr into the returned error string when rebase fails
- [x] 1.2 Add a test for `rebase_onto` verifying the error string includes content from both streams

## 2. Format error output with branch name

- [x] 2.1 Update the rebase error handling in `src/commands/rebase_all.rs` to print the color name on the status line with a one-line error summary
- [x] 2.2 Print remaining error detail lines indented below the status line
- [x] 2.3 Verify `cargo test` passes

## 3. Manual verification

- [x] 3.1 Confirm that a rebase conflict produces output like `yellow    rebase failed: could not apply ...` with detail lines indented below

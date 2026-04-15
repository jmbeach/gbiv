# Update `mark` for Multi-User

Modify `src/commands/mark.rs` to support the new `[assigned]` status.

## New Flag
- `--assigned`: mark the feature as `[assigned]`, alongside existing
  `--done`, `--in-progress`, `--unset`

No other resolution changes — `mark` still operates on the `[color]` entry
(explicit color arg or inferred from CWD).

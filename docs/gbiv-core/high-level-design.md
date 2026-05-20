# gbiv-core — High-Level Design

**Created**: 2026-05-16
**Status**: Brownfield seed (most components inferred from code; tmux-primitives is greenfield)

## Problem

The `gbiv` CLI and the `roy` daemon both need a small number of identical operations: discovering the gbiv project root, reasoning about ROYGBIV color names, managing local git-ignore entries, classifying tmux availability and sessions. Re-implementing those in each binary risks drift — a subtle disagreement about "what is a gbiv project?" or "is tmux installed?" would surface as confusing cross-binary bugs.

## Approach

A workspace-only library crate, `gbiv-core`, owns the primitives that must be answered the same way by every gbiv-family binary. Both `gbiv` and `roy` depend on it directly. The crate is not published; it is an implementation detail of the workspace.

`gbiv-core` does not own:

- Workflow orchestration (per-binary).
- HTTP, CLI, or skill surfaces (per-binary).
- Anything stateful — every primitive is either pure or wraps a single subprocess call.

The crate's design discipline is **typed errors and zero `anyhow`**: every module exports a `thiserror`-derived enum so callers can match on variants. Consumer binaries layer `anyhow` on top when they collapse a typed error into a user-facing message.

## Target Users

`gbiv-core` has exactly two callers: the `gbiv` binary and the `roy` daemon. There is no third caller and no plan for one. If a future binary needs the same primitives, it joins the workspace.

This narrow consumer set means `gbiv-core` does not need a stable public API in the cargo sense — breaking changes propagate to the workspace and are fixed in the same PR.

## Goals

- Each primitive returns the same answer regardless of which binary invokes it.
- Typed errors at every public boundary.
- Zero shared mutable state between callers.
- New shared primitives are cheap to add — one module, one error enum (or shared `TmuxError`-style enum where multiple primitives share failure modes), one trip through the standard LID phases.
- Speculative shared primitives are visibly speculative and have a removal trigger.

## Non-Goals

- **No published crate.** `publish = false`. The crate is workspace-local.
- **No async.** Every primitive is synchronous. Callers wrap in their own runtime if needed.
- **No state, no caching.** Each call re-evaluates. Callers that need caching add it at their layer.
- **No orchestration.** `gbiv-core` provides primitives; binaries compose them.
- **No CLI or HTTP surface.** Library only.
- **No third-party process wrappers beyond what the consumers both need.** See *Bar for inclusion* under Key Design Decisions for the exact rule.

## System Design

```
   ┌───────────────┐                   ┌───────────────┐
   │  gbiv binary  │                   │  roy daemon   │
   └───────┬───────┘                   └───────┬───────┘
           │                                   │
           └─────────────────┬─────────────────┘
                             ▼
                     ┌───────────────┐
                     │   gbiv-core   │
                     └───────┬───────┘
                             │
     ┌───────────┬───────────┼───────────┬───────────┐
     ▼           ▼           ▼           ▼           ▼
┌─────────┐ ┌─────────┐ ┌─────────┐ ┌─────────┐ ┌─────────┐
│ colors  │ │  error  │ │gitignore│ │  root   │ │  tmux   │
└─────────┘ └─────────┘ └─────────┘ └─────────┘ └─────────┘
```

Module-by-module purpose and LLD pointers are in the *Modules* table below.

### Modules

| Module | Purpose | LLD |
|---|---|---|
| `colors` | ROYGBIV constants, color validation. | *(not yet specified — brownfield)* |
| `error` | `CoreError` enum (currently wraps `io::Error`). | *(not yet specified — brownfield)* |
| `gitignore` | Local git-exclude file management for `.git/info/exclude`. | *(not yet specified — brownfield)* |
| `root` | Walks the filesystem to find a gbiv project root. | *(not yet specified — brownfield)* |
| `tmux` | Shared tmux primitives: `tmux_available`, `has_session`, `list_windows`, `session_name_for_root`. Owns the `TmuxError` enum. | `docs/gbiv-core/llds/tmux-primitives.md` |

Most modules are unmapped — they existed before linked-intent-development was applied to this workspace. They mature in place through normal LID cascades; this HLD is honest about what is and isn't specified.

## Key Design Decisions

### Bar for inclusion

A primitive belongs in `gbiv-core` when both binaries need it today. New primitives are added strictly on this rule.

There is one explicit relaxation: a primitive may land in `gbiv-core` for a **single consumer** if it is annotated `//! SPECULATIVE: <reason; second-consumer trigger>` at the module level. Speculative modules carry a removal trigger in the annotation (typically a backlog item ID or a date). If the trigger passes without the second consumer materializing, the next LID cascade that touches the module moves it back into the originating binary. Speculative-tagged code does not count toward "tests in gbiv-core mean both binaries depend on this" — only un-tagged code carries that invariant.

The four primitives this HLD seeds (`tmux_available`, `has_session`, `list_windows`, `session_name_for_root`) all clear the strict bar — gbiv exercises them today via inline tmux calls; roy will exercise them as soon as the next backlog item lands.

**Alternatives considered:**
- *Strict-only ("both binaries need it today").* Rejected: forces multiple roy-only primitives to be written twice (once in roy, then promoted), and the round-trip wastes time when the second consumer is already designed.
- *Relaxed ("shared today or shared soon").* Rejected: "soon" rots. Speculative tagging makes the exception visible at the code level rather than relying on disciplined headers.

### Library crate, not binary feature flag

A separate crate is more typing than feature-gating shared modules inside one binary, but it makes the contract explicit. Both binaries link against the same compiled artifact, and the `pub` surface of `gbiv-core` is the unambiguous boundary between "shared" and "per-binary."

**Alternatives considered:**
- *Feature-gated modules in `gbiv` that `roy` imports as a path dependency.* Rejected: blurs which code is shared, and dependency cycles become possible.
- *Duplicate the primitives in each binary.* Rejected: drift is the exact problem this crate exists to prevent.

### Typed errors, never `anyhow`

`gbiv-core` modules return `Result<T, ModuleError>` where `ModuleError` is a `thiserror`-derived enum. Consumer binaries are free to convert into `anyhow::Error` via `?`, but the library never erases types. This is non-negotiable — the whole point of the shared layer is precision.

### One `TmuxError` for all tmux primitives

`tmux_available`, `has_session`, and `list_windows` share enough failure modes (binary missing, session missing, parse failure) that splitting them into per-function enums creates more conversion than clarity. `roy`'s pane-driver primitives (`capture_pane`, `send_keys`, `list_panes`) layer their own variants on top — see `docs/roy/llds/tmux-driver.md`.

### Subprocess via `std::process::Command` only

Every tmux call uses `std::process::Command`. No async tmux library, no IPC client, no parallelism inside a single primitive call. `gbiv-core` is meant to be boring at the subprocess layer.

### No shared state

The crate has no `OnceCell`, no `Mutex`, no globals. Callers that need to cache "is tmux installed?" across many calls do so at their layer. This keeps `gbiv-core` test-trivial and reasoning-trivial.

## Success Metrics

The crate is working if:

- Both binaries can be compiled, tested, and run independently against the same `gbiv-core` revision with no per-binary divergence in shared-primitive behavior.
- A change to a primitive lands in one place and propagates to both binaries automatically.
- Adding a new shared primitive is a single LLD + spec + test + code pass — no schema migrations, no API contracts to negotiate beyond the LLD.

The crate is broken if:

- Either binary develops a parallel implementation of a `gbiv-core` primitive.
- The crate accumulates per-binary modules (a sign it should not be shared).

## References

- `docs/gbiv/high-level-design.md` — gbiv binary HLD; consumer.
- `docs/roy/high-level-design.md` — roy daemon HLD; consumer.
- `docs/gbiv-core/llds/tmux-primitives.md` — first greenfield LLD under this subtree.

//! Fleet orchestration domain (the `gbiv start` daemon and its client commands).
//!
//! This module is the binary-side home for the orchestration-only components.
//! See `docs/high-level-design.md` § orchestration domain. The shared tmux
//! primitives live in `gbiv_core::tmux`; the orchestration-only pane operations
//! live in `tmux_driver` here.
// TODO: remove once the fleet daemon/CLI first consumer lands — the lint should
// catch any public API that drifts back to unused.
#![allow(dead_code)]

pub mod pane_locator;
pub mod tmux_driver;

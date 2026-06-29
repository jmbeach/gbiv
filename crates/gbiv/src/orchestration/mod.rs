//! Fleet orchestration domain (the `gbiv start` daemon and its client commands).
//!
//! This module is the binary-side home for the orchestration-only components.
//! See `docs/high-level-design.md` § orchestration domain. The shared tmux
//! primitives live in `gbiv_core::tmux`; the orchestration-only pane operations
//! live in `tmux_driver` here.
#![allow(dead_code)] // skeleton: consumed by the fleet daemon/CLI components.

pub mod tmux_driver;

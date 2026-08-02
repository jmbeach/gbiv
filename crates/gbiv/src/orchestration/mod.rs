//! Fleet orchestration domain (the `gbiv start` daemon and its client commands).
//!
//! This module is the binary-side home for the orchestration-only components.
//! See `docs/high-level-design.md` § orchestration domain. The shared tmux
//! primitives live in `gbiv_core::tmux`; the orchestration-only pane operations
//! live in `tmux_driver` here.

pub mod clock;
pub mod daemon;
pub mod http_server;
pub mod pane_locator;
pub mod tmux_driver;

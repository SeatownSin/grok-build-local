//! Cross-platform child-process lifecycle helpers for `tokio::process::Command`.
//!
//! All implementations now live in the lightweight [`axon_tty_utils`] crate
//! so that every crate in the workspace can use them without pulling in the
//! heavyweight `axon-tools` dependency. This module re-exports the public
//! API for backward compatibility.

pub use axon_tty_utils::{
    ProcessGroup, ProcessScope, detach_command, global_process_scope, new_process_group,
};

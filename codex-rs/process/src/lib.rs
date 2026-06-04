//! Codex-specific process spawning helpers.
//!
//! Spawned children must be explicitly joined before their managed handle is
//! dropped. Debug builds enforce this with a drop bomb, while release builds
//! log an error.

mod command_ext;
mod drop_bomb;

pub use command_ext::CommandExt;

pub mod sync;
pub mod tokio;

#[cfg(test)]
mod test_support;

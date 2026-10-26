//! Bounded IPC and lifetime management for the dedicated V8 plugin process.

pub mod framing;
mod process;

pub use process::PluginExit;
pub use process::PluginProcess;
pub use process::PluginProcessError;

pub const HELPER_ARGUMENT: &str = "--codex-ui-plugin-process";
pub const READY: &[u8] = b"codex-ui-plugin-process/2";

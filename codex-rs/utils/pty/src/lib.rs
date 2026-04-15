#[cfg_attr(target_arch = "wasm32", path = "pipe_wasm.rs")]
pub mod pipe;
#[cfg_attr(target_arch = "wasm32", path = "process_wasm.rs")]
mod process;
#[cfg_attr(target_arch = "wasm32", path = "process_group_wasm.rs")]
pub mod process_group;
#[cfg_attr(target_arch = "wasm32", path = "pty_wasm.rs")]
pub mod pty;
#[cfg(test)]
mod tests;
#[cfg(windows)]
mod win;

pub const DEFAULT_OUTPUT_BYTES_CAP: usize = 1024 * 1024;

/// Spawn a non-interactive process using regular pipes for stdin/stdout/stderr.
pub use pipe::spawn_process as spawn_pipe_process;
/// Spawn a non-interactive process using regular pipes, but close stdin immediately.
pub use pipe::spawn_process_no_stdin as spawn_pipe_process_no_stdin;
/// Handle for interacting with a spawned process (PTY or pipe).
pub use process::ProcessHandle;
/// Bundle of process handles plus split output and exit receivers returned by spawn helpers.
pub use process::SpawnedProcess;
/// Terminal size in character cells used for PTY spawn and resize operations.
pub use process::TerminalSize;
/// Combine stdout/stderr receivers into a single broadcast receiver.
pub use process::combine_output_receivers;
/// Backwards-compatible alias for ProcessHandle.
pub type ExecCommandSession = ProcessHandle;
/// Backwards-compatible alias for SpawnedProcess.
pub type SpawnedPty = SpawnedProcess;
/// Report whether ConPTY is available on this platform (Windows only).
pub use pty::conpty_supported;
/// Spawn a process attached to a PTY for interactive use.
pub use pty::spawn_process as spawn_pty_process;
#[cfg(windows)]
pub use win::conpty::RawConPty;

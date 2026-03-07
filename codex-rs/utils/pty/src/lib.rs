pub mod pipe;
mod process;
pub mod process_group;
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
/// Bundle of process handles plus merged output and exit receivers returned by spawn helpers.
pub use process::SpawnedProcess;
/// Bundle of process handles plus split stdout/stderr receivers returned by pipe spawn helpers.
pub use process::SpawnedProcessSplit;
/// Terminal size in character cells used for PTY spawn and resize operations.
pub use process::TerminalSize;
/// Backwards-compatible alias for ProcessHandle.
pub type ExecCommandSession = ProcessHandle;
/// Backwards-compatible alias for SpawnedProcess.
pub type SpawnedPty = SpawnedProcess;
/// Spawn a non-interactive process using regular pipes and preserve split stdout/stderr streams.
pub use pipe::spawn_process_split as spawn_pipe_process_split;
/// Report whether ConPTY is available on this platform (Windows only).
pub use pty::conpty_supported;
/// Spawn a process attached to a PTY for interactive use.
pub use pty::spawn_process as spawn_pty_process;

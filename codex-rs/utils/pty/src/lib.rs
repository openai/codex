pub mod pipe;
mod process;
pub mod process_group;
pub mod pty;
#[cfg(test)]
mod tests;
#[cfg(windows)]
mod win;

/// Spawn a non-interactive process using regular pipes for stdin/stdout/stderr.
pub use pipe::spawn_process as spawn_pipe_process;
/// Spawn a non-interactive process using regular pipes, but close stdin immediately.
pub use pipe::spawn_process_no_stdin as spawn_pipe_process_no_stdin;
/// Handle for interacting with a spawned process (PTY or pipe).
pub use process::ProcessHandle;
/// Bundle of process handles plus output and exit receivers returned by spawn helpers.
pub use process::SpawnedProcess;
/// Backwards-compatible alias for ProcessHandle.
pub type ExecCommandSession = ProcessHandle;
/// Backwards-compatible alias for SpawnedProcess.
pub type SpawnedPty = SpawnedProcess;
/// Report whether ConPTY is available on this platform (Windows only).
pub use pty::conpty_supported;
/// Spawn a process attached to a PTY for interactive use.
pub use pty::spawn_process as spawn_pty_process;

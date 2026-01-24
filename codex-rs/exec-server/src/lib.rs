#[cfg(unix)]
mod posix;

#[cfg(unix)]
pub use posix::main_execve_wrapper;
// exec-server/src/lib.rs

#[cfg(unix)]
pub use posix::main_mcp_server;

#[cfg(unix)]
pub use posix::ExecResult;

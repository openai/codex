use std::io;

/// Extends process command types with Codex-specific process spawning.
///
/// Callers should use this trait to guarantee that child processes are joined.
/// Implementations return the corresponding managed child handle.
pub trait CommandExt {
    /// The managed child handle returned after spawning.
    type Child;

    /// Spawns this command and returns a child handle that must be joined.
    fn spawn_managed(&mut self) -> io::Result<Self::Child>;
}

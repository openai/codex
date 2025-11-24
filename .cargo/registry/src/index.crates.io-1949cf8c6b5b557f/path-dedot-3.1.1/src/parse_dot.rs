use std::{borrow::Cow, io, path::Path};

/// Let `Path` and `PathBuf` have `parse_dot` method.
pub trait ParseDot {
    /// Remove dots in the path and create a new `PathBuf` instance on demand.
    fn parse_dot(&self) -> io::Result<Cow<Path>>;

    /// Remove dots in the path and create a new `PathBuf` instance on demand. It gets the current working directory as the second argument.
    fn parse_dot_from(&self, cwd: impl AsRef<Path>) -> io::Result<Cow<Path>>;
}

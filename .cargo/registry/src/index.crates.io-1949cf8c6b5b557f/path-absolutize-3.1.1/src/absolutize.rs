use std::{borrow::Cow, io, path::Path};

/// Let `Path` and `PathBuf` have `absolutize` and `absolutize_virtually` method.
pub trait Absolutize {
    /// Get an absolute path. This works even if the path does not exist.
    fn absolutize(&self) -> io::Result<Cow<Path>>;

    /// Get an absolute path. This works even if the path does not exist. It gets the current working directory as the second argument.
    fn absolutize_from(&self, cwd: impl AsRef<Path>) -> io::Result<Cow<Path>>;

    /// Get an absolute path. This works even if the path does not exist.
    fn absolutize_virtually(&self, virtual_root: impl AsRef<Path>) -> io::Result<Cow<Path>>;
}

use std::fs::File;
use std::io;
use std::path::Path;

static ERROR_MESSAGE: &str = "same-file is not supported on this platform.";
// This implementation is to allow same-file to be compiled on
// unsupported platforms in case it was incidentally included
// as a transitive, unused dependency
#[derive(Debug, Hash)]
pub struct Handle;

impl Eq for Handle {}

impl PartialEq for Handle {
    fn eq(&self, _other: &Handle) -> bool {
        unreachable!(ERROR_MESSAGE);
    }
}

impl Handle {
    pub fn from_path<P: AsRef<Path>>(_p: P) -> io::Result<Handle> {
        error()
    }

    pub fn from_file(_file: File) -> io::Result<Handle> {
        error()
    }

    pub fn stdin() -> io::Result<Handle> {
        error()
    }

    pub fn stdout() -> io::Result<Handle> {
        error()
    }

    pub fn stderr() -> io::Result<Handle> {
        error()
    }

    pub fn as_file(&self) -> &File {
        unreachable!(ERROR_MESSAGE);
    }

    pub fn as_file_mut(&self) -> &mut File {
        unreachable!(ERROR_MESSAGE);
    }
}

fn error<T>() -> io::Result<T> {
    Err(io::Error::new(io::ErrorKind::Other, ERROR_MESSAGE))
}

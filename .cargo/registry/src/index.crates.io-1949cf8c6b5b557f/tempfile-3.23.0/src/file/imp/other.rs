use std::fs::{File, OpenOptions};
use std::io;
use std::path::Path;

fn not_supported<T>() -> io::Result<T> {
    Err(io::Error::new(
        io::ErrorKind::Other,
        "operation not supported on this platform",
    ))
}

pub fn create_named(
    _path: &Path,
    _open_options: &mut OpenOptions,
    _permissions: Option<&std::fs::Permissions>,
) -> io::Result<File> {
    not_supported()
}

pub fn create(_dir: &Path) -> io::Result<File> {
    not_supported()
}

pub fn reopen(_file: &File, _path: &Path) -> io::Result<File> {
    not_supported()
}

pub fn persist(_old_path: &Path, _new_path: &Path, _overwrite: bool) -> io::Result<()> {
    not_supported()
}

pub fn keep(_path: &Path) -> io::Result<()> {
    not_supported()
}

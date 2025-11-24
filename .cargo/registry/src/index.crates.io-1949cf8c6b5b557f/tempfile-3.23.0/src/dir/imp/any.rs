use crate::error::IoResultExt;
use crate::TempDir;
use std::path::PathBuf;
use std::{fs, io};

fn not_supported<T>(msg: &str) -> io::Result<T> {
    Err(io::Error::new(io::ErrorKind::Other, msg))
}

pub fn create(
    path: PathBuf,
    permissions: Option<&std::fs::Permissions>,
    disable_cleanup: bool,
) -> io::Result<TempDir> {
    if permissions.map_or(false, |p| p.readonly()) {
        return not_supported("changing permissions is not supported on this platform");
    }
    fs::create_dir(&path)
        .with_err_path(|| &path)
        .map(|_| TempDir {
            path: path.into_boxed_path(),
            disable_cleanup,
        })
}

use crate::error::IoResultExt;
use crate::TempDir;
use std::io;
use std::path::PathBuf;

pub fn create(
    path: PathBuf,
    permissions: Option<&std::fs::Permissions>,
    disable_cleanup: bool,
) -> io::Result<TempDir> {
    let mut dir_options = std::fs::DirBuilder::new();
    #[cfg(not(target_os = "wasi"))]
    {
        use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
        if let Some(p) = permissions {
            dir_options.mode(p.mode());
        }
    }
    dir_options
        .create(&path)
        .with_err_path(|| &path)
        .map(|_| TempDir {
            path: path.into_boxed_path(),
            disable_cleanup,
        })
}

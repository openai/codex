use std::path::Path;

use crate::GitToolingError;

#[cfg(unix)]
pub fn create_symlink(
    _source: &Path,
    link_target: &Path,
    destination: &Path,
) -> Result<(), GitToolingError> {
    use std::os::unix::fs::symlink;

    symlink(link_target, destination)?;
    Ok(())
}

#[cfg(windows)]
pub fn create_symlink(
    source: &Path,
    link_target: &Path,
    destination: &Path,
) -> Result<(), GitToolingError> {
    use std::os::windows::fs::FileTypeExt;
    use std::os::windows::fs::symlink_dir;
    use std::os::windows::fs::symlink_file;

    let metadata = std::fs::symlink_metadata(source)?;
    if metadata.file_type().is_symlink_dir() {
        symlink_dir(link_target, destination)?;
    } else {
        symlink_file(link_target, destination)?;
    }
    Ok(())
}

#[cfg(not(any(unix, windows)))]
compile_error!("codex-git symlink support is only implemented for Unix and Windows");

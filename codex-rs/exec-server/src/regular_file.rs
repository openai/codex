use std::io;
#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;
#[cfg(windows)]
use std::os::windows::fs::OpenOptionsExt;
use std::path::Path;
use tokio::io::AsyncReadExt;

pub(crate) async fn open(path: &Path) -> io::Result<tokio::fs::File> {
    let path = path.to_path_buf();
    tokio::task::spawn_blocking(move || open_sync(&path).map(tokio::fs::File::from_std))
        .await
        .map_err(|error| io::Error::other(format!("filesystem task failed: {error}")))?
}

/// Opens a regular file without blocking on Unix FIFOs or impersonating Windows pipe servers.
pub(crate) fn open_sync(path: &Path) -> io::Result<std::fs::File> {
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    configure_open(&mut options);

    let file = options.open(path)?;
    if !is_disk_file(&file) || !file.metadata()?.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("path `{}` is not a file", path.display()),
        ));
    }
    Ok(file)
}

/// Reads a regular UTF-8 file without following a symlink at its final path component.
pub async fn read_sensitive_file_to_string(path: &Path) -> io::Result<String> {
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    configure_open(&mut options);

    #[cfg(unix)]
    options.custom_flags(libc::O_NONBLOCK | libc::O_NOFOLLOW);

    #[cfg(windows)]
    {
        use windows_sys::Win32::Storage::FileSystem::FILE_FLAG_OPEN_REPARSE_POINT;

        options.custom_flags(FILE_FLAG_OPEN_REPARSE_POINT);
    }

    let mut file = tokio::fs::OpenOptions::from(options).open(path).await?;
    let metadata = file.metadata().await?;
    if !is_disk_file(&file) || !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("path `{}` is not a regular file", path.display()),
        ));
    }

    let mut contents = String::new();
    file.read_to_string(&mut contents).await?;
    Ok(contents)
}

#[cfg(unix)]
fn configure_open(options: &mut std::fs::OpenOptions) {
    options.custom_flags(libc::O_NONBLOCK);
}

#[cfg(windows)]
fn configure_open(options: &mut std::fs::OpenOptions) {
    use windows_sys::Win32::Storage::FileSystem::SECURITY_IDENTIFICATION;

    options.security_qos_flags(SECURITY_IDENTIFICATION);
}

#[cfg(not(any(unix, windows)))]
fn configure_open(_options: &mut std::fs::OpenOptions) {}

#[cfg(windows)]
pub(crate) fn is_disk_file(file: &impl std::os::windows::io::AsRawHandle) -> bool {
    use windows_sys::Win32::Foundation::HANDLE;
    use windows_sys::Win32::Storage::FileSystem::FILE_TYPE_DISK;
    use windows_sys::Win32::Storage::FileSystem::GetFileType;

    // SAFETY: `file` owns this handle for the duration of the call.
    unsafe { GetFileType(file.as_raw_handle() as HANDLE) == FILE_TYPE_DISK }
}

#[cfg(not(windows))]
fn is_disk_file<T>(_file: &T) -> bool {
    true
}

#[cfg(test)]
#[path = "regular_file_tests.rs"]
mod tests;

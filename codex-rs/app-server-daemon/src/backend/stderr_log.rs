//! Bounds managed stderr with the shared nonblocking logging worker and tail retention.
//! The worker serializes writes with pruning and pauses pruning during Windows handoff.
//! Native writers remain best effort.

use std::io;
use std::io::Read;
use std::io::Seek;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::Instant;
use tokio::sync::Mutex;
use tracing_appender::non_blocking::NonBlocking;
use tracing_appender::non_blocking::NonBlockingBuilder;
use tracing_appender::non_blocking::WorkerGuard;

const MAX_PREVIOUS_LOG_BYTES: u64 = 256 * 1024;
const MAX_CURRENT_LOG_BYTES: u64 = 1024 * 1024;

const QUEUED_RECORDS: usize = 64;
const MAX_RECORD_BYTES: usize = 16 * 1024;
// Process lifetime: dropping WorkerGuard would wait for a potentially stalled disk.
static LOGGER: OnceLock<(NonBlocking, WorkerGuard)> = OnceLock::new();
// Handoff holds this lock through startup and leaves predecessor pruning disabled on success.
// Both callers use try_lock so a stalled worker cannot make handoff wait on disk.
pub(super) static PRUNING_ENABLED: Mutex<bool> = Mutex::const_new(true);

pub(crate) fn start(path: PathBuf) {
    LOGGER.get_or_init(|| {
        nonblocking(LogFile {
            path,
            file: None,
            can_prune: None,
        })
    });
}

fn nonblocking(writer: impl Write + Send + 'static) -> (NonBlocking, WorkerGuard) {
    NonBlockingBuilder::default()
        .buffered_lines_limit(QUEUED_RECORDS)
        .lossy(/* is_lossy */ true)
        .finish(writer)
}

/// Writes complete records through the bounded managed queue, or ordinary stderr otherwise.
/// Each write must contain one complete record; oversized records are discarded before copying.
pub fn stderr_writer() -> impl Write {
    StderrWriter
}

struct StderrWriter;

impl Write for StderrWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if let Some((writer, _guard)) = LOGGER.get() {
            if bytes.len() <= MAX_RECORD_BYTES {
                writer.clone().write_all(bytes)?;
            }
            Ok(bytes.len())
        } else {
            io::stderr().write(bytes)
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        if LOGGER.get().is_none() {
            io::stderr().flush()?;
        }
        Ok(())
    }
}

struct LogFile {
    path: PathBuf,
    file: Option<std::fs::File>,
    can_prune: Option<bool>,
}

impl Write for LogFile {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        // All filesystem work, including first open and legacy handle setup, stays on the worker.
        if *self
            .can_prune
            .get_or_insert_with(|| enable_append().is_ok())
        {
            let _ = prune(&self.path, &PRUNING_ENABLED);
        }
        let file = match self.file {
            Some(ref mut file) => file,
            None => self.file.insert(
                std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(&self.path)?,
            ),
        };
        file.write_all(bytes)?;
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[cfg(unix)]
fn enable_append() -> io::Result<()> {
    use std::os::fd::AsRawFd;

    let fd = std::io::stderr().as_raw_fd();
    // SAFETY: F_GETFL reads flags from the borrowed descriptor and takes no pointers.
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
    if flags == -1 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: F_SETFL changes status flags on the same descriptor without taking ownership.
    if unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_APPEND) } == -1 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

#[cfg(windows)]
fn enable_append() -> io::Result<()> {
    use std::os::windows::io::AsRawHandle;
    use std::os::windows::io::FromRawHandle;
    use std::os::windows::io::IntoRawHandle;
    use std::os::windows::io::OwnedHandle;
    use windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE;
    use windows_sys::Win32::Storage::FileSystem::FILE_GENERIC_WRITE;
    use windows_sys::Win32::Storage::FileSystem::FILE_SHARE_DELETE;
    use windows_sys::Win32::Storage::FileSystem::FILE_SHARE_READ;
    use windows_sys::Win32::Storage::FileSystem::FILE_SHARE_WRITE;
    use windows_sys::Win32::Storage::FileSystem::FILE_WRITE_DATA;
    use windows_sys::Win32::Storage::FileSystem::ReOpenFile;
    use windows_sys::Win32::System::Console::STD_ERROR_HANDLE;
    use windows_sys::Win32::System::Console::SetStdHandle;

    let stderr = std::io::stderr();
    // SAFETY: stderr is borrowed and remains open; reopening does not truncate it.
    let handle = unsafe {
        ReOpenFile(
            stderr.as_raw_handle(),
            // Append-only access makes writes follow EOF after truncation.
            FILE_GENERIC_WRITE & !FILE_WRITE_DATA,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            /*dwflagsandattributes*/ 0,
        )
    };
    if handle == INVALID_HANDLE_VALUE {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: ReOpenFile returned a new, valid handle owned by this function.
    let handle = unsafe { OwnedHandle::from_raw_handle(handle) };
    // SAFETY: the append handle is valid and retained for the process lifetime on success.
    if unsafe { SetStdHandle(STD_ERROR_HANDLE, handle.as_raw_handle()) } == 0 {
        return Err(io::Error::last_os_error());
    }
    // The process owns its new stderr. Keep the borrowed original valid for cached users.
    let _ = handle.into_raw_handle();
    Ok(())
}

fn prune(path: &Path, pruning_enabled: &Mutex<bool>) -> io::Result<()> {
    let Ok(enabled) = pruning_enabled.try_lock() else {
        return Ok(());
    };
    if !*enabled {
        return Ok(());
    }
    let metadata = std::fs::symlink_metadata(path)?;
    if !metadata.is_file() || metadata.len() <= MAX_CURRENT_LOG_BYTES {
        return Ok(());
    }
    preserve_tail(path)?;
    // Keep inherited handles connected, holding the pruning lock through truncation.
    std::fs::OpenOptions::new()
        .write(true)
        .open(path)?
        .set_len(0)
}

pub(super) async fn preserve(path: &Path) {
    let started = Instant::now();
    let path = path.to_path_buf();
    let result = async {
        tokio::task::spawn_blocking(move || preserve_tail(&path)).await??;
        anyhow::Ok(())
    }
    .await;
    if let Err(error) = result {
        let _ = crate::diagnostics::result::<()>("stderr_preservation", started, Err(error));
    }
}

fn preserve_tail(path: &Path) -> io::Result<()> {
    match std::fs::symlink_metadata(path) {
        Ok(metadata) if !metadata.is_file() => return Ok(()),
        Ok(_) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    }
    let mut file = match std::fs::File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    };
    let metadata = file.metadata()?;
    let len = metadata.len();
    if !metadata.is_file() || len == 0 {
        return Ok(());
    }
    file.seek(io::SeekFrom::Start(
        len.saturating_sub(MAX_PREVIOUS_LOG_BYTES),
    ))?;
    let mut bytes = Vec::new();
    file.take(MAX_PREVIOUS_LOG_BYTES).read_to_end(&mut bytes)?;
    let previous = path.with_extension("log.previous");
    let temporary = path.with_extension("log.previous.tmp");
    let result =
        std::fs::write(&temporary, bytes).and_then(|()| std::fs::rename(&temporary, previous));
    if result.is_err() {
        let _ = std::fs::remove_file(&temporary);
    }
    result
}

#[cfg(test)]
#[path = "stderr_log_tests.rs"]
mod tests;

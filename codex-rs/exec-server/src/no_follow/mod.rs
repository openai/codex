use crate::FileMetadata;
use std::io;
use std::path::Path;
#[cfg(windows)]
use std::time::SystemTime;
#[cfg(windows)]
use std::time::UNIX_EPOCH;

#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;

#[cfg(unix)]
use unix as imp;
#[cfg(windows)]
use windows as imp;

fn invalid_output(reason: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, reason)
}

/// A destination bound before sandboxed tools run. Only ordinary path-based files are truncated.
pub struct BoundOutput {
    file: std::fs::File,
    destination: Destination,
}

enum Destination {
    Path(std::path::PathBuf),
    Stream { is_stdout: bool },
}

impl BoundOutput {
    pub fn open(path: &Path) -> io::Result<Self> {
        // Check raw syntax before absolute() or components() discards directory markers.
        let bytes = path.as_os_str().as_encoded_bytes();
        let leaf = bytes
            .rsplit(|b| *b == b'/' || (cfg!(windows) && *b == b'\\'))
            .next();
        if matches!(leaf, None | Some(b"" | b"." | b"..")) {
            return Err(invalid_output("output must name a file"));
        }
        #[cfg(windows)]
        if bytes.eq_ignore_ascii_case(b"NUL") || bytes.eq_ignore_ascii_case(br"\\.\NUL") {
            return Ok(Self {
                file: windows::open_null_output()?,
                destination: Destination::Stream { is_stdout: false },
            });
        }
        #[cfg(unix)]
        if let Some(output) = unix::inherited_output(path)? {
            return Ok(output);
        }
        #[cfg(unix)]
        let path = unix::output_path(path)?;
        #[cfg(windows)]
        let path = std::path::absolute(path)?;
        let file = imp::open_output(&path)?;
        if file.metadata()?.is_file() {
            imp::reject_multiple_links(&file)?;
        }
        Ok(Self {
            file,
            destination: Destination::Path(path),
        })
    }

    /// Whether the caller explicitly selected the inherited standard output stream.
    pub fn is_stdout(&self) -> bool {
        matches!(self.destination, Destination::Stream { is_stdout: true })
    }

    pub fn write(mut self, contents: &[u8]) -> io::Result<()> {
        use std::io::Write;
        if let Destination::Path(path) = &self.destination {
            imp::verify_output_path(&self.file, path)?;
            if self.file.metadata()?.is_file() {
                imp::reject_multiple_links(&self.file)?;
                self.file.set_len(0)?;
            }
        }
        self.file.write_all(contents)?;
        // A caller may reopen the pathname after a successful exit. Detect a visible swap.
        if let Destination::Path(path) = &self.destination {
            imp::verify_output_path(&self.file, path)?;
        }
        Ok(())
    }
}

pub(crate) fn open_file(path: &Path) -> io::Result<std::fs::File> {
    imp::open_file_sync(path)
}

pub(crate) async fn write_file(path: &Path, contents: Vec<u8>) -> io::Result<()> {
    imp::write_file(path.to_path_buf(), contents).await
}

#[cfg(unix)]
pub(crate) async fn metadata(path: &Path) -> io::Result<FileMetadata> {
    imp::metadata(path.to_path_buf()).await
}

#[cfg(windows)]
pub(crate) async fn metadata(path: &Path) -> io::Result<FileMetadata> {
    imp::metadata(path.to_path_buf())
        .await
        .map(|metadata| FileMetadata {
            is_directory: metadata.is_dir(),
            is_file: metadata.is_file(),
            is_symlink: false,
            size: metadata.len(),
            created_at_ms: metadata.created().ok().map_or(0, system_time_to_unix_ms),
            modified_at_ms: metadata.modified().ok().map_or(0, system_time_to_unix_ms),
        })
}

#[cfg(windows)]
fn system_time_to_unix_ms(time: SystemTime) -> i64 {
    time.duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|duration| i64::try_from(duration.as_millis()).ok())
        .unwrap_or(0)
}

pub(crate) async fn create_directory(path: &Path, recursive: bool) -> io::Result<()> {
    imp::create_directory(path.to_path_buf(), recursive).await
}

pub(crate) async fn remove(path: &Path, recursive: bool, force: bool) -> io::Result<()> {
    imp::remove(path.to_path_buf(), recursive, force).await
}

#[cfg(test)]
mod output_tests {
    //! Exercise the destination as a caller of the final-message writer.
    use super::BoundOutput;
    use pretty_assertions::assert_eq;
    use std::fs;

    #[test]
    fn regular_output_is_truncated_only_when_written() -> std::io::Result<()> {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("message");
        fs::write(&path, "old message")?;
        let output = BoundOutput::open(&path)?;
        assert_eq!(fs::read_to_string(&path)?, "old message");
        output.write(b"new")?;
        assert_eq!(fs::read_to_string(&path)?, "new");
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn rejects_added_links_and_replaced_paths_before_writing() -> std::io::Result<()> {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("message");
        fs::write(&path, "new")?;
        let output = BoundOutput::open(&path)?;
        let alias = dir.path().join("alias");
        fs::hard_link(&path, &alias)?;
        assert!(BoundOutput::open(&alias).is_err());
        assert!(output.write(b"overwritten").is_err());
        assert_eq!(fs::read(&alias)?, b"new");
        fs::remove_file(&alias)?;
        let output = BoundOutput::open(&path)?;
        fs::rename(&path, &alias)?;
        fs::write(&path, "replacement")?;
        assert!(output.write(b"overwritten").is_err());
        assert_eq!(fs::read(&path)?, b"replacement");
        assert_eq!(fs::read(&alias)?, b"new");
        Ok(())
    }

    #[cfg(windows)]
    #[test]
    fn windows_output_binds_the_chosen_stream_until_write() -> std::io::Result<()> {
        use std::os::windows::fs::OpenOptionsExt;
        use windows_sys::Win32::Foundation::ERROR_SHARING_VIOLATION;
        use windows_sys::Win32::Storage::FileSystem::DELETE;
        let dir = tempfile::tempdir()?;
        let base = dir.path().join("message");
        fs::write(&base, "base")?;
        let fs_name = super::windows::filesystem_name(&fs::File::open(&base)?)?;
        let ntfs = fs_name.eq_ignore_ascii_case("NTFS");
        let sibling = dir.path().join("message:sibling");
        if ntfs {
            fs::write(&sibling, "keep")?;
            let alias = dir.path().join("alias");
            fs::hard_link(&base, &alias)?;
            assert!(BoundOutput::open(&base).is_err());
            fs::remove_file(alias)?;
        }
        let names: &[&str] = if ntfs {
            &[
                "message",
                "message::$DATA",
                "message:answer",
                "message:answer:$DATA",
            ]
        } else {
            &["message"]
        };
        for name in names {
            let path = dir.path().join(name);
            fs::write(&base, "base")?;
            fs::write(&path, "previous contents")?;
            let open_for_rename = || fs::OpenOptions::new().access_mode(DELETE).open(&path);
            let rename_handle = open_for_rename()?;
            assert!(
                matches!(BoundOutput::open(&path), Err(e) if e.raw_os_error() == Some(ERROR_SHARING_VIOLATION as i32))
            );
            drop(rename_handle);

            let output = BoundOutput::open(&path)?;
            assert!(
                matches!(open_for_rename(), Err(e) if e.raw_os_error() == Some(ERROR_SHARING_VIOLATION as i32))
            );
            output.write(b"done")?;
            assert_eq!(fs::read(&path)?, b"done");
            drop(open_for_rename()?);
            if name.starts_with("message:answer") {
                assert_eq!(fs::read(&base)?, b"base");
            }
            if ntfs {
                assert_eq!(fs::read(&sibling)?, b"keep");
            }
        }
        Ok(())
    }

    #[cfg(windows)]
    #[test]
    fn nul_discards_output_without_accepting_other_devices() -> std::io::Result<()> {
        for name in ["NUL", "nul", r"\\.\NUL"] {
            BoundOutput::open(name.as_ref())?.write(b"discarded")?;
        }
        assert!(BoundOutput::open(r"\\.\CON".as_ref()).is_err());
        Ok(())
    }

    #[cfg(windows)]
    #[test]
    fn unavailable_full_ids_use_directory_entries_only_on_fat_volumes() -> std::io::Result<()> {
        use windows_sys::Win32::Foundation::ERROR_INVALID_PARAMETER;
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("message");
        let output = BoundOutput::open(&path)?;
        let other = dir.path().join("other");
        fs::write(&other, "other")?;
        // Legacy IDs on a ReFS temp volume are not a valid stand-in for FAT directory entries.
        let host = super::windows::filesystem_name(&output.file)?;
        let legacy_fixture = ["NTFS", "FAT", "FAT32", "exFAT"]
            .iter()
            .any(|name| host.eq_ignore_ascii_case(name));
        let verify = |candidate: &std::path::Path, volume: &str, named: Option<&str>| {
            super::windows::verify_output_path_with(
                &output.file,
                candidate,
                |_| {
                    Err(std::io::Error::from_raw_os_error(
                        ERROR_INVALID_PARAMETER as i32,
                    ))
                },
                |file| {
                    if std::ptr::eq(file, &output.file) {
                        return Ok(volume.to_owned());
                    }
                    named
                        .map(str::to_owned)
                        .ok_or(std::io::ErrorKind::Unsupported.into())
                },
            )
        };
        for volume in ["FAT", "FAT32", "exFAT"] {
            // Reaching the second volume query proves the first one selected the fallback.
            assert!(
                matches!(verify(&path, volume, None), Err(e) if e.kind() == std::io::ErrorKind::Unsupported)
            );
            if legacy_fixture {
                verify(&path, volume, Some(volume))?;
                assert!(verify(&other, volume, Some(volume)).is_err());
            }
        }
        for (bound, named) in [("ReFS", "exFAT"), ("exFAT", "ReFS")] {
            assert!(
                matches!(verify(&path, bound, Some(named)), Err(e) if e.raw_os_error() == Some(ERROR_INVALID_PARAMETER as i32))
            );
        }
        Ok(())
    }

    #[cfg(windows)]
    #[test]
    #[ignore = "requires CODEX_TEST_EXFAT_DIR on a writable exFAT volume"]
    fn writes_on_exfat() -> anyhow::Result<()> {
        let dir = tempfile::tempdir_in(std::env::var("CODEX_TEST_EXFAT_DIR")?)?;
        let path = dir.path().join("message");
        fs::write(&path, "previous message")?;
        assert!(
            super::windows::filesystem_name(&fs::File::open(&path)?)?.eq_ignore_ascii_case("exFAT")
        );
        BoundOutput::open(&path)?.write(b"done")?;
        assert_eq!(fs::read(&path)?, b"done");
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn output_only_accepts_leading_parent_components() -> std::io::Result<()> {
        let dir = tempfile::tempdir()?;
        fs::create_dir_all(dir.path().join("a/b"))?;
        let path = dir.path().join("a/b/../../message");
        assert!(
            matches!(BoundOutput::open(&path), Err(e) if e.kind() == std::io::ErrorKind::InvalidInput)
        );
        assert!(!dir.path().join("message").exists());
        let mut expected = std::env::current_dir()?;
        expected.pop();
        assert_eq!(
            super::unix::output_path("./../message".as_ref())?,
            expected.join("message")
        );
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn writes_fifo_device_and_inherited_pipe() -> anyhow::Result<()> {
        use std::io::Read;
        use std::os::fd::AsRawFd;
        use std::os::unix::fs::OpenOptionsExt;
        let dir = tempfile::tempdir()?;
        let fifo = dir.path().join("pipe");
        assert!(
            std::process::Command::new("mkfifo")
                .arg(&fifo)
                .status()?
                .success()
        );
        let mut reader = fs::OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NONBLOCK)
            .open(&fifo)?;
        BoundOutput::open(&fifo)?.write(b"fifo")?;
        let mut buf = [0; 4];
        reader.read_exact(&mut buf)?;
        assert_eq!(buf, *b"fifo");
        BoundOutput::open(std::path::Path::new("/dev/null"))?.write(b"device")?;
        let stdout = BoundOutput::open(std::path::Path::new("/dev/stdout"))?;
        assert!(stdout.is_stdout());
        let read_only = rustix::io::dup(&reader)?;
        let alias = format!("/dev/fd/{}", read_only.as_raw_fd());
        assert!(
            matches!(BoundOutput::open(alias.as_ref()), Err(e) if e.kind() == std::io::ErrorKind::InvalidInput)
        );
        let writer = fs::OpenOptions::new().write(true).open(&fifo)?;
        // Match the inheritable pipe descriptor passed by shell process substitution.
        let inherited = rustix::io::dup(&writer)?;
        let alias = format!("/dev/fd/{}", inherited.as_raw_fd());
        BoundOutput::open(alias.as_ref())?.write(b"pipe")?;
        reader.read_exact(&mut buf)?;
        assert_eq!(buf, *b"pipe");
        Ok(())
    }
}

//! Per-launch snapshot transport, created under the sandbox-protected daemon root.
//! No named file ever contains shell state, and the cache owns bytes, not descriptors.
//! The caller keeps the reader alive through spawn.

use codex_shell_command::shell_detect::ShellType;
use std::fs::File;
use std::io;
use std::io::Write;
use std::os::fd::AsRawFd;
use std::os::unix::fs::MetadataExt;

pub(super) fn materialize(shell: ShellType, state: &str) -> io::Result<File> {
    // Mode 0700 alone cannot stop a same-user sandboxed command from replacing
    // the pathname or retaining a writable handle before unlink. Reuse the root
    // that every filesystem-restricted sandbox hides, independent of CODEX_HOME.
    let root = codex_uds::prepare_shared_daemon_socket_directory()?;
    let directory = tempfile::tempdir_in(root)?;
    let writer = tempfile::NamedTempFile::new_in(directory.path())?;
    // Open independently, rather than dup: writing must not advance the reader.
    let reader = File::open(writer.path())?;
    let reader = File::from(rustix::io::fcntl_dupfd_cloexec(&reader, /*min*/ 10)?);
    let (mut writer, path) = writer.into_parts();
    path.close()?;
    directory.close()?;
    if reader.metadata()?.nlink() != 0 {
        return Err(io::Error::other("shell snapshot transport is still linked"));
    }

    let fd = reader.as_raw_fd();
    // Source has opened its own reader before this runs. Close the carrier before
    // restoring functions that may shadow exec/unset, without borrowing stdin.
    let close = match shell {
        ShellType::Bash => format!("\\exec {fd}<&-\n"),
        ShellType::Zsh => format!(
            "__codex_snapshot_fd={fd}\nexec {{__codex_snapshot_fd}}<&-\nunset __codex_snapshot_fd\n"
        ),
        ShellType::Sh | ShellType::PowerShell | ShellType::Cmd => {
            return Err(io::Error::other("unsupported snapshot shell"));
        }
    };
    writer.write_all(close.as_bytes())?;
    writer.write_all(state.as_bytes())?;
    Ok(reader)
}

#[cfg(test)]
#[path = "shell_snapshot_file_tests.rs"]
mod tests;

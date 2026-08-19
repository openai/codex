//! Authenticate descriptor-backed mounts before sandboxed code can inherit them.

use std::collections::HashSet;
use std::fs;
use std::fs::File;
use std::io;
use std::os::fd::AsRawFd;
use std::os::fd::FromRawFd;
use std::os::unix::fs::MetadataExt;
use std::path::Path;

pub(crate) fn verify_fd_mounts(mounts: &[String]) -> io::Result<()> {
    let mut claimed_descriptors = HashSet::with_capacity(mounts.len());

    for mount in mounts {
        let (descriptor, destination) = mount.split_once(':').ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("descriptor-backed mount must contain FD:DEST: {mount}"),
            )
        })?;
        let descriptor = descriptor.parse::<libc::c_int>().map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("descriptor-backed mount has an invalid descriptor: {mount}"),
            )
        })?;
        if descriptor <= libc::STDERR_FILENO {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("descriptor-backed mount cannot use a standard descriptor: {descriptor}"),
            ));
        }
        if !claimed_descriptors.insert(descriptor) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("descriptor-backed mount reuses descriptor {descriptor}"),
            ));
        }

        // The launcher transfers each distinct descriptor exactly once across
        // bubblewrap, so first confirm it is live before assuming ownership.
        let flags = unsafe { libc::fcntl(descriptor, libc::F_GETFD) };
        if flags < 0 {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: the launcher transferred this live descriptor to this stage,
        // and the set above prevents claiming the same descriptor twice.
        let file = unsafe { File::from_raw_fd(descriptor) };
        let result =
            unsafe { libc::fcntl(file.as_raw_fd(), libc::F_SETFD, flags | libc::FD_CLOEXEC) };
        if result < 0 {
            return Err(io::Error::last_os_error());
        }

        let destination = Path::new(destination);
        if !destination.is_absolute() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "descriptor-backed mount destination must be absolute: {}",
                    destination.display()
                ),
            ));
        }

        let descriptor_metadata = file.metadata()?;
        let destination_metadata = fs::symlink_metadata(destination)?;
        if (descriptor_metadata.dev(), descriptor_metadata.ino())
            != (destination_metadata.dev(), destination_metadata.ino())
        {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!(
                    "descriptor-backed mount does not match its destination: {}",
                    destination.display()
                ),
            ));
        }

        // Closing immediately prevents a writable host directory descriptor
        // from reaching bridge workers or the sandboxed command.
        drop(file);
    }

    Ok(())
}

#[cfg(test)]
#[path = "fd_mount_tests.rs"]
mod tests;

//! Reject host mount aliases that would bypass the privileged socket directory mask.
//! Mount roots describe filesystem identity; canonical paths alone miss bind mounts.

use std::collections::BTreeSet;
use std::fs;
use std::io;
use std::os::unix::ffi::OsStringExt;
use std::os::unix::fs::MetadataExt;
use std::path::Path;
use std::path::PathBuf;

pub(crate) fn reject_daemon_mount_aliases(
    directory: &Path,
    masked_root: Option<&Path>,
) -> io::Result<()> {
    let device = fs::metadata(directory)?.dev();
    check_mounts(
        directory,
        &format!("{}:{}", libc::major(device), libc::minor(device)),
        &fs::read("/proc/self/mountinfo")?,
        masked_root,
    )
}

fn check_mounts(
    directory: &Path,
    device: &str,
    mountinfo: &[u8],
    masked_root: Option<&Path>,
) -> io::Result<()> {
    let invalid = || io::Error::other("cannot establish app-server socket mount isolation");
    let mut mounts = Vec::new();
    let mut locations = BTreeSet::new();
    for line in mountinfo
        .split(|byte| *byte == b'\n')
        .filter(|line| !line.is_empty())
    {
        let fields: Vec<_> = line.split(|byte| *byte == b' ').take(5).collect();
        let [_, _, mount_device, root, destination] = fields.as_slice() else {
            return Err(invalid());
        };
        let root = mount_path(root)?;
        let destination = mount_path(destination)?;
        if *mount_device == device.as_bytes()
            && let Ok(relative) = directory.strip_prefix(&destination)
        {
            locations.insert(root.join(relative));
        }
        mounts.push((*mount_device, root, destination));
    }
    // Overmounts can leave hidden entries in mountinfo. Require every possible
    // containing mount to agree instead of guessing which root is visible.
    if locations.len() != 1 {
        return Err(invalid());
    }
    let location = locations.into_iter().next().ok_or_else(invalid)?;
    for (mount_device, root, destination) in &mounts {
        // Nested mounts can introduce another filesystem (or an individual socket) under the mask.
        let nested = destination != directory && destination.starts_with(directory);
        let alias = if *mount_device == device.as_bytes() {
            if let Ok(relative) = location.strip_prefix(root) {
                Some(destination.join(relative))
            } else if root.starts_with(&location) {
                Some(destination.clone())
            } else {
                None
            }
        } else {
            None
        };
        if nested
            || alias.is_some_and(|path| {
                !path.starts_with(directory)
                    && !masked_root.is_some_and(|root| path.starts_with(root))
            })
        {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!(
                    "app-server socket directory has an unsupported host mount at {}; remove the bind-mount alias or nested mount before starting the sandbox",
                    destination.display()
                ),
            ));
        }
    }
    Ok(())
}

fn mount_path(encoded: &[u8]) -> io::Result<PathBuf> {
    let mut decoded = Vec::new();
    let mut bytes = encoded.iter().copied();
    while let Some(byte) = bytes.next() {
        decoded.push(if byte == b'\\' {
            let digits: Vec<_> = bytes.by_ref().take(3).collect();
            match digits.as_slice() {
                b"040" => b' ',
                b"011" => b'\t',
                b"012" => b'\n',
                b"134" => b'\\',
                _ => return Err(io::Error::other("invalid mountinfo path escape")),
            }
        } else {
            byte
        });
    }
    let path = PathBuf::from(std::ffi::OsString::from_vec(decoded));
    if !path.is_absolute() {
        return Err(io::Error::other("mountinfo path is not absolute"));
    }
    Ok(path)
}

#[cfg(test)]
#[path = "daemon_mounts_tests.rs"]
mod tests;

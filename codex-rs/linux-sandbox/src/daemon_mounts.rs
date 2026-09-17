//! Reject host mount aliases that would bypass the privileged socket directory mask.
//! Mount roots describe filesystem identity; canonical paths alone miss bind mounts.

use rustix::fs::AtFlags;
use rustix::fs::StatxFlags;
use rustix::fs::statx;
use std::collections::BTreeSet;
use std::fs;
use std::io;
use std::os::fd::AsRawFd;
use std::os::unix::ffi::OsStringExt;
use std::os::unix::fs::MetadataExt;
use std::path::Path;
use std::path::PathBuf;

pub(crate) fn reject_daemon_mount_aliases(
    directory: &Path,
    masked_root: Option<&Path>,
) -> io::Result<()> {
    let directory_file = fs::File::open(directory)?;
    let device = directory_file.metadata()?.dev();
    let mount_id = fs::read_to_string(format!("/proc/self/fdinfo/{}", directory_file.as_raw_fd()))
        .ok()
        .and_then(|fdinfo| {
            fdinfo
                .lines()
                .find_map(|line| line.strip_prefix("mnt_id:"))
                .and_then(|id| id.trim().parse::<u64>().ok())
        })
        .or_else(|| {
            // Query the same open directory, using the ID shared with mountinfo.
            // Older kernels may succeed without returning the requested field.
            statx(&directory_file, "", AtFlags::EMPTY_PATH, StatxFlags::MNT_ID)
                .ok()
                .filter(|stat| stat.stx_mask & StatxFlags::MNT_ID.bits() != 0)
                .map(|stat| stat.stx_mnt_id)
        })
        .map(|id| id.to_string());
    check_mounts(
        directory,
        &format!("{}:{}", libc::major(device), libc::minor(device)),
        mount_id.as_deref(),
        &fs::read("/proc/self/mountinfo")?,
        masked_root,
    )
}

fn check_mounts(
    directory: &Path,
    device: &str,
    mount_id: Option<&str>,
    mountinfo: &[u8],
    masked_root: Option<&Path>,
) -> io::Result<()> {
    let invalid = || io::Error::other("cannot establish app-server socket mount isolation");
    let mut mounts = Vec::new();
    for line in mountinfo
        .split(|byte| *byte == b'\n')
        .filter(|line| !line.is_empty())
    {
        let fields: Vec<_> = line.split(|byte| *byte == b' ').take(5).collect();
        let [id, parent, mount_device, root, destination] = fields.as_slice() else {
            return Err(invalid());
        };
        let root = mount_path(root)?;
        let destination = mount_path(destination)?;
        mounts.push((*id, *parent, *mount_device, root, destination));
    }
    let (location, containing_mount) = if let Some(mount_id) = mount_id {
        // fdinfo/statx identifies the opened mount, which may have been covered
        // by another mount before we read mountinfo.
        let selected = mounts
            .iter()
            .find(|(id, ..)| *id == mount_id.as_bytes())
            .ok_or_else(invalid)?;
        let (_, _, mount_device, root, destination) = selected;
        if *mount_device != device.as_bytes() {
            return Err(invalid());
        }
        let relative = directory.strip_prefix(destination).map_err(|_| invalid())?;
        let mut current = Some(selected);
        let mut visible_child: Option<&Path> = None;
        let mut visited = BTreeSet::new();
        while let Some((id, parent, _, _, destination)) = current {
            if !visited.insert(id)
                || mounts.iter().any(|(child_id, child_parent, _, _, child)| {
                    child_id != id
                        && child_parent == id
                        && directory.starts_with(child)
                        && !visible_child.is_some_and(|visible| child.starts_with(visible))
                })
            {
                return Err(invalid());
            }
            if id == parent {
                break;
            }
            // Follow the selected branch towards the namespace root. Sibling
            // mounts below this branch are hidden; mounts above it cover it.
            visible_child = Some(destination);
            current = mounts.iter().find(|(id, ..)| id == parent);
        }
        (root.join(relative), Some((mount_id, destination)))
    } else {
        // Without a mount ID, require every possible containing mount to agree
        // on the backing location, and do not assume any aliases are hidden.
        let locations: BTreeSet<_> = mounts
            .iter()
            .filter(|(_, _, mount_device, ..)| *mount_device == device.as_bytes())
            .filter_map(|(_, _, _, root, destination)| {
                directory
                    .strip_prefix(destination)
                    .ok()
                    .map(|relative| root.join(relative))
            })
            .collect();
        if locations.len() != 1 {
            return Err(invalid());
        }
        (locations.into_iter().next().ok_or_else(invalid)?, None)
    };
    for (id, _, mount_device, root, destination) in &mounts {
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
                // An ancestor's path beneath this mount is hidden by it. Keep
                // checking other mounts, including aliases mounted beneath it.
                let hidden = containing_mount.is_some_and(|(mount_id, containing_mount)| {
                    *id != mount_id.as_bytes()
                        && containing_mount.starts_with(destination)
                        && path.starts_with(containing_mount)
                });
                !path.starts_with(directory)
                    && !masked_root.is_some_and(|root| path.starts_with(root))
                    && !hidden
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

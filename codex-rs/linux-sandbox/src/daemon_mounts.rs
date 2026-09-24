//! Find host mount aliases that need the privileged socket directory mask.
//! Mount roots describe filesystem identity; canonical paths alone miss bind mounts.
//! Btrfs subvolume device numbers can differ from the containing mount's device.

use rustix::fs::AtFlags;
use rustix::fs::StatxFlags;
use rustix::fs::fstatfs;
use rustix::fs::statx;
use std::collections::BTreeSet;
use std::fs;
use std::io;
use std::os::fd::AsRawFd;
use std::os::unix::ffi::OsStringExt;
use std::os::unix::fs::MetadataExt;
use std::path::Path;
use std::path::PathBuf;

#[derive(Clone, Copy, PartialEq, Eq)]
enum SocketFilesystem {
    Btrfs,
    Other,
}

pub(crate) fn daemon_socket_mask_paths(
    directory: &Path,
    masked_root: Option<&Path>,
) -> io::Result<BTreeSet<PathBuf>> {
    let directory_file = fs::File::open(directory)?;
    let device = directory_file.metadata()?.dev();
    let filesystem =
        if fstatfs(&directory_file).is_ok_and(|stat| stat.f_type == libc::BTRFS_SUPER_MAGIC) {
            SocketFilesystem::Btrfs
        } else {
            SocketFilesystem::Other
        };
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
        filesystem,
        mount_id.as_deref(),
        &fs::read("/proc/self/mountinfo")?,
        masked_root,
    )
}

fn check_mounts(
    directory: &Path,
    device: &str,
    filesystem: SocketFilesystem,
    mount_id: Option<&str>,
    mountinfo: &[u8],
    masked_root: Option<&Path>,
) -> io::Result<BTreeSet<PathBuf>> {
    let invalid = || io::Error::other("cannot establish app-server socket mount isolation");
    let mut mounts = Vec::new();
    for line in mountinfo
        .split(|byte| *byte == b'\n')
        .filter(|line| !line.is_empty())
    {
        let mut fields = line.split(|byte| *byte == b' ');
        let prefix: Vec<_> = fields.by_ref().take(5).collect();
        let [id, parent, mount_device, root, destination] = prefix.as_slice() else {
            return Err(invalid());
        };
        let destination = mount_path(destination)?;
        let mount_filesystem = fields
            .skip_while(|field| *field != b"-")
            .nth(1)
            .ok_or_else(invalid)?;
        mounts.push((
            *id,
            *parent,
            *mount_device,
            *root,
            destination,
            mount_filesystem,
        ));
    }
    let selected_index = mount_id
        .map(|mount_id| {
            let mut matching = mounts
                .iter()
                .enumerate()
                .filter(|(_, (id, ..))| *id == mount_id.as_bytes());
            let (index, (_, _, mount_device, _, _, mount_filesystem)) =
                matching.next().ok_or_else(invalid)?;
            if matching.next().is_some()
                || (*mount_device != device.as_bytes()
                    && (filesystem != SocketFilesystem::Btrfs || *mount_filesystem != b"btrfs"))
            {
                return Err(invalid());
            }
            Ok(index)
        })
        .transpose()?;
    // Btrfs reports a per-subvolume st_dev, while mountinfo uses the superblock
    // device. Only the verified descriptor's mount ID can authorize that mismatch.
    // Retain both device identities when looking for aliases.
    let mount_device = selected_index
        .map(|index| mounts[index].2)
        .unwrap_or(device.as_bytes());
    let mounts = mounts
        .into_iter()
        .map(|(id, parent, candidate_device, root, destination, _)| {
            // Only roots on the socket filesystem can identify aliases. Other
            // filesystems can use non-path roots such as nsfs `mnt:[inode]`, but
            // their destinations still matter for ancestry and nested-mount checks.
            let root = (candidate_device == device.as_bytes() || candidate_device == mount_device)
                .then(|| mount_path(root))
                .transpose()?;
            Ok((id, parent, candidate_device, root, destination))
        })
        .collect::<io::Result<Vec<_>>>()?;
    let (location, containing_mount) = if let Some((mount_id, index)) = mount_id.zip(selected_index)
    {
        // fdinfo/statx identifies the opened mount, which may have been covered
        // by another mount before we read mountinfo.
        let selected = &mounts[index];
        let (_, _, _, root, destination) = selected;
        let root = root.as_ref().ok_or_else(invalid)?;
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
            .filter_map(|(_, _, _, root, destination)| {
                let root = root.as_ref()?;
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
    let mut mask_paths = BTreeSet::from([directory.to_path_buf()]);
    for (id, _, _, root, destination) in &mounts {
        // Nested mounts can introduce another filesystem (or an individual socket) under the mask.
        let nested = destination != directory && destination.starts_with(directory);
        let alias = if let Some(root) = root {
            if let Ok(relative) = location.strip_prefix(root) {
                Some((destination.join(relative), !relative.as_os_str().is_empty()))
            } else if root.starts_with(&location) {
                Some((destination.clone(), false))
            } else {
                None
            }
        } else {
            None
        };
        let exposed_alias = alias.filter(|(path, _)| {
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
        });
        if nested
            || exposed_alias
                .as_ref()
                .is_some_and(|(_, can_mask)| !can_mask)
        {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!(
                    "app-server socket directory has an unsupported host mount at {}; remove the bind-mount alias or nested mount before starting the sandbox",
                    destination.display()
                ),
            ));
        }
        if let Some((path, _)) = exposed_alias {
            // An alias of an ancestor exposes the whole directory, so apply the
            // same mask there. Direct directory/socket aliases and nested mounts
            // remain unsupported because a later bind could reopen the mask.
            mask_paths.insert(path);
        }
    }
    Ok(mask_paths)
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

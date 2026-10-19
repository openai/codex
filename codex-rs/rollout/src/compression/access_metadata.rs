//! Preserves access to replacement rollout files before they are published.
//! Metadata failures leave the source intact; never silently replace ownership
//! or discard a macOS ACL when switching between plain and compressed files.
//! Apply ACLs last because they can prohibit subsequent metadata updates.
//! Reject delete-deny ACLs before installation so failed replacements remain removable.

use std::fs::File;
use std::fs::FileTimes;
use std::io;
#[cfg(target_os = "macos")]
use std::os::fd::AsRawFd;
#[cfg(unix)]
use std::os::unix::fs::MetadataExt;

// libc does not expose the macOS ACL APIs.
#[cfg(target_os = "macos")]
unsafe extern "C" {
    fn acl_get_fd(fd: libc::c_int) -> *mut libc::c_void;
    fn acl_get_entry(
        acl: *mut libc::c_void,
        entry_id: libc::c_int,
        entry: *mut *mut libc::c_void,
    ) -> libc::c_int;
    fn acl_get_tag_type(entry: *mut libc::c_void, tag: *mut libc::c_uint) -> libc::c_int;
    fn acl_get_permset_mask_np(entry: *mut libc::c_void, permissions: *mut u64) -> libc::c_int;
    fn acl_init(count: libc::c_int) -> *mut libc::c_void;
    fn acl_set_fd(fd: libc::c_int, acl: *mut libc::c_void) -> libc::c_int;
    fn acl_free(acl: *mut libc::c_void) -> libc::c_int;
}

pub(super) fn copy(source: &File, destination: &File) -> io::Result<()> {
    let metadata = source.metadata()?;
    #[cfg(unix)]
    {
        let destination_metadata = destination.metadata()?;
        if (destination_metadata.uid(), destination_metadata.gid())
            != (metadata.uid(), metadata.gid())
        {
            // Chown can clear mode bits, so restore permissions afterwards.
            // Failure must abort publication rather than change who owns the file.
            std::os::unix::fs::fchown(destination, Some(metadata.uid()), Some(metadata.gid()))?;
        }
    }
    destination.set_permissions(metadata.permissions())?;
    destination.set_times(FileTimes::new().set_modified(metadata.modified()?))?;
    #[cfg(target_os = "macos")]
    {
        // COPYFILE_ACL merges inherited destination entries and drops inherited
        // source entries. Replace the entire ACL to preserve its order and denies.
        // SAFETY: source stays open and the returned ACL is freed below.
        let mut acl = unsafe { acl_get_fd(source.as_raw_fd()) };
        let source_has_no_acl = acl.is_null();
        if source_has_no_acl {
            let error = io::Error::last_os_error();
            if error.raw_os_error() != Some(libc::ENOENT) {
                return Err(error);
            }
            // No source ACL: also clear any ACL inherited by the new file.
            // SAFETY: zero is a valid entry count; allocation failure returns null.
            acl = unsafe {
                acl_init(/*count*/ 0)
            };
            if acl.is_null() {
                return Err(io::Error::last_os_error());
            }
        }
        let result = (|| {
            // These values come from macOS <sys/acl.h>.
            const ACL_EXTENDED_DENY: libc::c_uint = 2;
            const ACL_DELETE: u64 = 1 << 4;
            let mut entry_id = 0;
            loop {
                let mut entry = std::ptr::null_mut();
                // SAFETY: acl is valid and entry points to writable pointer storage.
                if unsafe { acl_get_entry(acl, entry_id, &mut entry) } != 0 {
                    let error = io::Error::last_os_error();
                    // macOS returns EINVAL when the entry index reaches the end.
                    if error.raw_os_error() == Some(libc::EINVAL) {
                        break;
                    }
                    return Err(error);
                }
                let mut tag = 0;
                let mut permissions = 0;
                // SAFETY: entry belongs to the live acl; both outputs are writable.
                if unsafe { acl_get_tag_type(entry, &mut tag) } != 0
                    || unsafe { acl_get_permset_mask_np(entry, &mut permissions) } != 0
                {
                    return Err(io::Error::last_os_error());
                }
                // Conservatively reject any delete deny, regardless of principal or
                // ordering. Installing it can prevent both publication and cleanup,
                // and the ACL may also deny restoring permissions after a failure.
                if tag == ACL_EXTENDED_DENY && permissions & ACL_DELETE != 0 {
                    return Err(io::Error::new(
                        io::ErrorKind::PermissionDenied,
                        "cannot replace a rollout with a delete-deny ACL",
                    ));
                }
                entry_id += 1;
            }
            // SAFETY: destination stays open and acl is a valid owned allocation.
            if unsafe { acl_set_fd(destination.as_raw_fd(), acl) } == 0 {
                return Ok(());
            }
            let error = io::Error::last_os_error();
            // Filesystems without ACL support reject even an empty ACL. There
            // is nothing to preserve only when the source had no ACL.
            if source_has_no_acl && error.raw_os_error() == Some(libc::ENOTSUP) {
                return Ok(());
            }
            Err(error)
        })();
        // SAFETY: acl came from acl_get_fd or acl_init and is no longer used.
        unsafe { acl_free(acl) };
        result?;
    }
    Ok(())
}

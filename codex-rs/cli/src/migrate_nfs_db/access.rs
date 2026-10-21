//! Reject databases with access permissions that replacement cannot preserve.
//! Accept only files owned by the current user with no group or other access.

use anyhow::Context;
use anyhow::ensure;
use std::fs::File;
use std::os::unix::fs::MetadataExt;

pub(super) fn require_private_database(file: &File) -> anyhow::Result<()> {
    let metadata = file.metadata()?;
    // SAFETY: geteuid has no preconditions.
    let user = unsafe { libc::geteuid() };
    ensure!(
        metadata.uid() == user,
        "database must be owned by the current user"
    );
    ensure!(
        metadata.mode() & 0o077 == 0,
        "shared database access is not supported: group and other permissions must be absent"
    );
    check_acl(file).context("cannot verify private database access")
}

#[cfg(target_os = "linux")]
fn check_acl(file: &File) -> anyhow::Result<()> {
    use std::os::fd::AsRawFd;

    // POSIX ACL grants are limited by the group mode bits checked above.
    // NFSv4 named entries can grant access independently of those bits.
    let mut buffer = vec![0u8; 65536];
    // SAFETY: The descriptor is open. The name and writable buffer are valid for this call.
    let size = unsafe {
        libc::fgetxattr(
            file.as_raw_fd(),
            c"system.nfs4_acl".as_ptr(),
            buffer.as_mut_ptr().cast(),
            buffer.len(),
        )
    };
    if size < 0 {
        let error = std::io::Error::last_os_error();
        return match error.raw_os_error() {
            Some(libc::ENODATA | libc::EOPNOTSUPP) => Ok(()),
            _ => Err(error.into()),
        };
    }
    let mut bytes = &buffer[..size as usize];
    let read_word = |bytes: &mut &[u8]| -> anyhow::Result<u32> {
        let (word, rest) = bytes
            .split_at_checked(/*mid*/ 4)
            .context("invalid NFSv4 ACL")?;
        *bytes = rest;
        Ok(u32::from_be_bytes(word.try_into()?))
    };
    let entries = read_word(&mut bytes)?;
    for _ in 0..entries {
        let kind = read_word(&mut bytes)?;
        let _flags = read_word(&mut bytes)?;
        let permissions = read_word(&mut bytes)?;
        let length = read_word(&mut bytes)? as usize;
        let (name, rest) = bytes
            .split_at_checked(length)
            .context("invalid NFSv4 ACL")?;
        ensure!(
            matches!(name, b"OWNER@" | b"GROUP@" | b"EVERYONE@"),
            "databases with named NFSv4 ACL entries are not supported"
        );
        ensure!(kind <= 1, "unsupported NFSv4 ACL entry type");
        // Other users can read attributes and ACLs, and synchronize. Reject all other grants.
        const READ_ATTRIBUTES: u32 = 0x80;
        const READ_ACL: u32 = 0x20000;
        const SYNCHRONIZE: u32 = 0x100000;
        ensure!(
            kind != 0
                || name == b"OWNER@"
                || permissions & !(READ_ATTRIBUTES | READ_ACL | SYNCHRONIZE) == 0,
            "shared database access through an NFSv4 ACL is not supported"
        );
        bytes = rest
            .get((4 - length % 4) % 4..)
            .context("invalid NFSv4 ACL padding")?;
    }
    ensure!(bytes.is_empty(), "invalid NFSv4 ACL trailing data");
    Ok(())
}

#[cfg(target_os = "macos")]
fn check_acl(file: &File) -> anyhow::Result<()> {
    use std::ffi::c_void;
    use std::os::fd::AsRawFd;

    unsafe extern "C" {
        fn acl_get_fd(fd: libc::c_int) -> *mut c_void;
        fn acl_get_entry(
            acl: *mut c_void,
            entry_id: libc::c_int,
            entry: *mut *mut c_void,
        ) -> libc::c_int;
        fn acl_free(acl: *mut c_void) -> libc::c_int;
    }
    // SAFETY: The descriptor is open. The returned ACL remains valid until acl_free.
    let acl = unsafe { acl_get_fd(file.as_raw_fd()) };
    if acl.is_null() {
        let error = std::io::Error::last_os_error();
        return match error.raw_os_error() {
            // ENOENT means the open file has no extended ACL.
            Some(libc::ENOENT | libc::ENOTSUP) => Ok(()),
            _ => Err(error.into()),
        };
    }
    let mut entry = std::ptr::null_mut();
    // SAFETY: The ACL and output pointer are valid. Zero selects ACL_FIRST_ENTRY on macOS.
    let status = unsafe {
        acl_get_entry(acl, /*entry_id*/ 0, &mut entry)
    };
    let error = std::io::Error::last_os_error();
    // SAFETY: This call releases the ACL returned by acl_get_fd exactly once.
    unsafe { acl_free(acl) };
    ensure!(
        status != 0,
        "databases with extended macOS ACLs are not supported"
    );
    ensure!(
        error.raw_os_error() == Some(libc::EINVAL),
        "cannot read database ACL: {error}"
    );
    Ok(())
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn check_acl(_file: &File) -> anyhow::Result<()> {
    anyhow::bail!("database ACL checks are not supported on this operating system")
}

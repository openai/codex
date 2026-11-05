//! Upgrade the protected ACL written by the old legacy helper before SYSTEM reuses the bin.
use std::io;
use std::os::windows::io::AsRawHandle;
use std::os::windows::io::BorrowedHandle;
use std::path::Path;

use anyhow::Context;
use anyhow::Result;
use anyhow::ensure;
use codex_windows_sandbox::DirectoryOpenDisposition;
use windows::Win32::Foundation::BOOL;
use windows_sys::Win32::Foundation::ERROR_ACCESS_DENIED;
use windows_sys::Win32::Security as security;
use windows_sys::Win32::Storage::FileSystem as fs;

/// Admission has pinned the authenticated user's no-reparse path through the helper call.
pub(crate) fn prepare(home: &Path, owner: BorrowedHandle<'_>) -> Result<()> {
    let bin = home.join(".sandbox-bin");
    let open = || {
        codex_windows_sandbox::open_directory_no_reparse(
            &bin,
            fs::READ_CONTROL | fs::WRITE_DAC,
            fs::FILE_SHARE_READ | fs::FILE_SHARE_WRITE | fs::FILE_SHARE_DELETE,
            DirectoryOpenDisposition::OpenExisting,
        )
    };
    match open() {
        Ok(_) => return Ok(()),
        Err(error)
            if error
                .downcast_ref::<io::Error>()
                .is_some_and(|cause| cause.raw_os_error() == Some(ERROR_ACCESS_DENIED as i32)) => {}
        Err(error) => return Err(error),
    }
    let directory =
        crate::package_lifecycle::with_owner_impersonation(owner.as_raw_handle(), open)?;
    if let Some((mut acl, control)) = crate::registered_runtime::read_acl(&directory)? {
        upgrade_legacy_system_entry(&mut acl, control)?;
        crate::registered_runtime::write_acl(&directory, &mut acl, control)?;
    }
    open()
        .map(drop)
        .context("verify SYSTEM can protect the repaired sandbox bin")
}

// The old helper wrote a protected, allow-only ACL with one inheritable SYSTEM Modify entry.
// Alter only that entry in the in-memory ACL. A rejected edit is never written back.
fn upgrade_legacy_system_entry(acl: &mut [u32], control: u16) -> Result<()> {
    ensure!(
        control & security::SE_DACL_PROTECTED != 0,
        "expected a protected legacy sandbox bin ACL"
    );
    let system = codex_windows_sandbox::resolve_sid("SYSTEM")?;
    let mut matched = false;
    let dacl = acl.as_mut_ptr().cast::<security::ACL>();
    for index in 0..u32::from(unsafe { (*dacl).AceCount }) {
        let mut ace = std::ptr::null_mut();
        unsafe { BOOL(security::GetAce(dacl, index, &mut ace)).ok() }
            .context("read sandbox bin ACE")?;
        let header = unsafe { &*ace.cast::<security::ACE_HEADER>() };
        ensure!(
            header.AceType == 0 && header.AceSize >= 16,
            "unsupported legacy sandbox bin ACE"
        );
        let allow = unsafe { &mut *ace.cast::<security::ACCESS_ALLOWED_ACE>() };
        let sid = std::ptr::addr_of!(allow.SidStart).cast::<u8>();
        let sid_len = usize::from(allow.Header.AceSize)
            - std::mem::offset_of!(security::ACCESS_ALLOWED_ACE, SidStart);
        if unsafe { std::slice::from_raw_parts(sid, sid_len) } == system {
            let legacy_mask = fs::FILE_GENERIC_READ
                | fs::FILE_GENERIC_WRITE
                | fs::FILE_GENERIC_EXECUTE
                | fs::DELETE;
            ensure!(
                !matched
                    && allow.Mask == legacy_mask
                    && u32::from(allow.Header.AceFlags)
                        == (security::OBJECT_INHERIT_ACE | security::CONTAINER_INHERIT_ACE),
                "unexpected legacy SYSTEM permissions"
            );
            allow.Mask |= fs::WRITE_DAC;
            matched = true;
        }
    }
    ensure!(matched, "missing legacy SYSTEM allow entry");
    Ok(())
}

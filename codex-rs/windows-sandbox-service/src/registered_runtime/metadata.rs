//! Gives registered commands metadata access to the owner's two AppData roots.
//! No file contents or descendants are granted. Persist the managed account and
//! each owner-authorized root before granting access, so cleanup includes former roots.

use std::io;
use std::os::windows::io::AsRawHandle;
use std::os::windows::io::OwnedHandle;
use std::path::Path;
use std::ptr;

use anyhow::Context;
use anyhow::Result;
use anyhow::ensure;
use codex_windows_sandbox::DirectoryOpenDisposition;
use codex_windows_sandbox::LocalSid;
use codex_windows_sandbox::open_directory_no_reparse;
use windows::Win32::Foundation::BOOL;
use windows_sys::Win32::Foundation::HANDLE;
use windows_sys::Win32::Foundation::LocalFree;
use windows_sys::Win32::Foundation::RtlNtStatusToDosError;
use windows_sys::Win32::Security as security;
use windows_sys::Win32::Security::Authorization as authorization;
use windows_sys::Win32::Storage::FileSystem as filesystem;
use windows_sys::Win32::UI::Shell as shell;

use super::known_folder::path as known_folder;
use crate::installation_record::InstallationRecord;
use crate::package_lifecycle::with_owner_impersonation;

const OWNER_FOLDER_FLAGS: u32 =
    (shell::KF_FLAG_DONT_VERIFY | shell::KF_FLAG_NO_PACKAGE_REDIRECTION) as u32;

#[link(name = "ntdll")]
unsafe extern "system" {
    fn NtSetSecurityObject(
        handle: HANDLE,
        information: u32,
        descriptor: *const security::SECURITY_DESCRIPTOR,
    ) -> i32;
}

pub(super) fn grant(
    owner_token: HANDLE,
    user_sid: &str,
    record: &mut InstallationRecord,
) -> Result<()> {
    ensure!(
        record
            .runtime()?
            .accounts
            .iter()
            .any(|account| account.user_sid == user_sid),
        "AppData metadata access requires a recorded managed account"
    );
    let sid = LocalSid::from_string(user_sid)?;
    for folder in [shell::FOLDERID_LocalAppData, shell::FOLDERID_RoamingAppData] {
        let path = known_folder(owner_token, &folder, OWNER_FOLDER_FLAGS)?;
        edit_root_acl(owner_token, &path, |acl| {
            // The owner-authorized directory and its ancestors are pinned here.
            // Commit provenance before any ACE mutation, including a partial grant.
            if !record.runtime()?.metadata_roots.contains(&path) {
                record.runtime_mut()?.metadata_roots.push(path.clone());
                crate::installation_record::save_runtime(record)?;
            }
            if metadata_ace(acl, &sid)?.is_some() {
                return Ok(false);
            }
            // Packaged Known Folder APIs verify these directory objects even when
            // APPDATA is inherited. Do not grant listing, contents, or inheritance.
            insert_metadata_ace(acl, &sid)?;
            Ok(true)
        })?;
    }
    Ok(())
}

/// Removes exact grants from all recorded roots before account deletion.
pub(crate) fn remove(token: HANDLE, record: &InstallationRecord) -> Result<()> {
    let runtime = record.runtime()?;
    let accounts = &runtime.accounts;
    if accounts.is_empty() {
        return Ok(());
    }
    let mut errors = Vec::new();
    // Every grant records its root before changing permissions, including redirects.
    for path in &runtime.metadata_roots {
        let result = edit_root_acl(token, path, |acl| {
            let mut changed = false;
            for account in accounts {
                let sid = LocalSid::from_string(&account.user_sid)?;
                while let Some(index) = metadata_ace(acl, &sid)? {
                    unsafe { BOOL(security::DeleteAce(acl.as_mut_ptr().cast(), index)).ok() }
                        .context("remove AppData metadata ACE")?;
                    changed = true;
                }
            }
            Ok(changed)
        });
        if let Err(error) = result
            && error
                .downcast_ref::<io::Error>()
                .is_none_or(|error| error.kind() != io::ErrorKind::NotFound)
        {
            errors.push(format!("{error:#}"));
        }
    }
    ensure!(
        errors.is_empty(),
        "AppData metadata cleanup: {}",
        errors.join("; ")
    );
    Ok(())
}

fn edit_root_acl(
    token: HANDLE,
    path: &Path,
    edit: impl FnOnce(&mut Vec<u32>) -> Result<bool>,
) -> Result<()> {
    let (_pins, handle) = with_owner_impersonation(token, || {
        let mut pins = Vec::new();
        crate::ipc::pin_existing_ancestors(path, &mut pins)?;
        let handle = open_directory_no_reparse(
            path,
            filesystem::READ_CONTROL
                | filesystem::WRITE_DAC
                | filesystem::FILE_READ_ATTRIBUTES
                | filesystem::FILE_TRAVERSE,
            filesystem::FILE_SHARE_READ | filesystem::FILE_SHARE_WRITE,
            DirectoryOpenDisposition::OpenExisting,
        )?;
        Ok((pins, handle))
    })?;
    let Some((mut acl, control)) = read_acl(&handle)? else {
        return Ok(()); // A null DACL already allows metadata; never replace it.
    };
    if edit(&mut acl)? {
        write_acl(&handle, &mut acl, control)?;
    }
    Ok(())
}

fn read_acl(handle: &OwnedHandle) -> Result<Option<(Vec<u32>, u16)>> {
    let mut acl = ptr::null_mut();
    let mut descriptor = ptr::null_mut();
    let status = unsafe {
        authorization::GetSecurityInfo(
            handle.as_raw_handle() as _,
            authorization::SE_FILE_OBJECT,
            security::DACL_SECURITY_INFORMATION,
            ptr::null_mut(),
            ptr::null_mut(),
            &mut acl,
            ptr::null_mut(),
            &mut descriptor,
        )
    };
    if status != 0 {
        return Err(io::Error::from_raw_os_error(status as i32)).context("read AppData DACL");
    }
    let result = (|| {
        if acl.is_null() {
            return Ok(None);
        }
        ensure!(
            unsafe { security::IsValidAcl(acl) } != 0,
            "invalid AppData DACL"
        );
        let length = unsafe { (*acl).AclSize } as usize;
        let mut control = 0;
        let mut revision = 0;
        let queried = unsafe {
            security::GetSecurityDescriptorControl(descriptor, &mut control, &mut revision)
        };
        BOOL(queried).ok().context("read AppData DACL control")?;
        let mut buffer = vec![0u32; length.div_ceil(4)];
        unsafe {
            ptr::copy_nonoverlapping(acl.cast::<u8>(), buffer.as_mut_ptr().cast(), length);
        }
        Ok(Some((buffer, control)))
    })();
    unsafe { LocalFree(descriptor as _) };
    result
}

fn metadata_ace(acl: &mut [u32], sid: &LocalSid) -> Result<Option<u32>> {
    let acl = acl.as_mut_ptr().cast::<security::ACL>();
    for index in 0..u32::from(unsafe { (*acl).AceCount }) {
        let mut raw = ptr::null_mut();
        unsafe { BOOL(security::GetAce(acl, index, &mut raw)).ok() }.context("read AppData ACE")?;
        let offset = raw as usize - acl as usize;
        ensure!(
            offset + std::mem::size_of::<security::ACE_HEADER>()
                <= usize::from(unsafe { (*acl).AclSize }),
            "AppData ACE header exceeds ACL"
        );
        let header = unsafe { &*raw.cast::<security::ACE_HEADER>() };
        ensure!(
            offset + usize::from(header.AceSize) <= usize::from(unsafe { (*acl).AclSize }),
            "AppData ACE exceeds ACL"
        );
        if header.AceType == 0 && header.AceFlags == 0 {
            ensure!(header.AceSize >= 16, "AppData allow ACE is truncated");
            let ace = unsafe { &*raw.cast::<security::ACCESS_ALLOWED_ACE>() };
            let sid_pointer = ptr::addr_of!(ace.SidStart) as *mut std::ffi::c_void;
            let subauthorities = unsafe { *sid_pointer.cast::<u8>().add(1) } as usize;
            ensure!(
                subauthorities <= 15 && 16 + subauthorities * 4 <= usize::from(header.AceSize),
                "AppData allow ACE SID is truncated"
            );
            ensure!(
                unsafe { security::IsValidSid(sid_pointer) } != 0,
                "AppData allow ACE SID is invalid"
            );
            if ace.Mask == filesystem::FILE_READ_ATTRIBUTES
                && unsafe { security::EqualSid(sid_pointer, sid.as_ptr()) } != 0
            {
                return Ok(Some(index));
            }
        }
    }
    Ok(None)
}

fn insert_metadata_ace(acl: &mut Vec<u32>, sid: &LocalSid) -> Result<()> {
    let capacity = usize::from(unsafe { (*acl.as_ptr().cast::<security::ACL>()).AclSize })
        + 8
        + unsafe { security::GetLengthSid(sid.as_ptr()) } as usize;
    ensure!(capacity <= u16::MAX as usize, "AppData DACL is full");
    acl.resize(capacity.div_ceil(4), 0);
    unsafe { (*acl.as_mut_ptr().cast::<security::ACL>()).AclSize = capacity as u16 };
    let mut single = [0u32; 32];
    let single_acl = single.as_mut_ptr().cast();
    unsafe {
        BOOL(security::InitializeAcl(
            single_acl,
            std::mem::size_of_val(&single) as u32,
            security::ACL_REVISION_DS,
        ))
        .ok()
        .context("initialize AppData metadata ACE")?;
        BOOL(security::AddAccessAllowedAceEx(
            single_acl,
            security::ACL_REVISION_DS,
            0,
            filesystem::FILE_READ_ATTRIBUTES,
            sid.as_ptr(),
        ))
        .ok()
        .context("construct AppData metadata ACE")?;
    }
    let mut added = ptr::null_mut();
    unsafe { BOOL(security::GetAce(single_acl, 0, &mut added)).ok() }
        .context("read new AppData metadata ACE")?;
    let acl = acl.as_mut_ptr().cast::<security::ACL>();
    let mut position = u32::from(unsafe { (*acl).AceCount });
    for index in 0..position {
        let mut raw = ptr::null_mut();
        unsafe { BOOL(security::GetAce(acl, index, &mut raw)).ok() }
            .context("read AppData ACE order")?;
        if unsafe { (*raw.cast::<security::ACE_HEADER>()).AceFlags } & security::INHERITED_ACE as u8
            != 0
        {
            position = index;
            break;
        }
    }
    unsafe {
        BOOL(security::AddAce(
            acl,
            security::ACL_REVISION_DS,
            position,
            added,
            u32::from((*added.cast::<security::ACE_HEADER>()).AceSize),
        ))
        .ok()
    }
    .context("insert AppData metadata ACE")?;
    Ok(())
}

fn write_acl(handle: &OwnedHandle, acl: &mut [u32], control: u16) -> Result<()> {
    let mut descriptor: security::SECURITY_DESCRIPTOR = unsafe { std::mem::zeroed() };
    let preserved = security::SE_DACL_PROTECTED | security::SE_DACL_AUTO_INHERITED;
    let raw_descriptor = (&mut descriptor as *mut security::SECURITY_DESCRIPTOR).cast();
    unsafe {
        BOOL(security::InitializeSecurityDescriptor(raw_descriptor, 1))
            .ok()
            .context("initialize AppData security descriptor")?;
        BOOL(security::SetSecurityDescriptorDacl(
            raw_descriptor,
            1,
            acl.as_mut_ptr().cast(),
            i32::from(control & security::SE_DACL_DEFAULTED != 0),
        ))
        .ok()
        .context("set AppData DACL")?;
        BOOL(security::SetSecurityDescriptorControl(
            raw_descriptor,
            preserved,
            control & preserved,
        ))
        .ok()
        .context("preserve AppData DACL control")?;
    }
    // SetSecurityInfo recursively reapplies existing inheritable ACEs. The native
    // object setter changes only this retained, owner-authorized directory handle.
    let status = unsafe {
        NtSetSecurityObject(
            handle.as_raw_handle() as _,
            security::DACL_SECURITY_INFORMATION,
            &descriptor,
        )
    };
    if status < 0 {
        return Err(io::Error::from_raw_os_error(
            unsafe { RtlNtStatusToDosError(status) } as i32,
        ))
        .context("write AppData metadata DACL");
    }
    Ok(())
}

#[cfg(test)]
#[path = "metadata_tests.rs"]
mod tests;

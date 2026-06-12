use std::ffi::c_void;
use std::path::Path;

use anyhow::Result;
use anyhow::anyhow;
use windows_sys::Win32::Foundation::CloseHandle;
use windows_sys::Win32::Foundation::ERROR_SUCCESS;
use windows_sys::Win32::Foundation::HLOCAL;
use windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE;
use windows_sys::Win32::Foundation::LocalFree;
use windows_sys::Win32::Security::ACL;
use windows_sys::Win32::Security::Authorization::EXPLICIT_ACCESS_W;
use windows_sys::Win32::Security::Authorization::SetEntriesInAclW;
use windows_sys::Win32::Security::Authorization::SetSecurityInfo;
use windows_sys::Win32::Security::Authorization::TRUSTEE_IS_SID;
use windows_sys::Win32::Security::Authorization::TRUSTEE_IS_UNKNOWN;
use windows_sys::Win32::Security::Authorization::TRUSTEE_W;
use windows_sys::Win32::Security::DACL_SECURITY_INFORMATION;
use windows_sys::Win32::Security::PROTECTED_DACL_SECURITY_INFORMATION;
use windows_sys::Win32::Storage::FileSystem::CreateFileW;
use windows_sys::Win32::Storage::FileSystem::DELETE;
use windows_sys::Win32::Storage::FileSystem::FILE_DELETE_CHILD;
use windows_sys::Win32::Storage::FileSystem::FILE_FLAG_BACKUP_SEMANTICS;
use windows_sys::Win32::Storage::FileSystem::FILE_GENERIC_EXECUTE;
use windows_sys::Win32::Storage::FileSystem::FILE_GENERIC_READ;
use windows_sys::Win32::Storage::FileSystem::FILE_GENERIC_WRITE;
use windows_sys::Win32::Storage::FileSystem::FILE_SHARE_DELETE;
use windows_sys::Win32::Storage::FileSystem::FILE_SHARE_READ;
use windows_sys::Win32::Storage::FileSystem::FILE_SHARE_WRITE;
use windows_sys::Win32::Storage::FileSystem::OPEN_EXISTING;

use crate::acl::ensure_current_user_cleanup_access;
use crate::acl::path_mask_allows;
use crate::winutil::resolve_sid;
use crate::winutil::to_wide;

const CONTAINER_INHERIT_ACE: u32 = 0x2;
const OBJECT_INHERIT_ACE: u32 = 0x1;

#[test]
fn current_user_cleanup_access_grants_delete_rights() -> Result<()> {
    let temp_dir = tempfile::TempDir::new()?;
    let request_dir = temp_dir.path().join("wrapper-requests");
    std::fs::create_dir(&request_dir)?;

    let real_user = std::env::var("USERNAME").unwrap_or_else(|_| "Administrators".to_string());
    let real_user_sid = resolve_sid(&real_user)?;
    let real_user_sid = real_user_sid.as_ptr() as *mut c_void;
    set_delete_less_allow_ace_for_test(&request_dir, real_user_sid)?;
    assert!(!path_mask_allows(
        &request_dir,
        &[real_user_sid],
        DELETE | FILE_DELETE_CHILD,
        /*require_all_bits*/ true,
    )?);

    ensure_current_user_cleanup_access(&request_dir)?;

    assert!(path_mask_allows(
        &request_dir,
        &[real_user_sid],
        DELETE | FILE_DELETE_CHILD,
        /*require_all_bits*/ true,
    )?);
    Ok(())
}

fn set_delete_less_allow_ace_for_test(path: &Path, psid: *mut c_void) -> Result<()> {
    unsafe {
        let trustee = TRUSTEE_W {
            pMultipleTrustee: std::ptr::null_mut(),
            MultipleTrusteeOperation: 0,
            TrusteeForm: TRUSTEE_IS_SID,
            TrusteeType: TRUSTEE_IS_UNKNOWN,
            ptstrName: psid as *mut u16,
        };
        let mut explicit: EXPLICIT_ACCESS_W = std::mem::zeroed();
        explicit.grfAccessPermissions =
            FILE_GENERIC_READ | FILE_GENERIC_WRITE | FILE_GENERIC_EXECUTE;
        explicit.grfAccessMode = 2; // SET_ACCESS
        explicit.grfInheritance = CONTAINER_INHERIT_ACE | OBJECT_INHERIT_ACE;
        explicit.Trustee = trustee;
        let mut p_new_dacl: *mut ACL = std::ptr::null_mut();
        let code = SetEntriesInAclW(1, &explicit, std::ptr::null_mut(), &mut p_new_dacl);
        if code != ERROR_SUCCESS {
            return Err(anyhow!("SetEntriesInAclW failed: {code}"));
        }

        let desired = 0x00020000 | 0x00040000; // READ_CONTROL | WRITE_DAC
        let h = CreateFileW(
            to_wide(path).as_ptr(),
            desired,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            std::ptr::null_mut(),
            OPEN_EXISTING,
            FILE_FLAG_BACKUP_SEMANTICS,
            0,
        );
        if h == 0 || h == INVALID_HANDLE_VALUE {
            LocalFree(p_new_dacl as HLOCAL);
            return Err(anyhow!("CreateFileW failed for {}", path.display()));
        }
        let code = SetSecurityInfo(
            h,
            1,
            DACL_SECURITY_INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            p_new_dacl,
            std::ptr::null_mut(),
        );
        CloseHandle(h);
        if !p_new_dacl.is_null() {
            LocalFree(p_new_dacl as HLOCAL);
        }
        if code != ERROR_SUCCESS {
            return Err(anyhow!("SetSecurityInfo failed: {code}"));
        }
    }
    Ok(())
}

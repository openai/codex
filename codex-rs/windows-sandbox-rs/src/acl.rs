use crate::winutil::to_wide;
use anyhow::anyhow;
use anyhow::Result;
use std::ffi::c_void;
use std::path::Path;
use windows_sys::Win32::Foundation::CloseHandle;
use windows_sys::Win32::Foundation::LocalFree;
use windows_sys::Win32::Foundation::ERROR_SUCCESS;
use windows_sys::Win32::Foundation::HLOCAL;
use windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE;
use windows_sys::Win32::Security::AclSizeInformation;
use windows_sys::Win32::Security::Authorization::GetNamedSecurityInfoW;
use windows_sys::Win32::Security::Authorization::GetSecurityInfo;
use windows_sys::Win32::Security::Authorization::SetEntriesInAclW;
use windows_sys::Win32::Security::Authorization::SetNamedSecurityInfoW;
use windows_sys::Win32::Security::Authorization::SetSecurityInfo;
use windows_sys::Win32::Security::Authorization::EXPLICIT_ACCESS_W;
use windows_sys::Win32::Security::Authorization::TRUSTEE_IS_SID;
use windows_sys::Win32::Security::Authorization::TRUSTEE_IS_UNKNOWN;
use windows_sys::Win32::Security::Authorization::TRUSTEE_W;
use windows_sys::Win32::Security::EqualSid;
use windows_sys::Win32::Security::GetAce;
use windows_sys::Win32::Security::GetAclInformation;
use windows_sys::Win32::Security::MapGenericMask;
use windows_sys::Win32::Security::ACCESS_ALLOWED_ACE;
use windows_sys::Win32::Security::ACE_HEADER;
use windows_sys::Win32::Security::ACL;
use windows_sys::Win32::Security::ACL_SIZE_INFORMATION;
use windows_sys::Win32::Security::DACL_SECURITY_INFORMATION;
use windows_sys::Win32::Security::GENERIC_MAPPING;
use windows_sys::Win32::Storage::FileSystem::CreateFileW;
use windows_sys::Win32::Storage::FileSystem::FILE_ALL_ACCESS;
use windows_sys::Win32::Storage::FileSystem::FILE_APPEND_DATA;
use windows_sys::Win32::Storage::FileSystem::FILE_ATTRIBUTE_NORMAL;
use windows_sys::Win32::Storage::FileSystem::FILE_FLAG_BACKUP_SEMANTICS;
use windows_sys::Win32::Storage::FileSystem::FILE_DELETE_CHILD;
use windows_sys::Win32::Storage::FileSystem::FILE_GENERIC_EXECUTE;
use windows_sys::Win32::Storage::FileSystem::FILE_GENERIC_READ;
use windows_sys::Win32::Storage::FileSystem::FILE_GENERIC_WRITE;
use windows_sys::Win32::Storage::FileSystem::FILE_SHARE_DELETE;
use windows_sys::Win32::Storage::FileSystem::FILE_SHARE_READ;
use windows_sys::Win32::Storage::FileSystem::FILE_SHARE_WRITE;
use windows_sys::Win32::Storage::FileSystem::FILE_WRITE_ATTRIBUTES;
use windows_sys::Win32::Storage::FileSystem::FILE_WRITE_DATA;
use windows_sys::Win32::Storage::FileSystem::FILE_WRITE_EA;
use windows_sys::Win32::Storage::FileSystem::OPEN_EXISTING;
use windows_sys::Win32::Storage::FileSystem::READ_CONTROL;
use windows_sys::Win32::Storage::FileSystem::DELETE;
const SE_KERNEL_OBJECT: u32 = 6;
const INHERIT_ONLY_ACE: u8 = 0x08;
const GENERIC_WRITE_MASK: u32 = 0x4000_0000;
const DENY_ACCESS: i32 = 3;

/// Fetch DACL via handle-based query; caller must LocalFree the returned SD.
///
/// # Safety
/// Caller must free the returned security descriptor with `LocalFree` and pass an existing path.
pub unsafe fn fetch_dacl_handle(path: &Path) -> Result<(*mut ACL, *mut c_void)> {
    let wpath = to_wide(path);
    let h = CreateFileW(
        wpath.as_ptr(),
        READ_CONTROL,
        FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
        std::ptr::null_mut(),
        OPEN_EXISTING,
        FILE_FLAG_BACKUP_SEMANTICS,
        0,
    );
    if h == INVALID_HANDLE_VALUE {
        return Err(anyhow!("CreateFileW failed for {}", path.display()));
    }
    let mut p_sd: *mut c_void = std::ptr::null_mut();
    let mut p_dacl: *mut ACL = std::ptr::null_mut();
    let code = GetSecurityInfo(
        h,
        1, // SE_FILE_OBJECT
        DACL_SECURITY_INFORMATION,
        std::ptr::null_mut(),
        std::ptr::null_mut(),
        &mut p_dacl,
        std::ptr::null_mut(),
        &mut p_sd,
    );
    CloseHandle(h);
    if code != ERROR_SUCCESS {
        return Err(anyhow!(
            "GetSecurityInfo failed for {}: {}",
            path.display(),
            code
        ));
    }
    Ok((p_dacl, p_sd))
}

/// Fast mask-based check: does an ACE for provided SIDs grant the desired mask? Skips inherit-only.
/// When `require_all_bits` is true, all bits in `desired_mask` must be present; otherwise any bit suffices.
pub unsafe fn dacl_mask_allows(
    p_dacl: *mut ACL,
    psids: &[*mut c_void],
    desired_mask: u32,
    require_all_bits: bool,
) -> bool {
    if p_dacl.is_null() {
        return false;
    }
    let mut info: ACL_SIZE_INFORMATION = std::mem::zeroed();
    let ok = GetAclInformation(
        p_dacl as *const ACL,
        &mut info as *mut _ as *mut c_void,
        std::mem::size_of::<ACL_SIZE_INFORMATION>() as u32,
        AclSizeInformation,
    );
    if ok == 0 {
        return false;
    }
    let mapping = GENERIC_MAPPING {
        GenericRead: FILE_GENERIC_READ,
        GenericWrite: FILE_GENERIC_WRITE,
        GenericExecute: FILE_GENERIC_EXECUTE,
        GenericAll: FILE_ALL_ACCESS,
    };
    for i in 0..(info.AceCount as usize) {
        let mut p_ace: *mut c_void = std::ptr::null_mut();
        if GetAce(p_dacl as *const ACL, i as u32, &mut p_ace) == 0 {
            continue;
        }
        let hdr = &*(p_ace as *const ACE_HEADER);
        if hdr.AceType != 0 {
            continue; // not ACCESS_ALLOWED
        }
        if (hdr.AceFlags & INHERIT_ONLY_ACE) != 0 {
            continue;
        }
        let base = p_ace as usize;
        let sid_ptr =
            (base + std::mem::size_of::<ACE_HEADER>() + std::mem::size_of::<u32>()) as *mut c_void;
        let mut matched = false;
        for sid in psids {
            if EqualSid(sid_ptr, *sid) != 0 {
                matched = true;
                break;
            }
        }
        if !matched {
            continue;
        }
        let ace = &*(p_ace as *const ACCESS_ALLOWED_ACE);
        let mut mask = ace.Mask;
        MapGenericMask(&mut mask, &mapping);
        if (require_all_bits && (mask & desired_mask) == desired_mask)
            || (!require_all_bits && (mask & desired_mask) != 0)
        {
            return true;
        }
    }
    false
}

/// Path-based wrapper around the mask check (single DACL fetch).
pub fn path_mask_allows(
    path: &Path,
    psids: &[*mut c_void],
    desired_mask: u32,
    require_all_bits: bool,
) -> Result<bool> {
    unsafe {
        let (p_dacl, sd) = fetch_dacl_handle(path)?;
        let has = dacl_mask_allows(p_dacl, psids, desired_mask, require_all_bits);
        if !sd.is_null() {
            LocalFree(sd as HLOCAL);
        }
        Ok(has)
    }
}

pub unsafe fn dacl_has_write_allow_for_sid(p_dacl: *mut ACL, psid: *mut c_void) -> bool {
    if p_dacl.is_null() {
        return false;
    }
    let mut info: ACL_SIZE_INFORMATION = std::mem::zeroed();
    let ok = GetAclInformation(
        p_dacl as *const ACL,
        &mut info as *mut _ as *mut c_void,
        std::mem::size_of::<ACL_SIZE_INFORMATION>() as u32,
        AclSizeInformation,
    );
    if ok == 0 {
        return false;
    }
    let count = info.AceCount as usize;
    for i in 0..count {
        let mut p_ace: *mut c_void = std::ptr::null_mut();
        if GetAce(p_dacl as *const ACL, i as u32, &mut p_ace) == 0 {
            continue;
        }
        let hdr = &*(p_ace as *const ACE_HEADER);
        if hdr.AceType != 0 {
            continue; // ACCESS_ALLOWED_ACE_TYPE
        }
        // Ignore ACEs that are inherit-only (do not apply to the current object)
        if (hdr.AceFlags & INHERIT_ONLY_ACE) != 0 {
            continue;
        }
        let ace = &*(p_ace as *const ACCESS_ALLOWED_ACE);
        let mask = ace.Mask;
        let base = p_ace as usize;
        let sid_ptr =
            (base + std::mem::size_of::<ACE_HEADER>() + std::mem::size_of::<u32>()) as *mut c_void;
        let eq = EqualSid(sid_ptr, psid);
        if eq != 0 && (mask & FILE_GENERIC_WRITE) != 0 {
            return true;
        }
    }
    false
}

pub unsafe fn dacl_has_write_deny_for_sid(p_dacl: *mut ACL, psid: *mut c_void) -> bool {
    if p_dacl.is_null() {
        return false;
    }
    let mut info: ACL_SIZE_INFORMATION = std::mem::zeroed();
    let ok = GetAclInformation(
        p_dacl as *const ACL,
        &mut info as *mut _ as *mut c_void,
        std::mem::size_of::<ACL_SIZE_INFORMATION>() as u32,
        AclSizeInformation,
    );
    if ok == 0 {
        return false;
    }
    let deny_write_mask = FILE_GENERIC_WRITE
        | FILE_WRITE_DATA
        | FILE_APPEND_DATA
        | FILE_WRITE_EA
        | FILE_WRITE_ATTRIBUTES
        | GENERIC_WRITE_MASK;
    for i in 0..info.AceCount {
        let mut p_ace: *mut c_void = std::ptr::null_mut();
        if GetAce(p_dacl as *const ACL, i, &mut p_ace) == 0 {
            continue;
        }
        let hdr = &*(p_ace as *const ACE_HEADER);
        if hdr.AceType != 1 {
            continue; // ACCESS_DENIED_ACE_TYPE
        }
        if (hdr.AceFlags & INHERIT_ONLY_ACE) != 0 {
            continue;
        }
        let ace = &*(p_ace as *const ACCESS_ALLOWED_ACE);
        let base = p_ace as usize;
        let sid_ptr =
            (base + std::mem::size_of::<ACE_HEADER>() + std::mem::size_of::<u32>()) as *mut c_void;
        if EqualSid(sid_ptr, psid) != 0 && (ace.Mask & deny_write_mask) != 0 {
            return true;
        }
    }
    false
}

const WRITE_ALLOW_MASK: u32 = FILE_GENERIC_READ
    | FILE_GENERIC_WRITE
    | FILE_GENERIC_EXECUTE
    | DELETE
    | FILE_DELETE_CHILD;


unsafe fn ensure_allow_mask_aces_with_inheritance_impl(
    path: &Path,
    sids: &[*mut c_void],
    allow_mask: u32,
    inheritance: u32,
) -> Result<bool> {
    let (p_dacl, p_sd) = fetch_dacl_handle(path)?;
    let mut entries: Vec<EXPLICIT_ACCESS_W> = Vec::new();
    for sid in sids {
        if dacl_mask_allows(p_dacl, &[*sid], allow_mask, true) {
            continue;
        }
        entries.push(EXPLICIT_ACCESS_W {
            grfAccessPermissions: allow_mask,
            grfAccessMode: 2, // SET_ACCESS
            grfInheritance: inheritance,
            Trustee: TRUSTEE_W {
                pMultipleTrustee: std::ptr::null_mut(),
                MultipleTrusteeOperation: 0,
                TrusteeForm: TRUSTEE_IS_SID,
                TrusteeType: TRUSTEE_IS_UNKNOWN,
                ptstrName: *sid as *mut u16,
            },
        });
    }
    let mut added = false;
    if !entries.is_empty() {
        let mut p_new_dacl: *mut ACL = std::ptr::null_mut();
        let code2 = SetEntriesInAclW(
            entries.len() as u32,
            entries.as_ptr(),
            p_dacl,
            &mut p_new_dacl,
        );
        if code2 == ERROR_SUCCESS {
            let code3 = SetNamedSecurityInfoW(
                to_wide(path).as_ptr() as *mut u16,
                1,
                DACL_SECURITY_INFORMATION,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                p_new_dacl,
                std::ptr::null_mut(),
            );
            if code3 == ERROR_SUCCESS {
                added = true;
                if !p_new_dacl.is_null() {
                    LocalFree(p_new_dacl as HLOCAL);
                }
            } else {
                if !p_new_dacl.is_null() {
                    LocalFree(p_new_dacl as HLOCAL);
                }
                if !p_sd.is_null() {
                    LocalFree(p_sd as HLOCAL);
                }
                return Err(anyhow!("SetNamedSecurityInfoW failed: {}", code3));
            }
        } else {
            if !p_sd.is_null() {
                LocalFree(p_sd as HLOCAL);
            }
            return Err(anyhow!("SetEntriesInAclW failed: {}", code2));
        }
    }
    if !p_sd.is_null() {
        LocalFree(p_sd as HLOCAL);
    }
    Ok(added)
}

/// Ensure all provided SIDs have an allow ACE with the requested mask on the path.
/// Returns true if any ACE was added.
///
/// # Safety
/// Caller must pass valid SID pointers and an existing path; free the returned security descriptor with `LocalFree`.
pub unsafe fn ensure_allow_mask_aces_with_inheritance(
    path: &Path,
    sids: &[*mut c_void],
    allow_mask: u32,
    inheritance: u32,
) -> Result<bool> {
    ensure_allow_mask_aces_with_inheritance_impl(path, sids, allow_mask, inheritance)
}

/// Ensure all provided SIDs have an allow ACE with the requested mask on the path.
/// Returns true if any ACE was added.
///
/// # Safety
/// Caller must pass valid SID pointers and an existing path; free the returned security descriptor with `LocalFree`.
pub unsafe fn ensure_allow_mask_aces(
    path: &Path,
    sids: &[*mut c_void],
    allow_mask: u32,
) -> Result<bool> {
    ensure_allow_mask_aces_with_inheritance(
        path,
        sids,
        allow_mask,
        CONTAINER_INHERIT_ACE | OBJECT_INHERIT_ACE,
    )
}

/// Ensure all provided SIDs have a write-capable allow ACE on the path.
/// Returns true if any ACE was added.
///
/// # Safety
/// Caller must pass valid SID pointers and an existing path; free the returned security descriptor with `LocalFree`.
pub unsafe fn ensure_allow_write_aces(path: &Path, sids: &[*mut c_void]) -> Result<bool> {
    ensure_allow_mask_aces(path, sids, WRITE_ALLOW_MASK)
}

/// Adds an allow ACE granting read/write/execute to the given SID on the target path.
///
/// # Safety
/// Caller must ensure `psid` points to a valid SID and `path` refers to an existing file or directory.
pub unsafe fn add_allow_ace(path: &Path, psid: *mut c_void) -> Result<bool> {
    let mut p_sd: *mut c_void = std::ptr::null_mut();
    let mut p_dacl: *mut ACL = std::ptr::null_mut();
    let code = GetNamedSecurityInfoW(
        to_wide(path).as_ptr(),
        1,
        DACL_SECURITY_INFORMATION,
        std::ptr::null_mut(),
        std::ptr::null_mut(),
        &mut p_dacl,
        std::ptr::null_mut(),
        &mut p_sd,
    );
    if code != ERROR_SUCCESS {
        return Err(anyhow!("GetNamedSecurityInfoW failed: {}", code));
    }
    // Already has write? Skip costly DACL rewrite.
    if dacl_has_write_allow_for_sid(p_dacl, psid) {
        if !p_sd.is_null() {
            LocalFree(p_sd as HLOCAL);
        }
        return Ok(false);
    }
    let mut added = false;
    // Always ensure write is present: if an allow ACE exists without write, add one with write+RX.
    let trustee = TRUSTEE_W {
        pMultipleTrustee: std::ptr::null_mut(),
        MultipleTrusteeOperation: 0,
        TrusteeForm: TRUSTEE_IS_SID,
        TrusteeType: TRUSTEE_IS_UNKNOWN,
        ptstrName: psid as *mut u16,
    };
    let mut explicit: EXPLICIT_ACCESS_W = std::mem::zeroed();
    explicit.grfAccessPermissions = FILE_GENERIC_READ | FILE_GENERIC_WRITE | FILE_GENERIC_EXECUTE;
    explicit.grfAccessMode = 2; // SET_ACCESS
    explicit.grfInheritance = CONTAINER_INHERIT_ACE | OBJECT_INHERIT_ACE;
    explicit.Trustee = trustee;
    let mut p_new_dacl: *mut ACL = std::ptr::null_mut();
    let code2 = SetEntriesInAclW(1, &explicit, p_dacl, &mut p_new_dacl);
    if code2 == ERROR_SUCCESS {
        let code3 = SetNamedSecurityInfoW(
            to_wide(path).as_ptr() as *mut u16,
            1,
            DACL_SECURITY_INFORMATION,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            p_new_dacl,
            std::ptr::null_mut(),
        );
        if code3 == ERROR_SUCCESS {
            added = !dacl_has_write_allow_for_sid(p_dacl, psid);
        }
        if !p_new_dacl.is_null() {
            LocalFree(p_new_dacl as HLOCAL);
        }
    }
    if !p_sd.is_null() {
        LocalFree(p_sd as HLOCAL);
    }
    Ok(added)
}

/// Adds a deny ACE to prevent write/append/delete for the given SID on the target path.
///
/// # Safety
/// Caller must ensure `psid` points to a valid SID and `path` refers to an existing file or directory.
pub unsafe fn add_deny_write_ace(path: &Path, psid: *mut c_void) -> Result<bool> {
    let mut p_sd: *mut c_void = std::ptr::null_mut();
    let mut p_dacl: *mut ACL = std::ptr::null_mut();
    let code = GetNamedSecurityInfoW(
        to_wide(path).as_ptr(),
        1,
        DACL_SECURITY_INFORMATION,
        std::ptr::null_mut(),
        std::ptr::null_mut(),
        &mut p_dacl,
        std::ptr::null_mut(),
        &mut p_sd,
    );
    if code != ERROR_SUCCESS {
        return Err(anyhow!("GetNamedSecurityInfoW failed: {}", code));
    }
    let mut added = false;
    if !dacl_has_write_deny_for_sid(p_dacl, psid) {
        let trustee = TRUSTEE_W {
            pMultipleTrustee: std::ptr::null_mut(),
            MultipleTrusteeOperation: 0,
            TrusteeForm: TRUSTEE_IS_SID,
            TrusteeType: TRUSTEE_IS_UNKNOWN,
            ptstrName: psid as *mut u16,
        };
        let mut explicit: EXPLICIT_ACCESS_W = std::mem::zeroed();
        explicit.grfAccessPermissions = FILE_GENERIC_WRITE
            | FILE_WRITE_DATA
            | FILE_APPEND_DATA
            | FILE_WRITE_EA
            | FILE_WRITE_ATTRIBUTES
            | GENERIC_WRITE_MASK;
        explicit.grfAccessMode = DENY_ACCESS;
        explicit.grfInheritance = CONTAINER_INHERIT_ACE | OBJECT_INHERIT_ACE;
        explicit.Trustee = trustee;
        let mut p_new_dacl: *mut ACL = std::ptr::null_mut();
        let code2 = SetEntriesInAclW(1, &explicit, p_dacl, &mut p_new_dacl);
        if code2 == ERROR_SUCCESS {
            let code3 = SetNamedSecurityInfoW(
                to_wide(path).as_ptr() as *mut u16,
                1,
                DACL_SECURITY_INFORMATION,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                p_new_dacl,
                std::ptr::null_mut(),
            );
            if code3 == ERROR_SUCCESS {
                added = true;
            }
            if !p_new_dacl.is_null() {
                LocalFree(p_new_dacl as HLOCAL);
            }
        }
    }
    if !p_sd.is_null() {
        LocalFree(p_sd as HLOCAL);
    }
    Ok(added)
}

pub unsafe fn revoke_ace(path: &Path, psid: *mut c_void) {
    let mut p_sd: *mut c_void = std::ptr::null_mut();
    let mut p_dacl: *mut ACL = std::ptr::null_mut();
    let code = GetNamedSecurityInfoW(
        to_wide(path).as_ptr(),
        1,
        DACL_SECURITY_INFORMATION,
        std::ptr::null_mut(),
        std::ptr::null_mut(),
        &mut p_dacl,
        std::ptr::null_mut(),
        &mut p_sd,
    );
    if code != ERROR_SUCCESS {
        if !p_sd.is_null() {
            LocalFree(p_sd as HLOCAL);
        }
        return;
    }
    let trustee = TRUSTEE_W {
        pMultipleTrustee: std::ptr::null_mut(),
        MultipleTrusteeOperation: 0,
        TrusteeForm: TRUSTEE_IS_SID,
        TrusteeType: TRUSTEE_IS_UNKNOWN,
        ptstrName: psid as *mut u16,
    };
    let mut explicit: EXPLICIT_ACCESS_W = std::mem::zeroed();
    explicit.grfAccessPermissions = 0;
    explicit.grfAccessMode = 4; // REVOKE_ACCESS
    explicit.grfInheritance = CONTAINER_INHERIT_ACE | OBJECT_INHERIT_ACE;
    explicit.Trustee = trustee;
    let mut p_new_dacl: *mut ACL = std::ptr::null_mut();
    let code2 = SetEntriesInAclW(1, &explicit, p_dacl, &mut p_new_dacl);
    if code2 == ERROR_SUCCESS {
        let _ = SetNamedSecurityInfoW(
            to_wide(path).as_ptr() as *mut u16,
            1,
            DACL_SECURITY_INFORMATION,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            p_new_dacl,
            std::ptr::null_mut(),
        );
        if !p_new_dacl.is_null() {
            LocalFree(p_new_dacl as HLOCAL);
        }
    }
    if !p_sd.is_null() {
        LocalFree(p_sd as HLOCAL);
    }
}

/// Grants RX to the null device for the given SID to support stdout/stderr redirection.
///
/// # Safety
/// Caller must ensure `psid` is a valid SID pointer.
pub unsafe fn allow_null_device(psid: *mut c_void) {
    let desired = 0x00020000 | 0x00040000; // READ_CONTROL | WRITE_DAC
    let h = CreateFileW(
        to_wide(r"\\\\.\\NUL").as_ptr(),
        desired,
        FILE_SHARE_READ | FILE_SHARE_WRITE,
        std::ptr::null_mut(),
        OPEN_EXISTING,
        FILE_ATTRIBUTE_NORMAL,
        0,
    );
    if h == 0 || h == INVALID_HANDLE_VALUE {
        return;
    }
    let mut p_sd: *mut c_void = std::ptr::null_mut();
    let mut p_dacl: *mut ACL = std::ptr::null_mut();
    let code = GetSecurityInfo(
        h,
        SE_KERNEL_OBJECT as i32,
        DACL_SECURITY_INFORMATION,
        std::ptr::null_mut(),
        std::ptr::null_mut(),
        &mut p_dacl,
        std::ptr::null_mut(),
        &mut p_sd,
    );
    if code == ERROR_SUCCESS {
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
        explicit.grfInheritance = 0;
        explicit.Trustee = trustee;
        let mut p_new_dacl: *mut ACL = std::ptr::null_mut();
        let code2 = SetEntriesInAclW(1, &explicit, p_dacl, &mut p_new_dacl);
        if code2 == ERROR_SUCCESS {
            let _ = SetSecurityInfo(
                h,
                SE_KERNEL_OBJECT as i32,
                DACL_SECURITY_INFORMATION,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                p_new_dacl,
                std::ptr::null_mut(),
            );
            if !p_new_dacl.is_null() {
                LocalFree(p_new_dacl as HLOCAL);
            }
        }
    }
    if !p_sd.is_null() {
        LocalFree(p_sd as HLOCAL);
    }
    CloseHandle(h);
}
const CONTAINER_INHERIT_ACE: u32 = 0x2;
const OBJECT_INHERIT_ACE: u32 = 0x1;

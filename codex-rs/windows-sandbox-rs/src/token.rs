use crate::winutil::to_wide;
use anyhow::anyhow;
use anyhow::Result;
use std::ffi::c_void;
use windows_sys::Win32::Foundation::CloseHandle;
use windows_sys::Win32::Foundation::GetLastError;
use windows_sys::Win32::Foundation::LocalFree;
use windows_sys::Win32::Foundation::ERROR_SUCCESS;
use windows_sys::Win32::Foundation::HANDLE;
use windows_sys::Win32::Foundation::HLOCAL;
use windows_sys::Win32::Foundation::LUID;
use windows_sys::Win32::Security::AdjustTokenPrivileges;
use windows_sys::Win32::Security::Authorization::SetEntriesInAclW;
use windows_sys::Win32::Security::Authorization::EXPLICIT_ACCESS_W;
use windows_sys::Win32::Security::Authorization::GRANT_ACCESS;
use windows_sys::Win32::Security::Authorization::TRUSTEE_IS_SID;
use windows_sys::Win32::Security::Authorization::TRUSTEE_IS_UNKNOWN;
use windows_sys::Win32::Security::Authorization::TRUSTEE_W;
use windows_sys::Win32::Security::CopySid;
use windows_sys::Win32::Security::CreateRestrictedToken;
use windows_sys::Win32::Security::CreateWellKnownSid;
use windows_sys::Win32::Security::GetLengthSid;
use windows_sys::Win32::Security::GetTokenInformation;
use windows_sys::Win32::Security::LookupPrivilegeValueW;
use windows_sys::Win32::Security::SetTokenInformation;

use windows_sys::Win32::Security::TokenDefaultDacl;
use windows_sys::Win32::Security::TokenGroups;
use windows_sys::Win32::Security::ACL;
use windows_sys::Win32::Security::SID_AND_ATTRIBUTES;
use windows_sys::Win32::Security::TOKEN_ADJUST_DEFAULT;
use windows_sys::Win32::Security::TOKEN_ADJUST_PRIVILEGES;
use windows_sys::Win32::Security::TOKEN_ADJUST_SESSIONID;
use windows_sys::Win32::Security::TOKEN_ASSIGN_PRIMARY;
use windows_sys::Win32::Security::TOKEN_DUPLICATE;
use windows_sys::Win32::Security::TOKEN_PRIVILEGES;
use windows_sys::Win32::Security::TOKEN_QUERY;
use windows_sys::Win32::System::Threading::GetCurrentProcess;

const DISABLE_MAX_PRIVILEGE: u32 = 0x01;
const LUA_TOKEN: u32 = 0x04;
const WRITE_RESTRICTED: u32 = 0x08;
const GENERIC_ALL: u32 = 0x1000_0000;
const WIN_WORLD_SID: i32 = 1;
const SE_GROUP_LOGON_ID: u32 = 0xC0000000;

#[repr(C)]
struct TokenDefaultDaclInfo {
    default_dacl: *mut ACL,
}

/// Sets a permissive default DACL so sandboxed processes can create pipes/IPC objects
/// without hitting ACCESS_DENIED when PowerShell builds pipelines.
unsafe fn set_default_dacl(h_token: HANDLE, sids: &[*mut c_void]) -> Result<()> {
    if sids.is_empty() {
        return Ok(());
    }
    let entries: Vec<EXPLICIT_ACCESS_W> = sids
        .iter()
        .map(|sid| EXPLICIT_ACCESS_W {
            grfAccessPermissions: GENERIC_ALL,
            grfAccessMode: GRANT_ACCESS,
            grfInheritance: 0,
            Trustee: TRUSTEE_W {
                pMultipleTrustee: std::ptr::null_mut(),
                MultipleTrusteeOperation: 0,
                TrusteeForm: TRUSTEE_IS_SID,
                TrusteeType: TRUSTEE_IS_UNKNOWN,
                ptstrName: *sid as *mut u16,
            },
        })
        .collect();
    let mut p_new_dacl: *mut ACL = std::ptr::null_mut();
    let res = SetEntriesInAclW(
        entries.len() as u32,
        entries.as_ptr(),
        std::ptr::null_mut(),
        &mut p_new_dacl,
    );
    if res != ERROR_SUCCESS {
        return Err(anyhow!("SetEntriesInAclW failed: {}", res));
    }
    let mut info = TokenDefaultDaclInfo {
        default_dacl: p_new_dacl,
    };
    let ok = SetTokenInformation(
        h_token,
        TokenDefaultDacl,
        &mut info as *mut _ as *mut c_void,
        std::mem::size_of::<TokenDefaultDaclInfo>() as u32,
    );
    if ok == 0 {
        let err = GetLastError();
        if !p_new_dacl.is_null() {
            LocalFree(p_new_dacl as HLOCAL);
        }
        return Err(anyhow!(
            "SetTokenInformation(TokenDefaultDacl) failed: {}",
            err
        ));
    }
    if !p_new_dacl.is_null() {
        LocalFree(p_new_dacl as HLOCAL);
    }
    Ok(())
}

pub unsafe fn world_sid() -> Result<Vec<u8>> {
    let mut size: u32 = 0;
    CreateWellKnownSid(
        WIN_WORLD_SID,
        std::ptr::null_mut(),
        std::ptr::null_mut(),
        &mut size,
    );
    let mut buf: Vec<u8> = vec![0u8; size as usize];
    let ok = CreateWellKnownSid(
        WIN_WORLD_SID,
        std::ptr::null_mut(),
        buf.as_mut_ptr() as *mut c_void,
        &mut size,
    );
    if ok == 0 {
        return Err(anyhow!("CreateWellKnownSid failed: {}", GetLastError()));
    }
    Ok(buf)
}

/// # Safety
/// Caller is responsible for freeing the returned SID with `LocalFree`.
pub unsafe fn convert_string_sid_to_sid(s: &str) -> Option<*mut c_void> {
    #[link(name = "advapi32")]
    extern "system" {
        fn ConvertStringSidToSidW(StringSid: *const u16, Sid: *mut *mut c_void) -> i32;
    }
    let mut psid: *mut c_void = std::ptr::null_mut();
    let ok = unsafe { ConvertStringSidToSidW(to_wide(s).as_ptr(), &mut psid) };
    if ok != 0 {
        Some(psid)
    } else {
        None
    }
}

/// # Safety
/// Caller must close the returned token handle.
#[allow(dead_code)]
pub unsafe fn get_current_token_for_restriction() -> Result<HANDLE> {
    let desired = TOKEN_DUPLICATE
        | TOKEN_QUERY
        | TOKEN_ASSIGN_PRIMARY
        | TOKEN_ADJUST_DEFAULT
        | TOKEN_ADJUST_SESSIONID
        | TOKEN_ADJUST_PRIVILEGES;
    let mut h: HANDLE = 0;
    #[link(name = "advapi32")]
    extern "system" {
        fn OpenProcessToken(
            ProcessHandle: HANDLE,
            DesiredAccess: u32,
            TokenHandle: *mut HANDLE,
        ) -> i32;
    }
    let ok = unsafe { OpenProcessToken(GetCurrentProcess(), desired, &mut h) };
    if ok == 0 {
        return Err(anyhow!("OpenProcessToken failed: {}", GetLastError()));
    }
    Ok(h)
}

pub unsafe fn get_logon_sid_bytes(h_token: HANDLE) -> Result<Vec<u8>> {
    unsafe fn scan_token_groups_for_logon(h: HANDLE) -> Option<Vec<u8>> {
        let mut needed: u32 = 0;
        GetTokenInformation(h, TokenGroups, std::ptr::null_mut(), 0, &mut needed);
        if needed == 0 {
            return None;
        }
        let mut buf: Vec<u8> = vec![0u8; needed as usize];
        let ok = GetTokenInformation(
            h,
            TokenGroups,
            buf.as_mut_ptr() as *mut c_void,
            needed,
            &mut needed,
        );
        if ok == 0 || (needed as usize) < std::mem::size_of::<u32>() {
            return None;
        }
        let group_count = std::ptr::read_unaligned(buf.as_ptr() as *const u32) as usize;
        // TOKEN_GROUPS layout is: DWORD GroupCount; SID_AND_ATTRIBUTES Groups[];
        // On 64-bit, Groups is aligned to pointer alignment after 4-byte GroupCount.
        let after_count = unsafe { buf.as_ptr().add(std::mem::size_of::<u32>()) } as usize;
        let align = std::mem::align_of::<SID_AND_ATTRIBUTES>();
        let aligned = (after_count + (align - 1)) & !(align - 1);
        let groups_ptr = aligned as *const SID_AND_ATTRIBUTES;
        for i in 0..group_count {
            let entry: SID_AND_ATTRIBUTES = std::ptr::read_unaligned(groups_ptr.add(i));
            if (entry.Attributes & SE_GROUP_LOGON_ID) == SE_GROUP_LOGON_ID {
                let sid = entry.Sid;
                let sid_len = GetLengthSid(sid);
                if sid_len == 0 {
                    return None;
                }
                let mut out = vec![0u8; sid_len as usize];
                if CopySid(sid_len, out.as_mut_ptr() as *mut c_void, sid) == 0 {
                    return None;
                }
                return Some(out);
            }
        }
        None
    }

    if let Some(v) = scan_token_groups_for_logon(h_token) {
        return Ok(v);
    }

    #[repr(C)]
    struct TOKEN_LINKED_TOKEN {
        linked_token: HANDLE,
    }
    const TOKEN_LINKED_TOKEN_CLASS: i32 = 19; // TokenLinkedToken
    let mut ln_needed: u32 = 0;
    GetTokenInformation(
        h_token,
        TOKEN_LINKED_TOKEN_CLASS,
        std::ptr::null_mut(),
        0,
        &mut ln_needed,
    );
    if ln_needed >= std::mem::size_of::<TOKEN_LINKED_TOKEN>() as u32 {
        let mut ln_buf: Vec<u8> = vec![0u8; ln_needed as usize];
        let ok = GetTokenInformation(
            h_token,
            TOKEN_LINKED_TOKEN_CLASS,
            ln_buf.as_mut_ptr() as *mut c_void,
            ln_needed,
            &mut ln_needed,
        );
        if ok != 0 {
            let lt: TOKEN_LINKED_TOKEN =
                std::ptr::read_unaligned(ln_buf.as_ptr() as *const TOKEN_LINKED_TOKEN);
            if lt.linked_token != 0 {
                let res = scan_token_groups_for_logon(lt.linked_token);
                CloseHandle(lt.linked_token);
                if let Some(v) = res {
                    return Ok(v);
                }
            }
        }
    }

    Err(anyhow!("Logon SID not present on token"))
}
unsafe fn enable_single_privilege(h_token: HANDLE, name: &str) -> Result<()> {
    let mut luid = LUID {
        LowPart: 0,
        HighPart: 0,
    };
    let ok = LookupPrivilegeValueW(std::ptr::null(), to_wide(name).as_ptr(), &mut luid);
    if ok == 0 {
        return Err(anyhow!("LookupPrivilegeValueW failed: {}", GetLastError()));
    }
    let mut tp: TOKEN_PRIVILEGES = std::mem::zeroed();
    tp.PrivilegeCount = 1;
    tp.Privileges[0].Luid = luid;
    tp.Privileges[0].Attributes = 0x00000002; // SE_PRIVILEGE_ENABLED
    let ok2 = AdjustTokenPrivileges(
        h_token,
        0,
        &tp,
        0,
        std::ptr::null_mut(),
        std::ptr::null_mut(),
    );
    if ok2 == 0 {
        return Err(anyhow!("AdjustTokenPrivileges failed: {}", GetLastError()));
    }
    let err = GetLastError();
    if err != 0 {
        return Err(anyhow!("AdjustTokenPrivileges error {}", err));
    }
    Ok(())
}

/// # Safety
/// Caller must close the returned token handle.
#[allow(dead_code)]
pub unsafe fn create_workspace_write_token_with_cap(
    psid_capability: *mut c_void,
) -> Result<(HANDLE, *mut c_void)> {
    let base = get_current_token_for_restriction()?;
    let res = create_workspace_write_token_with_cap_from(base, psid_capability);
    CloseHandle(base);
    res
}

/// # Safety
/// Caller must close the returned token handle.
#[allow(dead_code)]
pub unsafe fn create_readonly_token_with_cap(
    psid_capability: *mut c_void,
) -> Result<(HANDLE, *mut c_void)> {
    let base = get_current_token_for_restriction()?;
    let res = create_readonly_token_with_cap_from(base, psid_capability);
    CloseHandle(base);
    res
}

/// # Safety
/// Caller must close the returned token handle; base_token must be a valid primary token.
pub unsafe fn create_workspace_write_token_with_cap_from(
    base_token: HANDLE,
    psid_capability: *mut c_void,
) -> Result<(HANDLE, *mut c_void)> {
    let mut logon_sid_bytes = get_logon_sid_bytes(base_token)?;
    let psid_logon = logon_sid_bytes.as_mut_ptr() as *mut c_void;
    let mut everyone = world_sid()?;
    let psid_everyone = everyone.as_mut_ptr() as *mut c_void;
    let mut entries: [SID_AND_ATTRIBUTES; 3] = std::mem::zeroed();
    // Exact set and order: Capability, Logon, Everyone
    entries[0].Sid = psid_capability;
    entries[0].Attributes = 0;
    entries[1].Sid = psid_logon;
    entries[1].Attributes = 0;
    entries[2].Sid = psid_everyone;
    entries[2].Attributes = 0;
    let mut new_token: HANDLE = 0;
    let flags = DISABLE_MAX_PRIVILEGE | LUA_TOKEN | WRITE_RESTRICTED;
    let ok = CreateRestrictedToken(
        base_token,
        flags,
        0,
        std::ptr::null(),
        0,
        std::ptr::null(),
        3,
        entries.as_mut_ptr(),
        &mut new_token,
    );
    if ok == 0 {
        return Err(anyhow!("CreateRestrictedToken failed: {}", GetLastError()));
    }
    set_default_dacl(new_token, &[psid_logon, psid_everyone, psid_capability])?;
    enable_single_privilege(new_token, "SeChangeNotifyPrivilege")?;
    Ok((new_token, psid_capability))
}

/// # Safety
/// Caller must close the returned token handle; base_token must be a valid primary token.
pub unsafe fn create_readonly_token_with_cap_from(
    base_token: HANDLE,
    psid_capability: *mut c_void,
) -> Result<(HANDLE, *mut c_void)> {
    let mut logon_sid_bytes = get_logon_sid_bytes(base_token)?;
    let psid_logon = logon_sid_bytes.as_mut_ptr() as *mut c_void;
    let mut everyone = world_sid()?;
    let psid_everyone = everyone.as_mut_ptr() as *mut c_void;
    let mut entries: [SID_AND_ATTRIBUTES; 3] = std::mem::zeroed();
    // Exact set and order: Capability, Logon, Everyone
    entries[0].Sid = psid_capability;
    entries[0].Attributes = 0;
    entries[1].Sid = psid_logon;
    entries[1].Attributes = 0;
    entries[2].Sid = psid_everyone;
    entries[2].Attributes = 0;
    let mut new_token: HANDLE = 0;
    let flags = DISABLE_MAX_PRIVILEGE | LUA_TOKEN | WRITE_RESTRICTED;
    let ok = CreateRestrictedToken(
        base_token,
        flags,
        0,
        std::ptr::null(),
        0,
        std::ptr::null(),
        3,
        entries.as_mut_ptr(),
        &mut new_token,
    );
    if ok == 0 {
        return Err(anyhow!("CreateRestrictedToken failed: {}", GetLastError()));
    }
    set_default_dacl(new_token, &[psid_logon, psid_everyone, psid_capability])?;
    enable_single_privilege(new_token, "SeChangeNotifyPrivilege")?;
    Ok((new_token, psid_capability))
}

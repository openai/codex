use crate::winutil::to_wide;
use anyhow::anyhow;
use anyhow::Result;
use std::ffi::c_void;
use windows_sys::Win32::Foundation::GetLastError;
use windows_sys::Win32::Foundation::HANDLE;
use windows_sys::Win32::Foundation::LUID;
use windows_sys::Win32::Security::AdjustTokenPrivileges;
use windows_sys::Win32::Security::CopySid;
use windows_sys::Win32::Security::CreateRestrictedToken;
use windows_sys::Win32::Security::CreateWellKnownSid;
use windows_sys::Win32::Security::GetLengthSid;
use windows_sys::Win32::Security::GetTokenInformation;
use windows_sys::Win32::Security::LookupPrivilegeValueW;
use windows_sys::Win32::Security::TokenGroups;
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
const WIN_WORLD_SID: i32 = 1;
const SE_GROUP_LOGON_ID: u32 = 0xC0000000;

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

#[allow(dead_code)]
pub unsafe fn get_logon_sid_bytes(h_token: HANDLE) -> Result<Vec<u8>> {
    let mut needed: u32 = 0;
    GetTokenInformation(h_token, TokenGroups, std::ptr::null_mut(), 0, &mut needed);
    if needed == 0 {
        return Err(anyhow!(
            "GetTokenInformation size failed: {}",
            GetLastError()
        ));
    }
    let mut buf: Vec<u8> = vec![0u8; needed as usize];
    let ok = GetTokenInformation(
        h_token,
        TokenGroups,
        buf.as_mut_ptr() as *mut c_void,
        needed,
        &mut needed,
    );
    if ok == 0 {
        return Err(anyhow!(
            "GetTokenInformation(TokenGroups) failed: {}",
            GetLastError()
        ));
    }
    let group_count = *(buf.as_ptr() as *const u32) as usize;
    let groups_ptr = buf.as_ptr().add(std::mem::size_of::<u32>()) as *const SID_AND_ATTRIBUTES;
    for i in 0..group_count {
        let entry = *groups_ptr.add(i);
        if (entry.Attributes & SE_GROUP_LOGON_ID) == SE_GROUP_LOGON_ID {
            let sid = entry.Sid;
            let sid_len = GetLengthSid(sid);
            let mut out = vec![0u8; sid_len as usize];
            let ok2 = CopySid(sid_len, out.as_mut_ptr() as *mut c_void, sid);
            if ok2 == 0 {
                return Err(anyhow!("CopySid failed: {}", GetLastError()));
            }
            return Ok(out);
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
        &mut tp,
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

#[allow(dead_code)]
pub unsafe fn create_write_restricted_token_strict() -> Result<(HANDLE, *mut c_void)> {
    let base = get_current_token_for_restriction()?;
    let logon_sid_bytes = get_logon_sid_bytes(base)?;
    let mut sid_buf = logon_sid_bytes.clone();
    let psid_logon = sid_buf.as_mut_ptr() as *mut c_void;
    let mut entries: [SID_AND_ATTRIBUTES; 1] = std::mem::zeroed();
    entries[0].Sid = psid_logon;
    entries[0].Attributes = 0;
    let mut new_token: HANDLE = 0;
    let flags = DISABLE_MAX_PRIVILEGE | LUA_TOKEN | WRITE_RESTRICTED;
    let ok = CreateRestrictedToken(
        base,
        flags,
        0,
        std::ptr::null(),
        0,
        std::ptr::null(),
        1,
        entries.as_mut_ptr(),
        &mut new_token,
    );
    if ok == 0 {
        return Err(anyhow!("CreateRestrictedToken failed: {}", GetLastError()));
    }
    enable_single_privilege(new_token, "SeChangeNotifyPrivilege")?;
    Ok((new_token, psid_logon))
}

pub unsafe fn create_workspace_write_token_with_cap(
    psid_capability: *mut c_void,
) -> Result<(HANDLE, *mut c_void)> {
    let base = get_current_token_for_restriction()?;
    let mut everyone = world_sid()?;
    let psid_everyone = everyone.as_mut_ptr() as *mut c_void;
    let mut entries: [SID_AND_ATTRIBUTES; 2] = std::mem::zeroed();
    entries[0].Sid = psid_capability;
    entries[0].Attributes = 0;
    entries[1].Sid = psid_everyone;
    entries[1].Attributes = 0;
    let mut new_token: HANDLE = 0;
    let flags = DISABLE_MAX_PRIVILEGE | LUA_TOKEN | WRITE_RESTRICTED;
    let ok = CreateRestrictedToken(
        base,
        flags,
        0,
        std::ptr::null(),
        0,
        std::ptr::null(),
        2,
        entries.as_mut_ptr(),
        &mut new_token,
    );
    if ok == 0 {
        return Err(anyhow!("CreateRestrictedToken failed: {}", GetLastError()));
    }
    enable_single_privilege(new_token, "SeChangeNotifyPrivilege")?;
    Ok((new_token, psid_capability))
}

pub unsafe fn create_readonly_token_with_cap(
    psid_capability: *mut c_void,
) -> Result<(HANDLE, *mut c_void)> {
    let base = get_current_token_for_restriction()?;
    let mut entries: [SID_AND_ATTRIBUTES; 1] = std::mem::zeroed();
    entries[0].Sid = psid_capability;
    entries[0].Attributes = 0;
    let mut new_token: HANDLE = 0;
    let flags = DISABLE_MAX_PRIVILEGE | LUA_TOKEN | WRITE_RESTRICTED;
    let ok = CreateRestrictedToken(
        base,
        flags,
        0,
        std::ptr::null(),
        0,
        std::ptr::null(),
        1,
        entries.as_mut_ptr(),
        &mut new_token,
    );
    if ok == 0 {
        return Err(anyhow!("CreateRestrictedToken failed: {}", GetLastError()));
    }
    enable_single_privilege(new_token, "SeChangeNotifyPrivilege")?;
    Ok((new_token, psid_capability))
}

use anyhow::Context;
use anyhow::Result;
use std::path::Path;
use windows::Win32::Foundation::CloseHandle;
use windows::Win32::Foundation::ERROR_NOT_ALL_ASSIGNED;
use windows::Win32::Foundation::ERROR_PRIVILEGE_NOT_HELD;
use windows::Win32::Foundation::GetLastError;
use windows::Win32::Foundation::HANDLE;
use windows::Win32::Foundation::LUID;
use windows::Win32::Foundation::WIN32_ERROR;
use windows::Win32::Security::AdjustTokenPrivileges;
use windows::Win32::Security::Authorization::ConvertStringSecurityDescriptorToSecurityDescriptorW;
use windows::Win32::Security::Authorization::SE_FILE_OBJECT;
use windows::Win32::Security::Authorization::SetNamedSecurityInfoW;
use windows::Win32::Security::GetSecurityDescriptorSacl;
use windows::Win32::Security::LABEL_SECURITY_INFORMATION;
use windows::Win32::Security::LUID_AND_ATTRIBUTES;
use windows::Win32::Security::LookupPrivilegeValueW;
use windows::Win32::Security::OBJECT_SECURITY_INFORMATION;
use windows::Win32::Security::PSECURITY_DESCRIPTOR;
use windows::Win32::Security::SACL_SECURITY_INFORMATION;
use windows::Win32::Security::SE_PRIVILEGE_ENABLED;
use windows::Win32::Security::TOKEN_ADJUST_PRIVILEGES;
use windows::Win32::Security::TOKEN_PRIVILEGES;
use windows::Win32::Security::TOKEN_QUERY;
use windows::Win32::Storage::FileSystem::GetFileAttributesW;
use windows::Win32::System::Threading::GetCurrentProcess;
use windows::Win32::System::Threading::OpenProcessToken;
use windows::core::PCWSTR;

struct TokenHandle(HANDLE);

impl Drop for TokenHandle {
    fn drop(&mut self) {
        if !self.0.is_invalid() {
            unsafe {
                let _ = CloseHandle(self.0);
            }
        }
    }
}

pub fn enable_required_privileges() -> Result<()> {
    unsafe {
        let mut token_raw: HANDLE = HANDLE::default();
        OpenProcessToken(
            GetCurrentProcess(),
            TOKEN_ADJUST_PRIVILEGES | TOKEN_QUERY,
            &mut token_raw,
        )
        .context("OpenProcessToken")?;
        let token = TokenHandle(token_raw);

        let privilege = widestring::U16CString::from_str("SeSecurityPrivilege")?;
        let mut luid = LUID::default();
        LookupPrivilegeValueW(None, PCWSTR(privilege.as_ptr()), &mut luid)
            .context("LookupPrivilegeValueW(SeSecurityPrivilege)")?;

        let mut privileges = TOKEN_PRIVILEGES {
            PrivilegeCount: 1,
            Privileges: [LUID_AND_ATTRIBUTES {
                Luid: luid,
                Attributes: SE_PRIVILEGE_ENABLED,
            }],
        };

        AdjustTokenPrivileges(token.0, false, Some(&mut privileges), 0, None, None)
            .context("AdjustTokenPrivileges")?;
        if GetLastError() == ERROR_NOT_ALL_ASSIGNED {
            anyhow::bail!("SeSecurityPrivilege is not assigned to the current process");
        }
    }
    Ok(())
}

fn to_w(s: &std::ffi::OsStr) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;
    s.encode_wide().chain(std::iter::once(0)).collect()
}

// Apply SDDL S:(ML;OICI;NW;;;LW)  => Low IL with object/container inherit + NoWriteUp
pub fn ensure_low_integrity_dir(dir: &Path) -> Result<()> {
    if !exists(dir)? {
        return Ok(());
    }
    apply_integrity_sddl(dir, "S:(ML;OICI;NW;;;LW)")
}

pub fn ensure_medium_integrity_dir(dir: &Path) -> Result<()> {
    if !exists(dir)? {
        return Ok(());
    }
    // Explicitly set Medium IL so the Low-IL process cannot write here.
    apply_integrity_sddl(dir, "S:(ML;OICI;NW;;;ME)")
}

fn exists(p: &Path) -> Result<bool> {
    let w = to_w(p.as_os_str());
    let attrs = unsafe { GetFileAttributesW(PCWSTR(w.as_ptr())) };
    Ok(attrs != u32::MAX)
}

fn apply_integrity_sddl(path: &Path, sddl: &str) -> Result<()> {
    unsafe {
        let mut psd = PSECURITY_DESCRIPTOR::default();
        let sddl_w = widestring::U16CString::from_str(sddl)?;
        ConvertStringSecurityDescriptorToSecurityDescriptorW(
            PCWSTR(sddl_w.as_ptr()),
            1,
            &mut psd,
            None,
        )
        .context("ConvertStringSecurityDescriptorToSecurityDescriptorW")?;

        let mut sacl_present = false.into();
        let mut sacl_defaulted = false.into();
        let mut sacl_ptr = std::ptr::null_mut();
        GetSecurityDescriptorSacl(psd, &mut sacl_present, &mut sacl_ptr, &mut sacl_defaulted)
            .context("GetSecurityDescriptorSacl")?;

        let wpath = to_w(path.as_os_str());
        let flags =
            OBJECT_SECURITY_INFORMATION(LABEL_SECURITY_INFORMATION.0 | SACL_SECURITY_INFORMATION.0);
        let status = SetNamedSecurityInfoW(
            PCWSTR(wpath.as_ptr()),
            SE_FILE_OBJECT,
            flags,
            None,
            None,
            None,
            Some(sacl_ptr),
        );
        if status != WIN32_ERROR(0) {
            if status == ERROR_PRIVILEGE_NOT_HELD {
                anyhow::bail!(
                    "setting integrity label requires SeSecurityPrivilege or object ownership: {path:?}"
                );
            }
            anyhow::bail!("SetNamedSecurityInfoW failed: {status:?}");
        }
    }
    Ok(())
}

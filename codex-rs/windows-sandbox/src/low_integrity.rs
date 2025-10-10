use anyhow::Context;
use anyhow::Result;
use std::path::Path;
use windows::Win32::Foundation::ERROR_PRIVILEGE_NOT_HELD;
use windows::Win32::Foundation::WIN32_ERROR;
use windows::Win32::Security::Authorization::ConvertStringSecurityDescriptorToSecurityDescriptorW;
use windows::Win32::Security::Authorization::GetSecurityDescriptorSacl;
use windows::Win32::Security::Authorization::SE_FILE_OBJECT;
use windows::Win32::Security::Authorization::SE_SACL_PRESENT;
use windows::Win32::Security::Authorization::SetNamedSecurityInfoW;
use windows::Win32::Security::LABEL_SECURITY_INFORMATION;
use windows::Win32::Security::PSECURITY_DESCRIPTOR;
use windows::Win32::Security::SACL_SECURITY_INFORMATION;
use windows::Win32::Storage::FileSystem::GetFileAttributesW;
use windows::core::PCWSTR;

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
    Ok(attrs.0 != u32::MAX)
}

fn apply_integrity_sddl(path: &Path, sddl: &str) -> Result<()> {
    unsafe {
        let mut psd = PSECURITY_DESCRIPTOR::default();
        ConvertStringSecurityDescriptorToSecurityDescriptorW(
            &widestring::U16CString::from_str(sddl)?.as_pcwstr(),
            1, // SDDL_REVISION_1
            &mut psd,
            None,
        )
        .ok()
        .context("ConvertStringSecurityDescriptorToSecurityDescriptorW")?;

        let mut sacl_present = false.into();
        let mut sacl_defaulted = false.into();
        let mut sacl_ptr = std::ptr::null_mut();
        GetSecurityDescriptorSacl(psd, &mut sacl_present, &mut sacl_ptr, &mut sacl_defaulted)
            .ok()
            .context("GetSecurityDescriptorSacl")?;

        let wpath = to_w(path.as_os_str());
        let status = SetNamedSecurityInfoW(
            PCWSTR(wpath.as_ptr()),
            SE_FILE_OBJECT,
            LABEL_SECURITY_INFORMATION | SACL_SECURITY_INFORMATION,
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

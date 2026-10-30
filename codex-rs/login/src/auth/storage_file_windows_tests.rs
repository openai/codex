#![cfg(test)]

use super::*;
use pretty_assertions::assert_eq;
use std::io::Write;
use windows_sys::Win32::Security::Authorization::ConvertStringSecurityDescriptorToSecurityDescriptorW;
use windows_sys::Win32::Security::Authorization::SetNamedSecurityInfoW;
use windows_sys::Win32::Security::GetSecurityDescriptorDacl;
use windows_sys::Win32::Security::PROTECTED_DACL_SECURITY_INFORMATION;

fn set_dacl(path: &Path, sddl: &str) -> io::Result<()> {
    let text: Vec<u16> = sddl.encode_utf16().chain(Some(0)).collect();
    let mut descriptor = ptr::null_mut();
    // SAFETY: text is NUL-terminated, the result is freed by Descriptor.
    if unsafe {
        ConvertStringSecurityDescriptorToSecurityDescriptorW(
            text.as_ptr(),
            1,
            &mut descriptor,
            ptr::null_mut(),
        )
    } == 0
    {
        return Err(io::Error::last_os_error());
    }
    let descriptor = Descriptor(descriptor);
    let mut present = 0;
    let mut defaulted = 0;
    let mut acl = ptr::null_mut();
    // SAFETY: all output pointers are valid; descriptor owns the returned ACL.
    if unsafe { GetSecurityDescriptorDacl(descriptor.0, &mut present, &mut acl, &mut defaulted) }
        == 0
    {
        return Err(io::Error::last_os_error());
    }
    assert_ne!(present, 0);
    let path: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    // SAFETY: the path and ACL remain valid for this call.
    let status = unsafe {
        SetNamedSecurityInfoW(
            path.as_ptr(),
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION,
            ptr::null_mut(),
            ptr::null_mut(),
            acl,
            ptr::null_mut(),
        )
    };
    if status != ERROR_SUCCESS {
        return Err(io::Error::from_raw_os_error(status as i32));
    }
    Ok(())
}

fn dacl(path: &Path) -> io::Result<Vec<u8>> {
    let file = OpenOptions::new().access_mode(READ_CONTROL).open(path)?;
    let mut acl = ptr::null_mut();
    let mut descriptor = ptr::null_mut();
    // SAFETY: the file and output pointers are valid.
    let status = unsafe {
        GetSecurityInfo(
            file.as_raw_handle(),
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION,
            ptr::null_mut(),
            ptr::null_mut(),
            &mut acl,
            ptr::null_mut(),
            &mut descriptor,
        )
    };
    if status != ERROR_SUCCESS {
        return Err(io::Error::from_raw_os_error(status as i32));
    }
    let _descriptor = Descriptor(descriptor);
    assert!(!acl.is_null());
    // SAFETY: the descriptor owns an ACL with AclSize bytes for this scope.
    Ok(unsafe { std::slice::from_raw_parts(acl.cast::<u8>(), (*acl).AclSize as usize) }.to_vec())
}

#[test]
fn replacement_keeps_restricted_dacl_before_secret_is_written() -> anyhow::Result<()> {
    let home = tempfile::tempdir()?;
    // Everyone can read child files by inheritance; the credential explicitly cannot.
    set_dacl(home.path(), "D:P(A;OICI;FA;;;WD)")?;
    let path = home.path().join("auth.json");
    std::fs::write(&path, "old")?;
    // OWNER RIGHTS grants access only to the owner, not Everyone from the parent.
    set_dacl(&path, "D:P(A;;FA;;;OW)")?;
    let expected = dacl(&path)?;
    let mut temporary = replacement(&path)?;
    assert_eq!(dacl(temporary.path())?, expected);
    temporary.write_all(b"fake-secret")?;
    let (_file, temporary_path) = temporary.keep()?;
    std::fs::rename(temporary_path, &path)?;
    assert_eq!(dacl(&path)?, expected);
    assert_eq!(std::fs::read(&path)?, b"fake-secret");
    Ok(())
}

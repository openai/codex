use super::acl_api_result;
use super::deny_ace_already_present;
use super::ensure_allow_mask_aces_with_inheritance;
use super::ensure_handle_is_not_filesystem_root;
use super::fetch_dacl_handle;
use crate::token::LocalSid;
use crate::winutil::to_wide;
use pretty_assertions::assert_eq;
use std::ffi::c_void;
use std::fs::OpenOptions;
use std::os::windows::fs::OpenOptionsExt;
use std::path::Path;
use windows_sys::Win32::Foundation::ERROR_ACCESS_DENIED;
use windows_sys::Win32::Foundation::HLOCAL;
use windows_sys::Win32::Foundation::LocalFree;
use windows_sys::Win32::Security::ACL_SIZE_INFORMATION;
use windows_sys::Win32::Security::AclSizeInformation;
use windows_sys::Win32::Security::Authorization::ConvertStringSecurityDescriptorToSecurityDescriptorW;
use windows_sys::Win32::Security::Authorization::SDDL_REVISION_1;
use windows_sys::Win32::Security::DACL_SECURITY_INFORMATION;
use windows_sys::Win32::Security::GetAclInformation;
use windows_sys::Win32::Security::GetSecurityDescriptorControl;
use windows_sys::Win32::Security::SE_DACL_PROTECTED;
use windows_sys::Win32::Security::SetFileSecurityW;
use windows_sys::Win32::Security::UNPROTECTED_DACL_SECURITY_INFORMATION;
use windows_sys::Win32::Storage::FileSystem::FILE_FLAG_BACKUP_SEMANTICS;
use windows_sys::Win32::Storage::FileSystem::FILE_READ_ATTRIBUTES;
use windows_sys::Win32::Storage::FileSystem::READ_CONTROL;

#[test]
fn deny_ace_update_failure_is_an_error() {
    let path = std::path::Path::new(r"C:\world-writable");
    let error = acl_api_result(path, "SetNamedSecurityInfoW", ERROR_ACCESS_DENIED)
        .expect_err("access denied must not look like an already-present ACE");

    assert_eq!(
        error.to_string(),
        r"SetNamedSecurityInfoW failed for C:\world-writable: 5"
    );
}

#[test]
fn deny_read_root_check_uses_the_open_handle() {
    let cwd = std::env::current_dir().expect("current directory");
    let root = cwd.ancestors().last().expect("filesystem root");
    let root_directory = OpenOptions::new()
        .access_mode(READ_CONTROL)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS)
        .open(root)
        .expect("open filesystem root");
    let diagnostic_path = std::path::Path::new(r"C:\not-a-root");

    let error = ensure_handle_is_not_filesystem_root(&root_directory, diagnostic_path)
        .expect_err("classification must follow the root handle, not the diagnostic path");

    assert_eq!(
        error.to_string(),
        r"refusing to apply a deny-read ACE to filesystem root C:\not-a-root"
    );
}

#[test]
fn existing_deny_ace_is_visible_without_write_dac() {
    let target = tempfile::NamedTempFile::new().expect("temporary file");
    let sid = LocalSid::from_string("S-1-5-21-10-20-30-40").expect("test SID");
    let path = target.path();
    let psid = sid.as_ptr();
    assert!(unsafe { super::add_deny_read_ace(path, psid) }.expect("add deny ACE"));
    let already_present =
        unsafe { deny_ace_already_present(target.as_file(), path, psid, super::DenyAceKind::Read) }
            .expect("read existing deny ACE");
    assert!(already_present);
}

#[test]
fn revoking_absent_sid_preserves_child_null_dacl() {
    let parent = tempfile::tempdir().expect("parent directory");
    let child = parent.path().join("child");
    std::fs::create_dir(&child).expect("child directory");
    let sid = LocalSid::from_string("S-1-5-21-10-20-30-40").expect("absent SID");
    let other_sid = LocalSid::from_string("S-1-5-21-10-20-30-41").expect("inherited SID");

    unsafe {
        super::add_allow_ace(parent.path(), other_sid.as_ptr()).expect("inheritable parent ACE");
        let mut descriptor = std::ptr::null_mut();
        assert_ne!(
            ConvertStringSecurityDescriptorToSecurityDescriptorW(
                crate::winutil::to_wide("D:NO_ACCESS_CONTROL").as_ptr(),
                SDDL_REVISION_1,
                &mut descriptor,
                std::ptr::null_mut(),
            ),
            0,
        );
        // The legacy setter preserves a null DACL without applying automatic inheritance.
        let set = SetFileSecurityW(
            crate::winutil::to_wide(&child).as_ptr(),
            DACL_SECURITY_INFORMATION | UNPROTECTED_DACL_SECURITY_INFORMATION,
            descriptor,
        );
        LocalFree(descriptor as HLOCAL);
        assert_ne!(set, 0, "set an unprotected null DACL");
        for revoke in [false, true] {
            if revoke {
                super::revoke_ace(parent.path(), sid.as_ptr()).expect("revoke absent SID");
            }
            let (dacl, descriptor) = super::fetch_dacl_handle(&child).expect("child permissions");
            let mut control = 0;
            let mut revision = 0;
            let valid = GetSecurityDescriptorControl(descriptor, &mut control, &mut revision);
            LocalFree(descriptor as HLOCAL);

            assert_ne!(valid, 0, "read child inheritance flags");
            assert_eq!(control & SE_DACL_PROTECTED, 0, "child permits inheritance");
            assert!(
                dacl.is_null(),
                "revocation must preserve the child's null DACL"
            );
        }
    }
}

fn acl_snapshot(path: &Path) -> (u16, Vec<u8>) {
    unsafe {
        let (acl, descriptor) = fetch_dacl_handle(path).expect("read directory ACL");
        let mut control = 0;
        let mut revision = 0;
        assert_ne!(
            GetSecurityDescriptorControl(descriptor, &mut control, &mut revision),
            0
        );
        let mut info: ACL_SIZE_INFORMATION = std::mem::zeroed();
        assert_ne!(
            GetAclInformation(
                acl,
                &mut info as *mut _ as *mut c_void,
                std::mem::size_of::<ACL_SIZE_INFORMATION>() as u32,
                AclSizeInformation,
            ),
            0
        );
        let dacl =
            std::slice::from_raw_parts(acl.cast::<u8>(), info.AclBytesInUse as usize).to_vec();
        LocalFree(descriptor as HLOCAL);
        (control, dacl)
    }
}

#[test]
fn noninheriting_read_attributes_preserve_unprotected_child_dacl() {
    let parent = tempfile::tempdir().expect("create parent");
    let child = parent.path().join("child");
    let marker = LocalSid::from_string("S-1-5-21-915763429-1123581321-271828182-4243").unwrap();
    let target = LocalSid::from_string("S-1-5-21-915763429-1123581321-271828182-4244").unwrap();
    std::fs::create_dir(&child).unwrap();
    let original = acl_snapshot(&child).1;

    // Restore the original child DACL after adding an inheritable parent ACE.
    // An ordinary parent update would re-propagate this missing permission.
    unsafe {
        let (_, descriptor) = fetch_dacl_handle(&child).unwrap();
        ensure_allow_mask_aces_with_inheritance(
            parent.path(),
            &[marker.as_ptr()],
            FILE_READ_ATTRIBUTES,
            super::CONTAINER_INHERIT_ACE | super::OBJECT_INHERIT_ACE,
        )
        .unwrap();
        assert_ne!(
            acl_snapshot(&child).1,
            original,
            "child should inherit the ACE"
        );
        assert_ne!(
            SetFileSecurityW(
                to_wide(&child).as_ptr(),
                DACL_SECURITY_INFORMATION,
                descriptor
            ),
            0
        );
        LocalFree(descriptor as HLOCAL);
    }
    let before = acl_snapshot(&child);
    assert_eq!(before.0 & SE_DACL_PROTECTED, 0);

    assert!(unsafe {
        ensure_allow_mask_aces_with_inheritance(
            parent.path(),
            &[target.as_ptr()],
            FILE_READ_ATTRIBUTES,
            /*inheritance*/ 0,
        )
        .unwrap()
    });
    assert_eq!(acl_snapshot(&child), before);
}

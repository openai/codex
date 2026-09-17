use super::ensure_runtime_tree_readable;
use super::runtime_paths;
use crate::LocalSid;
use crate::add_deny_read_ace;
use crate::add_deny_write_ace;
use crate::ensure_allow_mask_aces_with_inheritance;
use crate::path_mask_allows;
use pretty_assertions::assert_eq;
use std::fs;
use std::path::PathBuf;
use windows_sys::Win32::Foundation::HLOCAL;
use windows_sys::Win32::Foundation::LocalFree;
use windows_sys::Win32::Security::Authorization::ConvertStringSecurityDescriptorToSecurityDescriptorW;
use windows_sys::Win32::Security::Authorization::SDDL_REVISION_1;
use windows_sys::Win32::Security::CheckTokenMembership;
use windows_sys::Win32::Security::DACL_SECURITY_INFORMATION;
use windows_sys::Win32::Security::PROTECTED_DACL_SECURITY_INFORMATION;
use windows_sys::Win32::Security::SetFileSecurityW;
use windows_sys::Win32::Storage::FileSystem::DELETE;
use windows_sys::Win32::Storage::FileSystem::FILE_APPEND_DATA;
use windows_sys::Win32::Storage::FileSystem::FILE_GENERIC_EXECUTE;
use windows_sys::Win32::Storage::FileSystem::FILE_GENERIC_READ;
use windows_sys::Win32::Storage::FileSystem::FILE_WRITE_DATA;
use windows_sys::Win32::Storage::FileSystem::WRITE_DAC;

#[test]
fn runtime_repair_preserves_other_trustees_inherited_file_denial() {
    for (denied_trustee, granted_sid) in [("WD", "S-1-5-11"), ("S-1-5-32-545", "S-1-1-0")] {
        let ancestor = tempfile::tempdir().expect("runtime ancestor");
        let granted_sid = LocalSid::from_string(granted_sid).expect("grant SID");
        let mut member = 0;
        assert_ne!(
            unsafe {
                CheckTokenMembership(/*tokenhandle*/ 0, granted_sid.as_ptr(), &mut member)
            },
            0,
        );
        assert_ne!(member, 0, "the real read must exercise the added grant");

        unsafe {
            let mut descriptor = std::ptr::null_mut();
            assert_ne!(
                ConvertStringSecurityDescriptorToSecurityDescriptorW(
                    crate::winutil::to_wide(format!(
                        "D:P(D;OIIO;0x1;;;{denied_trustee})(A;OICI;FA;;;OW)"
                    ))
                    .as_ptr(),
                    SDDL_REVISION_1,
                    &mut descriptor,
                    std::ptr::null_mut(),
                ),
                0,
            );
            let set = SetFileSecurityW(
                crate::winutil::to_wide(ancestor.path()).as_ptr(),
                DACL_SECURITY_INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION,
                descriptor,
            );
            LocalFree(descriptor as HLOCAL);
            assert_ne!(set, 0, "install an inheritable file-data denial");
        }
        let runtime = ancestor.path().join("runtimes");
        fs::create_dir(&runtime).expect("runtime directory");
        let module = runtime.join("private.js");
        fs::write(&module, b"private runtime content").expect("runtime file");
        for repair in [false, true] {
            if repair {
                ensure_runtime_tree_readable(&runtime, granted_sid.as_ptr())
                    .expect("repair must preserve inherited denies");
            }
            assert_eq!(
                fs::read(&module)
                    .expect_err("file-data denial must remain effective")
                    .kind(),
                std::io::ErrorKind::PermissionDenied,
            );
        }
    }
}

#[test]
fn runtime_repair_does_not_follow_directory_junctions() {
    let runtime = tempfile::tempdir().expect("runtime directory");
    let external = tempfile::tempdir().expect("external directory");
    let external_file = external.path().join("private.js");
    fs::write(&external_file, b"external content").expect("external file");
    let alias = runtime.path().join("linked-runtime");
    let output = std::process::Command::new("cmd")
        .args(["/C", "mklink", "/J"])
        .arg(&alias)
        .arg(external.path())
        .output()
        .expect("create directory junction");
    assert!(output.status.success(), "mklink failed: {output:?}");
    let sandbox_sid = LocalSid::from_string("S-1-5-21-10-20-30-40").expect("test SID");
    for root in [runtime.path(), alias.as_path()] {
        ensure_runtime_tree_readable(root, sandbox_sid.as_ptr()).expect("repair runtimes");
        for path in [external.path(), external_file.as_path()] {
            assert!(
                !path_mask_allows(
                    path,
                    &[sandbox_sid.as_ptr()],
                    FILE_GENERIC_READ | FILE_GENERIC_EXECUTE,
                    /*require_all_bits*/ false,
                )
                .expect("external ACL must remain untouched")
            );
        }
    }
    fs::remove_dir(&alias).expect("remove junction");
}

#[test]
fn repairs_children_when_runtime_root_already_has_read_execute_access() {
    let runtime = tempfile::tempdir().expect("runtime directory");
    let node = runtime.path().join("bin/node.exe");
    let module = runtime.path().join("sky/dist/project/index.js");
    for path in [&node, &module] {
        fs::create_dir_all(path.parent().expect("parent")).expect("runtime subdirectory");
        fs::write(path, b"runtime content").expect("runtime file");
    }
    let sandbox_sid = LocalSid::from_string("S-1-5-21-10-20-30-40").expect("test SID");
    let read_execute_mask = FILE_GENERIC_READ | FILE_GENERIC_EXECUTE;
    unsafe {
        ensure_allow_mask_aces_with_inheritance(
            runtime.path(),
            &[sandbox_sid.as_ptr()],
            read_execute_mask,
            /*inheritance*/ 0,
        )
    }
    .expect("grant root-only read/execute access");
    assert!(
        path_mask_allows(
            runtime.path(),
            &[sandbox_sid.as_ptr()],
            read_execute_mask,
            /*require_all_bits*/ true,
        )
        .expect("root access")
    );
    for path in [&node, &module] {
        assert!(
            !path_mask_allows(
                path,
                &[sandbox_sid.as_ptr()],
                read_execute_mask,
                /*require_all_bits*/ true,
            )
            .expect("child access before repair")
        );
    }

    let denied_directory = runtime.path().join("denied");
    fs::create_dir(&denied_directory).expect("denied directory");
    assert!(
        unsafe { add_deny_read_ace(&denied_directory, sandbox_sid.as_ptr()) }
            .expect("deny directory reads")
    );
    let denied_child = denied_directory.join("private.js");
    fs::write(&denied_child, b"private runtime content").expect("denied child");
    let write_denied_file = runtime.path().join("read-only.js");
    fs::write(&write_denied_file, b"read-only runtime content").expect("write-denied file");
    assert!(
        unsafe { add_deny_write_ace(&write_denied_file, sandbox_sid.as_ptr()) }
            .expect("deny runtime writes")
    );

    for _ in 0..2 {
        ensure_runtime_tree_readable(runtime.path(), sandbox_sid.as_ptr())
            .expect("repair runtimes");
        for path in [&denied_directory, &denied_child] {
            assert!(
                !unsafe { add_deny_read_ace(path, sandbox_sid.as_ptr()) }
                    .expect("read denial must survive repair")
            );
            assert!(
                !path_mask_allows(
                    path,
                    &[sandbox_sid.as_ptr()],
                    read_execute_mask,
                    /*require_all_bits*/ false,
                )
                .expect("repair must not add an allow ahead of inherited denial")
            );
        }
        assert!(
            !unsafe { add_deny_write_ace(&write_denied_file, sandbox_sid.as_ptr()) }
                .expect("write denial must survive repair")
        );
        for path in [
            node.as_path(),
            module.as_path(),
            module.parent().expect("module directory"),
        ] {
            assert!(
                path_mask_allows(
                    path,
                    &[sandbox_sid.as_ptr()],
                    read_execute_mask,
                    /*require_all_bits*/ true,
                )
                .expect("child access after repair")
            );
            assert!(
                !path_mask_allows(
                    path,
                    &[sandbox_sid.as_ptr()],
                    FILE_WRITE_DATA | FILE_APPEND_DATA | DELETE | WRITE_DAC,
                    /*require_all_bits*/ false,
                )
                .expect("repair must not grant write or ACL management")
            );
        }
    }
}

#[test]
fn runtime_paths_include_desktop_parent_and_primary_runtime_roots() {
    let local_app_data = PathBuf::from(r"C:\Users\user\AppData\Local");
    let user_profile = PathBuf::from(r"C:\Users\user");

    assert_eq!(
        runtime_paths(Some(local_app_data), Some(user_profile)),
        vec![
            PathBuf::from(r"C:\Users\user\AppData\Local\OpenAI\Codex"),
            PathBuf::from(r"C:\Users\user\.cache\codex-runtimes"),
        ]
    );
}

#[test]
fn primary_runtime_path_does_not_depend_on_local_app_data() {
    let user_profile = PathBuf::from(r"C:\Users\user");

    assert_eq!(
        runtime_paths(/*local_app_data*/ None, Some(user_profile)),
        vec![PathBuf::from(r"C:\Users\user\.cache\codex-runtimes")]
    );
}

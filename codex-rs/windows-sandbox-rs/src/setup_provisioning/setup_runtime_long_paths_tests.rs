//! Exercises runtime ACL repair beyond the legacy Windows path limit while preserving read-only grants.

use super::ensure_runtime_tree_readable;
use crate::LocalSid;
use crate::acl::grant_read_execute_aces;
use crate::path_mask_allows;
use std::fs;
use std::os::windows::ffi::OsStrExt;
use windows_sys::Win32::Storage::FileSystem::DELETE;
use windows_sys::Win32::Storage::FileSystem::FILE_APPEND_DATA;
use windows_sys::Win32::Storage::FileSystem::FILE_GENERIC_EXECUTE;
use windows_sys::Win32::Storage::FileSystem::FILE_GENERIC_READ;
use windows_sys::Win32::Storage::FileSystem::FILE_WRITE_DATA;
use windows_sys::Win32::Storage::FileSystem::WRITE_DAC;

#[test]
fn runtime_repair_handles_long_directory_and_file_paths() {
    let runtime = tempfile::tempdir().expect("runtime directory");
    let mut directory = runtime.path().join("dependency-cache");
    while directory.as_os_str().encode_wide().count() <= 280 {
        directory.push("a".repeat(60));
    }
    fs::create_dir_all(&directory).expect("long runtime directory");
    let module = directory.join("module.js");
    fs::write(&module, b"runtime content").expect("long runtime file");
    let short = runtime.path().join("short.js");
    fs::write(&short, b"short-path control").expect("short runtime file");
    let sandbox_sid = LocalSid::from_string("S-1-5-21-10-20-30-40").expect("test SID");

    for path in [&directory, &module, &short] {
        assert!(
            !path_mask_allows(
                path,
                &[sandbox_sid.as_ptr()],
                FILE_GENERIC_READ | FILE_GENERIC_EXECUTE,
                /*require_all_bits*/ true,
            )
            .expect("read ACL before repair")
        );
        // Exercise the write even for the nested file: a parent grant during
        // traversal could otherwise make its inherited ACL a read-only no-op.
        assert!(
            unsafe {
                grant_read_execute_aces(path, &[sandbox_sid.as_ptr()], /*inheritance*/ 0)
            }
            .expect("grant read/execute on the existing path")
        );
    }

    for _ in 0..2 {
        ensure_runtime_tree_readable(runtime.path(), sandbox_sid.as_ptr())
            .expect("repair the full runtime tree");
        for path in [&directory, &module, &short] {
            assert!(
                path_mask_allows(
                    path,
                    &[sandbox_sid.as_ptr()],
                    FILE_GENERIC_READ | FILE_GENERIC_EXECUTE,
                    /*require_all_bits*/ true,
                )
                .expect("read repaired ACL")
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
            assert!(
                !unsafe {
                    grant_read_execute_aces(path, &[sandbox_sid.as_ptr()], /*inheritance*/ 0)
                }
                .expect("an already-correct ACL is a no-op")
            );
        }
    }
}

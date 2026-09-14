use super::DELETE;
use super::DaclInheritance;
use super::FILE_GENERIC_EXECUTE;
use super::FILE_GENERIC_READ;
use super::FILE_GENERIC_WRITE;
use super::GRANT_ACCESS;
use super::Payload;
use super::SETUP_VERSION;
use super::SetupMode;
use super::SetupRuntime;
use super::convert_string_sid_to_sid;
use super::lock_sandbox_bin_dir;
use super::lock_sandbox_dir;
use super::resolve_sid;
use super::sid_bytes_to_psid;
use crate::ensure_allow_write_aces;
use crate::path_mask_allows;
use crate::workspace_write_cap_sid_for_root;
use std::fs;
use windows_sys::Win32::Foundation::HLOCAL;
use windows_sys::Win32::Foundation::LocalFree;

#[test]
fn provision_only_locks_plain_directory_via_handle() {
    let temp = tempfile::tempdir().expect("tempdir");
    let sandbox_bin = temp.path().join(".sandbox-bin");
    let real_user = std::env::var("USERNAME").unwrap_or_else(|_| "Administrators".to_string());
    let real_sid = resolve_sid(&real_user).expect("resolve real user SID");

    lock_sandbox_dir(
        &sandbox_bin,
        &real_user,
        &real_sid,
        GRANT_ACCESS,
        FILE_GENERIC_READ | FILE_GENERIC_EXECUTE,
        FILE_GENERIC_READ | FILE_GENERIC_WRITE | FILE_GENERIC_EXECUTE | DELETE,
        DaclInheritance::Protected,
        SetupMode::ProvisionOnly,
    )
    .expect("lock sandbox bin via no-reparse handle");

    assert!(sandbox_bin.is_dir());
}

#[test]
fn lock_sandbox_dir_blocks_inherited_write_for_runner_files() {
    for setup_mode in [SetupMode::Full, SetupMode::ProvisionOnly] {
        let temp = tempfile::tempdir().expect("tempdir");
        let workspace = temp.path().join("workspace");
        let sandbox_bin = workspace.join(".sandbox-bin");
        fs::create_dir(&workspace).expect("create workspace");

        let workspace_sid = workspace_write_cap_sid_for_root(temp.path(), temp.path(), temp.path())
            .expect("workspace SID");
        let workspace_psid =
            unsafe { convert_string_sid_to_sid(&workspace_sid).expect("convert workspace SID") };
        let sandbox_group_sid =
            resolve_sid("Authenticated Users").expect("resolve sandbox group SID");
        let sandbox_group_psid =
            sid_bytes_to_psid(&sandbox_group_sid).expect("convert sandbox group SID");
        unsafe { ensure_allow_write_aces(&workspace, &[workspace_psid]) }
            .expect("grant inherited workspace write access");

        fs::create_dir(&sandbox_bin).expect("create sandbox bin");
        let existing_runner = sandbox_bin.join("existing-runner.exe");
        fs::write(&existing_runner, b"existing").expect("create existing runner");
        assert!(
            path_mask_allows(
                &existing_runner,
                &[workspace_psid],
                FILE_GENERIC_WRITE | DELETE,
                /*require_all_bits*/ true,
            )
            .expect("check inherited runner write and delete access")
        );

        let real_user = std::env::var("USERNAME").unwrap_or_else(|_| "Administrators".to_string());
        let payload = Payload {
            version: SETUP_VERSION,
            offline_username: String::new(),
            online_username: String::new(),
            codex_home: workspace.clone(),
            command_cwd: workspace.clone(),
            read_roots: Vec::new(),
            write_roots: Vec::new(),
            deny_read_paths: Vec::new(),
            deny_write_paths: Vec::new(),
            proxy_ports: Vec::new(),
            allow_local_binding: false,
            otel: None,
            real_user,
            mode: setup_mode,
            runtime: SetupRuntime::Legacy,
            refresh_only: false,
        };
        lock_sandbox_bin_dir(&payload, &sandbox_group_sid).expect("lock sandbox bin");
        let new_runner = sandbox_bin.join("new-runner.exe");
        fs::write(&new_runner, b"new").expect("create new runner");

        for path in [&sandbox_bin, &existing_runner, &new_runner] {
            assert!(
                !path_mask_allows(
                    path,
                    &[workspace_psid],
                    FILE_GENERIC_WRITE | DELETE,
                    /*require_all_bits*/ false,
                )
                .expect("check protected path write and delete access")
            );
            assert!(
                path_mask_allows(
                    path,
                    &[sandbox_group_psid],
                    FILE_GENERIC_READ | FILE_GENERIC_EXECUTE,
                    /*require_all_bits*/ true,
                )
                .expect("check protected path read and execute access")
            );
        }

        unsafe {
            LocalFree(workspace_psid as HLOCAL);
            LocalFree(sandbox_group_psid as HLOCAL);
        }
    }
}

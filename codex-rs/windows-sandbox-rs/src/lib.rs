// Rust 2024 surfaces this lint across the crate; keep the edition bump separate
// from the eventual unsafe cleanup.
#![allow(unsafe_op_in_unsafe_fn)]

#[cfg(any(target_os = "windows", test))]
mod ssh_config_dependencies;

macro_rules! windows_modules {
    ($($name:ident),+ $(,)?) => {
        $(#[cfg(target_os = "windows")] mod $name;)+
    };
}

windows_modules!(
    acl,
    allow,
    audit,
    cap,
    desktop,
    dpapi,
    env,
    helper_materialization,
    hide_users,
    identity,
    logging,
    path_normalization,
    policy,
    process,
    token,
    wfp,
    wfp_setup,
    winutil,
    workspace_acl
);

#[cfg(target_os = "windows")]
#[path = "conpty/mod.rs"]
mod conpty;

#[cfg(target_os = "windows")]
#[path = "proc_thread_attr.rs"]
mod proc_thread_attr;

#[cfg(target_os = "windows")]
#[path = "elevated/ipc_framed.rs"]
pub(crate) mod ipc_framed;

#[cfg(target_os = "windows")]
#[path = "setup_orchestrator.rs"]
mod setup;

#[cfg(target_os = "windows")]
mod elevated_impl;

#[cfg(target_os = "windows")]
#[path = "elevated/runner_pipe.rs"]
mod runner_pipe;

#[cfg(target_os = "windows")]
#[path = "elevated/runner_client.rs"]
mod runner_client;

#[cfg(target_os = "windows")]
mod setup_error;

#[cfg(target_os = "windows")]
#[path = "sandbox_utils.rs"]
mod sandbox_utils;

#[cfg(target_os = "windows")]
#[path = "spawn_prep.rs"]
mod spawn_prep;

#[cfg(target_os = "windows")]
#[path = "unified_exec/session.rs"]
mod session;

#[cfg(target_os = "windows")]
pub use acl::add_deny_write_ace;

#[cfg(target_os = "windows")]
pub use acl::allow_null_device;
#[cfg(target_os = "windows")]
pub use acl::ensure_allow_mask_aces;
#[cfg(target_os = "windows")]
pub use acl::ensure_allow_mask_aces_with_inheritance;
#[cfg(target_os = "windows")]
pub use acl::ensure_allow_write_aces;
#[cfg(target_os = "windows")]
pub use acl::fetch_dacl_handle;
#[cfg(target_os = "windows")]
pub use acl::path_mask_allows;
#[cfg(target_os = "windows")]
pub use audit::apply_world_writable_scan_and_denies;
#[cfg(target_os = "windows")]
pub use cap::load_or_create_cap_sids;
#[cfg(target_os = "windows")]
pub use cap::workspace_cap_sid_for_cwd;
#[cfg(target_os = "windows")]
pub use conpty::spawn_conpty_process_as_user;
#[cfg(target_os = "windows")]
pub use desktop::LaunchDesktop;
#[cfg(target_os = "windows")]
pub use dpapi::protect as dpapi_protect;
#[cfg(target_os = "windows")]
pub use dpapi::unprotect as dpapi_unprotect;
#[cfg(target_os = "windows")]
pub use elevated_impl::ElevatedSandboxCaptureRequest;
#[cfg(target_os = "windows")]
pub use elevated_impl::run_windows_sandbox_capture as run_windows_sandbox_capture_elevated;
#[cfg(target_os = "windows")]
pub use helper_materialization::resolve_current_exe_for_launch;
#[cfg(target_os = "windows")]
pub use hide_users::hide_current_user_profile_dir;
#[cfg(target_os = "windows")]
pub use hide_users::hide_newly_created_users;
#[cfg(target_os = "windows")]
pub use identity::require_logon_sandbox_creds;
#[cfg(target_os = "windows")]
pub use identity::sandbox_setup_is_complete;
#[cfg(target_os = "windows")]
pub use ipc_framed::ErrorPayload;
#[cfg(target_os = "windows")]
pub use ipc_framed::ExitPayload;
#[cfg(target_os = "windows")]
pub use ipc_framed::FramedMessage;
#[cfg(target_os = "windows")]
pub use ipc_framed::Message;
#[cfg(target_os = "windows")]
pub use ipc_framed::OutputPayload;
#[cfg(target_os = "windows")]
pub use ipc_framed::OutputStream;
#[cfg(target_os = "windows")]
pub use ipc_framed::ResizePayload;
#[cfg(target_os = "windows")]
pub use ipc_framed::SpawnReady;
#[cfg(target_os = "windows")]
pub use ipc_framed::SpawnRequest;
#[cfg(target_os = "windows")]
pub use ipc_framed::decode_bytes;
#[cfg(target_os = "windows")]
pub use ipc_framed::encode_bytes;
#[cfg(target_os = "windows")]
pub use ipc_framed::read_frame;
#[cfg(target_os = "windows")]
pub use ipc_framed::write_frame;
#[cfg(target_os = "windows")]
pub use logging::LOG_FILE_NAME;
#[cfg(target_os = "windows")]
pub use logging::log_note;
#[cfg(target_os = "windows")]
pub use path_normalization::canonicalize_path;
#[cfg(target_os = "windows")]
pub use policy::SandboxPolicy;
#[cfg(target_os = "windows")]
pub use policy::parse_policy;
#[cfg(target_os = "windows")]
pub use process::PipeSpawnHandles;
#[cfg(target_os = "windows")]
pub use process::StderrMode;
#[cfg(target_os = "windows")]
pub use process::StdinMode;
#[cfg(target_os = "windows")]
pub use process::create_process_as_user;
#[cfg(target_os = "windows")]
pub use process::read_handle_loop;
#[cfg(target_os = "windows")]
pub use process::spawn_process_with_pipes;
#[cfg(target_os = "windows")]
pub use session::spawn_windows_sandbox_session_elevated;
#[cfg(target_os = "windows")]
pub use session::spawn_windows_sandbox_session_legacy;
#[cfg(target_os = "windows")]
pub use setup::SETUP_VERSION;
#[cfg(target_os = "windows")]
pub use setup::SandboxSetupRequest;
#[cfg(target_os = "windows")]
pub use setup::SetupRootOverrides;
#[cfg(target_os = "windows")]
pub use setup::run_elevated_setup;
#[cfg(target_os = "windows")]
pub use setup::run_setup_refresh;
#[cfg(target_os = "windows")]
pub use setup::run_setup_refresh_with_extra_read_roots;
#[cfg(target_os = "windows")]
pub use setup::sandbox_bin_dir;
#[cfg(target_os = "windows")]
pub use setup::sandbox_dir;
#[cfg(target_os = "windows")]
pub use setup::sandbox_secrets_dir;
#[cfg(target_os = "windows")]
pub use setup_error::SetupErrorCode;
#[cfg(target_os = "windows")]
pub use setup_error::SetupErrorReport;
#[cfg(target_os = "windows")]
pub use setup_error::SetupFailure;
#[cfg(target_os = "windows")]
pub use setup_error::extract_failure as extract_setup_failure;
#[cfg(target_os = "windows")]
pub use setup_error::sanitize_setup_metric_tag_value;
#[cfg(target_os = "windows")]
pub use setup_error::setup_error_path;
#[cfg(target_os = "windows")]
pub use setup_error::write_setup_error_report;
#[cfg(target_os = "windows")]
#[doc(hidden)]
pub use spawn_prep::LocalSid;
#[cfg(target_os = "windows")]
pub use token::convert_string_sid_to_sid;
#[cfg(target_os = "windows")]
pub use token::create_readonly_token_with_cap_from;
#[cfg(target_os = "windows")]
pub use token::create_readonly_token_with_caps_and_user_from;
#[cfg(target_os = "windows")]
pub use token::create_readonly_token_with_caps_from;
#[cfg(target_os = "windows")]
pub use token::create_workspace_write_token_with_caps_and_user_from;
#[cfg(target_os = "windows")]
pub use token::create_workspace_write_token_with_caps_from;
#[cfg(target_os = "windows")]
pub use token::get_current_token_for_restriction;
#[cfg(target_os = "windows")]
pub use wfp::install_wfp_filters_for_account;
#[cfg(target_os = "windows")]
pub use wfp_setup::install_wfp_filters;
#[cfg(target_os = "windows")]
pub use windows_impl::CaptureResult;
#[cfg(target_os = "windows")]
pub use windows_impl::run_windows_sandbox_capture;
#[cfg(target_os = "windows")]
pub use windows_impl::run_windows_sandbox_capture_with_extra_deny_write_paths;
#[cfg(target_os = "windows")]
pub use windows_impl::run_windows_sandbox_legacy_preflight;
#[cfg(target_os = "windows")]
pub use winutil::quote_windows_arg;
#[cfg(target_os = "windows")]
pub use winutil::string_from_sid_bytes;
#[cfg(target_os = "windows")]
pub use winutil::to_wide;
#[cfg(target_os = "windows")]
pub use workspace_acl::is_command_cwd_root;

#[cfg(not(target_os = "windows"))]
pub use stub::CaptureResult;
#[cfg(not(target_os = "windows"))]
pub use stub::apply_world_writable_scan_and_denies;
#[cfg(not(target_os = "windows"))]
pub use stub::run_windows_sandbox_capture;
#[cfg(not(target_os = "windows"))]
pub use stub::run_windows_sandbox_legacy_preflight;

#[cfg(target_os = "windows")]
mod windows_impl {
    use super::acl::add_allow_ace;
    use super::acl::add_deny_write_ace;
    use super::acl::allow_null_device;
    use super::acl::revoke_ace;
    use super::allow::AllowDenyPaths;
    use super::allow::compute_allow_paths;
    use super::cap::load_or_create_cap_sids;
    use super::cap::workspace_cap_sid_for_cwd;
    use super::logging::log_failure;
    use super::logging::log_success;
    use super::path_normalization::canonicalize_path;
    use super::policy::SandboxPolicy;
    use super::process::create_process_as_user;
    use super::sandbox_utils::ensure_codex_home_exists;
    use super::spawn_prep::prepare_legacy_spawn_context;
    use super::token::convert_string_sid_to_sid;
    use super::token::create_workspace_write_token_with_caps_from;
    use super::workspace_acl::is_command_cwd_root;
    use anyhow::Result;
    use std::collections::HashMap;
    use std::ffi::c_void;
    use std::io;
    use std::path::Path;
    use std::path::PathBuf;
    use std::ptr;
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::Foundation::GetLastError;
    use windows_sys::Win32::Foundation::HANDLE;
    use windows_sys::Win32::Foundation::HANDLE_FLAG_INHERIT;
    use windows_sys::Win32::Foundation::SetHandleInformation;
    use windows_sys::Win32::System::Pipes::CreatePipe;
    use windows_sys::Win32::System::Threading::GetExitCodeProcess;
    use windows_sys::Win32::System::Threading::INFINITE;
    use windows_sys::Win32::System::Threading::WaitForSingleObject;

    type PipeHandles = ((HANDLE, HANDLE), (HANDLE, HANDLE), (HANDLE, HANDLE));

    unsafe fn setup_stdio_pipes() -> io::Result<PipeHandles> {
        let mut in_r: HANDLE = 0;
        let mut in_w: HANDLE = 0;
        let mut out_r: HANDLE = 0;
        let mut out_w: HANDLE = 0;
        let mut err_r: HANDLE = 0;
        let mut err_w: HANDLE = 0;
        if CreatePipe(&mut in_r, &mut in_w, ptr::null_mut(), 0) == 0 {
            return Err(io::Error::from_raw_os_error(GetLastError() as i32));
        }
        if CreatePipe(&mut out_r, &mut out_w, ptr::null_mut(), 0) == 0 {
            return Err(io::Error::from_raw_os_error(GetLastError() as i32));
        }
        if CreatePipe(&mut err_r, &mut err_w, ptr::null_mut(), 0) == 0 {
            return Err(io::Error::from_raw_os_error(GetLastError() as i32));
        }
        if SetHandleInformation(in_r, HANDLE_FLAG_INHERIT, HANDLE_FLAG_INHERIT) == 0 {
            return Err(io::Error::from_raw_os_error(GetLastError() as i32));
        }
        if SetHandleInformation(out_w, HANDLE_FLAG_INHERIT, HANDLE_FLAG_INHERIT) == 0 {
            return Err(io::Error::from_raw_os_error(GetLastError() as i32));
        }
        if SetHandleInformation(err_w, HANDLE_FLAG_INHERIT, HANDLE_FLAG_INHERIT) == 0 {
            return Err(io::Error::from_raw_os_error(GetLastError() as i32));
        }
        Ok(((in_r, in_w), (out_r, out_w), (err_r, err_w)))
    }

    pub struct CaptureResult {
        pub exit_code: i32,
        pub stdout: Vec<u8>,
        pub stderr: Vec<u8>,
        pub timed_out: bool,
    }

    #[allow(clippy::too_many_arguments)]
    pub fn run_windows_sandbox_capture(
        policy_json_or_preset: &str,
        sandbox_policy_cwd: &Path,
        codex_home: &Path,
        command: Vec<String>,
        cwd: &Path,
        env_map: HashMap<String, String>,
        timeout_ms: Option<u64>,
        use_private_desktop: bool,
    ) -> Result<CaptureResult> {
        run_windows_sandbox_capture_with_extra_deny_write_paths(
            policy_json_or_preset,
            sandbox_policy_cwd,
            codex_home,
            command,
            cwd,
            env_map,
            timeout_ms,
            &[],
            use_private_desktop,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn run_windows_sandbox_capture_with_extra_deny_write_paths(
        policy_json_or_preset: &str,
        sandbox_policy_cwd: &Path,
        codex_home: &Path,
        command: Vec<String>,
        cwd: &Path,
        mut env_map: HashMap<String, String>,
        timeout_ms: Option<u64>,
        additional_deny_write_paths: &[PathBuf],
        use_private_desktop: bool,
    ) -> Result<CaptureResult> {
        let common = prepare_legacy_spawn_context(
            policy_json_or_preset,
            codex_home,
            cwd,
            &mut env_map,
            &command,
            /*inherit_path*/ false,
            /*add_git_safe_directory*/ false,
        )?;
        let policy = common.policy;
        let current_dir = common.current_dir;
        let logs_base_dir = common.logs_base_dir.as_deref();
        let is_workspace_write = common.is_workspace_write;
        if !policy.has_full_disk_read_access() {
            anyhow::bail!(
                "Restricted read-only access requires the elevated Windows sandbox backend"
            );
        }
        let caps = load_or_create_cap_sids(codex_home)?;
        let (h_token, psid_generic, psid_workspace): (HANDLE, *mut c_void, Option<*mut c_void>) = unsafe {
            match &policy {
                SandboxPolicy::ReadOnly { .. } => {
                    #[allow(clippy::expect_used)]
                    let psid =
                        convert_string_sid_to_sid(&caps.readonly).expect("valid readonly SID");
                    let (h, _) = super::token::create_readonly_token_with_cap(psid)?;
                    (h, psid, None)
                }
                SandboxPolicy::WorkspaceWrite { .. } => {
                    #[allow(clippy::expect_used)]
                    let psid_generic =
                        convert_string_sid_to_sid(&caps.workspace).expect("valid workspace SID");
                    let ws_sid = workspace_cap_sid_for_cwd(codex_home, cwd)?;
                    #[allow(clippy::expect_used)]
                    let psid_workspace =
                        convert_string_sid_to_sid(&ws_sid).expect("valid workspace SID");
                    let base = super::token::get_current_token_for_restriction()?;
                    let h_res = create_workspace_write_token_with_caps_from(
                        base,
                        &[psid_generic, psid_workspace],
                    );
                    windows_sys::Win32::Foundation::CloseHandle(base);
                    let h = h_res?;
                    (h, psid_generic, Some(psid_workspace))
                }
                SandboxPolicy::DangerFullAccess | SandboxPolicy::ExternalSandbox { .. } => {
                    unreachable!("DangerFullAccess handled above")
                }
            }
        };

        unsafe {
            if is_workspace_write
                && let Ok(base) = super::token::get_current_token_for_restriction()
            {
                if let Ok(bytes) = super::token::get_logon_sid_bytes(base) {
                    let mut tmp = bytes;
                    let psid2 = tmp.as_mut_ptr() as *mut c_void;
                    allow_null_device(psid2);
                }
                windows_sys::Win32::Foundation::CloseHandle(base);
            }
        }

        let persist_aces = is_workspace_write;
        let AllowDenyPaths { allow, mut deny } =
            compute_allow_paths(&policy, sandbox_policy_cwd, &current_dir, &env_map);
        for path in additional_deny_write_paths {
            if path.exists() {
                deny.insert(path.clone());
            }
        }
        let canonical_cwd = canonicalize_path(&current_dir);
        let mut guards: Vec<(PathBuf, *mut c_void)> = Vec::new();
        unsafe {
            for p in &allow {
                let psid = if is_workspace_write && is_command_cwd_root(p, &canonical_cwd) {
                    psid_workspace.unwrap_or(psid_generic)
                } else {
                    psid_generic
                };
                if let Ok(added) = add_allow_ace(p, psid)
                    && added
                {
                    if persist_aces {
                        if p.is_dir() {
                            // best-effort seeding omitted intentionally
                        }
                    } else {
                        guards.push((p.clone(), psid));
                    }
                }
            }
            for p in &deny {
                if let Ok(added) = add_deny_write_ace(p, psid_generic)
                    && added
                    && !persist_aces
                {
                    guards.push((p.clone(), psid_generic));
                }
            }
            allow_null_device(psid_generic);
            if let Some(psid) = psid_workspace {
                allow_null_device(psid);
            }
        }
        let (stdin_pair, stdout_pair, stderr_pair) = unsafe { setup_stdio_pipes()? };
        let ((in_r, in_w), (out_r, out_w), (err_r, err_w)) = (stdin_pair, stdout_pair, stderr_pair);
        let spawn_res = unsafe {
            create_process_as_user(
                h_token,
                &command,
                cwd,
                &env_map,
                logs_base_dir,
                Some((in_r, out_w, err_w)),
                use_private_desktop,
            )
        };
        let created = match spawn_res {
            Ok(v) => v,
            Err(err) => {
                unsafe {
                    CloseHandle(in_r);
                    CloseHandle(in_w);
                    CloseHandle(out_r);
                    CloseHandle(out_w);
                    CloseHandle(err_r);
                    CloseHandle(err_w);
                    CloseHandle(h_token);
                }
                return Err(err);
            }
        };
        let pi = created.process_info;
        let _desktop = created;

        unsafe {
            CloseHandle(in_r);
            // Close the parent's stdin write end so the child sees EOF immediately.
            CloseHandle(in_w);
            CloseHandle(out_w);
            CloseHandle(err_w);
        }

        let (tx_out, rx_out) = std::sync::mpsc::channel::<Vec<u8>>();
        let (tx_err, rx_err) = std::sync::mpsc::channel::<Vec<u8>>();
        let t_out = std::thread::spawn(move || {
            let mut buf = Vec::new();
            let mut tmp = [0u8; 8192];
            loop {
                let mut read_bytes: u32 = 0;
                let ok = unsafe {
                    windows_sys::Win32::Storage::FileSystem::ReadFile(
                        out_r,
                        tmp.as_mut_ptr(),
                        tmp.len() as u32,
                        &mut read_bytes,
                        std::ptr::null_mut(),
                    )
                };
                if ok == 0 || read_bytes == 0 {
                    break;
                }
                buf.extend_from_slice(&tmp[..read_bytes as usize]);
            }
            let _ = tx_out.send(buf);
        });
        let t_err = std::thread::spawn(move || {
            let mut buf = Vec::new();
            let mut tmp = [0u8; 8192];
            loop {
                let mut read_bytes: u32 = 0;
                let ok = unsafe {
                    windows_sys::Win32::Storage::FileSystem::ReadFile(
                        err_r,
                        tmp.as_mut_ptr(),
                        tmp.len() as u32,
                        &mut read_bytes,
                        std::ptr::null_mut(),
                    )
                };
                if ok == 0 || read_bytes == 0 {
                    break;
                }
                buf.extend_from_slice(&tmp[..read_bytes as usize]);
            }
            let _ = tx_err.send(buf);
        });

        let timeout = timeout_ms.map(|ms| ms as u32).unwrap_or(INFINITE);
        let res = unsafe { WaitForSingleObject(pi.hProcess, timeout) };
        let timed_out = res == 0x0000_0102;
        let mut exit_code_u32: u32 = 1;
        if !timed_out {
            unsafe {
                GetExitCodeProcess(pi.hProcess, &mut exit_code_u32);
            }
        } else {
            unsafe {
                windows_sys::Win32::System::Threading::TerminateProcess(pi.hProcess, 1);
            }
        }

        unsafe {
            if pi.hThread != 0 {
                CloseHandle(pi.hThread);
            }
            if pi.hProcess != 0 {
                CloseHandle(pi.hProcess);
            }
            CloseHandle(h_token);
        }
        let _ = t_out.join();
        let _ = t_err.join();
        let stdout = rx_out.recv().unwrap_or_default();
        let stderr = rx_err.recv().unwrap_or_default();
        let exit_code = if timed_out {
            128 + 64
        } else {
            exit_code_u32 as i32
        };

        if exit_code == 0 {
            log_success(&command, logs_base_dir);
        } else {
            log_failure(&command, &format!("exit code {exit_code}"), logs_base_dir);
        }

        if !persist_aces {
            unsafe {
                for (p, sid) in guards {
                    revoke_ace(&p, sid);
                }
            }
        }
        Ok(CaptureResult {
            exit_code,
            stdout,
            stderr,
            timed_out,
        })
    }

    pub fn run_windows_sandbox_legacy_preflight(
        sandbox_policy: &SandboxPolicy,
        sandbox_policy_cwd: &Path,
        codex_home: &Path,
        cwd: &Path,
        env_map: &HashMap<String, String>,
    ) -> Result<()> {
        let is_workspace_write = matches!(sandbox_policy, SandboxPolicy::WorkspaceWrite { .. });
        if !is_workspace_write {
            return Ok(());
        }

        ensure_codex_home_exists(codex_home)?;
        let caps = load_or_create_cap_sids(codex_home)?;
        #[allow(clippy::expect_used)]
        let psid_generic =
            unsafe { convert_string_sid_to_sid(&caps.workspace) }.expect("valid workspace SID");
        let ws_sid = workspace_cap_sid_for_cwd(codex_home, cwd)?;
        #[allow(clippy::expect_used)]
        let psid_workspace =
            unsafe { convert_string_sid_to_sid(&ws_sid) }.expect("valid workspace SID");
        let current_dir = cwd.to_path_buf();
        let AllowDenyPaths { allow, deny } =
            compute_allow_paths(sandbox_policy, sandbox_policy_cwd, &current_dir, env_map);
        let canonical_cwd = canonicalize_path(&current_dir);
        unsafe {
            for p in &allow {
                let psid = if is_command_cwd_root(p, &canonical_cwd) {
                    psid_workspace
                } else {
                    psid_generic
                };
                let _ = add_allow_ace(p, psid);
            }
            for p in &deny {
                let _ = add_deny_write_ace(p, psid_generic);
            }
            allow_null_device(psid_generic);
            allow_null_device(psid_workspace);
        }

        Ok(())
    }

    #[cfg(test)]
    mod tests {
        use crate::policy::SandboxPolicy;
        use crate::spawn_prep::should_apply_network_block;

        fn workspace_policy(network_access: bool) -> SandboxPolicy {
            SandboxPolicy::WorkspaceWrite {
                writable_roots: Vec::new(),
                network_access,
                exclude_tmpdir_env_var: false,
                exclude_slash_tmp: false,
            }
        }

        #[test]
        fn applies_network_block_when_access_is_disabled() {
            assert!(should_apply_network_block(&workspace_policy(
                /*network_access*/ false
            )));
        }

        #[test]
        fn skips_network_block_when_access_is_allowed() {
            assert!(!should_apply_network_block(&workspace_policy(
                /*network_access*/ true
            )));
        }

        #[test]
        fn applies_network_block_for_read_only() {
            assert!(should_apply_network_block(
                &SandboxPolicy::new_read_only_policy()
            ));
        }
    }
}

#[cfg(not(target_os = "windows"))]
mod stub {
    use anyhow::Result;
    use anyhow::bail;
    use codex_protocol::protocol::SandboxPolicy;
    use std::collections::HashMap;
    use std::path::Path;

    #[derive(Debug, Default)]
    pub struct CaptureResult {
        pub exit_code: i32,
        pub stdout: Vec<u8>,
        pub stderr: Vec<u8>,
        pub timed_out: bool,
    }

    #[allow(clippy::too_many_arguments)]
    pub fn run_windows_sandbox_capture(
        _policy_json_or_preset: &str,
        _sandbox_policy_cwd: &Path,
        _codex_home: &Path,
        _command: Vec<String>,
        _cwd: &Path,
        _env_map: HashMap<String, String>,
        _timeout_ms: Option<u64>,
        _use_private_desktop: bool,
    ) -> Result<CaptureResult> {
        bail!("Windows sandbox is only available on Windows")
    }

    pub fn apply_world_writable_scan_and_denies(
        _codex_home: &Path,
        _cwd: &Path,
        _env_map: &HashMap<String, String>,
        _sandbox_policy: &SandboxPolicy,
        _logs_base_dir: Option<&Path>,
    ) -> Result<()> {
        bail!("Windows sandbox is only available on Windows")
    }

    pub fn run_windows_sandbox_legacy_preflight(
        _sandbox_policy: &SandboxPolicy,
        _sandbox_policy_cwd: &Path,
        _codex_home: &Path,
        _cwd: &Path,
        _env_map: &HashMap<String, String>,
    ) -> Result<()> {
        bail!("Windows sandbox is only available on Windows")
    }
}

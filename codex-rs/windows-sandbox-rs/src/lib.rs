macro_rules! windows_modules {
    ($($name:ident),+ $(,)?) => {
        $(#[cfg(target_os = "windows")] mod $name;)+
    };
}

windows_modules!(
    acl, allow, audit, cap, dpapi, env, identity, logging, policy, process, token, winutil
);

#[cfg(target_os = "windows")]
#[path = "setup_orchestrator.rs"]
mod setup;

#[cfg(target_os = "windows")]
mod elevated_impl;

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
pub use dpapi::protect as dpapi_protect;
#[cfg(target_os = "windows")]
pub use dpapi::unprotect as dpapi_unprotect;
#[cfg(target_os = "windows")]
pub use elevated_impl::run_windows_sandbox_capture as run_windows_sandbox_capture_elevated;
#[cfg(target_os = "windows")]
pub use identity::require_logon_sandbox_creds;
#[cfg(target_os = "windows")]
pub use logging::log_note;
#[cfg(target_os = "windows")]
pub use logging::LOG_FILE_NAME;
#[cfg(target_os = "windows")]
pub use policy::parse_policy;
#[cfg(target_os = "windows")]
pub use policy::SandboxPolicy;
#[cfg(target_os = "windows")]
pub use process::create_process_as_user;
#[cfg(target_os = "windows")]
pub use setup::run_elevated_setup;
#[cfg(target_os = "windows")]
pub use setup::run_setup_refresh;
#[cfg(target_os = "windows")]
pub use setup::sandbox_dir;
#[cfg(target_os = "windows")]
pub use setup::SETUP_VERSION;
#[cfg(target_os = "windows")]
pub use token::convert_string_sid_to_sid;
#[cfg(target_os = "windows")]
pub use token::create_readonly_token_with_cap_from;
#[cfg(target_os = "windows")]
pub use token::create_workspace_write_token_with_cap_from;
#[cfg(target_os = "windows")]
pub use token::get_current_token_for_restriction;
#[cfg(target_os = "windows")]
pub use windows_impl::run_windows_sandbox_capture;
#[cfg(target_os = "windows")]
pub use windows_impl::CaptureResult;
#[cfg(target_os = "windows")]
pub use winutil::string_from_sid_bytes;
#[cfg(target_os = "windows")]
pub use winutil::to_wide;

#[cfg(not(target_os = "windows"))]
pub use stub::apply_world_writable_scan_and_denies;
#[cfg(not(target_os = "windows"))]
pub use stub::run_windows_sandbox_capture;
#[cfg(not(target_os = "windows"))]
pub use stub::CaptureResult;

#[cfg(target_os = "windows")]
mod windows_impl {
    use super::acl::add_allow_ace;
    use super::acl::add_deny_write_ace;
    use super::acl::allow_null_device;
    use super::acl::revoke_ace;
    use super::allow::compute_allow_paths;
    use super::allow::AllowDenyPaths;
    use super::cap::load_or_create_cap_sids;
    use super::env::apply_no_network_to_env;
    use super::env::ensure_non_interactive_pager;
    use super::env::normalize_null_device_env;
    use super::logging::debug_log;
    use super::logging::log_failure;
    use super::logging::log_start;
    use super::logging::log_success;
    use super::policy::parse_policy;
    use super::policy::SandboxPolicy;
    use super::process::make_env_block;
    use super::token::convert_string_sid_to_sid;
    use super::winutil::format_last_error;
    use super::winutil::quote_windows_arg;
    use super::winutil::to_wide;
    use anyhow::Result;
    use std::collections::HashMap;
    use std::ffi::c_void;
    use std::io;
    use std::path::Path;
    use std::path::PathBuf;
    use std::ptr;
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::Foundation::GetLastError;
    use windows_sys::Win32::Foundation::SetHandleInformation;
    use windows_sys::Win32::Foundation::HANDLE;
    use windows_sys::Win32::Foundation::HANDLE_FLAG_INHERIT;
    use windows_sys::Win32::System::Pipes::CreatePipe;
    use windows_sys::Win32::System::Threading::CreateProcessAsUserW;
    use windows_sys::Win32::System::Threading::GetExitCodeProcess;
    use windows_sys::Win32::System::Threading::WaitForSingleObject;
    use windows_sys::Win32::System::Threading::CREATE_UNICODE_ENVIRONMENT;
    use windows_sys::Win32::System::Threading::INFINITE;
    use windows_sys::Win32::System::Threading::PROCESS_INFORMATION;
    use windows_sys::Win32::System::Threading::STARTF_USESTDHANDLES;
    use windows_sys::Win32::System::Threading::STARTUPINFOW;

    type PipeHandles = ((HANDLE, HANDLE), (HANDLE, HANDLE), (HANDLE, HANDLE));

    fn should_apply_network_block(policy: &SandboxPolicy) -> bool {
        !policy.has_full_network_access()
    }

    fn ensure_codex_home_exists(p: &Path) -> Result<()> {
        std::fs::create_dir_all(p)?;
        Ok(())
    }

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

    pub fn run_windows_sandbox_capture(
        policy_json_or_preset: &str,
        sandbox_policy_cwd: &Path,
        codex_home: &Path,
        command: Vec<String>,
        cwd: &Path,
        mut env_map: HashMap<String, String>,
        timeout_ms: Option<u64>,
    ) -> Result<CaptureResult> {
        let policy = parse_policy(policy_json_or_preset)?;
        let apply_network_block = should_apply_network_block(&policy);
        normalize_null_device_env(&mut env_map);
        ensure_non_interactive_pager(&mut env_map);
        if apply_network_block {
            apply_no_network_to_env(&mut env_map)?;
        }
        ensure_codex_home_exists(codex_home)?;
        let current_dir = cwd.to_path_buf();
        let sandbox_base = codex_home.join(".sandbox");
        std::fs::create_dir_all(&sandbox_base)?;
        let logs_base_dir = Some(sandbox_base.as_path());
        log_start(&command, logs_base_dir);
        let is_workspace_write = matches!(&policy, SandboxPolicy::WorkspaceWrite { .. });

        if matches!(
            &policy,
            SandboxPolicy::DangerFullAccess | SandboxPolicy::ExternalSandbox { .. }
        ) {
            anyhow::bail!("DangerFullAccess and ExternalSandbox are not supported for sandboxing")
        }
        let caps = load_or_create_cap_sids(codex_home)?;
        let (h_token, psid_to_use): (HANDLE, *mut c_void) = unsafe {
            match &policy {
                SandboxPolicy::ReadOnly => {
                    let psid = convert_string_sid_to_sid(&caps.readonly).unwrap();
                    super::token::create_readonly_token_with_cap(psid)?
                }
                SandboxPolicy::WorkspaceWrite { .. } => {
                    let psid = convert_string_sid_to_sid(&caps.workspace).unwrap();
                    super::token::create_workspace_write_token_with_cap(psid)?
                }
                SandboxPolicy::DangerFullAccess | SandboxPolicy::ExternalSandbox { .. } => {
                    unreachable!("DangerFullAccess handled above")
                }
            }
        };

        unsafe {
            if is_workspace_write {
                if let Ok(base) = super::token::get_current_token_for_restriction() {
                    if let Ok(bytes) = super::token::get_logon_sid_bytes(base) {
                        let mut tmp = bytes.clone();
                        let psid2 = tmp.as_mut_ptr() as *mut c_void;
                        allow_null_device(psid2);
                    }
                    windows_sys::Win32::Foundation::CloseHandle(base);
                }
            }
        }

        let persist_aces = is_workspace_write;
        let AllowDenyPaths { allow, deny } =
            compute_allow_paths(&policy, sandbox_policy_cwd, &current_dir, &env_map);
        let mut guards: Vec<(PathBuf, *mut c_void)> = Vec::new();
        unsafe {
            for p in &allow {
                if let Ok(added) = add_allow_ace(p, psid_to_use) {
                    if added {
                        if persist_aces {
                            if p.is_dir() {
                                // best-effort seeding omitted intentionally
                            }
                        } else {
                            guards.push((p.clone(), psid_to_use));
                        }
                    }
                }
            }
            for p in &deny {
                if let Ok(added) = add_deny_write_ace(p, psid_to_use) {
                    if added && !persist_aces {
                        guards.push((p.clone(), psid_to_use));
                    }
                }
            }
            allow_null_device(psid_to_use);
        }

        let (stdin_pair, stdout_pair, stderr_pair) = unsafe { setup_stdio_pipes()? };
        let ((in_r, in_w), (out_r, out_w), (err_r, err_w)) = (stdin_pair, stdout_pair, stderr_pair);
        let mut si: STARTUPINFOW = unsafe { std::mem::zeroed() };
        si.cb = std::mem::size_of::<STARTUPINFOW>() as u32;
        si.dwFlags |= STARTF_USESTDHANDLES;
        si.hStdInput = in_r;
        si.hStdOutput = out_w;
        si.hStdError = err_w;

        let mut pi: PROCESS_INFORMATION = unsafe { std::mem::zeroed() };
        let cmdline_str = command
            .iter()
            .map(|a| quote_windows_arg(a))
            .collect::<Vec<_>>()
            .join(" ");
        let mut cmdline: Vec<u16> = to_wide(&cmdline_str);
        let env_block = make_env_block(&env_map);
        let desktop = to_wide("Winsta0\\Default");
        si.lpDesktop = desktop.as_ptr() as *mut u16;
        let spawn_res = unsafe {
            CreateProcessAsUserW(
                h_token,
                ptr::null(),
                cmdline.as_mut_ptr(),
                ptr::null_mut(),
                ptr::null_mut(),
                1,
                CREATE_UNICODE_ENVIRONMENT,
                env_block.as_ptr() as *mut c_void,
                to_wide(cwd).as_ptr(),
                &si,
                &mut pi,
            )
        };
        if spawn_res == 0 {
            let err = unsafe { GetLastError() } as i32;
            let dbg = format!(
                "CreateProcessAsUserW failed: {} ({}) | cwd={} | cmd={} | env_u16_len={} | si_flags={}",
                err,
                format_last_error(err),
                cwd.display(),
                cmdline_str,
                env_block.len(),
                si.dwFlags,
            );
            debug_log(&dbg, logs_base_dir);
            unsafe {
                CloseHandle(in_r);
                CloseHandle(in_w);
                CloseHandle(out_r);
                CloseHandle(out_w);
                CloseHandle(err_r);
                CloseHandle(err_w);
                CloseHandle(h_token);
            }
            return Err(anyhow::anyhow!("CreateProcessAsUserW failed: {}", err));
        }

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
            log_failure(&command, &format!("exit code {}", exit_code), logs_base_dir);
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

    #[cfg(test)]
    mod tests {
        use super::should_apply_network_block;
        use crate::policy::SandboxPolicy;

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
            assert!(should_apply_network_block(&workspace_policy(false)));
        }

        #[test]
        fn skips_network_block_when_access_is_allowed() {
            assert!(!should_apply_network_block(&workspace_policy(true)));
        }

        #[test]
        fn applies_network_block_for_read_only() {
            assert!(should_apply_network_block(&SandboxPolicy::ReadOnly));
        }
    }
}

#[cfg(not(target_os = "windows"))]
mod stub {
    use anyhow::bail;
    use anyhow::Result;
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

    pub fn run_windows_sandbox_capture(
        _policy_json_or_preset: &str,
        _sandbox_policy_cwd: &Path,
        _codex_home: &Path,
        _command: Vec<String>,
        _cwd: &Path,
        _env_map: HashMap<String, String>,
        _timeout_ms: Option<u64>,
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
}

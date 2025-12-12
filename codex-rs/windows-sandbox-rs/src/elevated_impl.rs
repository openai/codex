mod windows_impl {
    use crate::acl::allow_null_device;
    use crate::allow::compute_allow_paths;
    use crate::allow::AllowDenyPaths;
    use crate::cap::cap_sid_file;
    use crate::cap::load_or_create_cap_sids;
    use crate::env::ensure_non_interactive_pager;
    use crate::env::inherit_path_env;
    use crate::env::normalize_null_device_env;
    use crate::identity::require_logon_sandbox_creds;
    use crate::logging::debug_log;
    use crate::logging::log_failure;
    use crate::logging::log_note;
    use crate::logging::log_start;
    use crate::logging::log_success;
    use crate::policy::parse_policy;
    use crate::policy::SandboxPolicy;
    use crate::token::convert_string_sid_to_sid;
    use crate::winutil::format_last_error;
    use crate::winutil::quote_windows_arg;
    use crate::winutil::to_wide;
    use anyhow::Result;
    use rand::rngs::SmallRng;
    use rand::Rng;
    use rand::SeedableRng;
    use std::collections::HashMap;
    use std::ffi::c_void;
    use std::fs;
    use std::io;
    use std::path::Path;
    use std::path::PathBuf;
    use std::ptr;
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::Foundation::GetLastError;
    use windows_sys::Win32::Foundation::HANDLE;
    use windows_sys::Win32::Security::Authorization::ConvertStringSecurityDescriptorToSecurityDescriptorW;
    use windows_sys::Win32::Security::PSECURITY_DESCRIPTOR;
    use windows_sys::Win32::Security::SECURITY_ATTRIBUTES;
    use windows_sys::Win32::System::Diagnostics::Debug::SetErrorMode;
    use windows_sys::Win32::System::Pipes::ConnectNamedPipe;
    use windows_sys::Win32::System::Pipes::CreateNamedPipeW;
    // PIPE_ACCESS_DUPLEX is 0x00000003; not exposed in windows-sys 0.52, so use the value directly.
    const PIPE_ACCESS_DUPLEX: u32 = 0x0000_0003;
    use windows_sys::Win32::System::Pipes::PIPE_READMODE_BYTE;
    use windows_sys::Win32::System::Pipes::PIPE_TYPE_BYTE;
    use windows_sys::Win32::System::Pipes::PIPE_WAIT;
    use windows_sys::Win32::System::Threading::CreateProcessWithLogonW;
    use windows_sys::Win32::System::Threading::GetExitCodeProcess;
    use windows_sys::Win32::System::Threading::WaitForSingleObject;
    use windows_sys::Win32::System::Threading::INFINITE;
    use windows_sys::Win32::System::Threading::LOGON_WITH_PROFILE;
    use windows_sys::Win32::System::Threading::PROCESS_INFORMATION;
    use windows_sys::Win32::System::Threading::STARTUPINFOW;

    /// Ensures the parent directory of a path exists before writing to it.
    fn ensure_dir(p: &Path) -> Result<()> {
        if let Some(d) = p.parent() {
            std::fs::create_dir_all(d)?;
        }
        Ok(())
    }

    /// Walks upward from `start` to locate the git worktree root, following gitfile redirects.
    fn find_git_root(start: &Path) -> Option<PathBuf> {
        let mut cur = dunce::canonicalize(start).ok()?;
        loop {
            let marker = cur.join(".git");
            if marker.is_dir() {
                return Some(cur);
            }
            if marker.is_file() {
                if let Ok(txt) = std::fs::read_to_string(&marker) {
                    if let Some(rest) = txt.trim().strip_prefix("gitdir:") {
                        let gitdir = rest.trim();
                        let resolved = if Path::new(gitdir).is_absolute() {
                            PathBuf::from(gitdir)
                        } else {
                            cur.join(gitdir)
                        };
                        return resolved.parent().map(|p| p.to_path_buf()).or(Some(cur));
                    }
                }
                return Some(cur);
            }
            let parent = cur.parent()?;
            if parent == cur {
                return None;
            }
            cur = parent.to_path_buf();
        }
    }

    /// Creates the sandbox user's Codex home directory if it does not already exist.
    fn ensure_codex_home_exists(p: &Path) -> Result<()> {
        std::fs::create_dir_all(p)?;
        Ok(())
    }

    /// Adds a git safe.directory entry to the environment when running inside a repository.
    /// git will not otherwise allow the Sandbox user to run git commands on the repo directory
    /// which is owned by the primary user.
    fn inject_git_safe_directory(
        env_map: &mut HashMap<String, String>,
        cwd: &Path,
        logs_base_dir: Option<&Path>,
    ) {
        if let Some(git_root) = find_git_root(cwd) {
            let mut cfg_count: usize = env_map
                .get("GIT_CONFIG_COUNT")
                .and_then(|v| v.parse::<usize>().ok())
                .unwrap_or(0);
            let git_path = git_root.to_string_lossy().replace("\\\\", "/");
            env_map.insert(
                format!("GIT_CONFIG_KEY_{cfg_count}"),
                "safe.directory".to_string(),
            );
            env_map.insert(format!("GIT_CONFIG_VALUE_{cfg_count}"), git_path);
            cfg_count += 1;
            env_map.insert("GIT_CONFIG_COUNT".to_string(), cfg_count.to_string());
            log_note(
                &format!("injected git safe.directory for {}", git_root.display()),
                logs_base_dir,
            );
        }
    }

    /// Locates `codex-command-runner.exe` next to the current binary.
    fn find_runner_exe() -> PathBuf {
        if let Ok(exe) = std::env::current_exe() {
            if let Some(dir) = exe.parent() {
                let candidate = dir.join("codex-command-runner.exe");
                if candidate.exists() {
                    return candidate;
                }
            }
        }
        PathBuf::from("codex-command-runner.exe")
    }

    /// Generates a unique named-pipe path used to communicate with the runner process.
    fn pipe_name(suffix: &str) -> String {
        let mut rng = SmallRng::from_entropy();
        format!(r"\\.\pipe\codex-runner-{:x}-{}", rng.gen::<u128>(), suffix)
    }

    /// Creates a named pipe with permissive ACLs so the sandbox user can connect.
    fn create_named_pipe(name: &str, access: u32) -> io::Result<HANDLE> {
        // Allow sandbox users to connect by granting Everyone full access on the pipe.
        let sddl = to_wide("D:(A;;GA;;;WD)");
        let mut sd: PSECURITY_DESCRIPTOR = ptr::null_mut();
        let ok = unsafe {
            ConvertStringSecurityDescriptorToSecurityDescriptorW(
                sddl.as_ptr(),
                1, // SDDL_REVISION_1
                &mut sd,
                ptr::null_mut(),
            )
        };
        if ok == 0 {
            return Err(io::Error::from_raw_os_error(unsafe {
                GetLastError() as i32
            }));
        }
        let mut sa = SECURITY_ATTRIBUTES {
            nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: sd,
            bInheritHandle: 0,
        };
        let wide = to_wide(name);
        let h = unsafe {
            CreateNamedPipeW(
                wide.as_ptr(),
                access,
                PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT,
                1,
                65536,
                65536,
                0,
                &mut sa as *mut SECURITY_ATTRIBUTES,
            )
        };
        if h == 0 || h == windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE {
            return Err(io::Error::from_raw_os_error(unsafe {
                GetLastError() as i32
            }));
        }
        Ok(h)
    }

    /// Waits for a client connection on the named pipe, tolerating an existing connection.
    fn connect_pipe(h: HANDLE) -> io::Result<()> {
        let ok = unsafe { ConnectNamedPipe(h, ptr::null_mut()) };
        if ok == 0 {
            let err = unsafe { GetLastError() };
            const ERROR_PIPE_CONNECTED: u32 = 535;
            if err != ERROR_PIPE_CONNECTED {
                return Err(io::Error::from_raw_os_error(err as i32));
            }
        }
        Ok(())
    }

    pub use crate::windows_impl::CaptureResult;

    #[derive(serde::Serialize)]
    struct RunnerPayload {
        policy_json_or_preset: String,
        sandbox_policy_cwd: PathBuf,
        // Writable log dir for sandbox user (.codex in sandbox profile).
        codex_home: PathBuf,
        // Real user's CODEX_HOME for shared data (caps, config).
        real_codex_home: PathBuf,
        cap_sid: String,
        request_file: Option<PathBuf>,
        command: Vec<String>,
        cwd: PathBuf,
        env_map: HashMap<String, String>,
        timeout_ms: Option<u64>,
        stdin_pipe: String,
        stdout_pipe: String,
        stderr_pipe: String,
    }

    /// Launches the command runner under the sandbox user and captures its output.
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
        normalize_null_device_env(&mut env_map);
        ensure_non_interactive_pager(&mut env_map);
        inherit_path_env(&mut env_map);
        inject_git_safe_directory(&mut env_map, cwd, None);
        let current_dir = cwd.to_path_buf();
        // Use a temp-based log dir that the sandbox user can write.
        let sandbox_base = codex_home.join(".sandbox");
        ensure_codex_home_exists(&sandbox_base)?;

        let logs_base_dir: Option<&Path> = Some(sandbox_base.as_path());
        log_start(&command, logs_base_dir);
        let sandbox_creds =
            require_logon_sandbox_creds(&policy, sandbox_policy_cwd, cwd, &env_map, codex_home)?;
        log_note("cli creds ready", logs_base_dir);
        let cap_sid_path = cap_sid_file(codex_home);

        // Build capability SID for ACL grants.
        let (psid_to_use, cap_sid_str) = match &policy {
            SandboxPolicy::ReadOnly => {
                let caps = load_or_create_cap_sids(codex_home);
                ensure_dir(&cap_sid_path)?;
                fs::write(&cap_sid_path, serde_json::to_string(&caps)?)?;
                (
                    unsafe { convert_string_sid_to_sid(&caps.readonly).unwrap() },
                    caps.readonly.clone(),
                )
            }
            SandboxPolicy::WorkspaceWrite { .. } => {
                let caps = load_or_create_cap_sids(codex_home);
                ensure_dir(&cap_sid_path)?;
                fs::write(&cap_sid_path, serde_json::to_string(&caps)?)?;
                (
                    unsafe { convert_string_sid_to_sid(&caps.workspace).unwrap() },
                    caps.workspace.clone(),
                )
            }
            SandboxPolicy::DangerFullAccess => {
                anyhow::bail!("DangerFullAccess is not supported for sandboxing")
            }
        };

        let AllowDenyPaths { allow, deny } =
            compute_allow_paths(&policy, sandbox_policy_cwd, &current_dir, &env_map);
        // Deny/allow ACEs are now applied during setup; avoid per-command churn.
        log_note(
            &format!(
                "cli skipping per-command ACL grants (allow_count={} deny_count={})",
                allow.len(),
                deny.len()
            ),
            logs_base_dir,
        );
        unsafe {
            allow_null_device(psid_to_use);
        }

        // Prepare named pipes for runner.
        let stdin_name = pipe_name("stdin");
        let stdout_name = pipe_name("stdout");
        let stderr_name = pipe_name("stderr");
        let h_stdin_pipe = create_named_pipe(
            &stdin_name,
            PIPE_ACCESS_DUPLEX | PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT,
        )?;
        let h_stdout_pipe = create_named_pipe(
            &stdout_name,
            PIPE_ACCESS_DUPLEX | PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT,
        )?;
        let h_stderr_pipe = create_named_pipe(
            &stderr_name,
            PIPE_ACCESS_DUPLEX | PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT,
        )?;
        log_note(
            &format!(
                "cli pipes created stdin={} stdout={} stderr={}",
                stdin_name, stdout_name, stderr_name
            ),
            logs_base_dir,
        );

        // Launch runner as sandbox user via CreateProcessWithLogonW.
        let runner_exe = find_runner_exe();
        let runner_cmdline = runner_exe
            .to_str()
            .map(|s| s.to_string())
            .unwrap_or_else(|| "codex-command-runner.exe".to_string());
        // Write request to a file under the sandbox base dir for the runner to read.
        // TODO(iceweasel) - use a different mechanism for invoking the runner.
        let base_tmp = sandbox_base.join("requests");
        std::fs::create_dir_all(&base_tmp)?;
        let mut rng = SmallRng::from_entropy();
        let req_file = base_tmp.join(format!("request-{:x}.json", rng.gen::<u128>()));
        let payload = RunnerPayload {
            policy_json_or_preset: policy_json_or_preset.to_string(),
            sandbox_policy_cwd: sandbox_policy_cwd.to_path_buf(),
            codex_home: sandbox_base.clone(),
            real_codex_home: codex_home.to_path_buf(),
            cap_sid: cap_sid_str.clone(),
            request_file: Some(req_file.clone()),
            command: command.clone(),
            cwd: cwd.to_path_buf(),
            env_map: env_map.clone(),
            timeout_ms,
            stdin_pipe: stdin_name.clone(),
            stdout_pipe: stdout_name.clone(),
            stderr_pipe: stderr_name.clone(),
        };
        let payload_json = serde_json::to_string(&payload)?;
        if let Err(e) = fs::write(&req_file, &payload_json) {
            log_note(
                &format!("error writing request file {}: {}", req_file.display(), e),
                logs_base_dir,
            );
            return Err(e.into());
        }
        log_note(
            &format!("cli request file written path={}", req_file.display()),
            logs_base_dir,
        );
        let runner_full_cmd = format!(
            "{} {}",
            quote_windows_arg(&runner_cmdline),
            quote_windows_arg(&format!("--request-file={}", req_file.display()))
        );
        let mut cmdline_vec: Vec<u16> = to_wide(&runner_full_cmd);
        let exe_w: Vec<u16> = to_wide(&runner_cmdline);
        let cwd_w: Vec<u16> = to_wide(cwd);
        log_note(
            &format!("cli prep done request_file={}", req_file.display()),
            logs_base_dir,
        );
        log_note(
            &format!("cli about to launch runner cmd={}", runner_full_cmd),
            logs_base_dir,
        );

        // Minimal CPWL launch: inherit env, no desktop override, no handle inheritance.
        let env_block: Option<Vec<u16>> = None;
        let mut si: STARTUPINFOW = unsafe { std::mem::zeroed() };
        si.cb = std::mem::size_of::<STARTUPINFOW>() as u32;
        let mut pi: PROCESS_INFORMATION = unsafe { std::mem::zeroed() };
        let user_w = to_wide(&sandbox_creds.username);
        let domain_w = to_wide(".");
        let password_w = to_wide(&sandbox_creds.password);
        // Suppress WER/UI popups from the runner process so we can collect exit codes.
        let _ = unsafe { SetErrorMode(0x0001 | 0x0002) }; // SEM_FAILCRITICALERRORS | SEM_NOGPFAULTERRORBOX

        // Ensure command line buffer is mutable and includes the exe as argv[0].
        let spawn_res = unsafe {
            CreateProcessWithLogonW(
                user_w.as_ptr(),
                domain_w.as_ptr(),
                password_w.as_ptr(),
                LOGON_WITH_PROFILE,
                exe_w.as_ptr(),
                cmdline_vec.as_mut_ptr(),
                windows_sys::Win32::System::Threading::CREATE_NO_WINDOW
                    | windows_sys::Win32::System::Threading::CREATE_UNICODE_ENVIRONMENT,
                env_block
                    .as_ref()
                    .map(|b| b.as_ptr() as *const c_void)
                    .unwrap_or(ptr::null()),
                cwd_w.as_ptr(),
                &si,
                &mut pi,
            )
        };
        if spawn_res == 0 {
            let err = unsafe { GetLastError() } as i32;
            let dbg = format!(
                "CreateProcessWithLogonW failed: {} ({}) | cwd={} | cmd={} | req_file={} | env=inherit | si_flags={}",
                err,
                format_last_error(err),
                cwd.display(),
                runner_full_cmd,
                req_file.display(),
                si.dwFlags,
            );
            debug_log(&dbg, logs_base_dir);
            log_note(&dbg, logs_base_dir);
            return Err(anyhow::anyhow!("CreateProcessWithLogonW failed: {}", err));
        }
        log_note(
            &format!("cli runner launched pid={}", pi.hProcess),
            logs_base_dir,
        );

        // Pipes are no longer passed as std handles; no stdin payload is sent.
        connect_pipe(h_stdin_pipe)?;
        connect_pipe(h_stdout_pipe)?;
        connect_pipe(h_stderr_pipe)?;
        unsafe {
            CloseHandle(h_stdin_pipe);
        }

        // Read stdout/stderr.
        let (tx_out, rx_out) = std::sync::mpsc::channel::<Vec<u8>>();
        let (tx_err, rx_err) = std::sync::mpsc::channel::<Vec<u8>>();
        let t_out = std::thread::spawn(move || {
            let mut buf = Vec::new();
            let mut tmp = [0u8; 8192];
            loop {
                let mut read_bytes: u32 = 0;
                let ok = unsafe {
                    windows_sys::Win32::Storage::FileSystem::ReadFile(
                        h_stdout_pipe,
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
                        h_stderr_pipe,
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
            CloseHandle(h_stdout_pipe);
            CloseHandle(h_stderr_pipe);
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

        Ok(CaptureResult {
            exit_code,
            stdout,
            stderr,
            timed_out,
        })
    }

    #[cfg(test)]
    mod tests {
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
            assert!(!workspace_policy(false).has_full_network_access());
        }

        #[test]
        fn skips_network_block_when_access_is_allowed() {
            assert!(workspace_policy(true).has_full_network_access());
        }

        #[test]
        fn applies_network_block_for_read_only() {
            assert!(!SandboxPolicy::ReadOnly.has_full_network_access());
        }
    }
}

#[cfg(target_os = "windows")]
pub use windows_impl::run_windows_sandbox_capture;

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

    /// Stub implementation for non-Windows targets; sandboxing only works on Windows.
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
}

#[cfg(not(target_os = "windows"))]
pub use stub::run_windows_sandbox_capture;

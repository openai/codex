macro_rules! windows_modules {
    ($($name:ident),+ $(,)?) => {
        $(#[cfg(target_os = "windows")] mod $name;)+
    };
}

windows_modules!(
    acl, allow, audit, cap, dpapi, env, identity, logging, policy, process, setup, token, winutil
);

#[cfg(target_os = "windows")]
pub use acl::{add_allow_ace, add_deny_write_ace, allow_null_device};
#[cfg(target_os = "windows")]
pub use allow::compute_allow_paths;
#[cfg(target_os = "windows")]
pub use audit::apply_world_writable_scan_and_denies;
#[cfg(target_os = "windows")]
pub use cap::{cap_sid_file, load_or_create_cap_sids};
#[cfg(target_os = "windows")]
pub use dpapi::protect as dpapi_protect;
#[cfg(target_os = "windows")]
pub use dpapi::unprotect as dpapi_unprotect;
#[cfg(target_os = "windows")]
pub use identity::require_logon_sandbox_creds;
#[cfg(target_os = "windows")]
pub use logging::log_note;
#[cfg(target_os = "windows")]
pub use policy::{parse_policy, SandboxPolicy};
#[cfg(target_os = "windows")]
pub use process::create_process_as_user;
#[cfg(target_os = "windows")]
pub use setup::run_elevated_setup;
#[cfg(target_os = "windows")]
pub use setup::sandbox_dir;
#[cfg(target_os = "windows")]
pub use setup::SETUP_VERSION;
#[cfg(target_os = "windows")]
pub use token::{
    convert_string_sid_to_sid, create_readonly_token_with_cap_from,
    create_workspace_write_token_with_cap_from, get_current_token_for_restriction,
};
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
pub use stub::run_elevated_setup;
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
    use super::cap::cap_sid_file;
    use super::cap::load_or_create_cap_sids;
    use super::env::apply_no_network_to_env;
    use super::env::ensure_non_interactive_pager;
    use super::env::normalize_null_device_env;
    use super::identity::require_logon_sandbox_creds;
    use super::logging::debug_log;
    use super::logging::log_failure;
    use super::logging::log_note;
    use super::logging::log_start;
    use super::logging::log_success;
    use super::policy::parse_policy;
    use super::policy::SandboxPolicy;
    use super::token::convert_string_sid_to_sid;
    use super::winutil::format_last_error;
    use super::winutil::to_wide;
    use anyhow::Result;
    use rand::rngs::SmallRng;
    use rand::Rng;
    use rand::SeedableRng;
    use std::collections::HashMap;
    use std::ffi::c_void;
    use std::fs;
    use std::io;
    use std::os::windows::io::FromRawHandle;
    use std::path::Path;
    use std::path::PathBuf;
    use std::ptr;
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::Foundation::GetLastError;
    use windows_sys::Win32::Foundation::HANDLE;
    use windows_sys::Win32::System::Pipes::ConnectNamedPipe;
    use windows_sys::Win32::System::Pipes::CreateNamedPipeW;
    // PIPE_ACCESS_DUPLEX is 0x00000003; not exposed in windows-sys 0.52, so use the value directly.
    const PIPE_ACCESS_DUPLEX: u32 = 0x0000_0003;
    use windows_sys::Win32::Security::Authorization::ConvertStringSecurityDescriptorToSecurityDescriptorW;
    use windows_sys::Win32::Security::LogonUserW;
    use windows_sys::Win32::Security::LOGON32_LOGON_INTERACTIVE;
    use windows_sys::Win32::Security::LOGON32_PROVIDER_DEFAULT;
    use windows_sys::Win32::Security::{PSECURITY_DESCRIPTOR, SECURITY_ATTRIBUTES};
    use windows_sys::Win32::System::Environment::CreateEnvironmentBlock;
    use windows_sys::Win32::System::Environment::DestroyEnvironmentBlock;
    use windows_sys::Win32::System::Pipes::PIPE_READMODE_BYTE;
    use windows_sys::Win32::System::Pipes::PIPE_TYPE_BYTE;
    use windows_sys::Win32::System::Pipes::PIPE_WAIT;
    use windows_sys::Win32::System::Threading::CreateProcessWithLogonW;
    use windows_sys::Win32::System::Threading::GetExitCodeProcess;
    use windows_sys::Win32::System::Threading::WaitForSingleObject;
    use windows_sys::Win32::System::Threading::CREATE_UNICODE_ENVIRONMENT;
    use windows_sys::Win32::System::Threading::INFINITE;
    use windows_sys::Win32::System::Threading::LOGON_WITH_PROFILE;
    use windows_sys::Win32::System::Threading::PROCESS_INFORMATION;
    use windows_sys::Win32::System::Threading::STARTUPINFOW;
    use windows_sys::Win32::UI::Shell::LoadUserProfileA;
    use windows_sys::Win32::UI::Shell::UnloadUserProfile;
    use windows_sys::Win32::UI::Shell::PROFILEINFOA;

    fn should_apply_network_block(policy: &SandboxPolicy) -> bool {
        !policy.has_full_network_access()
    }

    fn ensure_dir(p: &Path) -> Result<()> {
        if let Some(d) = p.parent() {
            std::fs::create_dir_all(d)?;
        }
        Ok(())
    }

    fn ensure_codex_home_exists(p: &Path) -> Result<()> {
        std::fs::create_dir_all(p)?;
        Ok(())
    }

    fn find_runner_exe() -> PathBuf {
        if let Ok(exe) = std::env::current_exe() {
            if let Some(dir) = exe.parent() {
                let candidate = dir.join("codex-command-runner.exe");
                if candidate.exists() {
                    return candidate;
                }
                let release_candidate = dir
                    .parent()
                    .map(|p| p.join("release").join("codex-command-runner.exe"));
                if let Some(rel) = release_candidate {
                    if rel.exists() {
                        return rel;
                    }
                }
            }
        }
        PathBuf::from("codex-command-runner.exe")
    }

    fn make_env_block(env: &HashMap<String, String>) -> Vec<u16> {
        let mut items: Vec<(String, String)> =
            env.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
        items.sort_by(|a, b| {
            a.0.to_uppercase()
                .cmp(&b.0.to_uppercase())
                .then(a.0.cmp(&b.0))
        });
        let mut w: Vec<u16> = Vec::new();
        for (k, v) in items {
            let mut s = to_wide(format!("{}={}", k, v));
            s.pop();
            w.extend_from_slice(&s);
            w.push(0);
        }
        w.push(0);
        w
    }

    // Quote a single Windows command-line argument following the rules used by
    // CommandLineToArgvW/CRT so that spaces, quotes, and backslashes are preserved.
    // Reference behavior matches Rust std::process::Command on Windows.
    fn quote_windows_arg(arg: &str) -> String {
        let needs_quotes = arg.is_empty()
            || arg
                .chars()
                .any(|c| matches!(c, ' ' | '\t' | '\n' | '\r' | '"'));
        if !needs_quotes {
            return arg.to_string();
        }

        let mut quoted = String::with_capacity(arg.len() + 2);
        quoted.push('"');
        let mut backslashes = 0;
        for ch in arg.chars() {
            match ch {
                '\\' => {
                    backslashes += 1;
                }
                '"' => {
                    quoted.push_str(&"\\".repeat(backslashes * 2 + 1));
                    quoted.push('"');
                    backslashes = 0;
                }
                _ => {
                    if backslashes > 0 {
                        quoted.push_str(&"\\".repeat(backslashes));
                        backslashes = 0;
                    }
                    quoted.push(ch);
                }
            }
        }
        if backslashes > 0 {
            quoted.push_str(&"\\".repeat(backslashes * 2));
        }
        quoted.push('"');
        quoted
    }

    fn pipe_name(suffix: &str) -> String {
        let mut rng = SmallRng::from_entropy();
        format!(r"\\.\pipe\codex-runner-{:x}-{}", rng.gen::<u128>(), suffix)
    }

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

    pub struct CaptureResult {
        pub exit_code: i32,
        pub stdout: Vec<u8>,
        pub stderr: Vec<u8>,
        pub timed_out: bool,
    }

    #[derive(serde::Serialize)]
    struct RunnerPayload {
        policy_json_or_preset: String,
        sandbox_policy_cwd: PathBuf,
        codex_home: PathBuf,
        command: Vec<String>,
        cwd: PathBuf,
        env_map: HashMap<String, String>,
        timeout_ms: Option<u64>,
        stdin_pipe: String,
        stdout_pipe: String,
        stderr_pipe: String,
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
        let logs_base_dir = Some(codex_home);
        log_start(&command, logs_base_dir);
        let cap_sid_path = cap_sid_file(codex_home);
        let is_workspace_write = matches!(&policy, SandboxPolicy::WorkspaceWrite { .. });
        let sandbox_creds =
            require_logon_sandbox_creds(&policy, sandbox_policy_cwd, cwd, &env_map, codex_home)?;

        // Build capability SID for ACL grants.
        let psid_to_use = match &policy {
            SandboxPolicy::ReadOnly => {
                let caps = load_or_create_cap_sids(codex_home);
                ensure_dir(&cap_sid_path)?;
                fs::write(&cap_sid_path, serde_json::to_string(&caps)?)?;
                unsafe { convert_string_sid_to_sid(&caps.readonly).unwrap() }
            }
            SandboxPolicy::WorkspaceWrite { .. } => {
                let caps = load_or_create_cap_sids(codex_home);
                ensure_dir(&cap_sid_path)?;
                fs::write(&cap_sid_path, serde_json::to_string(&caps)?)?;
                unsafe { convert_string_sid_to_sid(&caps.workspace).unwrap() }
            }
            SandboxPolicy::DangerFullAccess => {
                anyhow::bail!("DangerFullAccess is not supported for sandboxing")
            }
        };

        let persist_aces = is_workspace_write;
        let AllowDenyPaths { allow, deny } =
            compute_allow_paths(&policy, sandbox_policy_cwd, &current_dir, &env_map);
        let mut guards: Vec<(PathBuf, *mut c_void)> = Vec::new();
        for p in &deny {
            unsafe {
                if let Ok(added) = add_deny_write_ace(p, psid_to_use) {
                    if added && !persist_aces {
                        guards.push((p.clone(), psid_to_use));
                    }
                }
            }
        }
        if is_workspace_write {
            for p in &allow {
                unsafe {
                    if let Ok(added) = add_allow_ace(p, psid_to_use) {
                        if added && !persist_aces {
                            guards.push((p.clone(), psid_to_use));
                        }
                    }
                }
            }
        }
        unsafe {
            allow_null_device(psid_to_use);
        }

        // Prepare named pipes for runner.
        let stdin_name = pipe_name("stdin");
        let stdout_name = pipe_name("stdout");
        let stderr_name = pipe_name("stderr");
        log_note(
            &format!(
                "preparing pipes stdin={} stdout={} stderr={}",
                stdin_name, stdout_name, stderr_name
            ),
            logs_base_dir,
        );
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

        // Build runner payload.
        let payload = RunnerPayload {
            policy_json_or_preset: policy_json_or_preset.to_string(),
            sandbox_policy_cwd: sandbox_policy_cwd.to_path_buf(),
            codex_home: codex_home.to_path_buf(),
            command: command.clone(),
            cwd: cwd.to_path_buf(),
            env_map: env_map.clone(),
            timeout_ms,
            stdin_pipe: stdin_name.clone(),
            stdout_pipe: stdout_name.clone(),
            stderr_pipe: stderr_name.clone(),
        };
        let payload_json = serde_json::to_string(&payload)?;

        // Launch runner as sandbox user via CreateProcessWithLogonW.
        let runner_exe = find_runner_exe();
        let runner_cmdline = runner_exe
            .to_str()
            .map(|s| s.to_string())
            .unwrap_or_else(|| "codex-command-runner.exe".to_string());
        log_note(
            &format!(
                "launching runner exe={} as user={} cwd={}",
                runner_cmdline,
                sandbox_creds.username,
                cwd.display()
            ),
            logs_base_dir,
        );
        let cmdline_str = quote_windows_arg(&runner_cmdline);
        let mut cmdline: Vec<u16> = to_wide(&cmdline_str);

        fn build_sandbox_env_block(
            username: &str,
            password: &str,
            logs_base_dir: Option<&Path>,
        ) -> Option<Vec<u16>> {
            unsafe {
                let user_w = to_wide(username);
                let domain_w = to_wide(".");
                let password_w = to_wide(password);
                let mut h_tok: HANDLE = 0;
                let ok = LogonUserW(
                    user_w.as_ptr(),
                    domain_w.as_ptr(),
                    password_w.as_ptr(),
                    LOGON32_LOGON_INTERACTIVE,
                    LOGON32_PROVIDER_DEFAULT,
                    &mut h_tok,
                );
                if ok == 0 || h_tok == 0 {
                    log_note(
                        &format!(
                            "build_sandbox_env_block: LogonUserW failed for {} err={}",
                            username,
                            GetLastError()
                        ),
                        logs_base_dir,
                    );
                    return None;
                }

                let mut profile: PROFILEINFOA = std::mem::zeroed();
                profile.dwSize = std::mem::size_of::<PROFILEINFOA>() as u32;
                profile.lpUserName = user_w.as_ptr() as *mut _;
                let profile_loaded = LoadUserProfileA(h_tok, &mut profile as *mut _);
                if profile_loaded == 0 {
                    log_note(
                        &format!(
                            "build_sandbox_env_block: LoadUserProfile failed err={}",
                            GetLastError()
                        ),
                        logs_base_dir,
                    );
                }

                let mut env_block_ptr: *mut std::ffi::c_void = std::ptr::null_mut();
                let env_ok = CreateEnvironmentBlock(&mut env_block_ptr, h_tok, 0);
                if env_ok == 0 || env_block_ptr.is_null() {
                    log_note(
                        &format!(
                            "build_sandbox_env_block: CreateEnvironmentBlock failed err={}",
                            GetLastError()
                        ),
                        logs_base_dir,
                    );
                    if profile_loaded != 0 {
                        let _ = UnloadUserProfile(h_tok, profile.hProfile);
                    }
                    CloseHandle(h_tok);
                    return None;
                }

                // Convert env block to map for patch/logging.
                let mut map = HashMap::new();
                let mut ptr_u16 = env_block_ptr as *const u16;
                loop {
                    // find len to null
                    let mut len = 0;
                    while *ptr_u16.add(len) != 0 {
                        len += 1;
                    }
                    if len == 0 {
                        break;
                    }
                    let slice = std::slice::from_raw_parts(ptr_u16, len);
                    if let Ok(s) = String::from_utf16(slice) {
                        if let Some((k, v)) = s.split_once('=') {
                            map.insert(k.to_string(), v.to_string());
                        }
                    }
                    ptr_u16 = ptr_u16.add(len + 1);
                }

                // Patch critical vars to the sandbox profile.
                let profile_dir = format!(r"C:\Users\{}", username);
                map.insert("USERPROFILE".to_string(), profile_dir.clone());
                map.insert("HOMEDRIVE".to_string(), "C:".to_string());
                map.insert("HOMEPATH".to_string(), format!(r"\Users\{}", username));
                map.entry("SystemRoot".to_string())
                    .or_insert_with(|| "C:\\Windows".to_string());
                map.entry("WINDIR".to_string())
                    .or_insert_with(|| "C:\\Windows".to_string());
                let local_app = format!(r"{}\AppData\Local", profile_dir);
                let appdata = format!(r"{}\AppData\Roaming", profile_dir);
                map.insert("LOCALAPPDATA".to_string(), local_app.clone());
                map.insert("APPDATA".to_string(), appdata);
                let temp = format!(r"{}\Temp", local_app);
                map.insert("TEMP".to_string(), temp.clone());
                map.insert("TMP".to_string(), temp);

                // Log env
                let mut vars: Vec<String> =
                    map.iter().map(|(k, v)| format!("{}={}", k, v)).collect();
                vars.sort();
                log_note(
                    &format!(
                        "build_sandbox_env_block for {}:\n{}",
                        username,
                        vars.join("\n")
                    ),
                    logs_base_dir,
                );

                // Rebuild env block
                let env_block = make_env_block(&map);

                DestroyEnvironmentBlock(env_block_ptr);
                if profile_loaded != 0 {
                    let _ = UnloadUserProfile(h_tok, profile.hProfile);
                }
                CloseHandle(h_tok);

                Some(env_block)
            }
        }

        let env_block = build_sandbox_env_block(
            &sandbox_creds.username,
            &sandbox_creds.password,
            logs_base_dir,
        );
        let env_log = if env_block.is_some() {
            "runner env_block: custom sandbox profile env"
        } else {
            "runner env_block: inherit (sandbox user profile defaults)"
        };
        log_note(env_log, logs_base_dir);
        let desktop = to_wide("Winsta0\\Default");
        let mut si: STARTUPINFOW = unsafe { std::mem::zeroed() };
        si.cb = std::mem::size_of::<STARTUPINFOW>() as u32;
        si.lpDesktop = desktop.as_ptr() as *mut u16;
        let mut pi: PROCESS_INFORMATION = unsafe { std::mem::zeroed() };
        let user_w = to_wide(&sandbox_creds.username);
        let domain_w = to_wide(".");
        let password_w = to_wide(&sandbox_creds.password);
        let spawn_res = unsafe {
            CreateProcessWithLogonW(
                user_w.as_ptr(),
                domain_w.as_ptr(),
                password_w.as_ptr(),
                LOGON_WITH_PROFILE,
                ptr::null(),
                cmdline.as_mut_ptr(),
                CREATE_UNICODE_ENVIRONMENT,
                env_block
                    .as_ref()
                    .map(|b| b.as_ptr() as *const c_void)
                    .unwrap_or(ptr::null()),
                to_wide(cwd).as_ptr(),
                &si,
                &mut pi,
            )
        };
        if spawn_res == 0 {
            let err = unsafe { GetLastError() } as i32;
            let dbg = format!(
                "CreateProcessWithLogonW failed: {} ({}) | cwd={} | cmd={} | env=inherit | si_flags={}",
                err,
                format_last_error(err),
                cwd.display(),
                cmdline_str,
                si.dwFlags,
            );
            debug_log(&dbg, logs_base_dir);
            log_note(&dbg, logs_base_dir);
            return Err(anyhow::anyhow!("CreateProcessWithLogonW failed: {}", err));
        }
        log_note("runner process launched", logs_base_dir);

        // Connect pipes and send payload.
        connect_pipe(h_stdin_pipe)?;
        connect_pipe(h_stdout_pipe)?;
        connect_pipe(h_stderr_pipe)?;
        {
            use std::io::Write;
            let mut writer = unsafe { std::fs::File::from_raw_handle(h_stdin_pipe as _) };
            writer.write_all(payload_json.as_bytes())?;
        }
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
        log_note(
            &format!(
                "runner exited timed_out={} code={}",
                timed_out, exit_code_u32
            ),
            logs_base_dir,
        );

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

    pub fn run_elevated_setup(
        _policy: &SandboxPolicy,
        _policy_cwd: &Path,
        _command_cwd: &Path,
        _env_map: &HashMap<String, String>,
        _codex_home: &Path,
    ) -> Result<()> {
        bail!("Windows sandbox is only available on Windows")
    }
}

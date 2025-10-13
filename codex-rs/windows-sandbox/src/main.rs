//! Codex Windows sandbox (Option A): same-user, Low Integrity, no admin.
//! - Write allowed only in explicitly labeled Low-IL roots (MIC NoWriteUp enforces the rest)
//! - “Deny most network” via env/proxy/tool flags (no firewall rules)
//! - Process tree attached to a Job Object with KILL_ON_JOB_CLOSE
//! - No AppContainer; no global ACL edits; no extra users.

use clap::Parser;
use codex_protocol::protocol::SandboxPolicy;
use std::collections::HashMap;
use std::env;
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(
    name = "codex-windows-sandbox",
    about = "Run a command inside a Windows Low-Integrity sandbox (no admin)"
)]
struct WindowsSandboxCommand {
    /// Working directory used when resolving relative sandbox policy paths.
    #[arg(long)]
    sandbox_policy_cwd: Option<PathBuf>,

    /// Sandbox policy to apply. Accepts preset names ('workspace-write', 'read-only', 'danger-full-access')
    /// or a JSON-encoded SandboxPolicy.
    pub sandbox_policy: SandboxPolicyArg,

    /// Command and arguments to execute once sandboxing is configured.
    #[arg(trailing_var_arg = true, allow_hyphen_values = true, required = true)]
    pub command: Vec<String>,
}

#[derive(Debug, Clone)]
struct SandboxPolicyArg(SandboxPolicy);

impl SandboxPolicyArg {
    fn into_policy(self) -> SandboxPolicy {
        self.0
    }
}

impl std::str::FromStr for SandboxPolicyArg {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "read-only" => Ok(Self(SandboxPolicy::new_read_only_policy())),
            "workspace-write" => Ok(Self(SandboxPolicy::new_workspace_write_policy())),
            "danger-full-access" => Ok(Self(SandboxPolicy::DangerFullAccess)),
            other => serde_json::from_str::<SandboxPolicy>(other)
                .map(Self)
                .map_err(|err| format!("failed to parse sandbox policy: {err}")),
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum StdioPolicy {
    Inherit,
}

fn main() -> ! {
    let args = WindowsSandboxCommand::parse();
    let WindowsSandboxCommand {
        sandbox_policy_cwd,
        sandbox_policy,

        command,
    } = args;

    let policy = sandbox_policy.into_policy();

    if command.is_empty() {
        eprintln!("No command specified to execute.");
        std::process::exit(2);
    }

    let current_dir = match env::current_dir() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("failed to get current dir: {e}");
            std::process::exit(1);
        }
    };
    let sandbox_policy_cwd = sandbox_policy_cwd.unwrap_or_else(|| current_dir.clone());
    let env_map: HashMap<String, String> = env::vars().collect();

    let status = if cfg!(target_os = "windows") {
        imp::spawn_command_under_windows_low_il(
            command,
            current_dir,
            &policy,
            sandbox_policy_cwd.as_path(),
            StdioPolicy::Inherit,
            env_map,
        )
    } else {
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "Windows sandbox is only available on Windows",
        ))
    };

    match status {
        Ok(exit_status) => {
            if let Some(code) = exit_status.code() {
                std::process::exit(code);
            }
            if exit_status.success() {
                std::process::exit(0);
            }
            std::process::exit(1);
        }
        Err(err) => {
            eprintln!("failed to run sandboxed command: {err}");
            std::process::exit(1);
        }
    }
}

// -------------------------- Windows implementation --------------------------
#[cfg(target_os = "windows")]
mod imp {
    use super::SandboxPolicy;
    use super::StdioPolicy;
    use std::collections::HashMap;
    use std::ffi::OsStr;
    use std::io::ErrorKind;
    use std::io::{self};
    use std::mem::size_of;
    use std::os::windows::ffi::OsStrExt;

    use std::os::windows::process::ExitStatusExt;
    use std::path::Path;
    use std::path::PathBuf;
    use std::process::ExitStatus;

    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::Foundation::HANDLE;
    use windows::Win32::Foundation::HANDLE_FLAG_INHERIT;
    use windows::Win32::Foundation::HLOCAL;
    use windows::Win32::Foundation::INVALID_HANDLE_VALUE;
    use windows::Win32::Foundation::LocalFree;
    use windows::Win32::Foundation::PSID;
    use windows::Win32::Foundation::SetHandleInformation;
    use windows::Win32::Foundation::WAIT_OBJECT_0;
    use windows::Win32::Foundation::WIN32_ERROR;
    use windows::Win32::Security::Authorization::ConvertStringSecurityDescriptorToSecurityDescriptorW;
    use windows::Win32::Security::Authorization::ConvertStringSidToSidW;
    use windows::Win32::Security::Authorization::SE_FILE_OBJECT;
    use windows::Win32::Security::Authorization::SetNamedSecurityInfoW;
    use windows::Win32::Security::DuplicateTokenEx;
    use windows::Win32::Security::GetLengthSid;
    use windows::Win32::Security::GetSecurityDescriptorSacl;
    use windows::Win32::Security::LABEL_SECURITY_INFORMATION;
    use windows::Win32::Security::PSECURITY_DESCRIPTOR;
    use windows::Win32::Security::SACL_SECURITY_INFORMATION;
    use windows::Win32::Security::SID_AND_ATTRIBUTES;
    use windows::Win32::Security::SecurityImpersonation;
    use windows::Win32::Security::TOKEN_ACCESS_MASK;
    use windows::Win32::Security::TOKEN_ADJUST_DEFAULT;
    use windows::Win32::Security::TOKEN_ADJUST_PRIVILEGES;
    use windows::Win32::Security::TOKEN_ADJUST_SESSIONID;
    use windows::Win32::Security::TOKEN_ASSIGN_PRIMARY;
    use windows::Win32::Security::TOKEN_DUPLICATE;
    use windows::Win32::Security::TOKEN_MANDATORY_LABEL;
    use windows::Win32::Security::TOKEN_QUERY;
    use windows::Win32::Security::TokenIntegrityLevel;
    use windows::Win32::Security::TokenPrimary;
    use windows::Win32::Storage::FileSystem::GetFileAttributesW;
    use windows::Win32::System::Console::GetStdHandle;
    use windows::Win32::System::Console::STD_ERROR_HANDLE;
    use windows::Win32::System::Console::STD_INPUT_HANDLE;
    use windows::Win32::System::Console::STD_OUTPUT_HANDLE;
    use windows::Win32::System::JobObjects::AssignProcessToJobObject;
    use windows::Win32::System::JobObjects::CreateJobObjectW;
    use windows::Win32::System::JobObjects::JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
    use windows::Win32::System::JobObjects::JOBOBJECT_EXTENDED_LIMIT_INFORMATION;
    use windows::Win32::System::JobObjects::SetInformationJobObject;
    use windows::Win32::System::SystemServices::SE_GROUP_INTEGRITY;
    use windows::Win32::System::Threading::CREATE_UNICODE_ENVIRONMENT;
    use windows::Win32::System::Threading::CreateProcessAsUserW;
    use windows::Win32::System::Threading::GetCurrentProcess;
    use windows::Win32::System::Threading::GetExitCodeProcess;
    use windows::Win32::System::Threading::OpenProcessToken;
    use windows::Win32::System::Threading::PROCESS_CREATION_FLAGS;
    use windows::Win32::System::Threading::PROCESS_INFORMATION;
    use windows::Win32::System::Threading::STARTF_USESTDHANDLES;
    use windows::Win32::System::Threading::STARTUPINFOW;
    use windows::Win32::System::Threading::WaitForSingleObject;
    use windows::core::PCWSTR;
    use windows::core::PWSTR;

    pub(super) fn spawn_command_under_windows_low_il(
        command: Vec<String>,
        command_cwd: PathBuf,
        policy: &SandboxPolicy,
        policy_cwd: &Path,
        stdio: StdioPolicy,
        mut env_map: HashMap<String, String>,
    ) -> io::Result<ExitStatus> {
        if command.is_empty() {
            return Err(io::Error::new(
                ErrorKind::InvalidInput,
                "command args are empty",
            ));
        }

        // 1) Map SandboxPolicy → writable roots and read-only overrides.
        let (writable_roots, read_only_overrides) = roots_from_policy(policy, policy_cwd);

        // 2) Label only the allowed writable roots as Low IL (add guards to revert to Medium).
        let mut guards: Vec<LabelGuard> = Vec::new();
        match policy {
            SandboxPolicy::DangerFullAccess => {}
            SandboxPolicy::ReadOnly => {}
            SandboxPolicy::WorkspaceWrite { .. } => {
                for root in &writable_roots {
                    if path_exists(root)
                        && let Ok(g) = set_low_integrity_dir_guarded(root)
                    {
                        guards.push(g);
                    }
                }
                for ro in &read_only_overrides {
                    if path_exists(ro) {
                        let _ = set_medium_integrity_dir(ro);
                    }
                }
            }
        }

        // A private Low-IL temp/home subtree inside the workspace for common tools.
        if !matches!(policy, SandboxPolicy::ReadOnly) {
            let tmp = command_cwd.join(".codex-sbx").join("tmp");
            let _ = std::fs::create_dir_all(&tmp);
            if path_exists(&tmp) {
                if let Ok(g) = set_low_integrity_dir_guarded(&tmp) {
                    guards.push(g);
                }
                env_map.insert("TEMP".into(), tmp.display().to_string());
                env_map.insert("TMP".into(), tmp.display().to_string());
            }
            let home = command_cwd.join(".codex-sbx").join("home");
            let _ = std::fs::create_dir_all(&home);
            if path_exists(&home) {
                if let Ok(g) = set_low_integrity_dir_guarded(&home) {
                    guards.push(g);
                }
                env_map.insert("HOME".into(), home.display().to_string());
                env_map.insert("USERPROFILE".into(), home.display().to_string());
                let appdata = home.join("AppData").join("Roaming");
                let local = home.join("AppData").join("Local");
                let _ = std::fs::create_dir_all(&appdata);
                let _ = std::fs::create_dir_all(&local);
                env_map.insert("APPDATA".into(), appdata.display().to_string());
                env_map.insert("LOCALAPPDATA".into(), local.display().to_string());
            }
        }

        // 3) Best-effort network deny when not allowed.
        if !policy.has_full_network_access() {
            apply_best_effort_network_block(&mut env_map);
        }
        ensure_non_interactive_pager(&mut env_map);

        // 4) Create a restricted, Low-IL primary token from the current process token.
        let token = create_restricted_token()?;

        // 5) Startup info with inherited stdio.
        let mut si = STARTUPINFOW {
            cb: size_of::<STARTUPINFOW>() as u32,
            ..Default::default()
        };
        apply_stdio_policy(&mut si, stdio)?;

        // 6) Environment block
        let mut env_block = build_environment_block(&env_map);
        let env_ptr: Option<*const core::ffi::c_void> = if env_block.is_empty() {
            None
        } else {
            Some(env_block.as_mut_ptr().cast::<core::ffi::c_void>() as *const _)
        };

        // 7) Command line + cwd
        let mut cmdline = build_cmdline(&command);
        let mut cwd_w = to_wide(&command_cwd);
        let cwd_pcw = if cwd_w.is_empty() {
            PCWSTR::null()
        } else {
            PCWSTR(cwd_w.as_mut_ptr())
        };

        // 8) Spawn and attach job
        let mut pi = PROCESS_INFORMATION::default();
        unsafe {
            CreateProcessAsUserW(
                token.handle,
                PCWSTR::null(),
                PWSTR(cmdline.as_mut_ptr()),
                None,
                None,
                true,
                PROCESS_CREATION_FLAGS(CREATE_UNICODE_ENVIRONMENT.0),
                env_ptr,
                cwd_pcw,
                &si,
                &mut pi,
            )
            .map_err(|e| io::Error::from_raw_os_error(e.code().0))?;
            if !si.hStdInput.is_invalid() {
                let _ = CloseHandle(si.hStdInput);
            }
            if !si.hStdOutput.is_invalid() {
                let _ = CloseHandle(si.hStdOutput);
            }
            if !si.hStdError.is_invalid() {
                let _ = CloseHandle(si.hStdError);
            }
        }

        let job = JobGuard::create()?;
        unsafe {
            AssignProcessToJobObject(job.handle, pi.hProcess)
                .map_err(|e| io::Error::from_raw_os_error(e.code().0))?;
        }

        wait_for_process(&pi)
    }

    // Policy mapping
    fn roots_from_policy(policy: &SandboxPolicy, base: &Path) -> (Vec<PathBuf>, Vec<PathBuf>) {
        match policy {
            SandboxPolicy::DangerFullAccess => (vec![], vec![]),
            SandboxPolicy::ReadOnly => (vec![], vec![]),
            SandboxPolicy::WorkspaceWrite { .. } => {
                let roots = policy.get_writable_roots_with_cwd(base);
                let mut allows = Vec::new();
                let mut ro = Vec::new();
                for w in roots {
                    allows.push(w.root);
                    ro.extend(w.read_only_subpaths);
                }
                (allows, ro)
            }
        }
    }

    // stdio
    fn apply_stdio_policy(si: &mut STARTUPINFOW, policy: StdioPolicy) -> io::Result<()> {
        match policy {
            StdioPolicy::Inherit => unsafe {
                let stdin = ensure_valid(GetStdHandle(STD_INPUT_HANDLE)?)?;
                let stdout = ensure_valid(GetStdHandle(STD_OUTPUT_HANDLE)?)?;
                let stderr = ensure_valid(GetStdHandle(STD_ERROR_HANDLE)?)?;
                SetHandleInformation(stdin, HANDLE_FLAG_INHERIT.0, HANDLE_FLAG_INHERIT)
                    .map_err(|e| io::Error::from_raw_os_error(e.code().0))?;
                SetHandleInformation(stdout, HANDLE_FLAG_INHERIT.0, HANDLE_FLAG_INHERIT)
                    .map_err(|e| io::Error::from_raw_os_error(e.code().0))?;
                SetHandleInformation(stderr, HANDLE_FLAG_INHERIT.0, HANDLE_FLAG_INHERIT)
                    .map_err(|e| io::Error::from_raw_os_error(e.code().0))?;
                si.dwFlags |= STARTF_USESTDHANDLES;
                si.hStdInput = stdin;
                si.hStdOutput = stdout;
                si.hStdError = stderr;
                Ok(())
            },
        }
    }
    unsafe fn ensure_valid(h: HANDLE) -> io::Result<HANDLE> {
        if h == INVALID_HANDLE_VALUE || h.is_invalid() {
            Err(io::Error::last_os_error())
        } else {
            Ok(h)
        }
    }

    // command line & env
    fn build_cmdline(args: &[String]) -> Vec<u16> {
        let mut s = String::new();
        for (i, a) in args.iter().enumerate() {
            if i > 0 {
                s.push(' ');
            }
            s.push_str(&quote_arg(a));
        }
        let mut w: Vec<u16> = s.encode_utf16().collect();
        w.push(0);
        w
    }
    fn quote_arg(a: &str) -> String {
        if !needs_quotes(a) {
            return a.to_string();
        }
        let mut out = String::from("\"");
        let mut bs = 0;
        for ch in a.chars() {
            match ch {
                '\\' => bs += 1,
                '"' => {
                    out.extend(std::iter::repeat_n('\\', bs * 2 + 1));
                    out.push('"');
                    bs = 0;
                }
                _ => {
                    if bs > 0 {
                        out.extend(std::iter::repeat_n('\\', bs * 2));
                        bs = 0;
                    }
                    out.push(ch);
                }
            }
        }
        if bs > 0 {
            out.extend(std::iter::repeat_n('\\', bs * 2));
        }
        out.push('"');
        out
    }
    fn needs_quotes(s: &str) -> bool {
        s.is_empty()
            || s.chars()
                .any(|c| matches!(c, ' ' | '\t' | '\n' | '\r' | '\u{0b}' | '"'))
    }
    fn build_environment_block(env: &HashMap<String, String>) -> Vec<u16> {
        if env.is_empty() {
            return Vec::new();
        }
        let mut pairs: Vec<(String, String)> =
            env.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
        pairs.sort_by(|(ak, _), (bk, _)| {
            let au = ak.to_ascii_uppercase();
            let bu = bk.to_ascii_uppercase();
            match au.cmp(&bu) {
                std::cmp::Ordering::Equal => ak.cmp(bk),
                o => o,
            }
        });
        let mut block = Vec::new();
        for (k, v) in pairs {
            let entry = format!("{k}={v}");
            block.extend(entry.encode_utf16());
            block.push(0);
        }
        block.push(0);
        block
    }
    fn to_wide<S: AsRef<OsStr>>(s: S) -> Vec<u16> {
        s.as_ref().encode_wide().chain(std::iter::once(0)).collect()
    }

    // best-effort network block
    fn ensure_non_interactive_pager(env_map: &mut HashMap<String, String>) {
        env_map
            .entry("GIT_PAGER".into())
            .or_insert_with(|| "more.com".into());
        env_map
            .entry("PAGER".into())
            .or_insert_with(|| "more.com".into());
        env_map.entry("LESS".into()).or_insert_with(String::new);
    }

    fn apply_best_effort_network_block(env_map: &mut HashMap<String, String>) {
        let sink = "http://127.0.0.1:9";
        env_map
            .entry("HTTP_PROXY".into())
            .or_insert_with(|| sink.into());
        env_map
            .entry("HTTPS_PROXY".into())
            .or_insert_with(|| sink.into());
        env_map
            .entry("ALL_PROXY".into())
            .or_insert_with(|| sink.into());
        env_map
            .entry("NO_PROXY".into())
            .or_insert_with(|| "localhost,127.0.0.1,::1".into());
        env_map
            .entry("PIP_NO_INDEX".into())
            .or_insert_with(|| "1".into());
        env_map
            .entry("PIP_DISABLE_PIP_VERSION_CHECK".into())
            .or_insert_with(|| "1".into());
        env_map
            .entry("NPM_CONFIG_OFFLINE".into())
            .or_insert_with(|| "true".into());
        env_map
            .entry("CARGO_NET_OFFLINE".into())
            .or_insert_with(|| "true".into());
    }

    // integrity labeling (MIC)
    struct LabelGuard {
        path: PathBuf,
        reverted: bool,
    }
    impl Drop for LabelGuard {
        fn drop(&mut self) {
            if !self.reverted {
                let _ = set_medium_integrity_dir(&self.path);
                self.reverted = true;
            }
        }
    }
    fn set_low_integrity_dir_guarded(path: &Path) -> io::Result<LabelGuard> {
        set_integrity_sddl(path, "S:(ML;OICI;NW;;;LW)")?;
        Ok(LabelGuard {
            path: path.to_path_buf(),
            reverted: false,
        })
    }
    fn set_medium_integrity_dir(path: &Path) -> io::Result<()> {
        set_integrity_sddl(path, "S:(ML;OICI;NW;;;ME)")
    }
    fn set_integrity_sddl(path: &Path, sddl: &str) -> io::Result<()> {
        unsafe {
            let sddl_w: Vec<u16> = OsStr::new(sddl).encode_wide().chain(Some(0)).collect();
            let mut psd = PSECURITY_DESCRIPTOR::default();
            ConvertStringSecurityDescriptorToSecurityDescriptorW(
                PCWSTR(sddl_w.as_ptr()),
                1,
                &mut psd,
                None,
            )
            .map_err(|e| io::Error::from_raw_os_error(e.code().0))?;
            let mut sacl_present = false.into();
            let mut sacl_defaulted = false.into();
            let mut sacl_ptr = std::ptr::null_mut();
            GetSecurityDescriptorSacl(psd, &mut sacl_present, &mut sacl_ptr, &mut sacl_defaulted)
                .map_err(|e| io::Error::from_raw_os_error(e.code().0))?;
            let wide = to_wide(path.as_os_str());
            let status = SetNamedSecurityInfoW(
                PCWSTR(wide.as_ptr()),
                SE_FILE_OBJECT,
                LABEL_SECURITY_INFORMATION | SACL_SECURITY_INFORMATION,
                None,
                None,
                None,
                Some(sacl_ptr),
            );
            if !psd.is_invalid() {
                let _ = LocalFree(HLOCAL(psd.0.cast()));
            }
            if status != WIN32_ERROR(0) {
                return Err(io::Error::from_raw_os_error(status.0 as i32));
            }
        }
        Ok(())
    }
    fn path_exists(p: &Path) -> bool {
        let w = to_wide(p.as_os_str());
        unsafe { GetFileAttributesW(PCWSTR(w.as_ptr())) != u32::MAX }
    }

    // token + job object
    struct TokenGuard {
        handle: HANDLE,
    }
    impl Drop for TokenGuard {
        fn drop(&mut self) {
            unsafe {
                if !self.handle.is_invalid() {
                    let _ = CloseHandle(self.handle);
                }
            }
        }
    }
    struct JobGuard {
        handle: HANDLE,
    }
    impl JobGuard {
        fn create() -> io::Result<Self> {
            unsafe {
                let h = CreateJobObjectW(None, PCWSTR::null())
                    .map_err(|e| io::Error::from_raw_os_error(e.code().0))?;
                let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
                limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE
                    | windows::Win32::System::JobObjects::JOB_OBJECT_LIMIT_BREAKAWAY_OK
                    | windows::Win32::System::JobObjects::JOB_OBJECT_LIMIT_SILENT_BREAKAWAY_OK;
                SetInformationJobObject(
                    h,
                    windows::Win32::System::JobObjects::JobObjectExtendedLimitInformation,
                    &limits as *const _ as *const core::ffi::c_void,
                    size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
                )
                .map_err(|e| io::Error::from_raw_os_error(e.code().0))?;
                Ok(Self { handle: h })
            }
        }
    }
    impl Drop for JobGuard {
        fn drop(&mut self) {
            unsafe {
                if !self.handle.is_invalid() {
                    let _ = CloseHandle(self.handle);
                }
            }
        }
    }
    fn create_restricted_token() -> io::Result<TokenGuard> {
        unsafe {
            let mut process_token = HANDLE::default();
            let desired = TOKEN_ACCESS_MASK(
                TOKEN_DUPLICATE.0
                    | TOKEN_QUERY.0
                    | TOKEN_ASSIGN_PRIMARY.0
                    | TOKEN_ADJUST_DEFAULT.0
                    | TOKEN_ADJUST_SESSIONID.0
                    | TOKEN_ADJUST_PRIVILEGES.0,
            );
            OpenProcessToken(GetCurrentProcess(), desired, &mut process_token)
                .map_err(|e| io::Error::from_raw_os_error(e.code().0))?;
            let mut new_token = HANDLE::default();
            let duplicate = DuplicateTokenEx(
                process_token,
                desired,
                None,
                SecurityImpersonation,
                TokenPrimary,
                &mut new_token,
            );
            let _ = CloseHandle(process_token);
            duplicate.map_err(|e| io::Error::from_raw_os_error(e.code().0))?;
            set_integrity_low(new_token)?;
            Ok(TokenGuard { handle: new_token })
        }
    }
    unsafe fn set_integrity_low(token: HANDLE) -> io::Result<()> {
        unsafe {
            let mut sid = PSID::default();
            let s = OsStr::new("S-1-16-4096")
                .encode_wide()
                .chain(Some(0))
                .collect::<Vec<u16>>();
            ConvertStringSidToSidW(PCWSTR(s.as_ptr()), &mut sid)
                .map_err(|e| io::Error::from_raw_os_error(e.code().0))?;
            let tml = TOKEN_MANDATORY_LABEL {
                Label: SID_AND_ATTRIBUTES {
                    Sid: sid,
                    Attributes: SE_GROUP_INTEGRITY as u32,
                },
            };
            let size = (size_of::<TOKEN_MANDATORY_LABEL>() + GetLengthSid(sid) as usize) as u32;
            windows::Win32::Security::SetTokenInformation(
                token,
                TokenIntegrityLevel,
                &tml as *const _ as *const core::ffi::c_void,
                size,
            )
            .map_err(|e| io::Error::from_raw_os_error(e.code().0))?;
            Ok(())
        }
    }
    fn wait_for_process(
        pi: &windows::Win32::System::Threading::PROCESS_INFORMATION,
    ) -> io::Result<ExitStatus> {
        unsafe {
            let wait = WaitForSingleObject(pi.hProcess, u32::MAX);
            if wait != WAIT_OBJECT_0 {
                return Err(io::Error::last_os_error());
            }
            let mut code = 0u32;
            GetExitCodeProcess(pi.hProcess, &mut code)
                .map_err(|e| io::Error::from_raw_os_error(e.code().0))?;
            if !pi.hThread.is_invalid() {
                let _ = CloseHandle(pi.hThread);
            }
            if !pi.hProcess.is_invalid() {
                let _ = CloseHandle(pi.hProcess);
            }
            Ok(ExitStatus::from_raw(code))
        }
    }
}

#[cfg(not(target_os = "windows"))]
mod imp {
    use super::*;
    pub(super) fn spawn_command_under_windows_low_il(
        _command: Vec<String>,
        _command_cwd: PathBuf,
        _policy: &SandboxPolicy,
        _policy_cwd: &Path,
        _stdio: StdioPolicy,
        _env: HashMap<String, String>,
    ) -> std::io::Result<ExitStatus> {
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "Windows sandbox is only available on Windows",
        ))
    }
}

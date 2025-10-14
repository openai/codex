use codex_protocol::protocol::SandboxPolicy;
use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;
use tracing::trace;

#[cfg(target_os = "windows")]
use clap::Parser;
#[cfg(target_os = "windows")]
use std::env;

#[cfg(target_os = "windows")]
#[derive(Debug, Parser)]
#[command(
    name = "codex-windows-sandbox",
    about = "Run a command inside a Windows restricted-token sandbox (no admin)"
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

#[cfg(target_os = "windows")]
#[derive(Debug, Clone)]
struct SandboxPolicyArg(SandboxPolicy);

#[cfg(target_os = "windows")]
impl SandboxPolicyArg {
    fn into_policy(self) -> SandboxPolicy {
        self.0
    }
}

#[cfg(target_os = "windows")]
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
pub enum StdioPolicy {
    Inherit,
}

#[cfg(target_os = "windows")]
pub fn run_main() -> ! {
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
    let mut env_map: HashMap<String, String> = env::vars().collect();

    if !policy.has_full_network_access() {
        apply_best_effort_network_block(&mut env_map);
    }
    ensure_non_interactive_pager(&mut env_map);

    let status = spawn_command_under_restricted_token_v2(
        command,
        current_dir,
        &policy,
        sandbox_policy_cwd.as_path(),
        StdioPolicy::Inherit,
        env_map,
    );

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

#[cfg(not(target_os = "windows"))]
pub fn run_main() -> ! {
    panic!("codex-windows-sandbox is only supported on Windows");
}

#[cfg(target_os = "windows")]
fn ensure_non_interactive_pager(env_map: &mut HashMap<String, String>) {
    env_map
        .entry("GIT_PAGER".into())
        .or_insert_with(|| "more.com".into());
    env_map
        .entry("PAGER".into())
        .or_insert_with(|| "more.com".into());
    env_map.entry("LESS".into()).or_insert_with(String::new);
}

#[cfg(target_os = "windows")]
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

#[cfg(target_os = "windows")]
type PlatformStdioPolicy = crate::sandbox::StdioPolicy;

#[cfg(target_os = "windows")]
mod imp {
    use super::PlatformStdioPolicy;
    use super::SandboxPolicy;
    use super::StdioPolicy;
    use super::trace;
    use std::collections::BTreeSet;
    use std::collections::HashMap;
    use std::env;
    use std::ffi::OsStr;
    use std::ffi::c_void;
    use std::io::ErrorKind;
    use std::io::{self};
    use std::mem::size_of;
    use std::os::windows::ffi::OsStrExt;
    use std::os::windows::process::ExitStatusExt;
    use std::path::Path;
    use std::path::PathBuf;
    use std::process::ExitStatus;
    use std::ptr::null_mut;
    use std::sync::Arc;

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

    use windows::Win32::Security::Authorization::ACCESS_MODE;
    use windows::Win32::Security::Authorization::CONTAINER_INHERIT_ACE;
    use windows::Win32::Security::Authorization::EXPLICIT_ACCESS_W;
    use windows::Win32::Security::Authorization::OBJECT_INHERIT_ACE;
    use windows::Win32::Security::Authorization::REVOKE_ACCESS;
    use windows::Win32::Security::Authorization::SE_FILE_OBJECT;
    use windows::Win32::Security::Authorization::SET_ACCESS;
    use windows::Win32::Security::Authorization::SetEntriesInAclW;
    use windows::Win32::Security::Authorization::SetNamedSecurityInfoW;
    use windows::Win32::Security::Authorization::TRUSTEE_IS_SID;
    use windows::Win32::Security::Authorization::TRUSTEE_IS_UNKNOWN;
    use windows::Win32::Security::Authorization::TRUSTEE_W;
    use windows::Win32::Security::CopySid;
    use windows::Win32::Security::CreateRestrictedToken;
    use windows::Win32::Security::GetLengthSid;
    use windows::Win32::Security::GetTokenInformation;
    use windows::Win32::Security::SE_GROUP_LOGON_ID;
    use windows::Win32::Security::SID_AND_ATTRIBUTES;
    use windows::Win32::Security::TOKEN_ACCESS_MASK;
    use windows::Win32::Security::TOKEN_ADJUST_DEFAULT;
    use windows::Win32::Security::TOKEN_ADJUST_PRIVILEGES;
    use windows::Win32::Security::TOKEN_ADJUST_SESSIONID;
    use windows::Win32::Security::TOKEN_ASSIGN_PRIMARY;
    use windows::Win32::Security::TOKEN_DUPLICATE;
    use windows::Win32::Security::TOKEN_GROUPS;
    use windows::Win32::Security::TOKEN_QUERY;

    use windows::Win32::Storage::FileSystem::FILE_GENERIC_EXECUTE;
    use windows::Win32::Storage::FileSystem::FILE_GENERIC_READ;
    use windows::Win32::Storage::FileSystem::FILE_GENERIC_WRITE;

    use windows::Win32::System::Console::GetStdHandle;
    use windows::Win32::System::Console::STD_ERROR_HANDLE;
    use windows::Win32::System::Console::STD_INPUT_HANDLE;
    use windows::Win32::System::Console::STD_OUTPUT_HANDLE;
    use windows::Win32::System::JobObjects::AssignProcessToJobObject;
    use windows::Win32::System::JobObjects::CreateJobObjectW;
    use windows::Win32::System::JobObjects::JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
    use windows::Win32::System::JobObjects::JOBOBJECT_EXTENDED_LIMIT_INFORMATION;
    use windows::Win32::System::JobObjects::SetInformationJobObject;

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

    pub(super) fn spawn_command_under_restricted_token_v2(
        command: Vec<String>,
        command_cwd: PathBuf,
        sandbox_policy: &SandboxPolicy,
        sandbox_policy_cwd: &Path,
        stdio_policy: StdioPolicy,
        env_map: HashMap<String, String>,
    ) -> io::Result<ExitStatus> {
        trace!("windows restricted token v2 command = {:?}", command);
        if command.is_empty() {
            return Err(io::Error::new(
                ErrorKind::InvalidInput,
                "command args are empty",
            ));
        }

        let restricted = create_restricted_token()?;
        let logon_sid = restricted.restricting_sid.clone();

        let mut _path_guards = configure_allowed_paths(
            sandbox_policy,
            sandbox_policy_cwd,
            &command_cwd,
            &logon_sid,
            &env_map,
        )?;

        let mut startup_info = STARTUPINFOW {
            cb: size_of::<STARTUPINFOW>() as u32,
            ..Default::default()
        };
        let platform_stdio = map_stdio_policy(stdio_policy);
        apply_stdio_policy(&mut startup_info, platform_stdio)?;

        let mut command_line = build_command_line(&command);
        let mut environment_block = build_environment_block(&env_map);
        let mut cwd = to_wide(&command_cwd);

        let mut process_info = ProcessInfoGuard::new();
        let creation_flags = PROCESS_CREATION_FLAGS(CREATE_UNICODE_ENVIRONMENT.0);
        let env_ptr: Option<*const c_void> = if environment_block.is_empty() {
            None
        } else {
            Some(environment_block.as_mut_ptr().cast::<c_void>() as *const c_void)
        };
        let current_dir = if cwd.is_empty() {
            PCWSTR::null()
        } else {
            PCWSTR(cwd.as_mut_ptr())
        };

        unsafe {
            CreateProcessAsUserW(
                Some(restricted.token.handle()),
                PCWSTR::null(),
                Some(PWSTR(command_line.as_mut_ptr())),
                None,
                None,
                true,
                creation_flags,
                env_ptr,
                current_dir,
                &startup_info,
                process_info.as_mut_ptr(),
            )
            .map_err(|e| io::Error::from_raw_os_error(e.code().0))?;
            if !startup_info.hStdInput.is_invalid() {
                let _ = CloseHandle(startup_info.hStdInput);
            }
            if !startup_info.hStdOutput.is_invalid() {
                let _ = CloseHandle(startup_info.hStdOutput);
            }
            if !startup_info.hStdError.is_invalid() {
                let _ = CloseHandle(startup_info.hStdError);
            }
        }

        let job = create_job_object()?;
        unsafe {
            AssignProcessToJobObject(job.handle(), process_info.info().hProcess)
                .map_err(|e| io::Error::from_raw_os_error(e.code().0))?;
        }

        wait_for_process(process_info.info())
    }

    fn configure_allowed_paths(
        policy: &SandboxPolicy,
        policy_cwd: &Path,
        command_cwd: &Path,
        sid: &Arc<TokenSid>,
        env_map: &HashMap<String, String>,
    ) -> io::Result<Vec<AceGuard>> {
        let mut guards = Vec::new();
        let mut requested_paths: BTreeSet<PathBuf> = BTreeSet::new();
        let mut allow_command_cwd = true;
        let mut allow_temp = true;

        match policy {
            SandboxPolicy::ReadOnly => {
                allow_command_cwd = false;
                allow_temp = false;
            }
            SandboxPolicy::DangerFullAccess => {
                if command_cwd.exists() {
                    requested_paths.insert(command_cwd.to_path_buf());
                }
            }
            SandboxPolicy::WorkspaceWrite { .. } => {
                for writable in policy.get_writable_roots_with_cwd(policy_cwd) {
                    requested_paths.insert(writable.root);
                }
            }
        }

        if allow_command_cwd
            && command_cwd.exists()
            && !requested_paths.iter().any(|p| command_cwd.starts_with(p))
        {
            requested_paths.insert(command_cwd.to_path_buf());
        }

        for path in requested_paths {
            if !path.exists() {
                trace!("skipping missing allow-list path {}", path.display());
                continue;
            }
            match add_allow_ace(&path, sid) {
                Ok(guard) => {
                    trace!("allowed write for logon SID on {}", path.display());
                    guards.push(guard);
                }
                Err(err) => {
                    trace!("failed to allow {}: {}", path.display(), err);
                }
            }
        }

        if allow_temp {
            let mut temp_paths: BTreeSet<PathBuf> = BTreeSet::new();
            if let Some(temp) = env_map
                .get("TEMP")
                .cloned()
                .or_else(|| env::var("TEMP").ok())
            {
                let path = PathBuf::from(temp);
                if path.exists() {
                    temp_paths.insert(path);
                }
            }
            if let Some(tmp) = env_map.get("TMP").cloned().or_else(|| env::var("TMP").ok()) {
                let path = PathBuf::from(tmp);
                if path.exists() {
                    temp_paths.insert(path);
                }
            }
            for temp in temp_paths {
                match add_allow_ace(&temp, sid) {
                    Ok(guard) => {
                        trace!("allowed TEMP/TMP {}", temp.display());
                        guards.push(guard);
                    }
                    Err(err) => {
                        trace!("failed to allow TEMP/TMP {}: {}", temp.display(), err);
                    }
                }
            }
        }

        Ok(guards)
    }

    struct RestrictedToken {
        token: HandleGuard,
        restricting_sid: Arc<TokenSid>,
    }

    fn create_restricted_token() -> io::Result<RestrictedToken> {
        unsafe {
            let desired = TOKEN_ACCESS_MASK(
                TOKEN_DUPLICATE.0
                    | TOKEN_QUERY.0
                    | TOKEN_ASSIGN_PRIMARY.0
                    | TOKEN_ADJUST_DEFAULT.0
                    | TOKEN_ADJUST_SESSIONID.0
                    | TOKEN_ADJUST_PRIVILEGES.0,
            );
            let mut process_token = HANDLE::default();
            OpenProcessToken(GetCurrentProcess(), desired, &mut process_token)
                .map_err(|e| io::Error::from_raw_os_error(e.code().0))?;
            let process_guard = HandleGuard::new(process_token);

            let logon_sid_bytes = query_logon_sid(process_guard.handle())?;
            let logon_sid = Arc::new(TokenSid::from_bytes(logon_sid_bytes));

            let mut new_token = HANDLE::default();
            let restrict_entries = [SID_AND_ATTRIBUTES {
                Sid: logon_sid.as_psid().0.cast(),
                Attributes: 0,
            }];

            CreateRestrictedToken(
                process_guard.handle(),
                windows::Win32::Security::DISABLE_MAX_PRIVILEGE
                    | windows::Win32::Security::LUA_TOKEN
                    | windows::Win32::Security::WRITE_RESTRICTED,
                None::<&[SID_AND_ATTRIBUTES]>,
                None::<&[windows::Win32::Security::LUID_AND_ATTRIBUTES]>,
                Some(&restrict_entries),
                &mut new_token,
            )
            .map_err(|e| io::Error::from_raw_os_error(e.code().0))?;

            Ok(RestrictedToken {
                token: HandleGuard::new(new_token),
                restricting_sid: logon_sid,
            })
        }
    }

    fn query_logon_sid(token: HANDLE) -> io::Result<Vec<u8>> {
        unsafe {
            let mut needed = 0u32;
            let _ = GetTokenInformation(token, TOKEN_GROUPS, None, 0, &mut needed);
            if needed == 0 {
                return Err(io::Error::last_os_error());
            }
            let mut buffer = vec![0u8; needed as usize];
            GetTokenInformation(
                token,
                TOKEN_GROUPS,
                Some(buffer.as_mut_ptr().cast()),
                needed,
                &mut needed,
            )
            .map_err(|e| io::Error::from_raw_os_error(e.code().0))?;
            let token_groups = &*(buffer.as_ptr() as *const TOKEN_GROUPS);
            let count = token_groups.GroupCount as usize;
            let groups_slice = std::slice::from_raw_parts(token_groups.Groups.as_ptr(), count);
            for group in groups_slice {
                if (group.Attributes & SE_GROUP_LOGON_ID.0) != 0 {
                    let sid = group.Sid;
                    let sid_len = GetLengthSid(sid) as usize;
                    let mut sid_bytes = vec![0u8; sid_len];
                    CopySid(sid_len as u32, PSID(sid_bytes.as_mut_ptr().cast()), sid)
                        .map_err(|e| io::Error::from_raw_os_error(e.code().0))?;
                    return Ok(sid_bytes);
                }
            }
            Err(io::Error::new(
                ErrorKind::NotFound,
                "logon SID not present on token",
            ))
        }
    }

    struct TokenSid {
        buffer: Arc<Vec<u8>>,
    }

    impl TokenSid {
        fn from_bytes(bytes: Vec<u8>) -> Self {
            Self {
                buffer: Arc::new(bytes),
            }
        }

        fn as_psid(&self) -> PSID {
            PSID(self.buffer.as_ptr() as *mut _)
        }
    }

    struct HandleGuard {
        handle: HANDLE,
    }

    impl HandleGuard {
        fn new(handle: HANDLE) -> Self {
            Self { handle }
        }

        fn handle(&self) -> HANDLE {
            self.handle
        }
    }

    impl Drop for HandleGuard {
        fn drop(&mut self) {
            unsafe {
                if !self.handle.is_invalid() {
                    let _ = CloseHandle(self.handle);
                }
            }
        }
    }

    struct ProcessInfoGuard {
        info: PROCESS_INFORMATION,
    }

    impl ProcessInfoGuard {
        fn new() -> Self {
            Self {
                info: PROCESS_INFORMATION::default(),
            }
        }

        fn as_mut_ptr(&mut self) -> *mut PROCESS_INFORMATION {
            &mut self.info
        }

        fn info(&self) -> &PROCESS_INFORMATION {
            &self.info
        }
    }

    impl Drop for ProcessInfoGuard {
        fn drop(&mut self) {
            unsafe {
                if !self.info.hProcess.is_invalid() {
                    let _ = CloseHandle(self.info.hProcess);
                }
                if !self.info.hThread.is_invalid() {
                    let _ = CloseHandle(self.info.hThread);
                }
            }
        }
    }

    struct JobGuard {
        handle: HANDLE,
    }

    impl JobGuard {
        fn new(handle: HANDLE) -> Self {
            Self { handle }
        }

        fn handle(&self) -> HANDLE {
            self.handle
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

    fn create_job_object() -> io::Result<JobGuard> {
        unsafe {
            let job = CreateJobObjectW(None, PCWSTR::null())
                .map_err(|e| io::Error::from_raw_os_error(e.code().0))?;
            let job_guard = JobGuard::new(job);
            let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
            limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            SetInformationJobObject(
                job_guard.handle(),
                windows::Win32::System::JobObjects::JobObjectExtendedLimitInformation,
                &limits as *const _ as *const c_void,
                size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            )
            .map_err(|e| io::Error::from_raw_os_error(e.code().0))?;
            Ok(job_guard)
        }
    }

    struct AceGuard {
        path: PathBuf,
        sid: Arc<TokenSid>,
        permissions: u32,
    }

    impl Drop for AceGuard {
        fn drop(&mut self) {
            let _ = apply_acl_change(&self.path, &self.sid, self.permissions, REVOKE_ACCESS);
        }
    }

    fn add_allow_ace(path: &Path, sid: &Arc<TokenSid>) -> io::Result<AceGuard> {
        apply_acl_change(
            path,
            sid,
            (FILE_GENERIC_READ | FILE_GENERIC_WRITE | FILE_GENERIC_EXECUTE).0,
            SET_ACCESS,
        )?;
        Ok(AceGuard {
            path: path.to_path_buf(),
            sid: sid.clone(),
            permissions: (FILE_GENERIC_READ | FILE_GENERIC_WRITE | FILE_GENERIC_EXECUTE).0,
        })
    }

    fn apply_acl_change(
        path: &Path,
        sid: &Arc<TokenSid>,
        permissions: u32,
        mode: ACCESS_MODE,
    ) -> io::Result<()> {
        if !path.exists() {
            trace!("skipping missing path {}", path.display());
            return Ok(());
        }
        let wide = to_wide(path.as_os_str());
        unsafe {
            let mut existing_dacl = null_mut();
            let mut security_descriptor = windows::Win32::Security::PSECURITY_DESCRIPTOR::default();
            let status = windows::Win32::Security::Authorization::GetNamedSecurityInfoW(
                PCWSTR(wide.as_ptr()),
                SE_FILE_OBJECT,
                windows::Win32::Security::DACL_SECURITY_INFORMATION,
                None,
                None,
                Some(&mut existing_dacl),
                None,
                &mut security_descriptor,
            );
            if status != WIN32_ERROR(0) {
                if !security_descriptor.is_invalid() {
                    let _ = LocalFree(Some(HLOCAL(security_descriptor.0)));
                }
                return Err(io::Error::from_raw_os_error(status.0 as i32));
            }

            let explicit = EXPLICIT_ACCESS_W {
                grfAccessPermissions: permissions,
                grfAccessMode: mode,
                grfInheritance: OBJECT_INHERIT_ACE | CONTAINER_INHERIT_ACE,
                Trustee: TRUSTEE_W {
                    pMultipleTrustee: std::ptr::null_mut(),
                    MultipleTrusteeOperation:
                        windows::Win32::Security::Authorization::MULTIPLE_TRUSTEE_OPERATION(0),
                    TrusteeForm: TRUSTEE_IS_SID,
                    TrusteeType: TRUSTEE_IS_UNKNOWN,
                    ptstrName: PWSTR(sid.as_psid().0.cast()),
                },
            };

            let entries = [explicit];
            let mut new_dacl = null_mut();
            let result = SetEntriesInAclW(Some(&entries), Some(existing_dacl), &mut new_dacl);
            if result != WIN32_ERROR(0) {
                if !new_dacl.is_null() {
                    let _ = LocalFree(Some(HLOCAL(new_dacl.cast())));
                }
                if !security_descriptor.is_invalid() {
                    let _ = LocalFree(Some(HLOCAL(security_descriptor.0)));
                }
                return Err(io::Error::from_raw_os_error(result.0 as i32));
            }

            let set_result = SetNamedSecurityInfoW(
                PCWSTR(wide.as_ptr()),
                SE_FILE_OBJECT,
                windows::Win32::Security::DACL_SECURITY_INFORMATION,
                None,
                None,
                Some(new_dacl),
                None,
            );
            if set_result != WIN32_ERROR(0) {
                if !new_dacl.is_null() {
                    let _ = LocalFree(Some(HLOCAL(new_dacl.cast())));
                }
                if !security_descriptor.is_invalid() {
                    let _ = LocalFree(Some(HLOCAL(security_descriptor.0)));
                }
                return Err(io::Error::from_raw_os_error(set_result.0 as i32));
            }
            if !new_dacl.is_null() {
                let _ = LocalFree(Some(HLOCAL(new_dacl.cast())));
            }
            if !security_descriptor.is_invalid() {
                let _ = LocalFree(Some(HLOCAL(security_descriptor.0)));
            }
        }
        Ok(())
    }

    fn map_stdio_policy(policy: StdioPolicy) -> PlatformStdioPolicy {
        match policy {
            StdioPolicy::Inherit => PlatformStdioPolicy::Inherit,
        }
    }

    fn apply_stdio_policy(
        startup_info: &mut STARTUPINFOW,
        policy: PlatformStdioPolicy,
    ) -> io::Result<()> {
        match policy {
            PlatformStdioPolicy::Inherit => unsafe {
                let stdin_handle = ensure_valid_handle(GetStdHandle(STD_INPUT_HANDLE)?)?;
                let stdout_handle = ensure_valid_handle(GetStdHandle(STD_OUTPUT_HANDLE)?)?;
                let stderr_handle = ensure_valid_handle(GetStdHandle(STD_ERROR_HANDLE)?)?;

                SetHandleInformation(stdin_handle, HANDLE_FLAG_INHERIT.0, HANDLE_FLAG_INHERIT)
                    .map_err(|e| io::Error::from_raw_os_error(e.code().0))?;
                SetHandleInformation(stdout_handle, HANDLE_FLAG_INHERIT.0, HANDLE_FLAG_INHERIT)
                    .map_err(|e| io::Error::from_raw_os_error(e.code().0))?;
                SetHandleInformation(stderr_handle, HANDLE_FLAG_INHERIT.0, HANDLE_FLAG_INHERIT)
                    .map_err(|e| io::Error::from_raw_os_error(e.code().0))?;

                startup_info.dwFlags |= STARTF_USESTDHANDLES;
                startup_info.hStdInput = stdin_handle;
                startup_info.hStdOutput = stdout_handle;
                startup_info.hStdError = stderr_handle;
                Ok(())
            },
        }
    }

    fn ensure_valid_handle(handle: HANDLE) -> io::Result<HANDLE> {
        if handle == INVALID_HANDLE_VALUE || handle.is_invalid() {
            Err(io::Error::last_os_error())
        } else {
            Ok(handle)
        }
    }

    fn to_wide<S: AsRef<OsStr>>(s: S) -> Vec<u16> {
        s.as_ref().encode_wide().chain(std::iter::once(0)).collect()
    }

    fn build_command_line(command: &[String]) -> Vec<u16> {
        let mut combined = String::new();
        for (idx, arg) in command.iter().enumerate() {
            if idx != 0 {
                combined.push(' ');
            }
            combined.push_str(&quote_windows_argument(arg));
        }
        let mut wide: Vec<u16> = combined.encode_utf16().collect();
        wide.push(0);
        wide
    }

    fn quote_windows_argument(arg: &str) -> String {
        if !needs_quotes(arg) {
            return arg.to_string();
        }
        let mut result = String::with_capacity(arg.len() + 2);
        result.push('"');
        let mut backslashes = 0;
        for ch in arg.chars() {
            match ch {
                '\\' => backslashes += 1,
                '"' => {
                    for _ in 0..(backslashes * 2 + 1) {
                        result.push('\\');
                    }
                    result.push('"');
                    backslashes = 0;
                }
                _ => {
                    if backslashes > 0 {
                        for _ in 0..(backslashes * 2) {
                            result.push('\\');
                        }
                        backslashes = 0;
                    }
                    result.push(ch);
                }
            }
        }
        if backslashes > 0 {
            for _ in 0..(backslashes * 2) {
                result.push('\\');
            }
        }
        result.push('"');
        result
    }

    fn needs_quotes(arg: &str) -> bool {
        arg.is_empty()
            || arg
                .chars()
                .any(|ch| matches!(ch, ' ' | '\t' | '\n' | '\r' | '\u{0b}' | '"'))
    }

    fn build_environment_block(env: &HashMap<String, String>) -> Vec<u16> {
        if env.is_empty() {
            return Vec::new();
        }
        let mut pairs: Vec<(String, String)> =
            env.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
        pairs.sort_by(|(a_key, _), (b_key, _)| {
            let a_upper = a_key.to_ascii_uppercase();
            let b_upper = b_key.to_ascii_uppercase();
            match a_upper.cmp(&b_upper) {
                std::cmp::Ordering::Equal => a_key.cmp(b_key),
                other => other,
            }
        });
        let mut block = Vec::new();
        for (key, value) in pairs {
            let entry = format!("{key}={value}");
            block.extend(entry.encode_utf16());
            block.push(0);
        }
        block.push(0);
        block
    }

    fn wait_for_process(info: &PROCESS_INFORMATION) -> io::Result<ExitStatus> {
        unsafe {
            let wait_result = WaitForSingleObject(info.hProcess, u32::MAX);
            if wait_result != WAIT_OBJECT_0 {
                return Err(io::Error::last_os_error());
            }
            let mut exit_code = 0u32;
            GetExitCodeProcess(info.hProcess, &mut exit_code)
                .map_err(|e| io::Error::from_raw_os_error(e.code().0))?;
            Ok(ExitStatus::from_raw(exit_code))
        }
    }
}

#[cfg(target_os = "windows")]
pub use imp::spawn_command_under_restricted_token_v2;

#[cfg(not(target_os = "windows"))]
#[allow(unused_variables)]
pub fn spawn_command_under_restricted_token_v2(
    _command: Vec<String>,
    _command_cwd: PathBuf,
    _sandbox_policy: &SandboxPolicy,
    _sandbox_policy_cwd: &Path,
    _stdio_policy: StdioPolicy,
    _env_map: HashMap<String, String>,
) -> std::io::Result<std::process::ExitStatus> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "Windows restricted-token sandboxing is only available on Windows",
    ))
}

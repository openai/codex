use clap::Parser;
use codex_protocol::protocol::SandboxPolicy;
use std::collections::HashMap;
use std::env;
use std::path::PathBuf;
use tracing::trace;

#[derive(Debug, Parser)]
#[command(
    name = "codex-windows-sandbox",
    about = "Run a command inside the Codex Windows restricted-token sandbox."
)]
struct WindowsSandboxCommand {
    /// Working directory that should be used when resolving relative sandbox policy paths.
    #[arg(long)]
    sandbox_policy_cwd: Option<PathBuf>,

    /// JSON-encoded SandboxPolicy definition.
    pub sandbox_policy: SandboxPolicy,

    /// Command and arguments to execute once sandboxing is configured.
    #[arg(trailing_var_arg = true, allow_hyphen_values = true, required = true)]
    pub command: Vec<String>,
}

#[derive(Debug, Clone, Copy)]
enum StdioPolicy {
    Inherit,
}

pub fn run_main() -> ! {
    let args = WindowsSandboxCommand::parse();
    let WindowsSandboxCommand {
        sandbox_policy_cwd,
        sandbox_policy,
        command,
    } = args;

    if command.is_empty() {
        panic!("No command specified to execute.");
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

    let status = spawn_command_under_restricted_token(
        command,
        current_dir,
        &sandbox_policy,
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

#[cfg(target_os = "windows")]
mod imp {
    use super::SandboxPolicy;
    use super::StdioPolicy;
    use super::trace;
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
    use windows::Win32::Foundation::INVALID_HANDLE_VALUE;
    use windows::Win32::Foundation::LocalFree;
    use windows::Win32::Foundation::PSID;
    use windows::Win32::Foundation::SetHandleInformation;
    use windows::Win32::Foundation::WAIT_OBJECT_0;
    use windows::Win32::Foundation::WIN32_ERROR;
    use windows::Win32::NetworkManagement::WindowsFirewall::INetFwPolicy2;
    use windows::Win32::NetworkManagement::WindowsFirewall::INetFwRule;
    use windows::Win32::NetworkManagement::WindowsFirewall::INetFwRule3;
    use windows::Win32::NetworkManagement::WindowsFirewall::INetFwRules;
    use windows::Win32::NetworkManagement::WindowsFirewall::NET_FW_ACTION_BLOCK;
    use windows::Win32::NetworkManagement::WindowsFirewall::NET_FW_PROFILE2_ALL;
    use windows::Win32::NetworkManagement::WindowsFirewall::NET_FW_RULE_DIRECTION_OUT;
    use windows::Win32::NetworkManagement::WindowsFirewall::NetFwPolicy2;
    use windows::Win32::NetworkManagement::WindowsFirewall::NetFwRule;
    use windows::Win32::Security::Authorization::ACCESS_MODE;
    use windows::Win32::Security::Authorization::ConvertSidToStringSidW;
    use windows::Win32::Security::Authorization::DENY_ACCESS;
    use windows::Win32::Security::Authorization::EXPLICIT_ACCESS_W;
    use windows::Win32::Security::Authorization::GetNamedSecurityInfoW;
    use windows::Win32::Security::Authorization::MULTIPLE_TRUSTEE_NO_MULTIPLE_TRUSTEE;
    use windows::Win32::Security::Authorization::OBJECT_INHERIT_ACE;
    use windows::Win32::Security::Authorization::REVOKE_ACCESS;
    use windows::Win32::Security::Authorization::SE_FILE_OBJECT;
    use windows::Win32::Security::Authorization::SET_ACCESS;
    use windows::Win32::Security::Authorization::SUB_CONTAINERS_AND_OBJECTS_INHERIT;
    use windows::Win32::Security::Authorization::SetEntriesInAclW;
    use windows::Win32::Security::Authorization::SetNamedSecurityInfoW;
    use windows::Win32::Security::Authorization::TRUSTEE_IS_SID;
    use windows::Win32::Security::Authorization::TRUSTEE_IS_UNKNOWN;
    use windows::Win32::Security::Authorization::TRUSTEE_W;
    use windows::Win32::Security::CreateRestrictedToken;
    use windows::Win32::Security::CreateWellKnownSid;
    use windows::Win32::Security::DACL_SECURITY_INFORMATION;
    use windows::Win32::Security::LUID_AND_ATTRIBUTES;
    use windows::Win32::Security::SE_GROUP_USE_FOR_DENY_ONLY;
    use windows::Win32::Security::SECURITY_MAX_SID_SIZE;
    use windows::Win32::Security::SID_AND_ATTRIBUTES;
    use windows::Win32::Security::TOKEN_ACCESS_MASK;
    use windows::Win32::Security::TOKEN_ADJUST_DEFAULT;
    use windows::Win32::Security::TOKEN_ADJUST_PRIVILEGES;
    use windows::Win32::Security::TOKEN_ADJUST_SESSIONID;
    use windows::Win32::Security::TOKEN_ASSIGN_PRIMARY;
    use windows::Win32::Security::TOKEN_DUPLICATE;
    use windows::Win32::Security::TOKEN_QUERY;
    use windows::Win32::Security::WELL_KNOWN_SID_TYPE;
    use windows::Win32::Storage::FileSystem::FILE_GENERIC_EXECUTE;
    use windows::Win32::Storage::FileSystem::FILE_GENERIC_READ;
    use windows::Win32::Storage::FileSystem::FILE_GENERIC_WRITE;
    use windows::Win32::System::Com::CLSCTX_INPROC_SERVER;
    use windows::Win32::System::Com::COINIT_MULTITHREADED;
    use windows::Win32::System::Com::CoCreateInstance;
    use windows::Win32::System::Com::CoInitializeEx;
    use windows::Win32::System::Com::CoInitializeSecurity;
    use windows::Win32::System::Com::CoUninitialize;
    use windows::Win32::System::Com::EOAC_NONE;
    use windows::Win32::System::Com::RPC_C_AUTHN_LEVEL_DEFAULT;
    use windows::Win32::System::Com::RPC_C_AUTHN_WINNT;
    use windows::Win32::System::Com::RPC_C_AUTHZ_NONE;
    use windows::Win32::System::Com::RPC_C_IMP_LEVEL_IMPERSONATE;
    use windows::Win32::System::Com::RPC_E_TOO_LATE;
    use windows::Win32::System::Com::VARIANT_TRUE;
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
    use windows::core::BSTR;
    use windows::core::Interface;
    use windows::core::PCWSTR;
    use windows::core::PWSTR;

    pub(super) fn spawn_command_under_restricted_token(
        command: Vec<String>,
        command_cwd: PathBuf,
        sandbox_policy: &SandboxPolicy,
        sandbox_policy_cwd: &Path,
        stdio_policy: StdioPolicy,
        env: HashMap<String, String>,
    ) -> io::Result<ExitStatus> {
        trace!("windows restricted token sandbox command = {:?}", command);

        if command.is_empty() {
            return Err(io::Error::new(
                ErrorKind::InvalidInput,
                "command args are empty",
            ));
        }

        let restricted = create_restricted_token()?;
        let restricted_sid = restricted.restricted_sid.clone();

        let mut _acl_guards = configure_writable_paths(
            sandbox_policy,
            sandbox_policy_cwd,
            &command_cwd,
            &restricted_sid,
        )?;

        let mut temp_guards = configure_temp_directories(&restricted_sid)?;
        _acl_guards.append(&mut temp_guards);

        let _firewall_guard = configure_firewall(sandbox_policy, &restricted_sid)?;

        let mut startup_info = STARTUPINFOW {
            cb: size_of::<STARTUPINFOW>() as u32,
            ..Default::default()
        };
        apply_stdio_policy(&mut startup_info, stdio_policy)?;

        let mut command_line = build_command_line(&command);
        let mut environment_block = build_environment_block(&env);
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

    fn apply_stdio_policy(startup_info: &mut STARTUPINFOW, policy: StdioPolicy) -> io::Result<()> {
        match policy {
            StdioPolicy::Inherit => unsafe {
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

    fn to_wide<S: AsRef<OsStr>>(s: S) -> Vec<u16> {
        s.as_ref().encode_wide().chain(std::iter::once(0)).collect()
    }

    struct RestrictedToken {
        token: HandleGuard,
        restricted_sid: Arc<WellKnownSid>,
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

            let disable_sid_types = [
                WELL_KNOWN_SID_TYPE::WinBuiltinAdministratorsSid,
                WELL_KNOWN_SID_TYPE::WinLocalSystemSid,
                WELL_KNOWN_SID_TYPE::WinLocalServiceSid,
                WELL_KNOWN_SID_TYPE::WinNetworkServiceSid,
                WELL_KNOWN_SID_TYPE::WinBuiltinPowerUsersSid,
                WELL_KNOWN_SID_TYPE::WinBuiltinBackupOperatorsSid,
                WELL_KNOWN_SID_TYPE::WinBuiltinReplicatorSid,
            ];
            let mut disable_storage = Vec::new();
            for sid_type in disable_sid_types {
                disable_storage.push(WellKnownSid::new(sid_type)?);
            }
            let disable_entries: Vec<SID_AND_ATTRIBUTES> = disable_storage
                .iter()
                .map(|sid| SID_AND_ATTRIBUTES {
                    Sid: sid.as_psid(),
                    Attributes: SE_GROUP_USE_FOR_DENY_ONLY,
                })
                .collect();

            let restricted_sid = Arc::new(WellKnownSid::new(
                WELL_KNOWN_SID_TYPE::WinRestrictedCodeSid,
            )?);
            let restricted_entries = [SID_AND_ATTRIBUTES {
                Sid: restricted_sid.as_psid(),
                Attributes: 0,
            }];

            let mut new_token = HANDLE::default();
            CreateRestrictedToken(
                process_guard.handle(),
                windows::Win32::Security::DISABLE_MAX_PRIVILEGE
                    | windows::Win32::Security::LUA_TOKEN
                    | windows::Win32::Security::WRITE_RESTRICTED,
                disable_entries.len() as u32,
                if disable_entries.is_empty() {
                    std::ptr::null()
                } else {
                    disable_entries.as_ptr()
                },
                0,
                std::ptr::null::<LUID_AND_ATTRIBUTES>(),
                restricted_entries.len() as u32,
                restricted_entries.as_ptr(),
                &mut new_token,
            )
            .map_err(|e| io::Error::from_raw_os_error(e.code().0))?;

            Ok(RestrictedToken {
                token: HandleGuard::new(new_token),
                restricted_sid,
            })
        }
    }

    #[derive(Clone)]
    struct WellKnownSid {
        buffer: Arc<Vec<u8>>,
    }

    impl WellKnownSid {
        fn new(kind: WELL_KNOWN_SID_TYPE) -> io::Result<Self> {
            unsafe {
                let mut buffer = vec![0u8; SECURITY_MAX_SID_SIZE as usize];
                let mut size = buffer.len() as u32;
                CreateWellKnownSid(kind, None, buffer.as_mut_ptr().cast(), &mut size)
                    .map_err(|e| io::Error::from_raw_os_error(e.code().0))?;
                buffer.truncate(size as usize);
                Ok(Self {
                    buffer: Arc::new(buffer),
                })
            }
        }

        fn as_psid(&self) -> PSID {
            PSID(self.buffer.as_ptr().cast())
        }

        fn to_string(&self) -> io::Result<String> {
            unsafe {
                let mut sid_string = PWSTR::null();
                ConvertSidToStringSidW(self.as_psid(), &mut sid_string)
                    .map_err(|e| io::Error::from_raw_os_error(e.code().0))?;
                let s = if sid_string.is_null() {
                    String::new()
                } else {
                    let mut len = 0;
                    while *sid_string.0.add(len) != 0 {
                        len += 1;
                    }
                    let slice = std::slice::from_raw_parts(sid_string.0, len);
                    String::from_utf16_lossy(slice)
                };
                if !sid_string.is_null() {
                    let _ = LocalFree(sid_string.0.cast());
                }
                Ok(s)
            }
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

    fn configure_writable_paths(
        policy: &SandboxPolicy,
        sandbox_policy_cwd: &Path,
        command_cwd: &Path,
        restricted_sid: &Arc<WellKnownSid>,
    ) -> io::Result<Vec<AclGuard>> {
        let mut guards = Vec::new();
        match policy {
            SandboxPolicy::DangerFullAccess => {
                guards.push(allow_path(command_cwd, restricted_sid, true)?);
            }
            SandboxPolicy::ReadOnly => {
                guards.push(allow_path(sandbox_policy_cwd, restricted_sid, false)?);
            }
            SandboxPolicy::WorkspaceWrite { .. } => {
                let roots = policy.get_writable_roots_with_cwd(sandbox_policy_cwd);
                for writable in roots {
                    guards.push(allow_path(&writable.root, restricted_sid, true)?);
                    for sub in writable.read_only_subpaths {
                        guards.push(deny_write(&sub, restricted_sid)?);
                    }
                }
                if !command_cwd.starts_with(sandbox_policy_cwd) {
                    guards.push(allow_path(command_cwd, restricted_sid, true)?);
                }
            }
        }
        Ok(guards)
    }

    fn configure_temp_directories(restricted_sid: &Arc<WellKnownSid>) -> io::Result<Vec<AclGuard>> {
        let mut guards = Vec::new();
        if let Some(temp_os) = env::var_os("TEMP") {
            let temp_path = PathBuf::from(&temp_os);
            if temp_path.exists() {
                guards.push(allow_path(&temp_path, restricted_sid, true)?);
            }
        }
        if let Some(tmp_os) = env::var_os("TMP") {
            let tmp_path = PathBuf::from(&tmp_os);
            if tmp_path.exists() {
                let already_included = env::var_os("TEMP")
                    .map(PathBuf::from)
                    .is_some_and(|p| p == tmp_path);
                if !already_included {
                    guards.push(allow_path(&tmp_path, restricted_sid, true)?);
                }
            }
        }
        Ok(guards)
    }

    struct AclGuard {
        path: PathBuf,
        sid: Arc<WellKnownSid>,
        change: AclChange,
    }

    impl Drop for AclGuard {
        fn drop(&mut self) {
            let _ = apply_acl_change(&self.path, &self.sid, &self.change, REVOKE_ACCESS);
        }
    }

    #[derive(Clone)]
    enum AclChange {
        Allow { write: bool },
        DenyWrite,
    }

    impl AclChange {
        fn mode(&self) -> ACCESS_MODE {
            match self {
                AclChange::Allow { .. } => SET_ACCESS,
                AclChange::DenyWrite => DENY_ACCESS,
            }
        }

        fn permissions(&self) -> u32 {
            match self {
                AclChange::Allow { write } => {
                    if *write {
                        (FILE_GENERIC_READ | FILE_GENERIC_WRITE | FILE_GENERIC_EXECUTE).0
                    } else {
                        (FILE_GENERIC_READ | FILE_GENERIC_EXECUTE).0
                    }
                }
                AclChange::DenyWrite => (FILE_GENERIC_WRITE | FILE_GENERIC_EXECUTE).0,
            }
        }
    }

    fn allow_path(path: &Path, sid: &Arc<WellKnownSid>, write: bool) -> io::Result<AclGuard> {
        let change = AclChange::Allow { write };
        apply_acl_change(path, sid, &change, change.mode())?;
        Ok(AclGuard {
            path: path.to_path_buf(),
            sid: sid.clone(),
            change,
        })
    }

    fn deny_write(path: &Path, sid: &Arc<WellKnownSid>) -> io::Result<AclGuard> {
        let change = AclChange::DenyWrite;
        apply_acl_change(path, sid, &change, change.mode())?;
        Ok(AclGuard {
            path: path.to_path_buf(),
            sid: sid.clone(),
            change,
        })
    }

    fn apply_acl_change(
        path: &Path,
        sid: &Arc<WellKnownSid>,
        change: &AclChange,
        mode: ACCESS_MODE,
    ) -> io::Result<()> {
        if !path.exists() {
            return Ok(());
        }
        let wide = to_wide(path.as_os_str());
        unsafe {
            let mut existing_dacl = null_mut();
            let mut security_descriptor = windows::Win32::Security::PSECURITY_DESCRIPTOR::default();
            let status = GetNamedSecurityInfoW(
                PCWSTR(wide.as_ptr()),
                SE_FILE_OBJECT,
                DACL_SECURITY_INFORMATION,
                None,
                None,
                Some(&mut existing_dacl),
                None,
                &mut security_descriptor,
            );
            if status != WIN32_ERROR(0) {
                if !security_descriptor.is_invalid() {
                    let _ = LocalFree(Some(windows::Win32::Foundation::HLOCAL(
                        security_descriptor.0,
                    )));
                }
                return Err(io::Error::from_raw_os_error(status.0 as i32));
            }

            let permissions = change.permissions();
            let mut explicit = EXPLICIT_ACCESS_W {
                grfAccessPermissions: permissions,
                grfAccessMode: mode,
                grfInheritance: SUB_CONTAINERS_AND_OBJECTS_INHERIT | OBJECT_INHERIT_ACE,
                Trustee: TRUSTEE_W {
                    pMultipleTrustee: None,
                    MultipleTrusteeOperation: MULTIPLE_TRUSTEE_NO_MULTIPLE_TRUSTEE,
                    TrusteeForm: TRUSTEE_IS_SID,
                    TrusteeType: TRUSTEE_IS_UNKNOWN,
                    ptstrName: PWSTR(sid.as_psid().0.cast()),
                },
            };

            let entries = [explicit];
            let mut new_dacl = null_mut();
            let add_result = SetEntriesInAclW(Some(&entries), Some(existing_dacl), &mut new_dacl);
            if add_result != WIN32_ERROR(0) {
                if !new_dacl.is_null() {
                    let _ = LocalFree(Some(windows::Win32::Foundation::HLOCAL(new_dacl.cast())));
                }
                if !security_descriptor.is_invalid() {
                    let _ = LocalFree(Some(windows::Win32::Foundation::HLOCAL(
                        security_descriptor.0,
                    )));
                }
                return Err(io::Error::from_raw_os_error(add_result.0 as i32));
            }

            let set_result = SetNamedSecurityInfoW(
                PCWSTR(wide.as_ptr()),
                SE_FILE_OBJECT,
                DACL_SECURITY_INFORMATION,
                None,
                None,
                Some(new_dacl),
                None,
            );
            if set_result != WIN32_ERROR(0) {
                if !new_dacl.is_null() {
                    let _ = LocalFree(Some(windows::Win32::Foundation::HLOCAL(new_dacl.cast())));
                }
                if !security_descriptor.is_invalid() {
                    let _ = LocalFree(Some(windows::Win32::Foundation::HLOCAL(
                        security_descriptor.0,
                    )));
                }
                return Err(io::Error::from_raw_os_error(set_result.0 as i32));
            }
            if !new_dacl.is_null() {
                let _ = LocalFree(Some(windows::Win32::Foundation::HLOCAL(new_dacl.cast())));
            }
            if !security_descriptor.is_invalid() {
                let _ = LocalFree(Some(windows::Win32::Foundation::HLOCAL(
                    security_descriptor.0,
                )));
            }
        }
        Ok(())
    }

    struct FirewallGuard {
        rules: INetFwRules,
        name: String,
        _com: ComGuard,
    }

    impl Drop for FirewallGuard {
        fn drop(&mut self) {
            let wide_name: Vec<u16> = self.name.encode_utf16().chain(std::iter::once(0)).collect();
            let _ = unsafe { self.rules.Remove(PWSTR(wide_name.as_ptr() as *mut _)) };
        }
    }

    fn configure_firewall(
        policy: &SandboxPolicy,
        restricted_sid: &Arc<WellKnownSid>,
    ) -> io::Result<Option<FirewallGuard>> {
        if policy.has_full_network_access() {
            return Ok(None);
        }
        unsafe {
            let com = ComGuard::new()?;
            let policy: INetFwPolicy2 =
                CoCreateInstance(&NetFwPolicy2, None, CLSCTX_INPROC_SERVER)?;
            let rules = policy.Rules()?;
            let rule: INetFwRule3 = CoCreateInstance(&NetFwRule, None, CLSCTX_INPROC_SERVER)?;

            let sid_string = restricted_sid.to_string()?;
            let rule_name = format!("codex-restricted-token-block-{sid_string}");
            rule.SetName(&BSTR::from(rule_name.as_str()))?;
            rule.SetDescription(&BSTR::from("Codex sandbox network isolation"))?;
            rule.SetAction(NET_FW_ACTION_BLOCK)?;
            rule.SetDirection(NET_FW_RULE_DIRECTION_OUT)?;
            rule.SetEnabled(VARIANT_TRUE)?;
            rule.SetProfiles(NET_FW_PROFILE2_ALL.0 as i32)?;
            rule.SetInterfaceTypes(&BSTR::from("All"))?;
            rule.SetLocalUserAuthorizedList(&BSTR::from(sid_string.as_str()))?;

            let base_rule: INetFwRule = rule.cast()?;
            rules.Add(base_rule)?;
            Ok(Some(FirewallGuard {
                rules,
                name: rule_name,
                _com: com,
            }))
        }
    }

    struct ComGuard;

    impl ComGuard {
        fn new() -> io::Result<Self> {
            unsafe {
                CoInitializeEx(None, COINIT_MULTITHREADED)
                    .map_err(|e| io::Error::from_raw_os_error(e.code().0))?;
                match CoInitializeSecurity(
                    None,
                    -1,
                    None,
                    None,
                    RPC_C_AUTHN_LEVEL_DEFAULT,
                    RPC_C_IMP_LEVEL_IMPERSONATE,
                    None,
                    EOAC_NONE,
                    None,
                ) {
                    Ok(()) => {}
                    Err(err) if err.code() == RPC_E_TOO_LATE => {}
                    Err(err) => return Err(io::Error::from_raw_os_error(err.code().0)),
                }
            }
            Ok(Self)
        }
    }

    impl Drop for ComGuard {
        fn drop(&mut self) {
            unsafe {
                CoUninitialize();
            }
        }
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
                    result.extend(std::iter::repeat_n('\\', backslashes * 2 + 1));
                    result.push('"');
                    backslashes = 0;
                }
                _ => {
                    if backslashes > 0 {
                        result.extend(std::iter::repeat_n('\\', backslashes * 2));
                        backslashes = 0;
                    }
                    result.push(ch);
                }
            }
        }
        if backslashes > 0 {
            result.extend(std::iter::repeat_n('\\', backslashes * 2));
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

    unsafe fn ensure_valid_handle(handle: HANDLE) -> io::Result<HANDLE> {
        if handle == INVALID_HANDLE_VALUE || handle.is_invalid() {
            Err(io::Error::last_os_error())
        } else {
            Ok(handle)
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use assert_cmd::Command;
        use codex_protocol::protocol::SandboxPolicy;
        use tempfile::TempDir;

        fn sandbox_bin() -> Command {
            Command::cargo_bin("codex-windows-sandbox").expect("binary should exist")
        }

        fn policy_json(policy: &SandboxPolicy) -> String {
            serde_json::to_string(policy).expect("serialize policy")
        }

        #[test]
        fn cmd_can_create_file_in_workspace() {
            let temp = TempDir::new().expect("tempdir");
            let file_path = temp.path().join("allowed.txt");
            let mut cmd = sandbox_bin();
            cmd.current_dir(temp.path());
            cmd.arg(policy_json(&SandboxPolicy::new_workspace_write_policy()));
            cmd.arg("cmd");
            cmd.arg("/C");
            cmd.arg(format!("echo hi > {}", file_path.display()));
            cmd.assert().success();
            assert!(file_path.exists(), "file should be created");
        }

        #[test]
        fn cmd_cannot_write_outside_workspace() {
            let temp = TempDir::new().expect("tempdir");
            let outside = temp.path().parent().unwrap().join("blocked.txt");
            if outside.exists() {
                std::fs::remove_file(&outside).unwrap();
            }
            let mut cmd = sandbox_bin();
            cmd.current_dir(temp.path());
            cmd.arg(policy_json(&SandboxPolicy::new_workspace_write_policy()));
            cmd.arg("cmd");
            cmd.arg("/C");
            cmd.arg(format!("echo hi > {}", outside.display()));
            cmd.assert().failure();
            assert!(!outside.exists(), "outside file must not be created");
        }

        #[test]
        fn powershell_runs_in_sandbox() {
            let mut cmd = sandbox_bin();
            cmd.arg(policy_json(&SandboxPolicy::new_workspace_write_policy()));
            cmd.arg("powershell");
            cmd.arg("-NoLogo");
            cmd.arg("-NoProfile");
            cmd.arg("-Command");
            cmd.arg("Write-Output 'sandbox'");
            cmd.assert().success();
        }
    }
}

#[cfg(target_os = "windows")]
use imp::spawn_command_under_restricted_token;

#[cfg(not(target_os = "windows"))]
fn spawn_command_under_restricted_token(
    _command: Vec<String>,
    _command_cwd: PathBuf,
    _sandbox_policy: &SandboxPolicy,
    _sandbox_policy_cwd: &Path,
    _stdio_policy: StdioPolicy,
    _env: HashMap<String, String>,
) -> std::io::Result<std::process::ExitStatus> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "Windows restricted-token sandboxing is only available on Windows",
    ))
}

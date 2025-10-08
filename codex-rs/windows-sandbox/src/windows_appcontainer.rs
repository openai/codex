use clap::Parser;
use codex_protocol::protocol::SandboxPolicy;
use std::collections::HashMap;
use std::env;
use std::path::PathBuf;
use tracing::trace;

#[derive(Debug, Parser)]
#[command(
    name = "codex-windows-sandbox",
    about = "Run a command inside a Windows AppContainer sandbox."
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

    let status = spawn_command_under_windows_appcontainer(
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
    use std::ptr::null;
    use std::ptr::null_mut;
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::Foundation::ERROR_ALREADY_EXISTS;
    use windows::Win32::Foundation::GetLastError;
    use windows::Win32::Foundation::HANDLE;
    use windows::Win32::Foundation::HLOCAL;
    use windows::Win32::Foundation::LocalFree;
    use windows::Win32::Foundation::NTSTATUS;
    use windows::Win32::Foundation::WAIT_OBJECT_0;
    use windows::Win32::Foundation::WIN32_ERROR;
    use windows::Win32::Security::Authorization::ConvertStringSidToSidW;
    use windows::Win32::Security::Authorization::EXPLICIT_ACCESS_W;
    use windows::Win32::Security::Authorization::GetNamedSecurityInfoW;
    use windows::Win32::Security::Authorization::SE_FILE_OBJECT;
    use windows::Win32::Security::Authorization::SET_ACCESS;
    use windows::Win32::Security::Authorization::SetEntriesInAclW;
    use windows::Win32::Security::Authorization::SetNamedSecurityInfoW;
    use windows::Win32::Security::Authorization::TRUSTEE_IS_SID;
    use windows::Win32::Security::Authorization::TRUSTEE_IS_UNKNOWN;
    use windows::Win32::Security::Authorization::TRUSTEE_W;
    use windows::Win32::Security::DACL_SECURITY_INFORMATION;
    use windows::Win32::Security::FreeSid;
    use windows::Win32::Security::Isolation::CreateAppContainerProfile;
    use windows::Win32::Security::Isolation::DeriveAppContainerSidFromAppContainerName;
    use windows::Win32::Security::OBJECT_INHERIT_ACE;
    use windows::Win32::Security::PSECURITY_DESCRIPTOR;
    use windows::Win32::Security::PSID;
    use windows::Win32::Security::SID_AND_ATTRIBUTES;
    use windows::Win32::Security::SUB_CONTAINERS_AND_OBJECTS_INHERIT;
    use windows::Win32::Security::TOKEN_ACCESS_MASK;
    use windows::Win32::Security::TOKEN_ADJUST_DEFAULT;
    use windows::Win32::Security::TOKEN_ADJUST_SESSIONID;
    use windows::Win32::Security::TOKEN_ASSIGN_PRIMARY;
    use windows::Win32::Security::TOKEN_DUPLICATE;
    use windows::Win32::Security::TOKEN_QUERY;
    use windows::Win32::Storage::FileSystem::FILE_GENERIC_EXECUTE;
    use windows::Win32::Storage::FileSystem::FILE_GENERIC_READ;
    use windows::Win32::Storage::FileSystem::FILE_GENERIC_WRITE;
    use windows::Win32::System::Threading::CREATE_UNICODE_ENVIRONMENT;
    use windows::Win32::System::Threading::CreateProcessAsUserW;
    use windows::Win32::System::Threading::GetCurrentProcess;
    use windows::Win32::System::Threading::GetExitCodeProcess;
    use windows::Win32::System::Threading::OpenProcessToken;
    use windows::Win32::System::Threading::PROCESS_CREATION_FLAGS;
    use windows::Win32::System::Threading::PROCESS_INFORMATION;
    use windows::Win32::System::Threading::STARTUPINFOW;
    use windows::Win32::System::Threading::WaitForSingleObject;
    use windows::core::PCWSTR;
    use windows::core::PWSTR;

    const WINDOWS_APPCONTAINER_PROFILE_NAME: &str = "codex_appcontainer";
    const WINDOWS_APPCONTAINER_PROFILE_DESC: &str = "Codex Windows AppContainer profile";
    const INTERNET_CLIENT_SID: &str = "S-1-15-3-1";
    const PRIVATE_NETWORK_CLIENT_SID: &str = "S-1-15-3-3";

    pub(super) fn spawn_command_under_windows_appcontainer(
        command: Vec<String>,
        command_cwd: PathBuf,
        sandbox_policy: &SandboxPolicy,
        sandbox_policy_cwd: &Path,
        stdio_policy: StdioPolicy,
        env: HashMap<String, String>,
    ) -> io::Result<ExitStatus> {
        trace!("windows appcontainer sandbox command = {:?}", command);

        if command.is_empty() {
            return Err(io::Error::new(
                ErrorKind::InvalidInput,
                "command args are empty",
            ));
        }

        ensure_appcontainer_profile()?;
        let sid = derive_appcontainer_sid()?;
        let capability_sids = build_capabilities(sandbox_policy)?;

        configure_writable_roots(sandbox_policy, sandbox_policy_cwd, sid.sid())?;
        configure_writable_roots_for_command_cwd(&command_cwd, sid.sid())?;

        // Create an AppContainer (low-box) primary token via NtCreateLowBoxToken.
        let token = create_lowbox_token(sid.sid(), &capability_sids)?;

        // Basic STARTUPINFOW (no console handle tweaking, so no Console feature needed).
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
                Some(token.handle()),
                PCWSTR::null(),
                Some(PWSTR(command_line.as_mut_ptr())),
                None,
                None,
                false, // do not inherit handles
                creation_flags,
                env_ptr,
                current_dir,
                &startup_info,
                process_info.as_mut_ptr(),
            )
            .map_err(|e| io::Error::from_raw_os_error(e.code().0))?;
        }

        wait_for_process(process_info.info())
    }

    // No-op: keeps features minimal (no Win32_System_Console).
    fn apply_stdio_policy(
        _startup_info: &mut STARTUPINFOW,
        _policy: StdioPolicy,
    ) -> io::Result<()> {
        Ok(())
    }

    fn to_wide<S: AsRef<OsStr>>(s: S) -> Vec<u16> {
        s.as_ref().encode_wide().chain(std::iter::once(0)).collect()
    }

    fn ensure_appcontainer_profile() -> io::Result<()> {
        unsafe {
            let name = to_wide(WINDOWS_APPCONTAINER_PROFILE_NAME);
            let desc = to_wide(WINDOWS_APPCONTAINER_PROFILE_DESC);
            match CreateAppContainerProfile(
                PCWSTR(name.as_ptr()),
                PCWSTR(name.as_ptr()),
                PCWSTR(desc.as_ptr()),
                None,
            ) {
                Ok(profile_sid) => {
                    if !profile_sid.is_invalid() {
                        FreeSid(profile_sid);
                    }
                }
                Err(error) => {
                    let already_exists = ERROR_ALREADY_EXISTS;
                    if GetLastError() != already_exists {
                        return Err(io::Error::from_raw_os_error(error.code().0));
                    }
                }
            }
        }
        Ok(())
    }

    struct SidHandle {
        ptr: PSID,
    }

    impl SidHandle {
        fn sid(&self) -> PSID {
            self.ptr
        }
    }

    impl Drop for SidHandle {
        fn drop(&mut self) {
            unsafe {
                if !self.ptr.is_invalid() {
                    FreeSid(self.ptr);
                }
            }
        }
    }

    fn derive_appcontainer_sid() -> io::Result<SidHandle> {
        unsafe {
            let name = to_wide(WINDOWS_APPCONTAINER_PROFILE_NAME);
            let sid = DeriveAppContainerSidFromAppContainerName(PCWSTR(name.as_ptr()))
                .map_err(|e| io::Error::from_raw_os_error(e.code().0))?;
            Ok(SidHandle { ptr: sid })
        }
    }

    struct CapabilitySid {
        sid: PSID,
    }

    impl CapabilitySid {
        fn new_from_string(value: &str) -> io::Result<Self> {
            unsafe {
                let mut sid_ptr = PSID::default();
                let wide = to_wide(value);
                ConvertStringSidToSidW(PCWSTR(wide.as_ptr()), &mut sid_ptr)
                    .map_err(|e| io::Error::from_raw_os_error(e.code().0))?;
                Ok(Self { sid: sid_ptr })
            }
        }

        fn sid_and_attributes(&self) -> SID_AND_ATTRIBUTES {
            SID_AND_ATTRIBUTES {
                Sid: self.sid,
                Attributes: 0,
            }
        }
    }

    impl Drop for CapabilitySid {
        fn drop(&mut self) {
            unsafe {
                if !self.sid.is_invalid() {
                    let _ = LocalFree(Some(HLOCAL(self.sid.0)));
                }
            }
        }
    }

    fn build_capabilities(policy: &SandboxPolicy) -> io::Result<Vec<CapabilitySid>> {
        if policy.has_full_network_access() {
            Ok(vec![
                CapabilitySid::new_from_string(INTERNET_CLIENT_SID)?,
                CapabilitySid::new_from_string(PRIVATE_NETWORK_CLIENT_SID)?,
            ])
        } else {
            Ok(Vec::new())
        }
    }

    fn configure_writable_roots(
        policy: &SandboxPolicy,
        sandbox_policy_cwd: &Path,
        sid: PSID,
    ) -> io::Result<()> {
        match policy {
            SandboxPolicy::DangerFullAccess => Ok(()),
            SandboxPolicy::ReadOnly => grant_path_with_flags(sandbox_policy_cwd, sid, false),
            SandboxPolicy::WorkspaceWrite { .. } => {
                let roots = policy.get_writable_roots_with_cwd(sandbox_policy_cwd);
                for writable in roots {
                    grant_path_with_flags(&writable.root, sid, true)?;
                    for ro in writable.read_only_subpaths {
                        grant_path_with_flags(&ro, sid, false)?;
                    }
                }
                Ok(())
            }
        }
    }

    fn configure_writable_roots_for_command_cwd(command_cwd: &Path, sid: PSID) -> io::Result<()> {
        grant_path_with_flags(command_cwd, sid, true)
    }

    fn grant_path_with_flags(path: &Path, sid: PSID, write: bool) -> io::Result<()> {
        if !path.exists() {
            return Ok(());
        }

        let wide = to_wide(path.as_os_str());
        unsafe {
            let mut existing_dacl: *mut windows::Win32::Security::ACL = null_mut();
            let mut security_descriptor = PSECURITY_DESCRIPTOR::default();
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
                    let _ = LocalFree(Some(HLOCAL(security_descriptor.0)));
                }
                return Err(io::Error::from_raw_os_error(status.0 as i32));
            }

            let permissions = if write {
                (FILE_GENERIC_READ | FILE_GENERIC_WRITE | FILE_GENERIC_EXECUTE).0
            } else {
                (FILE_GENERIC_READ | FILE_GENERIC_EXECUTE).0
            };
            let explicit = EXPLICIT_ACCESS_W {
                grfAccessPermissions: permissions,
                grfAccessMode: SET_ACCESS,
                grfInheritance: SUB_CONTAINERS_AND_OBJECTS_INHERIT | OBJECT_INHERIT_ACE,
                Trustee: TRUSTEE_W {
                    TrusteeForm: TRUSTEE_IS_SID,
                    TrusteeType: TRUSTEE_IS_UNKNOWN,
                    ptstrName: PWSTR(sid.0.cast()),
                    ..Default::default()
                },
            };

            let explicit_entries = [explicit];
            let mut new_dacl: *mut windows::Win32::Security::ACL = null_mut();
            let add_result =
                SetEntriesInAclW(Some(&explicit_entries), Some(existing_dacl), &mut new_dacl);
            if add_result != WIN32_ERROR(0) {
                if !new_dacl.is_null() {
                    let _ = LocalFree(Some(HLOCAL(new_dacl.cast())));
                }
                if !security_descriptor.is_invalid() {
                    let _ = LocalFree(Some(HLOCAL(security_descriptor.0)));
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
                if !self.info.hThread.is_invalid() {
                    let _ = CloseHandle(self.info.hThread);
                }
                if !self.info.hProcess.is_invalid() {
                    let _ = CloseHandle(self.info.hProcess);
                }
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

    // ---------- helpers for command line & environment ----------

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
            result.extend(std::iter::repeat('\\').take(backslashes * 2));
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
        // Windows expects a double-NUL-terminated sequence of UTF-16 "key=value\0... \0"
        let mut pairs: Vec<(String, String)> =
            env.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
        // Preserve original implementation’s sort (case-insensitive, then by original)
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

    // ---------- NtCreateLowBoxToken via direct FFI (no LibraryLoader feature) ----------

    #[link(name = "ntdll")]
    unsafe extern "system" {
        fn NtCreateLowBoxToken(
            token_handle: *mut HANDLE,
            existing_token_handle: HANDLE,
            desired_access: u32,
            object_attributes: *const c_void,
            package_sid: PSID,
            capability_count: u32,
            capabilities: *const SID_AND_ATTRIBUTES,
            handle_count: u32,
            handles: *const HANDLE,
        ) -> NTSTATUS;
    }

    fn nt_success(status: NTSTATUS) -> bool {
        status.0 >= 0
    }

    fn nt_to_io(status: NTSTATUS) -> io::Error {
        io::Error::from_raw_os_error(status.0)
    }

    fn create_lowbox_token(
        appcontainer_sid: PSID,
        caps: &[CapabilitySid],
    ) -> io::Result<HandleGuard> {
        unsafe {
            // open current process token with enough rights to duplicate/assign
            let mut process_token = HANDLE::default();
            let desired = TOKEN_ACCESS_MASK(
                TOKEN_DUPLICATE.0
                    | TOKEN_QUERY.0
                    | TOKEN_ASSIGN_PRIMARY.0
                    | TOKEN_ADJUST_DEFAULT.0
                    | TOKEN_ADJUST_SESSIONID.0,
            );
            OpenProcessToken(GetCurrentProcess(), desired, &mut process_token)
                .map_err(|e| io::Error::from_raw_os_error(e.code().0))?;
            let process_guard = HandleGuard::new(process_token);

            let mut sid_and_attrs: Vec<SID_AND_ATTRIBUTES> =
                caps.iter().map(CapabilitySid::sid_and_attributes).collect();
            let caps_ptr = if sid_and_attrs.is_empty() {
                null()
            } else {
                sid_and_attrs.as_mut_ptr()
            };

            let mut new_token = HANDLE::default();
            let status = NtCreateLowBoxToken(
                &mut new_token,
                process_guard.handle(),
                desired.0,
                null(),
                appcontainer_sid,
                sid_and_attrs.len() as u32,
                caps_ptr,
                0,
                null(),
            );
            if !nt_success(status) {
                return Err(nt_to_io(status));
            }

            Ok(HandleGuard::new(new_token))
        }
    }
}

#[cfg(target_os = "windows")]
use imp::spawn_command_under_windows_appcontainer;

#[cfg(not(target_os = "windows"))]
fn spawn_command_under_windows_appcontainer(
    _command: Vec<String>,
    _command_cwd: PathBuf,
    _sandbox_policy: &SandboxPolicy,
    _sandbox_policy_cwd: &Path,
    _stdio_policy: StdioPolicy,
    _env: HashMap<String, String>,
) -> std::io::Result<std::process::ExitStatus> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "Windows AppContainer sandboxing is only available on Windows",
    ))
}

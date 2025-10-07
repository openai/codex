use clap::Parser;
use codex_protocol::protocol::SandboxPolicy;
use std::collections::HashMap;
use std::env;
use std::path::Path;
use std::path::PathBuf;
use tokio::runtime::Builder;
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

    let current_dir = env::current_dir().expect("current dir should exist");
    let sandbox_policy_cwd = sandbox_policy_cwd.unwrap_or_else(|| current_dir.clone());
    let env_map: HashMap<String, String> = env::vars().collect();

    let runtime = Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("failed to build Tokio runtime");

    let status = runtime.block_on(async {
        let child = spawn_command_under_windows_appcontainer(
            command,
            current_dir,
            &sandbox_policy,
            sandbox_policy_cwd.as_path(),
            StdioPolicy::Inherit,
            env_map,
        )
        .await?;
        child.wait().await
    });

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
    use std::io;
    use std::os::windows::ffi::OsStrExt;
    use std::os::windows::process::CommandExt;
    use std::path::Path;
    use std::path::PathBuf;
    use std::ptr::null_mut;
    use tokio::process::Child;
    use tokio::process::Command;
    use windows::Win32::Foundation::ERROR_ALREADY_EXISTS;
    use windows::Win32::Foundation::ERROR_SUCCESS;
    use windows::Win32::Foundation::GetLastError;
    use windows::Win32::Foundation::HANDLE;
    use windows::Win32::Foundation::HLOCAL;
    use windows::Win32::Foundation::LocalFree;
    use windows::Win32::Foundation::WIN32_ERROR;
    use windows::Win32::Security::ACL;
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
    use windows::Win32::Security::SECURITY_CAPABILITIES;
    use windows::Win32::Security::SID_AND_ATTRIBUTES;
    use windows::Win32::Security::SUB_CONTAINERS_AND_OBJECTS_INHERIT;
    use windows::Win32::Storage::FileSystem::FILE_GENERIC_EXECUTE;
    use windows::Win32::Storage::FileSystem::FILE_GENERIC_READ;
    use windows::Win32::Storage::FileSystem::FILE_GENERIC_WRITE;
    use windows::Win32::System::Memory::GetProcessHeap;
    use windows::Win32::System::Memory::HEAP_FLAGS;
    use windows::Win32::System::Memory::HEAP_ZERO_MEMORY;
    use windows::Win32::System::Memory::HeapAlloc;
    use windows::Win32::System::Memory::HeapFree;
    use windows::Win32::System::Threading::DeleteProcThreadAttributeList;
    use windows::Win32::System::Threading::EXTENDED_STARTUPINFO_PRESENT;
    use windows::Win32::System::Threading::InitializeProcThreadAttributeList;
    use windows::Win32::System::Threading::LPPROC_THREAD_ATTRIBUTE_LIST;
    use windows::Win32::System::Threading::PROC_THREAD_ATTRIBUTE_SECURITY_CAPABILITIES;
    use windows::Win32::System::Threading::UpdateProcThreadAttribute;
    use windows::core::PCWSTR;
    use windows::core::PWSTR;

    const WINDOWS_APPCONTAINER_PROFILE_NAME: &str = "codex_appcontainer";
    const WINDOWS_APPCONTAINER_PROFILE_DESC: &str = "Codex Windows AppContainer profile";
    const INTERNET_CLIENT_SID: &str = "S-1-15-3-1";
    const PRIVATE_NETWORK_CLIENT_SID: &str = "S-1-15-3-3";

    pub(super) async fn spawn_command_under_windows_appcontainer(
        command: Vec<String>,
        command_cwd: PathBuf,
        sandbox_policy: &SandboxPolicy,
        sandbox_policy_cwd: &Path,
        stdio_policy: StdioPolicy,
        env: HashMap<String, String>,
    ) -> io::Result<Child> {
        trace!("windows appcontainer sandbox command = {:?}", command);

        let (program, rest) = command
            .split_first()
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "command args are empty"))?;

        ensure_appcontainer_profile()?;
        let mut sid = derive_appcontainer_sid()?;
        let mut capability_sids = build_capabilities(sandbox_policy)?;
        let mut attribute_list = AttributeList::new(&mut sid, &mut capability_sids)?;

        configure_writable_roots(sandbox_policy, sandbox_policy_cwd, sid.sid())?;
        configure_writable_roots_for_command_cwd(&command_cwd, sid.sid())?;

        let mut cmd = Command::new(program);
        cmd.args(rest);
        cmd.current_dir(command_cwd);
        cmd.env_clear();
        cmd.envs(env);
        apply_stdio_policy(&mut cmd, stdio_policy);
        cmd.kill_on_drop(true);

        unsafe {
            let std_cmd = cmd.as_std_mut();
            std_cmd.creation_flags(EXTENDED_STARTUPINFO_PRESENT.0);
            std_cmd.raw_attribute_list(attribute_list.as_mut_ptr().0.cast());
        }

        let child = cmd.spawn();
        drop(attribute_list);
        child
    }

    fn apply_stdio_policy(cmd: &mut Command, policy: StdioPolicy) {
        match policy {
            StdioPolicy::Inherit => {
                cmd.stdin(std::process::Stdio::inherit());
                cmd.stdout(std::process::Stdio::inherit());
                cmd.stderr(std::process::Stdio::inherit());
            }
        }
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
                    let already_exists = WIN32_ERROR::from(ERROR_ALREADY_EXISTS);
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
                    let _ = LocalFree(HLOCAL(self.sid.0));
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

    struct AttributeList<'a> {
        heap: HANDLE,
        buffer: *mut c_void,
        list: LPPROC_THREAD_ATTRIBUTE_LIST,
        sec_caps: SECURITY_CAPABILITIES,
        sid_and_attributes: Vec<SID_AND_ATTRIBUTES>,
        #[allow(dead_code)]
        sid: &'a mut SidHandle,
        #[allow(dead_code)]
        capabilities: &'a mut Vec<CapabilitySid>,
    }

    impl<'a> AttributeList<'a> {
        fn new(sid: &'a mut SidHandle, caps: &'a mut Vec<CapabilitySid>) -> io::Result<Self> {
            unsafe {
                let mut list_size = 0usize;
                let _ = InitializeProcThreadAttributeList(
                    LPPROC_THREAD_ATTRIBUTE_LIST::default(),
                    1,
                    0,
                    &mut list_size,
                );
                let heap =
                    GetProcessHeap().map_err(|e| io::Error::from_raw_os_error(e.code().0))?;
                let buffer = HeapAlloc(heap, HEAP_ZERO_MEMORY, list_size);
                if buffer.is_null() {
                    return Err(io::Error::last_os_error());
                }
                let list = LPPROC_THREAD_ATTRIBUTE_LIST(buffer);
                InitializeProcThreadAttributeList(list, 1, 0, &mut list_size)
                    .map_err(|e| io::Error::from_raw_os_error(e.code().0))?;

                let mut sid_and_attributes: Vec<SID_AND_ATTRIBUTES> =
                    caps.iter().map(CapabilitySid::sid_and_attributes).collect();

                let mut sec_caps = SECURITY_CAPABILITIES {
                    AppContainerSid: sid.sid(),
                    Capabilities: if sid_and_attributes.is_empty() {
                        null_mut()
                    } else {
                        sid_and_attributes.as_mut_ptr()
                    },
                    CapabilityCount: sid_and_attributes.len() as u32,
                    Reserved: 0,
                };

                UpdateProcThreadAttribute(
                    list,
                    0,
                    PROC_THREAD_ATTRIBUTE_SECURITY_CAPABILITIES as usize,
                    Some(&mut sec_caps as *mut _ as *const std::ffi::c_void),
                    std::mem::size_of::<SECURITY_CAPABILITIES>(),
                    None,
                    None,
                )
                .map_err(|e| io::Error::from_raw_os_error(e.code().0))?;

                Ok(Self {
                    heap,
                    buffer,
                    list,
                    sec_caps,
                    sid_and_attributes,
                    sid,
                    capabilities: caps,
                })
            }
        }

        fn as_mut_ptr(&mut self) -> LPPROC_THREAD_ATTRIBUTE_LIST {
            self.list
        }
    }

    impl Drop for AttributeList<'_> {
        fn drop(&mut self) {
            unsafe {
                if !self.list.is_invalid() {
                    DeleteProcThreadAttributeList(self.list);
                }
                if !self.heap.is_invalid() && !self.buffer.is_null() {
                    let _ = HeapFree(self.heap, HEAP_FLAGS(0), Some(self.buffer));
                }
            }
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
            let mut existing_dacl: *mut ACL = null_mut();
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
            if status != WIN32_ERROR::from(ERROR_SUCCESS) {
                if !security_descriptor.is_invalid() {
                    let _ = LocalFree(HLOCAL(security_descriptor.0));
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
            let mut new_dacl: *mut ACL = null_mut();
            let add_result =
                SetEntriesInAclW(Some(&explicit_entries), Some(existing_dacl), &mut new_dacl);
            if add_result != WIN32_ERROR::from(ERROR_SUCCESS) {
                if !new_dacl.is_null() {
                    let _ = LocalFree(HLOCAL(new_dacl.cast()));
                }
                if !security_descriptor.is_invalid() {
                    let _ = LocalFree(HLOCAL(security_descriptor.0));
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
            if set_result != WIN32_ERROR::from(ERROR_SUCCESS) {
                if !new_dacl.is_null() {
                    let _ = LocalFree(HLOCAL(new_dacl.cast()));
                }
                if !security_descriptor.is_invalid() {
                    let _ = LocalFree(HLOCAL(security_descriptor.0));
                }
                return Err(io::Error::from_raw_os_error(set_result.0 as i32));
            }

            if !new_dacl.is_null() {
                let _ = LocalFree(HLOCAL(new_dacl.cast()));
            }
            if !security_descriptor.is_invalid() {
                let _ = LocalFree(HLOCAL(security_descriptor.0));
            }
        }

        Ok(())
    }
}

#[cfg(target_os = "windows")]
use imp::spawn_command_under_windows_appcontainer;

#[cfg(not(target_os = "windows"))]
async fn spawn_command_under_windows_appcontainer(
    _command: Vec<String>,
    _command_cwd: PathBuf,
    _sandbox_policy: &SandboxPolicy,
    _sandbox_policy_cwd: &Path,
    _stdio_policy: StdioPolicy,
    _env: HashMap<String, String>,
) -> std::io::Result<tokio::process::Child> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "Windows AppContainer sandboxing is only available on Windows",
    ))
}

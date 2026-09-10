use crate::allow::compute_allow_paths_for_permissions;
use crate::deny_read_acl::plan_deny_read_acl_paths;
use crate::logging;
use crate::path_normalization::canonicalize_path;
use crate::resolved_permissions::ResolvedWindowsSandboxPermissions;
use crate::setup::SandboxSetupRequest;
use crate::setup::SetupRootOverrides;
use crate::setup::build_payload_deny_write_paths;
use crate::setup::build_payload_roots;
use crate::setup::gather_read_roots;
use crate::spawn_prep::LegacySessionSecurity;
use crate::token::get_current_token_for_restriction;
use crate::token::get_logon_sid_bytes;
use crate::token::get_user_sid_bytes;
use crate::winutil::format_last_error;
use crate::winutil::resolve_sid;
use crate::winutil::sid_bytes_from_string;
use crate::winutil::string_from_sid_bytes;
use crate::winutil::to_wide;
use anyhow::Result;
use rand::Rng;
use rand::SeedableRng;
use rand::rngs::SmallRng;
use std::collections::BTreeSet;
use std::collections::HashMap;
use std::ffi::c_void;
use std::path::Path;
use std::path::PathBuf;
use std::ptr;
use std::sync::Mutex;
use std::sync::OnceLock;
use windows_sys::Win32::Foundation::CloseHandle;
use windows_sys::Win32::Foundation::ERROR_SUCCESS;
use windows_sys::Win32::Foundation::GetLastError;
use windows_sys::Win32::Foundation::HLOCAL;
use windows_sys::Win32::Foundation::LocalFree;
use windows_sys::Win32::Security::Authorization::ConvertStringSecurityDescriptorToSecurityDescriptorW;
use windows_sys::Win32::Security::Authorization::EXPLICIT_ACCESS_W;
use windows_sys::Win32::Security::Authorization::GRANT_ACCESS;
use windows_sys::Win32::Security::Authorization::SE_WINDOW_OBJECT;
use windows_sys::Win32::Security::Authorization::SetEntriesInAclW;
use windows_sys::Win32::Security::Authorization::SetSecurityInfo;
use windows_sys::Win32::Security::Authorization::TRUSTEE_IS_SID;
use windows_sys::Win32::Security::Authorization::TRUSTEE_IS_UNKNOWN;
use windows_sys::Win32::Security::Authorization::TRUSTEE_W;
use windows_sys::Win32::Security::DACL_SECURITY_INFORMATION;
use windows_sys::Win32::Security::PSECURITY_DESCRIPTOR;
use windows_sys::Win32::Security::SECURITY_ATTRIBUTES;
use windows_sys::Win32::System::StationsAndDesktops::CloseDesktop;
use windows_sys::Win32::System::StationsAndDesktops::CreateDesktopW;
use windows_sys::Win32::System::StationsAndDesktops::DESKTOP_CREATEMENU;
use windows_sys::Win32::System::StationsAndDesktops::DESKTOP_CREATEWINDOW;
use windows_sys::Win32::System::StationsAndDesktops::DESKTOP_DELETE;
use windows_sys::Win32::System::StationsAndDesktops::DESKTOP_ENUMERATE;
use windows_sys::Win32::System::StationsAndDesktops::DESKTOP_HOOKCONTROL;
use windows_sys::Win32::System::StationsAndDesktops::DESKTOP_JOURNALPLAYBACK;
use windows_sys::Win32::System::StationsAndDesktops::DESKTOP_JOURNALRECORD;
use windows_sys::Win32::System::StationsAndDesktops::DESKTOP_READ_CONTROL;
use windows_sys::Win32::System::StationsAndDesktops::DESKTOP_READOBJECTS;
use windows_sys::Win32::System::StationsAndDesktops::DESKTOP_SWITCHDESKTOP;
use windows_sys::Win32::System::StationsAndDesktops::DESKTOP_WRITE_DAC;
use windows_sys::Win32::System::StationsAndDesktops::DESKTOP_WRITE_OWNER;
use windows_sys::Win32::System::StationsAndDesktops::DESKTOP_WRITEOBJECTS;
use windows_sys::Win32::System::StationsAndDesktops::OpenDesktopW;

const PRIVATE_DESKTOP_PREFIX: &str = "CodexSandboxDesktop-";

const DESKTOP_ALL_ACCESS: u32 = DESKTOP_READOBJECTS
    | DESKTOP_CREATEWINDOW
    | DESKTOP_CREATEMENU
    | DESKTOP_HOOKCONTROL
    | DESKTOP_JOURNALRECORD
    | DESKTOP_JOURNALPLAYBACK
    | DESKTOP_ENUMERATE
    | DESKTOP_WRITEOBJECTS
    | DESKTOP_SWITCHDESKTOP
    | DESKTOP_DELETE
    | DESKTOP_READ_CONTROL
    | DESKTOP_WRITE_DAC
    | DESKTOP_WRITE_OWNER;

const DESKTOP_PARTICIPANT_ACCESS: u32 =
    DESKTOP_ALL_ACCESS & !(DESKTOP_WRITE_DAC | DESKTOP_WRITE_OWNER | DESKTOP_DELETE);

static SHARED_PRIVATE_DESKTOPS: OnceLock<Mutex<HashMap<(String, DesktopPolicy), PrivateDesktop>>> =
    OnceLock::new();

#[derive(Clone, PartialEq, Eq, Hash)]
pub(crate) struct DesktopPolicy {
    uses_write_capabilities: bool,
    capability_sids: BTreeSet<Vec<u8>>,
    network_enabled: bool,
    network_proxy_restricting_sid: Option<Vec<u8>>,
    // None denotes the legacy backend's unrestricted reads and different desktop ACLs.
    read_roots: Option<BTreeSet<PathBuf>>,
    write_roots: BTreeSet<PathBuf>,
    deny_read_paths: BTreeSet<PathBuf>,
    deny_write_paths: BTreeSet<PathBuf>,
}

impl DesktopPolicy {
    pub(crate) fn elevated(
        request: SandboxSetupRequest<'_>,
        mut overrides: SetupRootOverrides,
        capability_sids: &[String],
        network_proxy_restricting_sid: Option<&str>,
    ) -> Result<Self> {
        // Match the complete read override passed by credential setup to the ACL helper.
        overrides.read_roots.get_or_insert_with(|| {
            gather_read_roots(
                request.command_cwd,
                request.permissions,
                request.env_map,
                request.codex_home,
            )
        });
        let (read_roots, write_roots) = build_payload_roots(&request, &overrides);
        Ok(Self {
            uses_write_capabilities: request
                .permissions
                .uses_write_capabilities_for_cwd(request.command_cwd, request.env_map),
            capability_sids: capability_sids
                .iter()
                .map(|sid| sid_bytes_from_string(sid))
                .collect::<Result<_>>()?,
            network_enabled: request.permissions.network_policy().is_enabled(),
            network_proxy_restricting_sid: network_proxy_restricting_sid
                .map(sid_bytes_from_string)
                .transpose()?,
            read_roots: Some(read_roots.into_iter().collect()),
            write_roots: write_roots.into_iter().collect(),
            deny_read_paths: plan_deny_read_acl_paths(
                overrides.deny_read_paths.as_deref().unwrap_or_default(),
            )?
            .into_iter()
            .collect(),
            deny_write_paths: build_payload_deny_write_paths(&request, overrides.deny_write_paths)
                .into_iter()
                .map(|path| canonicalize_path(&path))
                .collect(),
        })
    }
}

pub struct LaunchDesktop {
    _private_desktop: Option<PrivateDesktop>,
    startup_name: Vec<u16>,
}

impl LaunchDesktop {
    pub(crate) fn prepare_legacy(
        use_private_desktop: bool,
        permissions: &ResolvedWindowsSandboxPermissions,
        cwd: &Path,
        env: &HashMap<String, String>,
        security: &LegacySessionSecurity,
        additional_deny_write_paths: &[PathBuf],
        logs_base_dir: Option<&Path>,
    ) -> Result<Self> {
        if !use_private_desktop {
            return Self::prepare(/*use_private_desktop*/ false, logs_base_dir);
        }
        Self::open_private(&Self::shared_legacy_name(
            permissions,
            cwd,
            env,
            security,
            additional_deny_write_paths,
            logs_base_dir,
        )?)
    }

    pub(crate) fn shared_legacy_name(
        permissions: &ResolvedWindowsSandboxPermissions,
        cwd: &Path,
        env: &HashMap<String, String>,
        security: &LegacySessionSecurity,
        additional_deny_write_paths: &[PathBuf],
        logs_base_dir: Option<&Path>,
    ) -> Result<String> {
        let sandbox_sid = unsafe { get_user_sid_bytes(security.h_token)? };
        let sandbox_sid = string_from_sid_bytes(&sandbox_sid).map_err(anyhow::Error::msg)?;
        let paths = compute_allow_paths_for_permissions(permissions, cwd, env);
        let policy = DesktopPolicy {
            uses_write_capabilities: security.readonly_sid.is_none(),
            capability_sids: security
                .readonly_sid_str
                .iter()
                .chain(security.write_root_sids.iter().map(|root| &root.sid_str))
                .map(|sid| sid_bytes_from_string(sid))
                .collect::<Result<_>>()?,
            network_enabled: permissions.network_policy().is_enabled(),
            network_proxy_restricting_sid: None,
            read_roots: None,
            write_roots: paths.allow.into_iter().collect(),
            deny_read_paths: BTreeSet::new(),
            deny_write_paths: paths
                .deny
                .into_iter()
                .chain(additional_deny_write_paths.iter().cloned())
                .map(|path| canonicalize_path(&path))
                .collect(),
        };
        let mut desktops = SHARED_PRIVATE_DESKTOPS
            .get_or_init(|| Mutex::new(HashMap::new()))
            .lock()
            .map_err(|_| anyhow::anyhow!("shared private desktop cache was poisoned"))?;
        let desktop = match desktops.entry((sandbox_sid, policy)) {
            std::collections::hash_map::Entry::Occupied(entry) => entry.into_mut(),
            std::collections::hash_map::Entry::Vacant(entry) => {
                entry.insert(PrivateDesktop::create(logs_base_dir)?)
            }
        };
        Ok(desktop.name.clone())
    }

    pub fn prepare(use_private_desktop: bool, logs_base_dir: Option<&Path>) -> Result<Self> {
        if use_private_desktop {
            let private_desktop = PrivateDesktop::create(logs_base_dir)?;
            let startup_name = to_wide(format!("Winsta0\\{}", private_desktop.name));
            Ok(Self {
                _private_desktop: Some(private_desktop),
                startup_name,
            })
        } else {
            Ok(Self {
                _private_desktop: None,
                startup_name: to_wide("Winsta0\\Default"),
            })
        }
    }

    /// Opens the caller-owned private desktop without creating one or falling back to Default.
    pub fn open_private(name: &str) -> Result<Self> {
        if !name
            .strip_prefix(PRIVATE_DESKTOP_PREFIX)
            .is_some_and(|nonce| {
                !nonce.is_empty()
                    && nonce.len() <= 32
                    && nonce.bytes().all(|byte| byte.is_ascii_hexdigit())
            })
        {
            anyhow::bail!("invalid private desktop name");
        }
        let name_wide = to_wide(name);
        let handle = unsafe {
            OpenDesktopW(
                name_wide.as_ptr(),
                /*dwflags*/ 0,
                /*finherit*/ 0,
                DESKTOP_PARTICIPANT_ACCESS,
            )
        };
        if handle == 0 {
            anyhow::bail!("OpenDesktopW failed: {}", unsafe { GetLastError() });
        }
        Ok(Self {
            _private_desktop: Some(PrivateDesktop {
                handle,
                name: name.to_owned(),
            }),
            startup_name: to_wide(format!("Winsta0\\{name}")),
        })
    }

    pub fn startup_info_desktop(&self) -> *mut u16 {
        self.startup_name.as_ptr() as *mut u16
    }
}

/// Reuses a private desktop only for the same sandbox account and effective permissions.
pub(crate) fn shared_private_desktop_for_user(
    sandbox_username: &str,
    policy: &DesktopPolicy,
    logs_base_dir: Option<&Path>,
) -> Result<String> {
    let sandbox_sid =
        string_from_sid_bytes(&resolve_sid(sandbox_username)?).map_err(anyhow::Error::msg)?;
    let mut desktops = SHARED_PRIVATE_DESKTOPS
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .map_err(|_| anyhow::anyhow!("shared private desktop cache was poisoned"))?;
    let key = (sandbox_sid.clone(), policy.clone());
    if let Some(desktop) = desktops.get(&key) {
        return Ok(desktop.name.clone());
    }

    let owner_user_sid = unsafe {
        let token = get_current_token_for_restriction()?;
        let sid = get_user_sid_bytes(token);
        CloseHandle(token);
        sid?
    };
    let owner_user_sid = string_from_sid_bytes(&owner_user_sid).map_err(anyhow::Error::msg)?;
    // CreateProcessWithLogonW shares the caller's logon SID with the sandbox account.
    // Grant ACL-management rights to the caller's user SID instead.
    // https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-createprocesswithlogonw
    let sddl = to_wide(format!(
        "D:P(A;;0x{DESKTOP_ALL_ACCESS:x};;;{owner_user_sid})(A;;0x{DESKTOP_PARTICIPANT_ACCESS:x};;;{sandbox_sid})"
    ));
    let mut security_descriptor: PSECURITY_DESCRIPTOR = ptr::null_mut();
    if unsafe {
        ConvertStringSecurityDescriptorToSecurityDescriptorW(
            sddl.as_ptr(),
            /*stringsdrevision*/ 1,
            &mut security_descriptor,
            ptr::null_mut(),
        )
    } == 0
    {
        anyhow::bail!(
            "ConvertStringSecurityDescriptorToSecurityDescriptorW failed: {}",
            unsafe { GetLastError() }
        );
    }

    let attributes = SECURITY_ATTRIBUTES {
        nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: security_descriptor,
        bInheritHandle: 0,
    };
    let mut rng = SmallRng::from_entropy();
    let name = format!("{PRIVATE_DESKTOP_PREFIX}{:032x}", rng.r#gen::<u128>());
    let name_wide = to_wide(&name);
    let handle = unsafe {
        CreateDesktopW(
            name_wide.as_ptr(),
            ptr::null(),
            ptr::null_mut(),
            /*dwflags*/ 0,
            DESKTOP_ALL_ACCESS,
            &attributes,
        )
    };
    let error = unsafe { GetLastError() };
    unsafe {
        LocalFree(security_descriptor as HLOCAL);
    }
    if handle == 0 {
        logging::debug_log(
            &format!("CreateDesktopW failed for shared private desktop: {error}"),
            logs_base_dir,
        );
        anyhow::bail!("CreateDesktopW failed for shared private desktop: {error}");
    }

    // Retain ownership across runner exits and idle gaps; different policies stay on separate
    // desktops so GUI hooks do not automatically cross those policies.
    desktops.insert(
        key,
        PrivateDesktop {
            handle,
            name: name.clone(),
        },
    );
    Ok(name)
}

struct PrivateDesktop {
    handle: isize,
    name: String,
}

impl PrivateDesktop {
    fn create(logs_base_dir: Option<&Path>) -> Result<Self> {
        let mut rng = SmallRng::from_entropy();
        let name = format!("CodexSandboxDesktop-{:x}", rng.r#gen::<u128>());
        let name_wide = to_wide(&name);
        let handle = unsafe {
            CreateDesktopW(
                name_wide.as_ptr(),
                ptr::null(),
                ptr::null_mut(),
                0,
                DESKTOP_ALL_ACCESS,
                ptr::null_mut(),
            )
        };
        if handle == 0 {
            let err = unsafe { GetLastError() } as i32;
            logging::debug_log(
                &format!(
                    "CreateDesktopW failed for {name}: {} ({})",
                    err,
                    format_last_error(err),
                ),
                logs_base_dir,
            );
            return Err(anyhow::anyhow!("CreateDesktopW failed: {err}"));
        }

        unsafe {
            if let Err(err) = grant_desktop_access(handle, logs_base_dir) {
                let _ = CloseDesktop(handle);
                return Err(err);
            }
        }

        Ok(Self { handle, name })
    }
}

unsafe fn grant_desktop_access(handle: isize, logs_base_dir: Option<&Path>) -> Result<()> {
    let token = get_current_token_for_restriction()?;
    let mut logon_sid = get_logon_sid_bytes(token)?;
    CloseHandle(token);

    let entries = [EXPLICIT_ACCESS_W {
        grfAccessPermissions: DESKTOP_ALL_ACCESS,
        grfAccessMode: GRANT_ACCESS,
        grfInheritance: 0,
        Trustee: TRUSTEE_W {
            pMultipleTrustee: ptr::null_mut(),
            MultipleTrusteeOperation: 0,
            TrusteeForm: TRUSTEE_IS_SID,
            TrusteeType: TRUSTEE_IS_UNKNOWN,
            ptstrName: logon_sid.as_mut_ptr() as *mut c_void as *mut u16,
        },
    }];

    let mut updated_dacl = ptr::null_mut();
    let set_entries_code = SetEntriesInAclW(
        entries.len() as u32,
        entries.as_ptr(),
        ptr::null_mut(),
        &mut updated_dacl,
    );
    if set_entries_code != ERROR_SUCCESS {
        logging::debug_log(
            &format!("SetEntriesInAclW failed for private desktop: {set_entries_code}"),
            logs_base_dir,
        );
        return Err(anyhow::anyhow!(
            "SetEntriesInAclW failed for private desktop: {set_entries_code}"
        ));
    }

    let set_security_code = SetSecurityInfo(
        handle,
        SE_WINDOW_OBJECT,
        DACL_SECURITY_INFORMATION,
        ptr::null_mut(),
        ptr::null_mut(),
        updated_dacl,
        ptr::null_mut(),
    );
    if !updated_dacl.is_null() {
        LocalFree(updated_dacl as HLOCAL);
    }
    if set_security_code != ERROR_SUCCESS {
        logging::debug_log(
            &format!("SetSecurityInfo failed for private desktop: {set_security_code}"),
            logs_base_dir,
        );
        return Err(anyhow::anyhow!(
            "SetSecurityInfo failed for private desktop: {set_security_code}"
        ));
    }

    Ok(())
}

impl Drop for PrivateDesktop {
    fn drop(&mut self) {
        unsafe {
            if self.handle != 0 {
                let _ = CloseDesktop(self.handle);
            }
        }
    }
}

#[cfg(test)]
#[path = "desktop_tests.rs"]
mod tests;

use serde::Deserialize;
use serde::Serialize;
use std::collections::BTreeSet;
use std::collections::HashMap;
use std::collections::HashSet;
use std::ffi::c_void;
use std::os::windows::process::CommandExt;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::process::Stdio;

use crate::allow::AllowDenyPaths;
use crate::allow::compute_allow_paths_for_permissions;
use crate::helper_materialization::HelperExecutable;
use crate::helper_materialization::bundled_executable_path_for_exe;
use crate::helper_materialization::helper_bin_dir;
use crate::helper_materialization::resolve_helper_for_launch;
use crate::identity::sandbox_setup_is_complete;
use crate::logging::log_note;
use crate::path_normalization::canonical_path_key;
use crate::path_normalization::canonicalize_path;
use crate::resolved_permissions::ResolvedWindowsSandboxPermissions;
use crate::setup_error::SetupErrorCode;
use crate::setup_error::SetupFailure;
use crate::setup_error::clear_setup_error_report;
use crate::setup_error::failure;
use crate::setup_error::read_setup_error_report;
use crate::ssh_config_dependencies::ssh_config_dependency_paths;
use anyhow::Context;
use anyhow::Result;
use anyhow::anyhow;
use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use codex_protocol::models::PermissionProfile;
use codex_utils_absolute_path::AbsolutePathBuf;

use windows_sys::Win32::Foundation::CloseHandle;
use windows_sys::Win32::Foundation::GetLastError;
use windows_sys::Win32::Security::AllocateAndInitializeSid;
use windows_sys::Win32::Security::CheckTokenMembership;
use windows_sys::Win32::Security::FreeSid;
use windows_sys::Win32::Security::SECURITY_NT_AUTHORITY;

pub const SETUP_VERSION: u32 = 5;
pub const OFFLINE_USERNAME: &str = "CodexSandboxOffline";
pub const ONLINE_USERNAME: &str = "CodexSandboxOnline";
const ERROR_CANCELLED: u32 = 1223;
const SECURITY_BUILTIN_DOMAIN_RID: u32 = 0x0000_0020;
const DOMAIN_ALIAS_RID_ADMINS: u32 = 0x0000_0220;
const SETUP_EXE_FILENAME: &str = "codex-windows-sandbox-setup.exe";
const USERPROFILE_ROOT_EXCLUSIONS: &[&str] = &[
    ".ssh",
    ".tsh",
    ".brev",
    ".gnupg",
    ".aws",
    ".azure",
    ".kube",
    ".docker",
    ".config",
    ".npm",
    ".pki",
    ".terraform.d",
];
const WINDOWS_PLATFORM_DEFAULT_READ_ROOTS: &[&str] = &[
    r"C:\Windows",
    r"C:\Program Files",
    r"C:\Program Files (x86)",
    r"C:\ProgramData",
];

pub fn sandbox_dir(codex_home: &Path) -> PathBuf {
    codex_home.join(".sandbox")
}

pub fn sandbox_bin_dir(codex_home: &Path) -> PathBuf {
    codex_home.join(".sandbox-bin")
}

pub fn sandbox_secrets_dir(codex_home: &Path) -> PathBuf {
    codex_home.join(".sandbox-secrets")
}

pub fn setup_marker_path(codex_home: &Path) -> PathBuf {
    sandbox_dir(codex_home).join("setup_marker.json")
}

pub fn sandbox_users_path(codex_home: &Path) -> PathBuf {
    sandbox_secrets_dir(codex_home).join("sandbox_users.json")
}

pub struct SandboxSetupRequest<'a> {
    pub permissions: &'a ResolvedWindowsSandboxPermissions,
    pub command_cwd: &'a Path,
    pub env_map: &'a HashMap<String, String>,
    pub codex_home: &'a Path,
    pub proxy_enforced: bool,
}

#[derive(Default)]
pub struct SetupRootOverrides {
    pub read_roots: Option<Vec<PathBuf>>,
    pub read_roots_include_platform_defaults: bool,
    pub write_roots: Option<Vec<PathBuf>>,
    pub deny_read_paths: Option<Vec<PathBuf>>,
    pub deny_write_paths: Option<Vec<PathBuf>>,
}

pub fn run_setup_refresh(
    permission_profile: &PermissionProfile,
    workspace_roots: &[AbsolutePathBuf],
    command_cwd: &Path,
    env_map: &HashMap<String, String>,
    codex_home: &Path,
    proxy_enforced: bool,
) -> Result<()> {
    let Ok(permissions) =
        ResolvedWindowsSandboxPermissions::try_from_permission_profile_for_workspace_roots(
            permission_profile,
            workspace_roots,
        )
    else {
        return Ok(());
    };
    run_setup_refresh_inner(
        SandboxSetupRequest {
            permissions: &permissions,
            command_cwd,
            env_map,
            codex_home,
            proxy_enforced,
        },
        SetupRootOverrides::default(),
    )
}

pub fn run_setup_refresh_with_overrides(
    request: SandboxSetupRequest<'_>,
    overrides: SetupRootOverrides,
) -> Result<()> {
    run_setup_refresh_inner(request, overrides)
}

pub fn run_setup_refresh_with_extra_read_roots(
    permission_profile: &PermissionProfile,
    workspace_roots: &[AbsolutePathBuf],
    command_cwd: &Path,
    env_map: &HashMap<String, String>,
    codex_home: &Path,
    extra_read_roots: Vec<PathBuf>,
    proxy_enforced: bool,
) -> Result<()> {
    let Ok(permissions) =
        ResolvedWindowsSandboxPermissions::try_from_permission_profile_for_workspace_roots(
            permission_profile,
            workspace_roots,
        )
    else {
        return Ok(());
    };
    let mut read_roots = gather_read_roots(command_cwd, &permissions, env_map, codex_home);
    read_roots.extend(extra_read_roots);
    run_setup_refresh_inner(
        SandboxSetupRequest {
            permissions: &permissions,
            command_cwd,
            env_map,
            codex_home,
            proxy_enforced,
        },
        SetupRootOverrides {
            read_roots: Some(read_roots),
            read_roots_include_platform_defaults: false,
            write_roots: Some(Vec::new()),
            deny_read_paths: None,
            deny_write_paths: None,
        },
    )
}

fn run_setup_refresh_inner(
    request: SandboxSetupRequest<'_>,
    overrides: SetupRootOverrides,
) -> Result<()> {
    if !request.permissions.is_enforceable_by_windows_sandbox() {
        anyhow::bail!("unsupported filesystem permissions for Windows sandbox setup");
    }
    let (read_roots, write_roots) = build_payload_roots(&request, &overrides);
    let deny_read_paths = build_payload_deny_read_paths(overrides.deny_read_paths);
    let deny_write_paths = build_payload_deny_write_paths(&request, overrides.deny_write_paths);
    let network_identity =
        SandboxNetworkIdentity::from_permissions(request.permissions, request.proxy_enforced);
    let offline_proxy_settings = offline_proxy_settings_from_env(request.env_map, network_identity);
    let payload = ElevationPayload {
        version: SETUP_VERSION,
        offline_username: OFFLINE_USERNAME.to_string(),
        online_username: ONLINE_USERNAME.to_string(),
        codex_home: request.codex_home.to_path_buf(),
        command_cwd: request.command_cwd.to_path_buf(),
        read_roots,
        write_roots,
        deny_read_paths,
        deny_write_paths,
        proxy_ports: offline_proxy_settings.proxy_ports,
        allow_local_binding: offline_proxy_settings.allow_local_binding,
        otel: None,
        real_user: std::env::var("USERNAME").unwrap_or_else(|_| "Administrators".to_string()),
        mode: SetupMode::Full,
        refresh_only: true,
    };
    let json = serde_json::to_vec(&payload)?;
    let b64 = BASE64_STANDARD.encode(json);
    let log_dir = sandbox_dir(request.codex_home);
    let exe = resolve_helper_for_launch(
        HelperExecutable::SetupHelper,
        request.codex_home,
        Some(&log_dir),
    );
    // Refresh should never request elevation, so prefer the copied helper
    // instead of launching the packaged setup executable directly.
    let mut cmd = Command::new(&exe);
    cmd.arg(&b64).stdout(Stdio::null()).stderr(Stdio::null());
    let cwd = std::env::current_dir().unwrap_or_else(|_| request.codex_home.to_path_buf());
    log_note(
        &format!(
            "setup refresh: spawning {} (cwd={}, payload_len={})",
            exe.display(),
            cwd.display(),
            b64.len()
        ),
        Some(&log_dir),
    );
    let status = cmd
        .status()
        .map_err(|e| {
            log_note(
                &format!("setup refresh: failed to spawn {}: {e}", exe.display()),
                Some(&log_dir),
            );
            e
        })
        .context("spawn setup refresh")?;
    if !status.success() {
        log_note(
            &format!("setup refresh: exited with status {status:?}"),
            Some(&log_dir),
        );
        return Err(anyhow!("setup refresh failed with status {status}"));
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SetupMarker {
    pub version: u32,
    pub offline_username: String,
    pub online_username: String,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub proxy_ports: Vec<u16>,
    #[serde(default)]
    pub allow_local_binding: bool,
}

impl SetupMarker {
    pub fn version_matches(&self) -> bool {
        self.version == SETUP_VERSION
    }

    pub(crate) fn request_mismatch_reason(
        &self,
        network_identity: SandboxNetworkIdentity,
        offline_proxy_settings: &OfflineProxySettings,
    ) -> Option<String> {
        if !network_identity.uses_offline_identity() {
            return None;
        }
        if self.proxy_ports == offline_proxy_settings.proxy_ports
            && self.allow_local_binding == offline_proxy_settings.allow_local_binding
        {
            return None;
        }
        Some(format!(
            "offline firewall settings changed (stored_ports={:?}, desired_ports={:?}, stored_allow_local_binding={}, desired_allow_local_binding={})",
            self.proxy_ports,
            offline_proxy_settings.proxy_ports,
            self.allow_local_binding,
            offline_proxy_settings.allow_local_binding
        ))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SandboxUserRecord {
    pub username: String,
    /// DPAPI-encrypted password blob, base64 encoded.
    pub password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SandboxUsersFile {
    pub version: u32,
    pub offline: SandboxUserRecord,
    pub online: SandboxUserRecord,
}

impl SandboxUsersFile {
    pub fn version_matches(&self) -> bool {
        self.version == SETUP_VERSION
    }
}

fn is_elevated() -> Result<bool> {
    unsafe {
        let mut administrators_group: *mut c_void = std::ptr::null_mut();
        let ok = AllocateAndInitializeSid(
            &SECURITY_NT_AUTHORITY,
            2,
            SECURITY_BUILTIN_DOMAIN_RID,
            DOMAIN_ALIAS_RID_ADMINS,
            0,
            0,
            0,
            0,
            0,
            0,
            &mut administrators_group,
        );
        if ok == 0 {
            return Err(anyhow!(
                "AllocateAndInitializeSid failed: {}",
                GetLastError()
            ));
        }
        let mut is_member = 0i32;
        let check = CheckTokenMembership(0, administrators_group, &mut is_member as *mut _);
        FreeSid(administrators_group as *mut _);
        if check == 0 {
            return Err(anyhow!("CheckTokenMembership failed: {}", GetLastError()));
        }
        Ok(is_member != 0)
    }
}

fn canonical_existing(paths: &[PathBuf]) -> Vec<PathBuf> {
    paths
        .iter()
        .filter_map(|p| {
            if !p.exists() {
                return None;
            }
            Some(dunce::canonicalize(p).unwrap_or_else(|_| p.clone()))
        })
        .collect()
}

fn profile_read_roots(user_profile: &Path) -> Vec<PathBuf> {
    let entries = match std::fs::read_dir(user_profile) {
        Ok(entries) => entries,
        Err(_) => return vec![user_profile.to_path_buf()],
    };

    entries
        .filter_map(Result::ok)
        .map(|entry| (entry.file_name(), entry.path()))
        .filter(|(name, _)| {
            let name = name.to_string_lossy();
            !USERPROFILE_ROOT_EXCLUSIONS
                .iter()
                .any(|excluded| name.eq_ignore_ascii_case(excluded))
        })
        .map(|(_, path)| path)
        .collect()
}

fn gather_helper_read_roots(codex_home: &Path) -> Vec<PathBuf> {
    let helper_dir = helper_bin_dir(codex_home);
    let _ = std::fs::create_dir_all(&helper_dir);
    vec![helper_dir]
}

fn gather_full_read_roots_for_permissions(
    command_cwd: &Path,
    permissions: &ResolvedWindowsSandboxPermissions,
    env_map: &HashMap<String, String>,
    codex_home: &Path,
) -> Vec<PathBuf> {
    let mut roots = gather_helper_read_roots(codex_home);
    roots.extend(
        WINDOWS_PLATFORM_DEFAULT_READ_ROOTS
            .iter()
            .map(PathBuf::from),
    );
    if let Ok(up) = std::env::var("USERPROFILE") {
        roots.extend(profile_read_roots(Path::new(&up)));
    }
    roots.push(command_cwd.to_path_buf());
    roots.extend(
        permissions
            .writable_roots_for_cwd(command_cwd, env_map)
            .into_iter()
            .map(|root| root.root),
    );
    canonical_existing(&roots)
}

pub(crate) fn gather_read_roots(
    command_cwd: &Path,
    permissions: &ResolvedWindowsSandboxPermissions,
    env_map: &HashMap<String, String>,
    codex_home: &Path,
) -> Vec<PathBuf> {
    if permissions.has_full_disk_read_access() {
        return gather_full_read_roots_for_permissions(
            command_cwd,
            permissions,
            env_map,
            codex_home,
        );
    }

    let mut roots = gather_helper_read_roots(codex_home);
    if permissions.include_platform_defaults() {
        roots.extend(
            WINDOWS_PLATFORM_DEFAULT_READ_ROOTS
                .iter()
                .map(PathBuf::from),
        );
    }
    roots.extend(permissions.readable_roots_for_cwd(command_cwd));
    canonical_existing(&roots)
}

pub(crate) fn gather_write_roots_for_permissions(
    permissions: &ResolvedWindowsSandboxPermissions,
    command_cwd: &Path,
    env_map: &HashMap<String, String>,
) -> Vec<PathBuf> {
    let roots = permissions
        .writable_roots_for_cwd(command_cwd, env_map)
        .into_iter()
        .map(|root| root.root)
        .collect::<Vec<_>>();
    let mut dedup: HashSet<PathBuf> = HashSet::new();
    let mut out: Vec<PathBuf> = Vec::new();
    for r in canonical_existing(&roots) {
        if dedup.insert(r.clone()) {
            out.push(r);
        }
    }
    out
}

pub(crate) fn effective_write_roots_for_setup(
    permissions: &ResolvedWindowsSandboxPermissions,
    command_cwd: &Path,
    env_map: &HashMap<String, String>,
    codex_home: &Path,
    write_roots_override: Option<&[PathBuf]>,
) -> Vec<PathBuf> {
    effective_write_roots_for_permissions(
        permissions,
        command_cwd,
        env_map,
        codex_home,
        write_roots_override,
    )
}

pub(crate) fn effective_write_roots_for_permissions(
    permissions: &ResolvedWindowsSandboxPermissions,
    command_cwd: &Path,
    env_map: &HashMap<String, String>,
    codex_home: &Path,
    write_roots_override: Option<&[PathBuf]>,
) -> Vec<PathBuf> {
    let write_roots = if let Some(roots) = write_roots_override {
        canonical_existing(roots)
    } else {
        gather_write_roots_for_permissions(permissions, command_cwd, env_map)
    };
    let write_roots = expand_user_profile_root(write_roots);
    let write_roots = filter_user_profile_root(write_roots);
    let write_roots = filter_user_profile_root_exclusions(write_roots);
    let write_roots = filter_ssh_config_dependency_roots(write_roots);
    filter_sensitive_write_roots(write_roots, codex_home)
}

#[derive(Serialize)]
struct ElevationPayload {
    version: u32,
    offline_username: String,
    online_username: String,
    codex_home: PathBuf,
    command_cwd: PathBuf,
    read_roots: Vec<PathBuf>,
    write_roots: Vec<PathBuf>,
    #[serde(default)]
    deny_read_paths: Vec<PathBuf>,
    #[serde(default)]
    deny_write_paths: Vec<PathBuf>,
    proxy_ports: Vec<u16>,
    #[serde(default)]
    allow_local_binding: bool,
    otel: Option<codex_otel::StatsigMetricsSettings>,
    real_user: String,
    mode: SetupMode,
    #[serde(default)]
    refresh_only: bool,
}

#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "kebab-case")]
enum SetupMode {
    Full,
    ProvisionOnly,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct OfflineProxySettings {
    pub proxy_ports: Vec<u16>,
    pub allow_local_binding: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SandboxNetworkIdentity {
    Offline,
    Online,
}

impl SandboxNetworkIdentity {
    pub(crate) fn from_permissions(
        permissions: &ResolvedWindowsSandboxPermissions,
        proxy_enforced: bool,
    ) -> Self {
        if proxy_enforced || !permissions.network_policy().is_enabled() {
            Self::Offline
        } else {
            Self::Online
        }
    }

    pub(crate) fn uses_offline_identity(self) -> bool {
        matches!(self, Self::Offline)
    }
}

const PROXY_ENV_KEYS: &[&str] = &[
    "HTTP_PROXY",
    "HTTPS_PROXY",
    "ALL_PROXY",
    "WS_PROXY",
    "WSS_PROXY",
    "http_proxy",
    "https_proxy",
    "all_proxy",
    "ws_proxy",
    "wss_proxy",
];
const ALLOW_LOCAL_BINDING_ENV_KEY: &str = "CODEX_NETWORK_ALLOW_LOCAL_BINDING";

pub(crate) fn offline_proxy_settings_from_env(
    env_map: &HashMap<String, String>,
    network_identity: SandboxNetworkIdentity,
) -> OfflineProxySettings {
    if !network_identity.uses_offline_identity() {
        return OfflineProxySettings {
            proxy_ports: vec![],
            allow_local_binding: false,
        };
    }
    OfflineProxySettings {
        proxy_ports: proxy_ports_from_env(env_map),
        allow_local_binding: env_map
            .get(ALLOW_LOCAL_BINDING_ENV_KEY)
            .is_some_and(|value| value == "1"),
    }
}

pub(crate) fn proxy_ports_from_env(env_map: &HashMap<String, String>) -> Vec<u16> {
    let mut ports = BTreeSet::new();
    for key in PROXY_ENV_KEYS {
        if let Some(value) = env_map.get(*key)
            && let Some(port) = loopback_proxy_port_from_url(value)
        {
            ports.insert(port);
        }
    }
    ports.into_iter().collect()
}

fn loopback_proxy_port_from_url(url: &str) -> Option<u16> {
    let authority = url.trim().split_once("://")?.1.split('/').next()?;
    let host_port = authority.rsplit_once('@').map_or(authority, |(_, hp)| hp);

    if let Some(host) = host_port.strip_prefix('[') {
        let (host, rest) = host.split_once(']')?;
        if host != "::1" {
            return None;
        }
        let port = rest.strip_prefix(':')?.parse::<u16>().ok()?;
        return (port != 0).then_some(port);
    }

    let (host, port) = host_port.rsplit_once(':')?;
    if !(host.eq_ignore_ascii_case("localhost") || host == "127.0.0.1") {
        return None;
    }
    let port = port.parse::<u16>().ok()?;
    (port != 0).then_some(port)
}

fn quote_arg(arg: &str) -> String {
    let needs = arg.is_empty()
        || arg
            .chars()
            .any(|c| matches!(c, ' ' | '\t' | '\n' | '\r' | '"'));
    if !needs {
        return arg.to_string();
    }
    let mut out = String::from("\"");
    let mut bs = 0;
    for ch in arg.chars() {
        match ch {
            '\\' => {
                bs += 1;
            }
            '"' => {
                out.push_str(&"\\".repeat(bs * 2 + 1));
                out.push('"');
                bs = 0;
            }
            _ => {
                if bs > 0 {
                    out.push_str(&"\\".repeat(bs));
                    bs = 0;
                }
                out.push(ch);
            }
        }
    }
    if bs > 0 {
        out.push_str(&"\\".repeat(bs * 2));
    }
    out.push('"');
    out
}

fn find_setup_exe() -> PathBuf {
    if let Ok(exe) = std::env::current_exe()
        && let Some(setup_exe) = find_setup_exe_for_current_exe(&exe)
    {
        return setup_exe;
    }
    PathBuf::from(SETUP_EXE_FILENAME)
}

fn find_setup_exe_for_current_exe(exe: &Path) -> Option<PathBuf> {
    bundled_executable_path_for_exe(exe, SETUP_EXE_FILENAME)
}

fn report_helper_failure(
    codex_home: &Path,
    cleared_report: bool,
    exit_code: Option<i32>,
) -> anyhow::Error {
    let exit_detail = format!("setup helper exited with status {exit_code:?}");
    if !cleared_report {
        return failure(SetupErrorCode::OrchestratorHelperExitNonzero, exit_detail);
    }
    match read_setup_error_report(codex_home) {
        Ok(Some(report)) => anyhow::Error::new(SetupFailure::from_report(report)),
        Ok(None) => failure(SetupErrorCode::OrchestratorHelperExitNonzero, exit_detail),
        Err(err) => failure(
            SetupErrorCode::OrchestratorHelperReportReadFailed,
            format!("{exit_detail}; failed to read setup_error.json: {err}"),
        ),
    }
}

fn run_setup_exe(
    payload: &ElevationPayload,
    needs_elevation: bool,
    codex_home: &Path,
) -> Result<()> {
    use windows_sys::Win32::System::Threading::GetExitCodeProcess;
    use windows_sys::Win32::System::Threading::INFINITE;
    use windows_sys::Win32::System::Threading::WaitForSingleObject;
    use windows_sys::Win32::UI::Shell::SEE_MASK_NOCLOSEPROCESS;
    use windows_sys::Win32::UI::Shell::SHELLEXECUTEINFOW;
    use windows_sys::Win32::UI::Shell::ShellExecuteExW;
    let exe = find_setup_exe();
    let payload_json = serde_json::to_string(payload).map_err(|err| {
        failure(
            SetupErrorCode::OrchestratorPayloadSerializeFailed,
            format!("failed to serialize elevation payload: {err}"),
        )
    })?;
    let payload_b64 = BASE64_STANDARD.encode(payload_json.as_bytes());
    let cleared_report = match clear_setup_error_report(codex_home) {
        Ok(()) => true,
        Err(err) => {
            log_note(
                &format!(
                    "setup orchestrator: failed to clear setup_error.json before launch: {err}"
                ),
                Some(&sandbox_dir(codex_home)),
            );
            false
        }
    };

    if !needs_elevation {
        let status = Command::new(&exe)
            .arg(&payload_b64)
            .creation_flags(0x08000000) // CREATE_NO_WINDOW
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map_err(|err| {
                failure(
                    SetupErrorCode::OrchestratorHelperLaunchFailed,
                    format!("failed to launch setup helper (non-elevated): {err}"),
                )
            })?;
        if !status.success() {
            return Err(report_helper_failure(
                codex_home,
                cleared_report,
                status.code(),
            ));
        }
        if let Err(err) = clear_setup_error_report(codex_home) {
            log_note(
                &format!(
                    "setup orchestrator: failed to clear setup_error.json after success: {err}"
                ),
                Some(&sandbox_dir(codex_home)),
            );
        }
        return Ok(());
    }

    let file = exe.display().to_string();
    let verb = "runas".to_string();
    let params = quote_arg(&payload_b64);
    let cwd = std::env::current_dir().unwrap_or_else(|_| codex_home.to_path_buf());
    let file_w = widestring::U16CString::from_str(&file).map_err(|err| {
        failure(
            SetupErrorCode::OrchestratorHelperLaunchFailed,
            format!("setup helper path contains interior NUL: {err}"),
        )
    })?;
    let verb_w = widestring::U16CString::from_str(&verb).map_err(|err| {
        failure(
            SetupErrorCode::OrchestratorHelperLaunchFailed,
            format!("ShellExecute verb contains interior NUL: {err}"),
        )
    })?;
    let params_w = widestring::U16CString::from_str(&params).map_err(|err| {
        failure(
            SetupErrorCode::OrchestratorHelperLaunchFailed,
            format!("setup helper params contain interior NUL: {err}"),
        )
    })?;
    let cwd_w = widestring::U16CString::from_os_str(cwd.as_os_str()).map_err(|err| {
        failure(
            SetupErrorCode::OrchestratorHelperLaunchFailed,
            format!("setup helper cwd contains interior NUL: {err}"),
        )
    })?;

    let mut sei = SHELLEXECUTEINFOW {
        cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
        fMask: SEE_MASK_NOCLOSEPROCESS,
        hwnd: 0,
        lpVerb: verb_w.as_ptr(),
        lpFile: file_w.as_ptr(),
        lpParameters: params_w.as_ptr(),
        lpDirectory: cwd_w.as_ptr(),
        nShow: 0, // SW_HIDE
        hInstApp: 0,
        lpIDList: std::ptr::null_mut(),
        lpClass: std::ptr::null(),
        hkeyClass: 0,
        dwHotKey: 0,
        Anonymous: windows_sys::Win32::UI::Shell::SHELLEXECUTEINFOW_0 {
            hMonitor: 0,
        },
        hProcess: 0,
    };
    let ok = unsafe { ShellExecuteExW(&mut sei as *mut _) };
    if ok == 0 {
        let last_error = unsafe { GetLastError() };
        if last_error == ERROR_CANCELLED {
            return Err(failure(
                SetupErrorCode::OrchestratorHelperElevationCancelled,
                "setup helper elevation was cancelled by the user",
            ));
        }
        return Err(failure(
            SetupErrorCode::OrchestratorHelperLaunchFailed,
            format!("ShellExecuteExW failed to launch setup helper: {last_error}"),
        ));
    }

    if sei.hProcess == 0 {
        return Err(failure(
            SetupErrorCode::OrchestratorHelperLaunchFailed,
            "ShellExecuteExW did not return a process handle for the setup helper",
        ));
    }

    let wait_code = unsafe { WaitForSingleObject(sei.hProcess, INFINITE) };
    if wait_code != 0 {
        let last_error = unsafe { GetLastError() };
        unsafe {
            CloseHandle(sei.hProcess);
        }
        return Err(failure(
            SetupErrorCode::OrchestratorHelperWaitFailed,
            format!("WaitForSingleObject failed for setup helper: wait_code={wait_code}, last_error={last_error}"),
        ));
    }

    let mut exit_code: u32 = 0;
    let got_exit_code = unsafe { GetExitCodeProcess(sei.hProcess, &mut exit_code as *mut _) };
    unsafe {
        CloseHandle(sei.hProcess);
    }
    if got_exit_code == 0 {
        let last_error = unsafe { GetLastError() };
        return Err(failure(
            SetupErrorCode::OrchestratorHelperExitCodeReadFailed,
            format!("GetExitCodeProcess failed for setup helper: {last_error}"),
        ));
    }

    if exit_code != 0 {
        return Err(report_helper_failure(
            codex_home,
            cleared_report,
            i32::try_from(exit_code).ok(),
        ));
    }

    if let Err(err) = clear_setup_error_report(codex_home) {
        log_note(
            &format!(
                "setup orchestrator: failed to clear setup_error.json after success: {err}"
            ),
            Some(&sandbox_dir(codex_home)),
        );
    }
    Ok(())
}

pub fn ensure_setup(request: SandboxSetupRequest<'_>) -> Result<()> {
    ensure_setup_with_overrides(request, SetupRootOverrides::default())
}

pub fn ensure_setup_with_overrides(
    request: SandboxSetupRequest<'_>,
    overrides: SetupRootOverrides,
) -> Result<()> {
    let permission_profile = request.permissions.permission_profile();
    if !request.permissions.is_enforceable_by_windows_sandbox() {
        return Err(failure(
            SetupErrorCode::UnsupportedPermissions,
            format!(
                "permission profile {permission_profile:?} is not enforceable by Windows sandbox"
            ),
        ));
    }

    let marker_path = setup_marker_path(request.codex_home);
    let marker_matches = std::fs::read(&marker_path)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<SetupMarker>(&bytes).ok())
        .is_some_and(|marker| {
            marker.version_matches()
                && marker
                    .request_mismatch_reason(
                        SandboxNetworkIdentity::from_permissions(
                            request.permissions,
                            request.proxy_enforced,
                        ),
                        &offline_proxy_settings_from_env(
                            request.env_map,
                            SandboxNetworkIdentity::from_permissions(
                                request.permissions,
                                request.proxy_enforced,
                            ),
                        ),
                    )
                    .is_none()
        });

    if marker_matches && sandbox_setup_is_complete(request.codex_home).unwrap_or(false) {
        return Ok(());
    }

    std::fs::create_dir_all(sandbox_dir(request.codex_home)).map_err(|err| {
        failure(
            SetupErrorCode::PrepareSandboxDirFailed,
            format!("failed to create sandbox dir: {err}"),
        )
    })?;
    std::fs::create_dir_all(sandbox_bin_dir(request.codex_home)).map_err(|err| {
        failure(
            SetupErrorCode::PrepareSandboxDirFailed,
            format!("failed to create sandbox bin dir: {err}"),
        )
    })?;

    let (read_roots, write_roots) = build_payload_roots(&request, &overrides);
    let deny_read_paths = build_payload_deny_read_paths(overrides.deny_read_paths);
    let deny_write_paths = build_payload_deny_write_paths(&request, overrides.deny_write_paths);
    let network_identity =
        SandboxNetworkIdentity::from_permissions(request.permissions, request.proxy_enforced);
    let offline_proxy_settings = offline_proxy_settings_from_env(request.env_map, network_identity);
    let needs_elevation = !is_elevated().unwrap_or(false);
    let payload = ElevationPayload {
        version: SETUP_VERSION,
        offline_username: OFFLINE_USERNAME.to_string(),
        online_username: ONLINE_USERNAME.to_string(),
        codex_home: request.codex_home.to_path_buf(),
        command_cwd: request.command_cwd.to_path_buf(),
        read_roots,
        write_roots,
        deny_read_paths,
        deny_write_paths,
        proxy_ports: offline_proxy_settings.proxy_ports,
        allow_local_binding: offline_proxy_settings.allow_local_binding,
        otel: None,
        real_user: std::env::var("USERNAME").unwrap_or_else(|_| "Administrators".to_string()),
        mode: SetupMode::Full,
        refresh_only: false,
    };

    run_setup_exe(&payload, needs_elevation, request.codex_home)
}

fn build_payload_roots(
    request: &SandboxSetupRequest<'_>,
    overrides: &SetupRootOverrides,
) -> (Vec<PathBuf>, Vec<PathBuf>) {
    let helper_roots = gather_helper_read_roots(request.codex_home);
    let mut read_roots = if let Some(mut roots) = overrides.read_roots.clone() {
        roots.extend(helper_roots);
        if overrides.read_roots_include_platform_defaults {
            roots.extend(
                WINDOWS_PLATFORM_DEFAULT_READ_ROOTS
                    .iter()
                    .map(PathBuf::from),
            );
        }
        canonical_existing(&roots)
    } else {
        gather_read_roots(
            request.command_cwd,
            request.permissions,
            request.env_map,
            request.codex_home,
        )
    };
    read_roots.sort_by_key(|path| canonical_path_key(path));
    read_roots.dedup_by(|a, b| canonical_path_key(a) == canonical_path_key(b));

    let write_roots = effective_write_roots_for_permissions(
        request.permissions,
        request.command_cwd,
        request.env_map,
        request.codex_home,
        overrides.write_roots.as_deref(),
    );

    (read_roots, write_roots)
}

fn build_payload_deny_read_paths(deny_read_paths_override: Option<Vec<PathBuf>>) -> Vec<PathBuf> {
    deny_read_paths_override.unwrap_or_default()
}

fn build_payload_deny_write_paths(
    request: &SandboxSetupRequest<'_>,
    deny_write_paths_override: Option<Vec<PathBuf>>,
) -> Vec<PathBuf> {
    let protected = request
        .permissions
        .writable_roots_for_cwd(request.command_cwd, request.env_map)
        .into_iter()
        .flat_map(|root| {
            let mut paths = Vec::new();
            let root_path = root.root;
            let dot_git = root_path.join(".git");
            if dot_git.exists() {
                paths.push(dot_git);
            }
            let dot_codex = root_path.join(".codex");
            if dot_codex.exists() {
                paths.push(dot_codex);
            }
            paths
        })
        .collect::<Vec<_>>();
    let mut merged = protected;
    if let Some(extra) = deny_write_paths_override {
        merged.extend(extra);
    }
    let mut dedup = HashSet::new();
    merged.retain(|path| dedup.insert(canonical_path_key(path)));
    merged
}

fn expand_user_profile_root(roots: Vec<PathBuf>) -> Vec<PathBuf> {
    if let Ok(user_profile) = std::env::var("USERPROFILE") {
        return expand_user_profile_root_for(roots, Path::new(&user_profile));
    }
    roots
}

fn expand_user_profile_root_for(roots: Vec<PathBuf>, user_profile: &Path) -> Vec<PathBuf> {
    let user_profile_key = canonical_path_key(user_profile);
    let mut out = Vec::new();
    for root in roots {
        if canonical_path_key(&root) == user_profile_key {
            out.extend(profile_read_roots(user_profile));
        } else {
            out.push(root);
        }
    }
    out
}

fn filter_user_profile_root(roots: Vec<PathBuf>) -> Vec<PathBuf> {
    if let Ok(user_profile) = std::env::var("USERPROFILE") {
        let user_profile_key = canonical_path_key(Path::new(&user_profile));
        return roots
            .into_iter()
            .filter(|root| canonical_path_key(root) != user_profile_key)
            .collect();
    }
    roots
}

fn filter_user_profile_root_exclusions(roots: Vec<PathBuf>) -> Vec<PathBuf> {
    let Ok(user_profile) = std::env::var("USERPROFILE") else {
        return roots;
    };
    let user_profile = PathBuf::from(user_profile);
    roots
        .into_iter()
        .filter(|root| !is_user_profile_root_exclusion(root, &user_profile))
        .collect()
}

fn is_user_profile_root_exclusion(path: &Path, user_profile: &Path) -> bool {
    let Ok(relative) = path.strip_prefix(user_profile) else {
        return false;
    };
    let Some(first_component) = relative.components().next() else {
        return false;
    };
    let std::path::Component::Normal(name) = first_component else {
        return false;
    };
    let name = name.to_string_lossy();
    USERPROFILE_ROOT_EXCLUSIONS
        .iter()
        .any(|excluded| name.eq_ignore_ascii_case(excluded))
}

fn filter_ssh_config_dependency_roots(roots: Vec<PathBuf>) -> Vec<PathBuf> {
    let Ok(user_profile) = std::env::var("USERPROFILE") else {
        return roots;
    };
    let user_profile = PathBuf::from(user_profile);
    let dependency_paths = ssh_config_dependency_paths(&user_profile);
    roots
        .into_iter()
        .filter(|root| !is_ssh_config_dependency_root(root, &user_profile, &dependency_paths))
        .collect()
}

fn is_ssh_config_dependency_root(
    path: &Path,
    user_profile: &Path,
    dependency_paths: &[PathBuf],
) -> bool {
    dependency_paths.iter().any(|dependency| {
        path == dependency || (path.starts_with(user_profile) && dependency.starts_with(path))
    })
}

fn filter_sensitive_write_roots(roots: Vec<PathBuf>, codex_home: &Path) -> Vec<PathBuf> {
    let forbidden = [
        canonical_path_key(codex_home),
        canonical_path_key(&sandbox_dir(codex_home)),
        canonical_path_key(&sandbox_bin_dir(codex_home)),
        canonical_path_key(&sandbox_secrets_dir(codex_home)),
    ];
    roots
        .into_iter()
        .filter(|root| {
            let key = canonical_path_key(root);
            !forbidden.contains(&key)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::SandboxNetworkIdentity;
    use super::canonical_existing;
    use super::effective_write_roots_for_permissions;
    use super::gather_full_read_roots_for_permissions;
    use super::gather_read_roots;
    use super::helper_bin_dir;
    use super::offline_proxy_settings_from_env;
    use super::proxy_ports_from_env;
    use super::quote_arg;
    use super::sandbox_bin_dir;
    use super::workspace_write_roots_remain_readable;
    use super::*;
    use codex_protocol::models::FilesystemAllowlistPolicy;
    use codex_protocol::models::NetworkAccess;
    use codex_protocol::models::PermissionProfile;
    use codex_utils_absolute_path::AbsolutePathBuf;
    use pretty_assertions::assert_eq;
    use std::collections::HashSet;
    use std::fs;
    use std::path::PathBuf;
    use tempfile::TempDir;

    fn workspace_roots_for(path: &Path) -> Vec<AbsolutePathBuf> {
        vec![AbsolutePathBuf::from_absolute_path(path).expect("absolute workspace root")]
    }

    fn workspace_write_profile(
        writable_roots: &[AbsolutePathBuf],
        exclude_tmpdir_env_var: bool,
        exclude_slash_tmp: bool,
    ) -> PermissionProfile {
        PermissionProfile::WorkspaceWrite {
            writable_roots: writable_roots.to_vec(),
            network_access: NetworkAccess::Restricted,
            exclude_tmpdir_env_var,
            exclude_slash_tmp,
        }
    }

    fn permissions_for(
        permission_profile: &PermissionProfile,
        workspace_roots: &[AbsolutePathBuf],
    ) -> ResolvedWindowsSandboxPermissions {
        ResolvedWindowsSandboxPermissions::try_from_permission_profile_for_workspace_roots(
            permission_profile,
            workspace_roots,
        )
        .expect("permissions")
    }

    fn canonical_windows_platform_default_roots() -> Vec<PathBuf> {
        WINDOWS_PLATFORM_DEFAULT_READ_ROOTS
            .iter()
            .map(PathBuf::from)
            .map(|path| canonicalize_path(&path).unwrap_or(path))
            .collect()
    }

    #[test]
    fn quote_arg_leaves_simple_args_unquoted() {
        assert_eq!(quote_arg("plain"), "plain");
    }

    #[test]
    fn quote_arg_quotes_and_escapes_spaces_and_quotes() {
        assert_eq!(quote_arg("two words"), "\"two words\"");
        assert_eq!(quote_arg("quote\"here"), "\"quote\\\"here\"");
        assert_eq!(quote_arg(r"trailing\\"), "\"trailing\\\\\"");
    }

    #[test]
    fn proxy_ports_from_env_extracts_only_loopback_ports() {
        let env_map = HashMap::from([
            ("HTTP_PROXY".to_string(), "http://127.0.0.1:8080".to_string()),
            (
                "HTTPS_PROXY".to_string(),
                "https://localhost:443/proxy".to_string(),
            ),
            ("ALL_PROXY".to_string(), "http://[::1]:9000".to_string()),
            (
                "WS_PROXY".to_string(),
                "ws://example.com:8123/ignored".to_string(),
            ),
            (
                "http_proxy".to_string(),
                "http://127.0.0.1:8080".to_string(),
            ),
        ]);

        assert_eq!(proxy_ports_from_env(&env_map), vec![443, 8080, 9000]);
    }

    #[test]
    fn offline_proxy_settings_from_env_ignores_online_identity() {
        let env_map = HashMap::from([
            ("HTTP_PROXY".to_string(), "http://127.0.0.1:8080".to_string()),
            (
                "CODEX_NETWORK_ALLOW_LOCAL_BINDING".to_string(),
                "1".to_string(),
            ),
        ]);

        assert_eq!(
            offline_proxy_settings_from_env(&env_map, SandboxNetworkIdentity::Online),
            OfflineProxySettings {
                proxy_ports: vec![],
                allow_local_binding: false,
            }
        );
    }

    #[test]
    fn offline_proxy_settings_from_env_extracts_loopback_ports_and_binding() {
        let env_map = HashMap::from([
            ("HTTP_PROXY".to_string(), "http://127.0.0.1:8080".to_string()),
            (
                "HTTPS_PROXY".to_string(),
                "https://localhost:443/proxy".to_string(),
            ),
            (
                "CODEX_NETWORK_ALLOW_LOCAL_BINDING".to_string(),
                "1".to_string(),
            ),
        ]);

        assert_eq!(
            offline_proxy_settings_from_env(&env_map, SandboxNetworkIdentity::Offline),
            OfflineProxySettings {
                proxy_ports: vec![443, 8080],
                allow_local_binding: true,
            }
        );
    }

    #[test]
    fn canonical_existing_filters_missing_paths() {
        let tmp = TempDir::new().expect("tempdir");
        let existing = tmp.path().join("existing");
        let missing = tmp.path().join("missing");
        fs::create_dir_all(&existing).expect("create existing dir");

        let roots = canonical_existing(&[existing.clone(), missing]);

        assert_eq!(
            roots,
            vec![dunce::canonicalize(existing).expect("canonical existing dir")]
        );
    }

    #[test]
    fn profile_read_roots_excludes_sensitive_config_dirs() {
        let tmp = TempDir::new().expect("tempdir");
        let user_profile = tmp.path();
        let allowed_dir = user_profile.join("Documents");
        let allowed_file = user_profile.join("settings.json");
        let excluded_dir = user_profile.join(".ssh");
        let excluded_tsh = user_profile.join(".tsh");
        let excluded_case_variant = user_profile.join(".AWS");

        fs::create_dir_all(&allowed_dir).expect("create allowed dir");
        fs::write(&allowed_file, "safe").expect("create allowed file");
        fs::create_dir_all(&excluded_dir).expect("create excluded dir");
        fs::create_dir_all(&excluded_tsh).expect("create excluded tsh dir");
        fs::create_dir_all(&excluded_case_variant).expect("create excluded case variant");

        let roots = profile_read_roots(user_profile);
        let actual: HashSet<PathBuf> = roots.into_iter().collect();
        let expected: HashSet<PathBuf> = [allowed_dir, allowed_file].into_iter().collect();

        assert_eq!(expected, actual);
    }

    #[test]
    fn profile_read_roots_falls_back_to_profile_root_when_enumeration_fails() {
        let tmp = TempDir::new().expect("tempdir");
        let missing_profile = tmp.path().join("missing-user-profile");

        let roots = profile_read_roots(&missing_profile);

        assert_eq!(vec![missing_profile], roots);
    }

    #[test]
    fn is_user_profile_root_exclusion_blocks_configured_children() {
        let tmp = TempDir::new().expect("tempdir");
        let user_profile = tmp.path().join("user-profile");
        let documents = user_profile.join("Documents");
        let app_data = user_profile.join("AppData");
        let ssh_child = user_profile.join(".ssh").join("config");
        let tsh_child = user_profile.join(".tsh").join("keys");
        let other_root = tmp.path().join("other-root");
        fs::create_dir_all(&documents).expect("create documents");
        fs::create_dir_all(&app_data).expect("create app data");
        fs::create_dir_all(&ssh_child).expect("create ssh child");
        fs::create_dir_all(&tsh_child).expect("create tsh child");
        fs::create_dir_all(&other_root).expect("create other root");

        assert!(!super::is_user_profile_root_exclusion(
            &documents,
            &user_profile
        ));
        assert!(!super::is_user_profile_root_exclusion(
            &app_data,
            &user_profile
        ));
        assert!(super::is_user_profile_root_exclusion(
            &ssh_child,
            &user_profile
        ));
        assert!(super::is_user_profile_root_exclusion(
            &tsh_child,
            &user_profile
        ));
        assert!(!super::is_user_profile_root_exclusion(
            &other_root,
            &user_profile
        ));
    }

    #[test]
    fn is_ssh_config_dependency_root_blocks_config_dependencies() {
        let tmp = TempDir::new().expect("tempdir");
        let user_profile = tmp.path().join("user-profile");
        let documents = user_profile.join("Documents");
        let ssh_dir = user_profile.join(".ssh");
        let key_dir = user_profile.join(".keys");
        let include_dir = user_profile.join(".included");
        let other_root = tmp.path().join("other-root");
        fs::create_dir_all(&documents).expect("create documents");
        fs::create_dir_all(&ssh_dir).expect("create .ssh");
        fs::create_dir_all(&key_dir).expect("create key dir");
        fs::create_dir_all(&include_dir).expect("create include dir");
        fs::create_dir_all(&other_root).expect("create other root");
        fs::write(
            ssh_dir.join("config"),
            "IdentityFile ~/.keys/id_ed25519\nInclude ~/.included/config\n",
        )
        .expect("write ssh config");
        fs::write(key_dir.join("id_ed25519"), "").expect("write key");
        fs::write(include_dir.join("config"), "User git\n").expect("write included config");

        let dependency_paths = super::ssh_config_dependency_paths(&user_profile);

        assert!(!super::is_ssh_config_dependency_root(
            &documents,
            &user_profile,
            &dependency_paths
        ));
        assert!(super::is_ssh_config_dependency_root(
            &key_dir,
            &user_profile,
            &dependency_paths
        ));
        assert!(super::is_ssh_config_dependency_root(
            &include_dir.join("config"),
            &user_profile,
            &dependency_paths
        ));
        assert!(!super::is_ssh_config_dependency_root(
            &other_root,
            &user_profile,
            &dependency_paths
        ));
    }

    #[test]
    fn expand_user_profile_root_for_replaces_profile_root_with_children() {
        let tmp = TempDir::new().expect("tempdir");
        let user_profile = tmp.path().join("user-profile");
        let documents = user_profile.join("Documents");
        let excluded = user_profile.join(".local");
        let other_root = tmp.path().join("other-root");
        fs::create_dir_all(&documents).expect("create documents");
        fs::create_dir_all(&excluded).expect("create excluded dir");
        fs::create_dir_all(&other_root).expect("create other root");

        let roots = super::expand_user_profile_root_for(
            vec![user_profile.clone(), other_root.clone()],
            &user_profile,
        );
        let actual: HashSet<PathBuf> = roots.into_iter().collect();
        let expected: HashSet<PathBuf> = [documents, excluded, other_root].into_iter().collect();

        assert_eq!(expected, actual);
    }

    #[test]
    fn expanded_write_roots_still_drop_protected_codex_home() {
        let tmp = TempDir::new().expect("tempdir");
        let user_profile = tmp.path().join("user-profile");
        let codex_home = user_profile.join("CodexHome");
        let documents = user_profile.join("Documents");
        fs::create_dir_all(&codex_home).expect("create codex home");
        fs::create_dir_all(&documents).expect("create documents");

        let mut roots =
            super::expand_user_profile_root_for(vec![user_profile.clone()], &user_profile);
        let user_profile_key = super::canonical_path_key(&user_profile);
        roots.retain(|root| super::canonical_path_key(root) != user_profile_key);
        roots.retain(|root| !super::is_user_profile_root_exclusion(root, &user_profile));
        let roots = super::filter_sensitive_write_roots(roots, &codex_home);

        assert_eq!(vec![documents], roots);
    }

    #[test]
    fn gather_read_roots_includes_helper_bin_dir() {
        let tmp = TempDir::new().expect("tempdir");
        let codex_home = tmp.path().join("codex-home");
        let command_cwd = tmp.path().join("workspace");
        fs::create_dir_all(&command_cwd).expect("create workspace");
        let permission_profile = PermissionProfile::read_only();
        let workspace_roots = workspace_roots_for(command_cwd.as_path());
        let permissions = permissions_for(&permission_profile, workspace_roots.as_slice());

        let roots = gather_read_roots(&command_cwd, &permissions, &HashMap::new(), &codex_home);
        let expected =
            dunce::canonicalize(helper_bin_dir(&codex_home)).expect("canonical helper dir");

        assert!(roots.contains(&expected));
    }

    #[test]
    fn workspace_write_roots_remain_readable() {
        let tmp = TempDir::new().expect("tempdir");
        let codex_home = tmp.path().join("codex-home");
        let command_cwd = tmp.path().join("workspace");
        let writable_root = tmp.path().join("extra-write-root");
        fs::create_dir_all(&command_cwd).expect("create workspace");
        fs::create_dir_all(&writable_root).expect("create writable root");
        let writable_roots = vec![
            AbsolutePathBuf::from_absolute_path(&writable_root).expect("absolute writable root"),
        ];
        let permission_profile = workspace_write_profile(
            &writable_roots,
            /*exclude_tmpdir_env_var*/ true,
            /*exclude_slash_tmp*/ true,
        );
        let workspace_roots = workspace_roots_for(command_cwd.as_path());
        let permissions = permissions_for(&permission_profile, workspace_roots.as_slice());

        let roots = gather_read_roots(&command_cwd, &permissions, &HashMap::new(), &codex_home);
        let expected_writable =
            dunce::canonicalize(&writable_root).expect("canonical writable root");

        assert!(roots.contains(&expected_writable));
    }

    #[test]
    fn build_payload_roots_preserves_helper_roots_when_read_override_is_provided() {
        let tmp = TempDir::new().expect("tempdir");
        let codex_home = tmp.path().join("codex-home");
        let workspace_root = tmp.path().join("workspace-root");
        let command_cwd = tmp.path().join("workspace");
        let readable_root = tmp.path().join("docs");
        fs::create_dir_all(&workspace_root).expect("create workspace root");
        fs::create_dir_all(&command_cwd).expect("create workspace");
        fs::create_dir_all(&readable_root).expect("create readable root");
        let permission_profile = PermissionProfile::read_only();
        let workspace_roots = workspace_roots_for(workspace_root.as_path());
        let permissions = permissions_for(&permission_profile, workspace_roots.as_slice());

        let (read_roots, write_roots) = build_payload_roots(
            &super::SandboxSetupRequest {
                permissions: &permissions,
                command_cwd: &command_cwd,
                env_map: &HashMap::new(),
                codex_home: &codex_home,
                proxy_enforced: false,
            },
            &super::SetupRootOverrides {
                read_roots: Some(vec![readable_root.clone()]),
                read_roots_include_platform_defaults: true,
                write_roots: None,
                deny_read_paths: None,
                deny_write_paths: None,
            },
        );
        let expected_helper =
            dunce::canonicalize(helper_bin_dir(&codex_home)).expect("canonical helper dir");
        let expected_cwd = dunce::canonicalize(&command_cwd).expect("canonical workspace");
        let expected_readable =
            dunce::canonicalize(&readable_root).expect("canonical readable root");

        assert_eq!(write_roots, Vec::<PathBuf>::new());
        assert!(read_roots.contains(&expected_helper));
        assert!(!read_roots.contains(&expected_cwd));
        assert!(read_roots.contains(&expected_readable));
        assert!(
            canonical_windows_platform_default_roots()
                .into_iter()
                .all(|path| read_roots.contains(&path))
        );
    }

    #[test]
    fn build_payload_roots_replaces_full_read_policy_when_read_override_is_provided() {
        let tmp = TempDir::new().expect("tempdir");
        let codex_home = tmp.path().join("codex-home");
        let workspace_root = tmp.path().join("workspace-root");
        let command_cwd = tmp.path().join("workspace");
        let readable_root = tmp.path().join("docs");
        fs::create_dir_all(&workspace_root).expect("create workspace root");
        fs::create_dir_all(&command_cwd).expect("create workspace");
        fs::create_dir_all(&readable_root).expect("create readable root");
        let permission_profile = PermissionProfile::read_only();
        let workspace_roots = workspace_roots_for(workspace_root.as_path());
        let permissions = permissions_for(&permission_profile, workspace_roots.as_slice());

        let (read_roots, write_roots) = build_payload_roots(
            &super::SandboxSetupRequest {
                permissions: &permissions,
                command_cwd: &command_cwd,
                env_map: &HashMap::new(),
                codex_home: &codex_home,
                proxy_enforced: false,
            },
            &super::SetupRootOverrides {
                read_roots: Some(vec![readable_root.clone()]),
                read_roots_include_platform_defaults: false,
                write_roots: None,
                deny_read_paths: None,
                deny_write_paths: None,
            },
        );
        let expected_helper =
            dunce::canonicalize(helper_bin_dir(&codex_home)).expect("canonical helper dir");
        let expected_cwd = dunce::canonicalize(&command_cwd).expect("canonical workspace");
        let expected_readable =
            dunce::canonicalize(&readable_root).expect("canonical readable root");

        assert_eq!(write_roots, Vec::<PathBuf>::new());
        assert!(read_roots.contains(&expected_helper));
        assert!(!read_roots.contains(&expected_cwd));
        assert!(read_roots.contains(&expected_readable));
        assert!(
            canonical_windows_platform_default_roots()
                .into_iter()
                .all(|path| !read_roots.contains(&path))
        );
    }

    #[test]
    fn effective_write_roots_match_payload_filtering_for_overrides() {
        let tmp = TempDir::new().expect("tempdir");
        let codex_home = tmp.path().join("codex-home");
        let command_cwd = tmp.path().join("workspace");
        let extra_root = tmp.path().join("extra-root");
        let sandbox_root = super::sandbox_dir(&codex_home);
        fs::create_dir_all(&codex_home).expect("create codex home");
        fs::create_dir_all(&command_cwd).expect("create workspace");
        fs::create_dir_all(&extra_root).expect("create extra root");
        fs::create_dir_all(&sandbox_root).expect("create sandbox root");
        let permission_profile = workspace_write_profile(
            &[],
            /*exclude_tmpdir_env_var*/ true,
            /*exclude_slash_tmp*/ true,
        );
        let workspace_roots = workspace_roots_for(command_cwd.as_path());
        let permissions = permissions_for(&permission_profile, workspace_roots.as_slice());
        let override_roots = vec![
            command_cwd.clone(),
            extra_root.clone(),
            codex_home.clone(),
            sandbox_root.clone(),
        ];
        let request = super::SandboxSetupRequest {
            permissions: &permissions,
            command_cwd: &command_cwd,
            env_map: &HashMap::new(),
            codex_home: &codex_home,
            proxy_enforced: false,
        };
        let overrides = super::SetupRootOverrides {
            read_roots: None,
            read_roots_include_platform_defaults: false,
            write_roots: Some(override_roots.clone()),
            deny_read_paths: None,
            deny_write_paths: None,
        };

        let effective_write_roots = super::effective_write_roots_for_setup(
            &permissions,
            &command_cwd,
            &HashMap::new(),
            &codex_home,
            Some(&override_roots),
        );
        let (_read_roots, payload_write_roots) = build_payload_roots(&request, &overrides);

        let expected_workspace = dunce::canonicalize(&command_cwd).expect("canonical workspace");
        let expected_extra = dunce::canonicalize(&extra_root).expect("canonical extra root");
        let forbidden_codex_home = dunce::canonicalize(&codex_home).expect("canonical codex home");
        let forbidden_sandbox = dunce::canonicalize(&sandbox_root).expect("canonical sandbox root");
        assert_eq!(effective_write_roots, payload_write_roots);
        assert!(effective_write_roots.contains(&expected_workspace));
        assert!(effective_write_roots.contains(&expected_extra));
        assert!(!effective_write_roots.contains(&forbidden_codex_home));
        assert!(!effective_write_roots.contains(&forbidden_sandbox));
    }

    #[test]
    fn effective_write_roots_use_runtime_workspace_roots_for_workspace_root() {
        let tmp = TempDir::new().expect("tempdir");
        let codex_home = tmp.path().join("codex-home");
        let workspace_root = tmp.path().join("workspace");
        let command_cwd = workspace_root.join("subdir");
        fs::create_dir_all(&codex_home).expect("create codex home");
        fs::create_dir_all(&command_cwd).expect("create command cwd");

        let permission_profile = workspace_write_profile(
            &[],
            /*exclude_tmpdir_env_var*/ true,
            /*exclude_slash_tmp*/ true,
        );
        let workspace_roots = workspace_roots_for(workspace_root.as_path());
        let permissions = permissions_for(&permission_profile, workspace_roots.as_slice());

        let effective_write_roots = super::effective_write_roots_for_setup(
            &permissions,
            &command_cwd,
            &HashMap::new(),
            &codex_home,
            /*write_roots_override*/ None,
        );

        assert_eq!(
            effective_write_roots,
            vec![dunce::canonicalize(&workspace_root).expect("canonical workspace root")]
        );
    }

    #[test]
    fn payload_deny_write_paths_merge_explicit_and_protected_children() {
        let tmp = TempDir::new().expect("tempdir");
        let codex_home = tmp.path().join("codex-home");
        let command_cwd = tmp.path().join("workspace");
        let extra_write_root = tmp.path().join("extra-write-root");
        let command_git = command_cwd.join(".git");
        let extra_codex = extra_write_root.join(".codex");
        let explicit_deny = tmp.path().join("explicit-deny");
        fs::create_dir_all(&command_git).expect("create command .git");
        fs::create_dir_all(&extra_codex).expect("create extra .codex");
        let writable_roots = vec![
            AbsolutePathBuf::from_absolute_path(&extra_write_root).expect("absolute writable root"),
        ];
        let permission_profile = workspace_write_profile(
            &writable_roots,
            /*exclude_tmpdir_env_var*/ true,
            /*exclude_slash_tmp*/ true,
        );
        let workspace_roots = workspace_roots_for(command_cwd.as_path());
        let permissions = permissions_for(&permission_profile, workspace_roots.as_slice());
        let request = super::SandboxSetupRequest {
            permissions: &permissions,
            command_cwd: &command_cwd,
            env_map: &HashMap::new(),
            codex_home: &codex_home,
            proxy_enforced: false,
        };

        let deny_write_paths =
            super::build_payload_deny_write_paths(&request, Some(vec![explicit_deny.clone()]));

        assert_eq!(
            [
                dunce::canonicalize(&command_git).expect("canonical command .git"),
                dunce::canonicalize(&extra_codex).expect("canonical extra .codex"),
                explicit_deny,
            ]
            .into_iter()
            .collect::<HashSet<PathBuf>>(),
            deny_write_paths.into_iter().collect()
        );
    }

    #[test]
    fn full_read_roots_preserve_legacy_platform_defaults() {
        let tmp = TempDir::new().expect("tempdir");
        let codex_home = tmp.path().join("codex-home");
        let command_cwd = tmp.path().join("workspace");
        fs::create_dir_all(&command_cwd).expect("create workspace");
        let permission_profile = PermissionProfile::read_only();
        let workspace_roots = workspace_roots_for(command_cwd.as_path());
        let permissions = permissions_for(&permission_profile, workspace_roots.as_slice());

        let roots = gather_full_read_roots_for_permissions(
            &command_cwd,
            &permissions,
            &HashMap::new(),
            &codex_home,
        );

        assert!(
            canonical_windows_platform_default_roots()
                .into_iter()
                .all(|path| roots.contains(&path))
        );
    }

    #[test]
    fn build_payload_deny_read_paths_preserves_explicit_paths() {
        let tmp = TempDir::new().expect("tempdir");
        let existing = tmp.path().join("secret.env");
        let missing = tmp.path().join("future.env");
        fs::write(&existing, "secret").expect("write existing");

        assert_eq!(
            super::build_payload_deny_read_paths(Some(vec![existing.clone(), missing.clone()])),
            vec![existing, missing]
        );
    }
}

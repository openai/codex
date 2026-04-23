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
use crate::allow::compute_allow_paths;
use crate::helper_materialization::helper_bin_dir;
use crate::logging::log_note;
use crate::path_normalization::canonical_path_key;
use crate::policy::SandboxPolicy;
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
    pub policy: &'a SandboxPolicy,
    pub policy_cwd: &'a Path,
    pub command_cwd: &'a Path,
    pub env_map: &'a HashMap<String, String>,
    pub codex_home: &'a Path,
    pub proxy_enforced: bool,
}

#[derive(Default)]
pub struct SetupRootOverrides {
    pub read_roots: Option<Vec<PathBuf>>,
    pub write_roots: Option<Vec<PathBuf>>,
    pub deny_write_paths: Option<Vec<PathBuf>>,
}

pub fn run_setup_refresh(
    policy: &SandboxPolicy,
    policy_cwd: &Path,
    command_cwd: &Path,
    env_map: &HashMap<String, String>,
    codex_home: &Path,
    proxy_enforced: bool,
) -> Result<()> {
    run_setup_refresh_inner(
        SandboxSetupRequest {
            policy,
            policy_cwd,
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
    policy: &SandboxPolicy,
    policy_cwd: &Path,
    command_cwd: &Path,
    env_map: &HashMap<String, String>,
    codex_home: &Path,
    extra_read_roots: Vec<PathBuf>,
    proxy_enforced: bool,
) -> Result<()> {
    let mut read_roots = gather_read_roots(command_cwd, policy, codex_home);
    read_roots.extend(extra_read_roots);
    run_setup_refresh_inner(
        SandboxSetupRequest {
            policy,
            policy_cwd,
            command_cwd,
            env_map,
            codex_home,
            proxy_enforced,
        },
        SetupRootOverrides {
            read_roots: Some(read_roots),
            write_roots: Some(Vec::new()),
            deny_write_paths: None,
        },
    )
}

fn run_setup_refresh_inner(
    request: SandboxSetupRequest<'_>,
    overrides: SetupRootOverrides,
) -> Result<()> {
    // Skip in danger-full-access.
    if matches!(
        request.policy,
        SandboxPolicy::DangerFullAccess | SandboxPolicy::ExternalSandbox { .. }
    ) {
        return Ok(());
    }
    let (read_roots, write_roots) = build_payload_roots(&request, &overrides);
    let deny_write_paths = build_payload_deny_write_paths(&request, overrides.deny_write_paths);
    let network_identity =
        SandboxNetworkIdentity::from_policy(request.policy, request.proxy_enforced);
    let offline_proxy_settings = offline_proxy_settings_from_env(request.env_map, network_identity);
    let payload = ElevationPayload {
        version: SETUP_VERSION,
        offline_username: OFFLINE_USERNAME.to_string(),
        online_username: ONLINE_USERNAME.to_string(),
        codex_home: request.codex_home.to_path_buf(),
        command_cwd: request.command_cwd.to_path_buf(),
        read_roots,
        write_roots,
        deny_write_paths,
        proxy_ports: offline_proxy_settings.proxy_ports,
        allow_local_binding: offline_proxy_settings.allow_local_binding,
        real_user: std::env::var("USERNAME").unwrap_or_else(|_| "Administrators".to_string()),
        refresh_only: true,
    };
    let json = serde_json::to_vec(&payload)?;
    let b64 = BASE64_STANDARD.encode(json);
    let exe = find_setup_exe();
    // Refresh should never request elevation; ensure verb isn't set and we don't trigger UAC.
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
        Some(&sandbox_dir(request.codex_home)),
    );
    let status = cmd
        .status()
        .map_err(|e| {
            log_note(
                &format!("setup refresh: failed to spawn {}: {e}", exe.display()),
                Some(&sandbox_dir(request.codex_home)),
            );
            e
        })
        .context("spawn setup refresh")?;
    if !status.success() {
        log_note(
            &format!("setup refresh: exited with status {status:?}"),
            Some(&sandbox_dir(request.codex_home)),
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

fn gather_legacy_full_read_roots(
    command_cwd: &Path,
    policy: &SandboxPolicy,
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
    if let SandboxPolicy::WorkspaceWrite { writable_roots, .. } = policy {
        for root in writable_roots {
            roots.push(root.to_path_buf());
        }
    }
    canonical_existing(&roots)
}

fn gather_restricted_read_roots(
    command_cwd: &Path,
    policy: &SandboxPolicy,
    codex_home: &Path,
) -> Vec<PathBuf> {
    let mut roots = gather_helper_read_roots(codex_home);
    if policy.include_platform_defaults() {
        roots.extend(
            WINDOWS_PLATFORM_DEFAULT_READ_ROOTS
                .iter()
                .map(PathBuf::from),
        );
    }
    roots.extend(
        policy
            .get_readable_roots_with_cwd(command_cwd)
            .into_iter()
            .map(|path| path.to_path_buf()),
    );
    canonical_existing(&roots)
}

pub(crate) fn gather_read_roots(
    command_cwd: &Path,
    policy: &SandboxPolicy,
    codex_home: &Path,
) -> Vec<PathBuf> {
    if policy.has_full_disk_read_access() {
        gather_legacy_full_read_roots(command_cwd, policy, codex_home)
    } else {
        gather_restricted_read_roots(command_cwd, policy, codex_home)
    }
}

pub(crate) fn gather_write_roots(
    policy: &SandboxPolicy,
    policy_cwd: &Path,
    command_cwd: &Path,
    env_map: &HashMap<String, String>,
) -> Vec<PathBuf> {
    let mut roots: Vec<PathBuf> = Vec::new();
    // Always include the command CWD for workspace-write.
    if matches!(policy, SandboxPolicy::WorkspaceWrite { .. }) {
        roots.push(command_cwd.to_path_buf());
    }
    let AllowDenyPaths { allow, .. } =
        compute_allow_paths(policy, policy_cwd, command_cwd, env_map);
    roots.extend(allow);
    let mut dedup: HashSet<PathBuf> = HashSet::new();
    let mut out: Vec<PathBuf> = Vec::new();
    for r in canonical_existing(&roots) {
        if dedup.insert(r.clone()) {
            out.push(r);
        }
    }
    out
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
    deny_write_paths: Vec<PathBuf>,
    proxy_ports: Vec<u16>,
    #[serde(default)]
    allow_local_binding: bool,
    real_user: String,
    #[serde(default)]
    refresh_only: bool,
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
    pub(crate) fn from_policy(policy: &SandboxPolicy, proxy_enforced: bool) -> Self {
        if proxy_enforced || !policy.has_full_network_access() {
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
        && let Some(dir) = exe.parent()
    {
        let candidate = dir.join("codex-windows-sandbox-setup.exe");
        if candidate.exists() {
            return candidate;
        }

        // Standalone installs keep Windows helper binaries under
        // `codex-resources/` next to `codex.exe`, so elevation needs to probe
        // that sibling folder before falling back to PATH.
        let resource_candidate = dir
            .join("codex-resources")
            .join("codex-windows-sandbox-setup.exe");
        if resource_candidate.exists() {
            return resource_candidate;
        }
    }
    PathBuf::from("codex-windows-sandbox-setup.exe")
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

    let exe_w = crate::winutil::to_wide(&exe);
    let params = quote_arg(&payload_b64);
    let params_w = crate::winutil::to_wide(params);
    let verb_w = crate::winutil::to_wide("runas");
    let mut sei: SHELLEXECUTEINFOW = unsafe { std::mem::zeroed() };
    sei.cbSize = std::mem::size_of::<SHELLEXECUTEINFOW>() as u32;
    sei.fMask = SEE_MASK_NOCLOSEPROCESS;
    sei.lpVerb = verb_w.as_ptr();
    sei.lpFile = exe_w.as_ptr();
    sei.lpParameters = params_w.as_ptr();
    // Hide the window for the elevated helper.
    sei.nShow = 0; // SW_HIDE
    let ok = unsafe { ShellExecuteExW(&mut sei) };
    if ok == 0 || sei.hProcess == 0 {
        let last_error = unsafe { GetLastError() };
        let code = if last_error == ERROR_CANCELLED {
            SetupErrorCode::OrchestratorHelperLaunchCanceled
        } else {
            SetupErrorCode::OrchestratorHelperLaunchFailed
        };
        return Err(failure(
            code,
            format!("ShellExecuteExW failed to launch setup helper: {last_error}"),
        ));
    }
    unsafe {
        WaitForSingleObject(sei.hProcess, INFINITE);
        let mut code: u32 = 1;
        GetExitCodeProcess(sei.hProcess, &mut code);
        CloseHandle(sei.hProcess);
        if code != 0 {
            return Err(report_helper_failure(
                codex_home,
                cleared_report,
                Some(code as i32),
            ));
        }
    }
    if let Err(err) = clear_setup_error_report(codex_home) {
        log_note(
            &format!("setup orchestrator: failed to clear setup_error.json after success: {err}"),
            Some(&sandbox_dir(codex_home)),
        );
    }
    Ok(())
}

pub fn run_elevated_setup(
    request: SandboxSetupRequest<'_>,
    overrides: SetupRootOverrides,
) -> Result<()> {
    // Ensure the shared sandbox directory exists before we send it to the elevated helper.
    let sbx_dir = sandbox_dir(request.codex_home);
    std::fs::create_dir_all(&sbx_dir).map_err(|err| {
        failure(
            SetupErrorCode::OrchestratorSandboxDirCreateFailed,
            format!("failed to create sandbox dir {}: {err}", sbx_dir.display()),
        )
    })?;
    let (read_roots, write_roots) = build_payload_roots(&request, &overrides);
    let deny_write_paths = build_payload_deny_write_paths(&request, overrides.deny_write_paths);
    let network_identity =
        SandboxNetworkIdentity::from_policy(request.policy, request.proxy_enforced);
    let offline_proxy_settings = offline_proxy_settings_from_env(request.env_map, network_identity);
    let payload = ElevationPayload {
        version: SETUP_VERSION,
        offline_username: OFFLINE_USERNAME.to_string(),
        online_username: ONLINE_USERNAME.to_string(),
        codex_home: request.codex_home.to_path_buf(),
        command_cwd: request.command_cwd.to_path_buf(),
        read_roots,
        write_roots,
        deny_write_paths,
        proxy_ports: offline_proxy_settings.proxy_ports,
        allow_local_binding: offline_proxy_settings.allow_local_binding,
        real_user: std::env::var("USERNAME").unwrap_or_else(|_| "Administrators".to_string()),
        refresh_only: false,
    };
    let needs_elevation = !is_elevated().map_err(|err| {
        failure(
            SetupErrorCode::OrchestratorElevationCheckFailed,
            format!("failed to determine elevation state: {err}"),
        )
    })?;
    run_setup_exe(&payload, needs_elevation, request.codex_home)
}

fn build_payload_roots(
    request: &SandboxSetupRequest<'_>,
    overrides: &SetupRootOverrides,
) -> (Vec<PathBuf>, Vec<PathBuf>) {
    let write_roots = if let Some(roots) = overrides.write_roots.as_deref() {
        canonical_existing(roots)
    } else {
        gather_write_roots(
            request.policy,
            request.policy_cwd,
            request.command_cwd,
            request.env_map,
        )
    };
    let write_roots = expand_user_profile_root(write_roots);
    let write_roots = filter_user_profile_root(write_roots);
    let write_roots = filter_user_profile_root_exclusions(write_roots);
    let write_roots = filter_ssh_config_dependency_roots(write_roots);
    let write_roots = filter_sensitive_write_roots(write_roots, request.codex_home);
    let mut read_roots = if let Some(roots) = overrides.read_roots.as_deref() {
        // An explicit override is the split policy's complete readable set. Keep only the
        // helper/platform roots the elevated setup needs; do not re-add legacy cwd/full-read roots.
        let mut read_roots = gather_helper_read_roots(request.codex_home);
        if request.policy.include_platform_defaults() {
            read_roots.extend(
                WINDOWS_PLATFORM_DEFAULT_READ_ROOTS
                    .iter()
                    .map(PathBuf::from),
            );
        }
        read_roots.extend(roots.iter().cloned());
        canonical_existing(&read_roots)
    } else {
        gather_read_roots(request.command_cwd, request.policy, request.codex_home)
    };
    read_roots = expand_user_profile_root(read_roots);
    read_roots = filter_user_profile_root(read_roots);
    read_roots = filter_user_profile_root_exclusions(read_roots);
    read_roots = filter_ssh_config_dependency_roots(read_roots);
    let write_root_set: HashSet<PathBuf> = write_roots.iter().cloned().collect();
    read_roots.retain(|root| !write_root_set.contains(root));
    (read_roots, write_roots)
}

fn build_payload_deny_write_paths(
    request: &SandboxSetupRequest<'_>,
    explicit_deny_write_paths: Option<Vec<PathBuf>>,
) -> Vec<PathBuf> {
    let allow_deny_paths: AllowDenyPaths = compute_allow_paths(
        request.policy,
        request.policy_cwd,
        request.command_cwd,
        request.env_map,
    );
    let mut deny_write_paths: Vec<PathBuf> = explicit_deny_write_paths
        .unwrap_or_default()
        .into_iter()
        .map(|path| {
            if path.exists() {
                dunce::canonicalize(&path).unwrap_or(path)
            } else {
                path
            }
        })
        .collect();
    deny_write_paths.extend(allow_deny_paths.deny);
    deny_write_paths
}

fn expand_user_profile_root(roots: Vec<PathBuf>) -> Vec<PathBuf> {
    let Ok(user_profile) = std::env::var("USERPROFILE") else {
        return roots;
    };
    expand_user_profile_root_for(roots, Path::new(&user_profile))
}

fn expand_user_profile_root_for(roots: Vec<PathBuf>, user_profile: &Path) -> Vec<PathBuf> {
    let user_profile_key = canonical_path_key(user_profile);
    let mut expanded = Vec::new();
    for root in roots {
        if canonical_path_key(&root) == user_profile_key {
            expanded.extend(profile_read_roots(user_profile));
        } else {
            expanded.push(root);
        }
    }

    expanded.sort_by_key(|root| canonical_path_key(root));
    expanded.dedup_by(|a, b| canonical_path_key(a.as_path()) == canonical_path_key(b.as_path()));
    expanded
}

fn filter_user_profile_root(mut roots: Vec<PathBuf>) -> Vec<PathBuf> {
    let Ok(user_profile) = std::env::var("USERPROFILE") else {
        return roots;
    };
    let user_profile_key = canonical_path_key(Path::new(&user_profile));
    roots.retain(|root| canonical_path_key(root) != user_profile_key);
    roots
}

fn filter_user_profile_root_exclusions(mut roots: Vec<PathBuf>) -> Vec<PathBuf> {
    let Ok(user_profile) = std::env::var("USERPROFILE") else {
        return roots;
    };
    let user_profile = Path::new(&user_profile);
    roots.retain(|root| !is_user_profile_root_exclusion(root, user_profile));
    roots
}

fn is_user_profile_root_exclusion(root: &Path, user_profile: &Path) -> bool {
    let root_key = canonical_path_key(root);
    let profile_key = canonical_path_key(user_profile);
    let profile_prefix = format!("{}/", profile_key.trim_end_matches('/'));
    let Some(relative_key) = root_key.strip_prefix(&profile_prefix) else {
        return false;
    };
    let Some(child_name) = relative_key
        .split('/')
        .next()
        .filter(|name| !name.is_empty())
    else {
        return false;
    };

    USERPROFILE_ROOT_EXCLUSIONS
        .iter()
        .any(|excluded| child_name.eq_ignore_ascii_case(excluded))
}

fn filter_ssh_config_dependency_roots(mut roots: Vec<PathBuf>) -> Vec<PathBuf> {
    let Ok(user_profile) = std::env::var("USERPROFILE") else {
        return roots;
    };
    let user_profile = Path::new(&user_profile);
    let dependency_paths = ssh_config_dependency_paths(user_profile);
    roots.retain(|root| !is_ssh_config_dependency_root(root, user_profile, &dependency_paths));
    roots
}

fn is_ssh_config_dependency_root(
    root: &Path,
    user_profile: &Path,
    dependency_paths: &[PathBuf],
) -> bool {
    let Some(child_name) = user_profile_child_name(root, user_profile) else {
        return false;
    };

    dependency_paths.iter().any(|path| {
        user_profile_child_name(path, user_profile)
            .is_some_and(|dependency_child| child_name.eq_ignore_ascii_case(&dependency_child))
    })
}

fn user_profile_child_name(path: &Path, user_profile: &Path) -> Option<String> {
    let root_key = canonical_path_key(path);
    let profile_key = canonical_path_key(user_profile);
    let profile_prefix = format!("{}/", profile_key.trim_end_matches('/'));
    let relative_key = root_key.strip_prefix(&profile_prefix)?;
    relative_key
        .split('/')
        .next()
        .filter(|name| !name.is_empty())
        .map(str::to_string)
}

fn filter_sensitive_write_roots(mut roots: Vec<PathBuf>, codex_home: &Path) -> Vec<PathBuf> {
    // Never grant capability write access to CODEX_HOME or anything under CODEX_HOME/.sandbox,
    // CODEX_HOME/.sandbox-bin, or CODEX_HOME/.sandbox-secrets. These locations contain sandbox
    // control/state and helper binaries and must remain tamper-resistant.
    let codex_home_key = canonical_path_key(codex_home);
    let sbx_dir_key = canonical_path_key(&sandbox_dir(codex_home));
    let sbx_dir_prefix = format!("{}/", sbx_dir_key.trim_end_matches('/'));
    let sbx_bin_dir_key = canonical_path_key(&sandbox_bin_dir(codex_home));
    let sbx_bin_dir_prefix = format!("{}/", sbx_bin_dir_key.trim_end_matches('/'));
    let secrets_dir_key = canonical_path_key(&sandbox_secrets_dir(codex_home));
    let secrets_dir_prefix = format!("{}/", secrets_dir_key.trim_end_matches('/'));

    roots.retain(|root| {
        let key = canonical_path_key(root);
        key != codex_home_key
            && key != sbx_dir_key
            && !key.starts_with(&sbx_dir_prefix)
            && key != sbx_bin_dir_key
            && !key.starts_with(&sbx_bin_dir_prefix)
            && key != secrets_dir_key
            && !key.starts_with(&secrets_dir_prefix)
    });
    roots
}

#[cfg(test)]
mod tests {
    use super::WINDOWS_PLATFORM_DEFAULT_READ_ROOTS;
    use super::build_payload_roots;
    use super::gather_legacy_full_read_roots;
    use super::gather_read_roots;
    use super::loopback_proxy_port_from_url;
    use super::offline_proxy_settings_from_env;
    use super::profile_read_roots;
    use super::proxy_ports_from_env;
    use crate::helper_materialization::helper_bin_dir;
    use crate::policy::SandboxPolicy;
    use codex_protocol::protocol::ReadOnlyAccess;
    use codex_utils_absolute_path::AbsolutePathBuf;
    use pretty_assertions::assert_eq;
    use std::collections::HashMap;
    use std::collections::HashSet;
    use std::fs;
    use std::path::PathBuf;
    use tempfile::TempDir;

    fn canonical_windows_platform_default_roots() -> Vec<PathBuf> {
        WINDOWS_PLATFORM_DEFAULT_READ_ROOTS
            .iter()
            .map(|path| dunce::canonicalize(path).unwrap_or_else(|_| PathBuf::from(path)))
            .collect()
    }

    #[test]
    fn loopback_proxy_url_parsing_supports_common_forms() {
        assert_eq!(
            loopback_proxy_port_from_url("http://localhost:3128"),
            Some(3128)
        );
        assert_eq!(
            loopback_proxy_port_from_url("https://127.0.0.1:8080"),
            Some(8080)
        );
        assert_eq!(
            loopback_proxy_port_from_url("socks5h://user:pass@[::1]:1080"),
            Some(1080)
        );
    }

    #[test]
    fn loopback_proxy_url_parsing_rejects_non_loopback_and_zero_port() {
        assert_eq!(
            loopback_proxy_port_from_url("http://example.com:3128"),
            None
        );
        assert_eq!(loopback_proxy_port_from_url("http://127.0.0.1:0"), None);
        assert_eq!(loopback_proxy_port_from_url("localhost:8080"), None);
    }

    #[test]
    fn proxy_ports_from_env_dedupes_and_sorts() {
        let mut env = HashMap::new();
        env.insert(
            "HTTP_PROXY".to_string(),
            "http://127.0.0.1:8080".to_string(),
        );
        env.insert(
            "http_proxy".to_string(),
            "http://localhost:8080".to_string(),
        );
        env.insert("ALL_PROXY".to_string(), "socks5h://[::1]:1081".to_string());
        env.insert(
            "HTTPS_PROXY".to_string(),
            "https://example.com:9999".to_string(),
        );

        assert_eq!(proxy_ports_from_env(&env), vec![1081, 8080]);
    }

    #[test]
    fn offline_proxy_settings_ignore_proxy_env_when_online_identity_selected() {
        let mut env = HashMap::new();
        env.insert(
            "HTTP_PROXY".to_string(),
            "http://127.0.0.1:8080".to_string(),
        );
        env.insert(
            "CODEX_NETWORK_ALLOW_LOCAL_BINDING".to_string(),
            "1".to_string(),
        );

        assert_eq!(
            offline_proxy_settings_from_env(&env, super::SandboxNetworkIdentity::Online),
            super::OfflineProxySettings {
                proxy_ports: vec![],
                allow_local_binding: false,
            }
        );
    }

    #[test]
    fn offline_proxy_settings_capture_proxy_ports_and_local_binding_for_offline_identity() {
        let mut env = HashMap::new();
        env.insert(
            "HTTP_PROXY".to_string(),
            "http://127.0.0.1:8080".to_string(),
        );
        env.insert(
            "ALL_PROXY".to_string(),
            "socks5h://127.0.0.1:1081".to_string(),
        );
        env.insert(
            "CODEX_NETWORK_ALLOW_LOCAL_BINDING".to_string(),
            "1".to_string(),
        );

        assert_eq!(
            offline_proxy_settings_from_env(&env, super::SandboxNetworkIdentity::Offline),
            super::OfflineProxySettings {
                proxy_ports: vec![1081, 8080],
                allow_local_binding: true,
            }
        );
    }

    #[test]
    fn setup_marker_request_mismatch_reason_ignores_proxy_drift_for_online_identity() {
        let marker = super::SetupMarker {
            version: super::SETUP_VERSION,
            offline_username: "offline".to_string(),
            online_username: "online".to_string(),
            created_at: None,
            proxy_ports: vec![3128],
            allow_local_binding: false,
        };
        let desired = super::OfflineProxySettings {
            proxy_ports: vec![1081, 8080],
            allow_local_binding: true,
        };

        assert_eq!(
            marker.request_mismatch_reason(super::SandboxNetworkIdentity::Online, &desired),
            None
        );
    }

    #[test]
    fn setup_marker_request_mismatch_reason_reports_offline_firewall_drift() {
        let marker = super::SetupMarker {
            version: super::SETUP_VERSION,
            offline_username: "offline".to_string(),
            online_username: "online".to_string(),
            created_at: None,
            proxy_ports: vec![3128],
            allow_local_binding: false,
        };
        let desired = super::OfflineProxySettings {
            proxy_ports: vec![1081, 8080],
            allow_local_binding: true,
        };

        assert_eq!(
            marker.request_mismatch_reason(super::SandboxNetworkIdentity::Offline, &desired),
            Some(
                "offline firewall settings changed (stored_ports=[3128], desired_ports=[1081, 8080], stored_allow_local_binding=false, desired_allow_local_binding=true)"
                    .to_string()
            )
        );
    }

    #[test]
    fn profile_read_roots_excludes_configured_top_level_entries() {
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
        let policy = SandboxPolicy::new_read_only_policy();

        let roots = gather_read_roots(&command_cwd, &policy, &codex_home);
        let expected =
            dunce::canonicalize(helper_bin_dir(&codex_home)).expect("canonical helper dir");

        assert!(roots.contains(&expected));
    }

    #[test]
    fn restricted_read_roots_skip_platform_defaults_when_disabled() {
        let tmp = TempDir::new().expect("tempdir");
        let codex_home = tmp.path().join("codex-home");
        let command_cwd = tmp.path().join("workspace");
        let readable_root = tmp.path().join("docs");
        fs::create_dir_all(&command_cwd).expect("create workspace");
        fs::create_dir_all(&readable_root).expect("create readable root");
        let policy = SandboxPolicy::ReadOnly {
            access: ReadOnlyAccess::Restricted {
                include_platform_defaults: false,
                readable_roots: vec![
                    AbsolutePathBuf::from_absolute_path(&readable_root)
                        .expect("absolute readable root"),
                ],
            },
            network_access: false,
        };

        let roots = gather_read_roots(&command_cwd, &policy, &codex_home);
        let expected_helper =
            dunce::canonicalize(helper_bin_dir(&codex_home)).expect("canonical helper dir");
        let expected_cwd = dunce::canonicalize(&command_cwd).expect("canonical workspace");
        let expected_readable =
            dunce::canonicalize(&readable_root).expect("canonical readable root");

        assert!(roots.contains(&expected_helper));
        assert!(roots.contains(&expected_cwd));
        assert!(roots.contains(&expected_readable));
        assert!(
            canonical_windows_platform_default_roots()
                .into_iter()
                .all(|path| !roots.contains(&path))
        );
    }

    #[test]
    fn restricted_read_roots_include_platform_defaults_when_enabled() {
        let tmp = TempDir::new().expect("tempdir");
        let codex_home = tmp.path().join("codex-home");
        let command_cwd = tmp.path().join("workspace");
        fs::create_dir_all(&command_cwd).expect("create workspace");
        let policy = SandboxPolicy::ReadOnly {
            access: ReadOnlyAccess::Restricted {
                include_platform_defaults: true,
                readable_roots: Vec::new(),
            },
            network_access: false,
        };

        let roots = gather_read_roots(&command_cwd, &policy, &codex_home);

        assert!(
            canonical_windows_platform_default_roots()
                .into_iter()
                .all(|path| roots.contains(&path))
        );
    }

    #[test]
    fn restricted_workspace_write_roots_remain_readable() {
        let tmp = TempDir::new().expect("tempdir");
        let codex_home = tmp.path().join("codex-home");
        let command_cwd = tmp.path().join("workspace");
        let writable_root = tmp.path().join("extra-write-root");
        fs::create_dir_all(&command_cwd).expect("create workspace");
        fs::create_dir_all(&writable_root).expect("create writable root");
        let policy = SandboxPolicy::WorkspaceWrite {
            writable_roots: vec![
                AbsolutePathBuf::from_absolute_path(&writable_root)
                    .expect("absolute writable root"),
            ],
            read_only_access: ReadOnlyAccess::Restricted {
                include_platform_defaults: false,
                readable_roots: Vec::new(),
            },
            network_access: false,
            exclude_tmpdir_env_var: true,
            exclude_slash_tmp: true,
        };

        let roots = gather_read_roots(&command_cwd, &policy, &codex_home);
        let expected_writable =
            dunce::canonicalize(&writable_root).expect("canonical writable root");

        assert!(roots.contains(&expected_writable));
    }

    #[test]
    fn build_payload_roots_preserves_restricted_read_policy_when_no_override_is_needed() {
        let tmp = TempDir::new().expect("tempdir");
        let codex_home = tmp.path().join("codex-home");
        let policy_cwd = tmp.path().join("policy-cwd");
        let command_cwd = tmp.path().join("workspace");
        let readable_root = tmp.path().join("docs");
        fs::create_dir_all(&policy_cwd).expect("create policy cwd");
        fs::create_dir_all(&command_cwd).expect("create workspace");
        fs::create_dir_all(&readable_root).expect("create readable root");
        let policy = SandboxPolicy::ReadOnly {
            access: ReadOnlyAccess::Restricted {
                include_platform_defaults: false,
                readable_roots: vec![
                    AbsolutePathBuf::from_absolute_path(&readable_root)
                        .expect("absolute readable root"),
                ],
            },
            network_access: false,
        };

        let (read_roots, write_roots) = build_payload_roots(
            &super::SandboxSetupRequest {
                policy: &policy,
                policy_cwd: &policy_cwd,
                command_cwd: &command_cwd,
                env_map: &HashMap::new(),
                codex_home: &codex_home,
                proxy_enforced: false,
            },
            &super::SetupRootOverrides::default(),
        );
        let expected_helper =
            dunce::canonicalize(helper_bin_dir(&codex_home)).expect("canonical helper dir");
        let expected_cwd = dunce::canonicalize(&command_cwd).expect("canonical workspace");
        let expected_readable =
            dunce::canonicalize(&readable_root).expect("canonical readable root");

        assert_eq!(write_roots, Vec::<PathBuf>::new());
        assert!(read_roots.contains(&expected_helper));
        assert!(read_roots.contains(&expected_cwd));
        assert!(read_roots.contains(&expected_readable));
        assert!(
            canonical_windows_platform_default_roots()
                .into_iter()
                .all(|path| !read_roots.contains(&path))
        );
    }

    #[test]
    fn build_payload_roots_preserves_helper_roots_when_read_override_is_provided() {
        let tmp = TempDir::new().expect("tempdir");
        let codex_home = tmp.path().join("codex-home");
        let policy_cwd = tmp.path().join("policy-cwd");
        let command_cwd = tmp.path().join("workspace");
        let readable_root = tmp.path().join("docs");
        fs::create_dir_all(&policy_cwd).expect("create policy cwd");
        fs::create_dir_all(&command_cwd).expect("create workspace");
        fs::create_dir_all(&readable_root).expect("create readable root");
        let policy = SandboxPolicy::ReadOnly {
            access: ReadOnlyAccess::Restricted {
                include_platform_defaults: true,
                readable_roots: Vec::new(),
            },
            network_access: false,
        };

        let (read_roots, write_roots) = build_payload_roots(
            &super::SandboxSetupRequest {
                policy: &policy,
                policy_cwd: &policy_cwd,
                command_cwd: &command_cwd,
                env_map: &HashMap::new(),
                codex_home: &codex_home,
                proxy_enforced: false,
            },
            &super::SetupRootOverrides {
                read_roots: Some(vec![readable_root.clone()]),
                write_roots: None,
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
        let policy_cwd = tmp.path().join("policy-cwd");
        let command_cwd = tmp.path().join("workspace");
        let readable_root = tmp.path().join("docs");
        fs::create_dir_all(&policy_cwd).expect("create policy cwd");
        fs::create_dir_all(&command_cwd).expect("create workspace");
        fs::create_dir_all(&readable_root).expect("create readable root");
        let policy = SandboxPolicy::ReadOnly {
            access: ReadOnlyAccess::FullAccess,
            network_access: false,
        };

        let (read_roots, write_roots) = build_payload_roots(
            &super::SandboxSetupRequest {
                policy: &policy,
                policy_cwd: &policy_cwd,
                command_cwd: &command_cwd,
                env_map: &HashMap::new(),
                codex_home: &codex_home,
                proxy_enforced: false,
            },
            &super::SetupRootOverrides {
                read_roots: Some(vec![readable_root.clone()]),
                write_roots: None,
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
        let policy = SandboxPolicy::WorkspaceWrite {
            writable_roots: vec![
                AbsolutePathBuf::from_absolute_path(&extra_write_root)
                    .expect("absolute writable root"),
            ],
            read_only_access: ReadOnlyAccess::Restricted {
                include_platform_defaults: false,
                readable_roots: Vec::new(),
            },
            network_access: false,
            exclude_tmpdir_env_var: true,
            exclude_slash_tmp: true,
        };
        let request = super::SandboxSetupRequest {
            policy: &policy,
            policy_cwd: &command_cwd,
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
        let policy = SandboxPolicy::new_read_only_policy();

        let roots = gather_legacy_full_read_roots(&command_cwd, &policy, &codex_home);

        assert!(
            canonical_windows_platform_default_roots()
                .into_iter()
                .all(|path| roots.contains(&path))
        );
    }
}

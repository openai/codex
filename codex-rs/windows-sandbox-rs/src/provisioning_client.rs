//! Authenticated client for the packaged Windows sandbox provisioning service.

use crate::WindowsSandboxProvisioningSettings;
use crate::WindowsSandboxProxyListeners;
use std::collections::HashMap;
use std::fs::File;
use std::fs::OpenOptions;
use std::io;
use std::mem::size_of;
use std::os::windows::fs::OpenOptionsExt;
use std::os::windows::io::AsRawHandle;
use std::path::Path;
use std::ptr;
use std::time::Duration;
use std::time::Instant;

use anyhow::Context;
use anyhow::anyhow;
use anyhow::bail;
use codex_protocol::models::PermissionProfile;
use codex_protocol::shell_environment::OPENAI_FEDERATION_RULE_ID_ENV_VAR;
use codex_protocol::shell_environment::OPENAI_IDENTITY_TOKEN_FILE_ENV_VAR;
use windows_sys::Win32::Foundation::ERROR_BROKEN_PIPE;
use windows_sys::Win32::Foundation::ERROR_FILE_NOT_FOUND;
use windows_sys::Win32::Foundation::ERROR_NO_DATA;
use windows_sys::Win32::Foundation::ERROR_PIPE_BUSY;
use windows_sys::Win32::Foundation::ERROR_PIPE_NOT_CONNECTED;
use windows_sys::Win32::Foundation::ERROR_SEM_TIMEOUT;
use windows_sys::Win32::Foundation::ERROR_SERVICE_ALREADY_RUNNING;
use windows_sys::Win32::Foundation::ERROR_SERVICE_DOES_NOT_EXIST;
use windows_sys::Win32::Foundation::ERROR_SERVICE_MARKED_FOR_DELETE;
use windows_sys::Win32::Foundation::HANDLE;
use windows_sys::Win32::Storage::FileSystem::SECURITY_IMPERSONATION;
use windows_sys::Win32::Storage::FileSystem::SECURITY_SQOS_PRESENT;
use windows_sys::Win32::System::Pipes::GetNamedPipeServerProcessId;
use windows_sys::Win32::System::Pipes::WaitNamedPipeW;
use windows_sys::Win32::System::Services;
use windows_sys::Win32::System::Services::SC_HANDLE;

const PROVISIONING_TIMEOUT: Duration = Duration::from_secs(120);
const SERVICE_STARTUP_TIMEOUT: Duration = Duration::from_secs(5);

mod group_change;
mod refresh_retry;

impl WindowsSandboxProvisioningSettings {
    /// Derives the full firewall settings using the same environment handling as elevated setup.
    pub fn from_environment(
        permission_profile: &PermissionProfile,
        env_map: &HashMap<String, String>,
    ) -> Self {
        let network_identity = if permission_profile.network_sandbox_policy().is_enabled() {
            crate::setup::SandboxNetworkIdentity::Online
        } else {
            crate::setup::SandboxNetworkIdentity::Offline
        };
        let settings = crate::setup::offline_proxy_settings_from_env(env_map, network_identity);
        Self {
            proxy_ports: settings.proxy_ports,
            allow_local_binding: settings.allow_local_binding,
        }
    }
}

impl WindowsSandboxProxyListeners {
    /// Identifies known listener protocols without restricting the ports allowed by setup.
    pub fn from_environment(
        permission_profile: &PermissionProfile,
        env_map: &HashMap<String, String>,
    ) -> Self {
        if permission_profile.network_sandbox_policy().is_enabled() {
            return Self::default();
        }
        Self::from_proxy_environment(env_map)
    }

    pub(crate) fn from_proxy_environment(env_map: &HashMap<String, String>) -> Self {
        let mut listeners = Self::default();
        for value in crate::setup::PROXY_ENV_KEYS
            .iter()
            .filter_map(|key| env_map.get(*key))
        {
            let Some((scheme, _)) = value.trim().split_once("://") else {
                continue;
            };
            let Some(port) = crate::setup::loopback_proxy_port_from_url(value) else {
                continue;
            };
            match scheme.to_ascii_lowercase().as_str() {
                "http" | "https" => listeners.http_ports.push(port),
                "socks4" | "socks4a" | "socks5" | "socks5h" => {
                    listeners.socks_ports.push(port);
                }
                _ => {}
            }
        }
        listeners.http_ports.sort_unstable();
        listeners.http_ports.dedup();
        listeners.socks_ports.sort_unstable();
        listeners.socks_ports.dedup();
        listeners
    }
}

/// Result of attempting setup through the packaged provisioning service.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum WindowsSandboxProvisioningOutcome {
    /// The service completed elevated provisioning.
    Provisioned,
    /// The service was absent, did not support this caller, or timed out.
    Unavailable,
}

enum ProvisioningIntent {
    Setup,
    RefreshRegistration,
}

/// Refreshes only an already-provisioned gated runtime; no helper or setup fallback.
#[doc(hidden)]
pub fn refresh_registered_core_via_service(
    codex_home: &Path,
    settings: WindowsSandboxProvisioningSettings,
    listeners: WindowsSandboxProxyListeners,
) -> anyhow::Result<WindowsSandboxProvisioningOutcome> {
    anyhow::ensure!(
        crate::registered_core_requested(),
        "registered Core is not enabled"
    );
    provision(
        codex_home,
        settings,
        listeners,
        ProvisioningIntent::RefreshRegistration,
    )
}

/// Provisions the elevated Windows sandbox through the authenticated packaged service.
pub fn provision_windows_sandbox_via_service(
    codex_home: &Path,
    settings: WindowsSandboxProvisioningSettings,
    listeners: WindowsSandboxProxyListeners,
) -> anyhow::Result<WindowsSandboxProvisioningOutcome> {
    // The IPC request does not carry the caller's workload-identity environment.
    // Select helper fallback here: a service error would propagate, not fall back.
    // service_unavailable still rejects helper fallback for registered Core.
    if std::env::var_os(OPENAI_FEDERATION_RULE_ID_ENV_VAR).is_some()
        || std::env::var_os(OPENAI_IDENTITY_TOKEN_FILE_ENV_VAR).is_some()
    {
        return service_unavailable();
    }
    provision(codex_home, settings, listeners, ProvisioningIntent::Setup)
}

fn provision(
    codex_home: &Path,
    settings: WindowsSandboxProvisioningSettings,
    listeners: WindowsSandboxProxyListeners,
    intent: ProvisioningIntent,
) -> anyhow::Result<WindowsSandboxProvisioningOutcome> {
    let request = crate::FramedProvisioningMessage {
        version: crate::PROVISIONING_PROTOCOL_VERSION,
        message: crate::ProvisioningMessage::ProvisionSandboxRequest {
            payload: crate::SandboxProvisioningRequest {
                codex_home: codex_home
                    .to_str()
                    .context("sandbox provisioning home is not valid UTF-8")?
                    .to_owned(),
                registered_core: crate::registered_core_requested(),
                refresh_only: matches!(intent, ProvisioningIntent::RefreshRegistration),
                settings,
                listeners,
            },
        },
    };

    match send_service_request(&request, PROVISIONING_TIMEOUT)? {
        crate::SandboxProvisioningResponse::Ok => {
            Ok(WindowsSandboxProvisioningOutcome::Provisioned)
        }
        crate::SandboxProvisioningResponse::Unavailable => service_unavailable(),
        crate::SandboxProvisioningResponse::Error { message } => Err(anyhow!(message)),
    }
}

fn service_unavailable() -> anyhow::Result<WindowsSandboxProvisioningOutcome> {
    if crate::registered_core_requested() {
        bail!("app runtime provisioning service is unavailable; refusing helper fallback");
    }
    Ok(WindowsSandboxProvisioningOutcome::Unavailable)
}

/// Starts a stopped service only for explicit setup. The normal connection path
/// waits for startup and authenticates the pipe server; SCM authorizes the start.
pub fn start_windows_sandbox_service_for_setup() -> anyhow::Result<()> {
    query_service_status()
        .and_then(|status| {
            if status.dwCurrentState == Services::SERVICE_STOPPED {
                let service = open_service(Services::SERVICE_START)
                    .context("request permission to start sandbox service for setup")?;
                if unsafe { Services::StartServiceW(service.0, 0, ptr::null()) } == 0 {
                    let error = io::Error::last_os_error();
                    if error.raw_os_error() != Some(ERROR_SERVICE_ALREADY_RUNNING as i32) {
                        return Err(error).context("start sandbox service for setup");
                    }
                }
            }
            Ok(())
        })
        .or_else(|error| {
            if error.downcast_ref::<io::Error>().is_some_and(|error| {
                matches!(error.raw_os_error(), Some(code)
                if code == ERROR_SERVICE_DOES_NOT_EXIST as i32
                    || code == ERROR_SERVICE_MARKED_FOR_DELETE as i32)
            }) {
                // The connection path preserves registered Core's no-fallback error.
                Ok(())
            } else {
                Err(error)
            }
        })
}

/// Records desktop uninstall ownership without creating or enabling a sandbox.
pub fn register_desktop_installation(codex_home: &Path) -> anyhow::Result<()> {
    let request = crate::FramedProvisioningMessage {
        version: crate::PROVISIONING_PROTOCOL_VERSION,
        message: crate::ProvisioningMessage::RegisterInstallationRequest {
            codex_home: codex_home
                .to_str()
                .context("desktop home is not valid UTF-8")?
                .to_owned(),
        },
    };
    match send_service_request(&request, Duration::from_secs(5))? {
        crate::SandboxProvisioningResponse::Ok => Ok(()),
        crate::SandboxProvisioningResponse::Unavailable => {
            bail!("desktop uninstall registration service is unavailable")
        }
        crate::SandboxProvisioningResponse::Error { message } => Err(anyhow!(message)),
    }
}

fn send_service_request(
    request: &crate::FramedProvisioningMessage,
    timeout: Duration,
) -> anyhow::Result<crate::SandboxProvisioningResponse> {
    let deadline = Instant::now() + timeout;
    let Some(mut pipe) = connect(deadline)? else {
        return Ok(crate::SandboxProvisioningResponse::Unavailable);
    };
    if let crate::ProvisioningMessage::ProvisionSandboxRequest { payload } = &request.message
        && payload.registered_core
        && payload.refresh_only
    {
        return refresh_retry::send(pipe, request, deadline);
    }
    let response = match exchange_request(&mut pipe, request, deadline) {
        Ok(response) => response,
        Err(error)
            if error.downcast_ref::<io::Error>().is_some_and(|error| {
                matches!(
                    error.kind(),
                    io::ErrorKind::UnexpectedEof | io::ErrorKind::TimedOut
                ) || matches!(
                    error.raw_os_error(),
                    Some(code)
                        if code == ERROR_BROKEN_PIPE as i32
                            || code == ERROR_NO_DATA as i32
                            || code == ERROR_PIPE_NOT_CONNECTED as i32
                )
            }) =>
        {
            return Ok(crate::SandboxProvisioningResponse::Unavailable);
        }
        Err(error) => return Err(error),
    };
    if matches!(&response, crate::SandboxProvisioningResponse::Error { message }
        if message == crate::SANDBOX_GROUP_CHANGED)
    {
        drop(pipe);
        return group_change::retry(request, deadline);
    }
    Ok(response)
}

fn exchange_request(
    pipe: &mut File,
    request: &crate::FramedProvisioningMessage,
    deadline: Instant,
) -> anyhow::Result<crate::SandboxProvisioningResponse> {
    verify_server(pipe.as_raw_handle() as HANDLE)
        .context("authenticate provisioning pipe server")?;
    if Instant::now() >= deadline {
        return Err(io::Error::new(
            io::ErrorKind::TimedOut,
            "sandbox provisioning deadline expired before sending request",
        )
        .into());
    }
    crate::write_provisioning_frame(&mut *pipe, request)
        .context("send sandbox provisioning request")?;
    read_response(pipe, deadline)
}

fn read_response(
    pipe: &mut File,
    deadline: Instant,
) -> anyhow::Result<crate::SandboxProvisioningResponse> {
    crate::framed_io::wait_for_complete_frame(pipe, deadline)
        .context("wait for sandbox provisioning response")?;
    let response = crate::read_provisioning_frame(pipe)
        .context("read sandbox provisioning response")?
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "sandbox provisioning service closed the pipe without a response",
            )
        })
        .context("read sandbox provisioning response")?;
    if response.version != crate::PROVISIONING_PROTOCOL_VERSION {
        return Ok(crate::SandboxProvisioningResponse::Unavailable);
    }
    let crate::ProvisioningMessage::ProvisionSandboxResponse { payload } = response.message else {
        bail!("unexpected sandbox provisioning response message");
    };
    Ok(payload)
}

fn connect(deadline: Instant) -> anyhow::Result<Option<File>> {
    // Startup should not hold up the desktop's readiness check for the full
    // provisioning timeout. Keep the caller's original deadline for real work.
    let startup_deadline = deadline.min(Instant::now() + SERVICE_STARTUP_TIMEOUT);
    let pipe_name = crate::windows_sandbox_service_pipe_name()?;
    let open_pipe = || {
        OpenOptions::new()
            .read(true)
            .write(true)
            .custom_flags(SECURITY_SQOS_PRESENT | SECURITY_IMPERSONATION)
            .open(&pipe_name)
    };

    let pipe_name = crate::to_wide(&pipe_name);
    loop {
        match query_service_status() {
            Ok(status) if status.dwCurrentState == Services::SERVICE_START_PENDING => {
                let remaining = startup_deadline.saturating_duration_since(Instant::now());
                if remaining.is_zero() {
                    return Ok(None);
                }
                std::thread::sleep(remaining.min(Duration::from_millis(25)));
                continue;
            }
            Ok(status) if status.dwCurrentState == Services::SERVICE_RUNNING => {}
            Ok(_) => return Ok(None),
            Err(error)
                if error.downcast_ref::<io::Error>().is_some_and(|error| {
                    matches!(
                        error.raw_os_error(),
                        Some(code)
                            if code == ERROR_SERVICE_DOES_NOT_EXIST as i32
                                || code == ERROR_SERVICE_MARKED_FOR_DELETE as i32
                    )
                }) =>
            {
                return Ok(None);
            }
            Err(error) => return Err(error),
        }
        match open_pipe() {
            Ok(pipe) => return Ok(Some(pipe)),
            Err(error) if error.raw_os_error() == Some(ERROR_FILE_NOT_FOUND as i32) => {
                return Ok(None);
            }
            Err(error) if error.raw_os_error() == Some(ERROR_PIPE_BUSY as i32) => {
                let remaining = deadline.saturating_duration_since(Instant::now());
                if remaining.is_zero() {
                    return Ok(None);
                }
                let wait_ms = u32::try_from(remaining.as_millis())
                    .unwrap_or(u32::MAX)
                    .max(1);
                if unsafe { WaitNamedPipeW(pipe_name.as_ptr(), wait_ms) } == 0 {
                    let error = io::Error::last_os_error();
                    if error.raw_os_error() == Some(ERROR_FILE_NOT_FOUND as i32) {
                        continue;
                    }
                    if error.raw_os_error() == Some(ERROR_SEM_TIMEOUT as i32) {
                        return Ok(None);
                    }
                    return Err(error).context("wait for sandbox provisioning pipe");
                }
            }
            Err(error) => return Err(error).context("open sandbox provisioning pipe"),
        }
    }
}

fn pipe_server_process_id(pipe: HANDLE) -> anyhow::Result<u32> {
    let mut pipe_process_id = 0;
    if unsafe { GetNamedPipeServerProcessId(pipe, &mut pipe_process_id) } == 0 {
        return Err(io::Error::last_os_error()).context("identify provisioning pipe server");
    }
    Ok(pipe_process_id)
}

fn verify_server(pipe: HANDLE) -> anyhow::Result<u32> {
    let pipe_process_id = pipe_server_process_id(pipe)?;
    let status = query_service_status()?;
    if status.dwCurrentState != Services::SERVICE_RUNNING
        || status.dwProcessId == 0
        || status.dwProcessId != pipe_process_id
    {
        bail!("the provisioning pipe server does not match the running service");
    }
    Ok(pipe_process_id)
}

fn query_service_status() -> anyhow::Result<Services::SERVICE_STATUS_PROCESS> {
    let service = open_service(Services::SERVICE_QUERY_STATUS)?;
    let mut status: Services::SERVICE_STATUS_PROCESS = unsafe { std::mem::zeroed() };
    let mut bytes_needed = 0;
    if unsafe {
        Services::QueryServiceStatusEx(
            service.0,
            Services::SC_STATUS_PROCESS_INFO,
            ptr::from_mut(&mut status).cast(),
            size_of::<Services::SERVICE_STATUS_PROCESS>() as u32,
            &mut bytes_needed,
        )
    } == 0
    {
        return Err(io::Error::last_os_error()).context("query sandbox provisioning service");
    }
    if status.dwCurrentState == Services::SERVICE_STOPPED {
        crate::service_diagnostics::record_stopped(&status, service.0);
    }
    Ok(status)
}

fn open_service(access: u32) -> anyhow::Result<ServiceHandle> {
    let manager =
        unsafe { Services::OpenSCManagerW(ptr::null(), ptr::null(), Services::SC_MANAGER_CONNECT) };
    if manager.is_null() {
        return Err(io::Error::last_os_error()).context("open service control manager");
    }
    let manager = ServiceHandle(manager);

    let service_name = crate::to_wide(crate::windows_sandbox_service_name()?);
    let service = unsafe { Services::OpenServiceW(manager.0, service_name.as_ptr(), access) };
    if service.is_null() {
        return Err(io::Error::last_os_error()).context("open sandbox provisioning service");
    }
    Ok(ServiceHandle(service))
}

struct ServiceHandle(SC_HANDLE);

impl Drop for ServiceHandle {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe { Services::CloseServiceHandle(self.0) };
        }
    }
}

#[cfg(test)]
#[path = "provisioning_client_tests.rs"]
mod tests;

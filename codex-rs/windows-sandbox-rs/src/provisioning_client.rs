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
use windows_sys::Win32::Foundation::ERROR_BROKEN_PIPE;
use windows_sys::Win32::Foundation::ERROR_FILE_NOT_FOUND;
use windows_sys::Win32::Foundation::ERROR_NO_DATA;
use windows_sys::Win32::Foundation::ERROR_PIPE_BUSY;
use windows_sys::Win32::Foundation::ERROR_PIPE_NOT_CONNECTED;
use windows_sys::Win32::Foundation::ERROR_SEM_TIMEOUT;
use windows_sys::Win32::Foundation::HANDLE;
use windows_sys::Win32::Security::SC_HANDLE;
use windows_sys::Win32::Storage::FileSystem::SECURITY_IMPERSONATION;
use windows_sys::Win32::Storage::FileSystem::SECURITY_SQOS_PRESENT;
use windows_sys::Win32::System::Pipes::GetNamedPipeServerProcessId;
use windows_sys::Win32::System::Pipes::WaitNamedPipeW;
use windows_sys::Win32::System::Services;

const SERVICE_NAME: &str = "CodexSandboxService";
const PROVISIONING_TIMEOUT: Duration = Duration::from_secs(120);

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

/// Provisions the elevated Windows sandbox through the authenticated packaged service.
pub fn provision_windows_sandbox_via_service(
    codex_home: &Path,
    settings: WindowsSandboxProvisioningSettings,
    listeners: WindowsSandboxProxyListeners,
) -> anyhow::Result<WindowsSandboxProvisioningOutcome> {
    let request = crate::FramedProvisioningMessage {
        version: crate::PROVISIONING_PROTOCOL_VERSION,
        message: crate::ProvisioningMessage::ProvisionSandboxRequest {
            payload: crate::SandboxProvisioningRequest {
                codex_home: codex_home
                    .to_str()
                    .context("sandbox provisioning home is not valid UTF-8")?
                    .to_owned(),
                settings,
                listeners,
            },
        },
    };

    match send_service_request(request, PROVISIONING_TIMEOUT)? {
        crate::SandboxProvisioningResponse::Ok => {
            Ok(WindowsSandboxProvisioningOutcome::Provisioned)
        }
        crate::SandboxProvisioningResponse::Unavailable => {
            Ok(WindowsSandboxProvisioningOutcome::Unavailable)
        }
        crate::SandboxProvisioningResponse::Error { message } => Err(anyhow!(message)),
    }
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
    match send_service_request(request, Duration::from_secs(5))? {
        crate::SandboxProvisioningResponse::Ok => Ok(()),
        crate::SandboxProvisioningResponse::Unavailable => {
            bail!("desktop uninstall registration service is unavailable")
        }
        crate::SandboxProvisioningResponse::Error { message } => Err(anyhow!(message)),
    }
}

fn send_service_request(
    request: crate::FramedProvisioningMessage,
    timeout: Duration,
) -> anyhow::Result<crate::SandboxProvisioningResponse> {
    let deadline = Instant::now() + timeout;
    let Some(mut pipe) = connect(deadline)? else {
        return Ok(crate::SandboxProvisioningResponse::Unavailable);
    };
    let response = (|| -> anyhow::Result<crate::FramedProvisioningMessage> {
        verify_server(pipe.as_raw_handle() as HANDLE)
            .context("authenticate provisioning pipe server")?;
        crate::write_provisioning_frame(&mut pipe, &request)
            .context("send sandbox provisioning request")?;
        crate::framed_io::wait_for_complete_frame(&pipe, deadline)
            .context("wait for sandbox provisioning response")?;
        crate::read_provisioning_frame(&mut pipe)
            .context("read sandbox provisioning response")?
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "sandbox provisioning service closed the pipe without a response",
                )
            })
            .context("read sandbox provisioning response")
    })();
    let response = match response {
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

    if response.version != crate::PROVISIONING_PROTOCOL_VERSION {
        return Ok(crate::SandboxProvisioningResponse::Unavailable);
    }
    let crate::ProvisioningMessage::ProvisionSandboxResponse { payload } = response.message else {
        bail!("unexpected sandbox provisioning response message");
    };
    Ok(payload)
}

fn connect(deadline: Instant) -> anyhow::Result<Option<File>> {
    let open_pipe = || {
        OpenOptions::new()
            .read(true)
            .write(true)
            .custom_flags(SECURITY_SQOS_PRESENT | SECURITY_IMPERSONATION)
            .open(crate::SANDBOX_PROVISIONING_PIPE_NAME)
    };

    let pipe_name = crate::to_wide(crate::SANDBOX_PROVISIONING_PIPE_NAME);
    loop {
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
                    if matches!(
                        error.raw_os_error(),
                        Some(code)
                            if code == ERROR_FILE_NOT_FOUND as i32
                                || code == ERROR_SEM_TIMEOUT as i32
                    ) {
                        return Ok(None);
                    }
                    return Err(error).context("wait for sandbox provisioning pipe");
                }
            }
            Err(error) => return Err(error).context("open sandbox provisioning pipe"),
        }
    }
}

fn verify_server(pipe: HANDLE) -> anyhow::Result<()> {
    let mut pipe_process_id = 0;
    if unsafe { GetNamedPipeServerProcessId(pipe, &mut pipe_process_id) } == 0 {
        return Err(io::Error::last_os_error()).context("identify provisioning pipe server");
    }

    let manager =
        unsafe { Services::OpenSCManagerW(ptr::null(), ptr::null(), Services::SC_MANAGER_CONNECT) };
    if manager == 0 {
        return Err(io::Error::last_os_error()).context("open service control manager");
    }
    let manager = ServiceHandle(manager);

    let service_name = crate::to_wide(SERVICE_NAME);
    let service = unsafe {
        Services::OpenServiceW(
            manager.0,
            service_name.as_ptr(),
            Services::SERVICE_QUERY_STATUS,
        )
    };
    if service == 0 {
        return Err(io::Error::last_os_error()).context("open sandbox provisioning service");
    }
    let service = ServiceHandle(service);

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
    if status.dwCurrentState != Services::SERVICE_RUNNING
        || status.dwProcessId == 0
        || status.dwProcessId != pipe_process_id
    {
        bail!("the provisioning pipe server does not match the running service");
    }
    Ok(())
}

struct ServiceHandle(SC_HANDLE);

impl Drop for ServiceHandle {
    fn drop(&mut self) {
        if self.0 != 0 {
            unsafe { Services::CloseServiceHandle(self.0) };
        }
    }
}

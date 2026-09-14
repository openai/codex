//! Validates framed provisioning requests and normalizes proxy settings.

use anyhow::Context;
use anyhow::Result;
use anyhow::bail;
use codex_windows_sandbox::PROVISIONING_PROTOCOL_VERSION;
use codex_windows_sandbox::ProvisioningMessage;
use codex_windows_sandbox::WindowsSandboxProvisioningSettings;
use codex_windows_sandbox::WindowsSandboxProxyListeners;
use codex_windows_sandbox::read_provisioning_frame;
use std::path::PathBuf;

use super::MAX_REQUEST_BYTES;

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum ServiceRequest {
    RegisterInstallation { codex_home: PathBuf },
    ProvisionSandbox(ProvisioningRequest),
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct ProvisioningRequest {
    pub(crate) codex_home: PathBuf,
    pub(crate) registered_core: bool,
    pub(crate) refresh_only: bool,
    pub(crate) listeners: WindowsSandboxProxyListeners,
    pub(crate) settings: WindowsSandboxProvisioningSettings,
}

pub(super) fn validate_request(request: &[u8]) -> Result<ServiceRequest> {
    if request.len() > MAX_REQUEST_BYTES {
        bail!("provisioning request exceeds size limit");
    }
    let mut reader = request;
    let frame = read_provisioning_frame(&mut reader)
        .context("invalid framed provisioning request")?
        .context("provisioning client sent an empty request")?;
    if !reader.is_empty() {
        bail!("provisioning requests must contain exactly one IPC frame");
    }

    if frame.version != PROVISIONING_PROTOCOL_VERSION {
        bail!(
            "unsupported provisioning request version: {}",
            frame.version
        );
    }
    let request = match frame.message {
        ProvisioningMessage::RegisterInstallationRequest { codex_home } => {
            validate_home(&codex_home)?;
            return Ok(ServiceRequest::RegisterInstallation {
                codex_home: PathBuf::from(codex_home),
            });
        }
        ProvisioningMessage::ProvisionSandboxRequest { payload } => payload,
        ProvisioningMessage::ProvisionSandboxResponse { .. } => bail!("expected a service request"),
    };
    validate_home(&request.codex_home)?;
    if request.refresh_only && !request.registered_core {
        bail!("registration refresh requires registered Core");
    }
    let mut settings = request.settings;
    let mut listeners = request.listeners;
    for ports in [
        &mut settings.proxy_ports,
        &mut listeners.http_ports,
        &mut listeners.socks_ports,
    ] {
        if ports.contains(&0) {
            bail!("provisioning request includes an invalid proxy port");
        }
        ports.sort_unstable();
        ports.dedup();
    }
    if listeners
        .http_ports
        .iter()
        .chain(&listeners.socks_ports)
        .any(|port| !settings.proxy_ports.contains(port))
    {
        bail!("provisioning listener is absent from the proxy settings");
    }
    Ok(ServiceRequest::ProvisionSandbox(ProvisioningRequest {
        codex_home: PathBuf::from(request.codex_home),
        registered_core: request.registered_core,
        refresh_only: request.refresh_only,
        listeners,
        settings,
    }))
}

fn validate_home(home: &str) -> Result<()> {
    if home.is_empty() || home.contains(['\0', '\r', '\n']) {
        bail!("Codex home is empty or contains an invalid control character");
    }
    Ok(())
}

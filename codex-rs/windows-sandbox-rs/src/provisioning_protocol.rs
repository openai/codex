//! Dedicated protocol exchanged with the Windows sandbox provisioning service.

use crate::WindowsSandboxProvisioningSettings;
use anyhow::Result;
use serde::Deserialize;
use serde::Serialize;
use std::io::Read;
use std::io::Write;

/// Protocol version shared by provisioning clients and the Windows service.
pub const PROVISIONING_PROTOCOL_VERSION: u8 = 1;

/// Named pipe used by the machine-wide Windows sandbox provisioning service.
pub const SANDBOX_PROVISIONING_PIPE_NAME: &str = r"\\.\pipe\OpenAI.CodexSandbox";

/// Pre-dispatch refusal: the caller may reconnect once to the refreshed pipe.
pub const SANDBOX_GROUP_CHANGED: &str = "sandbox group changed before authentication";

/// Versioned provisioning-service message carried in a length-prefixed JSON frame.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FramedProvisioningMessage {
    pub version: u8,
    #[serde(flatten)]
    pub message: ProvisioningMessage,
}

/// Request and response messages exchanged with the provisioning service.
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ProvisioningMessage {
    /// Records uninstall ownership without provisioning sandbox resources.
    RegisterInstallationRequest {
        codex_home: String,
    },
    ProvisionSandboxRequest {
        payload: SandboxProvisioningRequest,
    },
    ProvisionSandboxResponse {
        payload: SandboxProvisioningResponse,
    },
}

/// Sandbox setup parameters sent to the provisioning service.
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct SandboxProvisioningRequest {
    pub codex_home: String,
    /// Routing request; the service independently authenticates its installed caller.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub registered_core: bool,
    /// Refresh an existing registration without creating or repairing sandbox accounts.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub refresh_only: bool,
    pub settings: WindowsSandboxProvisioningSettings,
    pub listeners: WindowsSandboxProxyListeners,
}

/// Known proxy protocols used for managed-policy validation, separate from firewall settings.
#[derive(Clone, Debug, Default, Deserialize, Serialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct WindowsSandboxProxyListeners {
    pub http_ports: Vec<u16>,
    pub socks_ports: Vec<u16>,
}

/// Result returned by the sandbox provisioning service.
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum SandboxProvisioningResponse {
    Ok,
    /// The client should fall back to its elevated setup helper.
    Unavailable,
    Error {
        message: String,
    },
}

/// Write a length-prefixed provisioning-service message.
pub fn write_provisioning_frame<W: Write>(
    writer: W,
    message: &FramedProvisioningMessage,
) -> Result<()> {
    crate::framed_io::write_frame(writer, message)
}

/// Read a length-prefixed provisioning-service message, returning `None` on EOF.
pub fn read_provisioning_frame<R: Read>(reader: R) -> Result<Option<FramedProvisioningMessage>> {
    crate::framed_io::read_frame(reader)
}

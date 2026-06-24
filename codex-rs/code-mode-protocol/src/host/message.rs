use std::fmt;

use serde::Deserialize;
use serde::Serialize;

use super::Capability;
use super::CapabilitySet;
use super::HandshakeRejectReason;
use super::ProtocolVersion;
use super::SessionId;
use super::SupportedProtocolVersions;

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientHello {
    supported_versions: SupportedProtocolVersions,
    required_capabilities: CapabilitySet,
    optional_capabilities: CapabilitySet,
}

impl ClientHello {
    pub fn new(
        supported_versions: SupportedProtocolVersions,
        required_capabilities: CapabilitySet,
        optional_capabilities: CapabilitySet,
    ) -> Result<Self, ClientHelloError> {
        if let Some(capability) = required_capabilities
            .iter()
            .find(|capability| optional_capabilities.contains(capability))
        {
            return Err(ClientHelloError::OverlappingCapability(capability.clone()));
        }
        Ok(Self {
            supported_versions,
            required_capabilities,
            optional_capabilities,
        })
    }

    pub fn supported_versions(&self) -> &SupportedProtocolVersions {
        &self.supported_versions
    }

    pub fn required_capabilities(&self) -> &CapabilitySet {
        &self.required_capabilities
    }

    pub fn optional_capabilities(&self) -> &CapabilitySet {
        &self.optional_capabilities
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct ClientHelloWire {
    supported_versions: SupportedProtocolVersions,
    required_capabilities: CapabilitySet,
    optional_capabilities: CapabilitySet,
}

impl<'de> Deserialize<'de> for ClientHello {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let wire = ClientHelloWire::deserialize(deserializer)?;
        Self::new(
            wire.supported_versions,
            wire.required_capabilities,
            wire.optional_capabilities,
        )
        .map_err(serde::de::Error::custom)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ClientHelloError {
    OverlappingCapability(Capability),
}

impl fmt::Display for ClientHelloError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OverlappingCapability(capability) => write!(
                formatter,
                "capability `{capability}` cannot be both required and optional"
            ),
        }
    }
}

impl std::error::Error for ClientHelloError {}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct HostHello {
    selected_version: ProtocolVersion,
    capabilities: CapabilitySet,
}

impl HostHello {
    pub fn new(selected_version: ProtocolVersion, capabilities: CapabilitySet) -> Self {
        Self {
            selected_version,
            capabilities,
        }
    }

    pub fn selected_version(&self) -> ProtocolVersion {
        self.selected_version
    }

    pub fn capabilities(&self) -> &CapabilitySet {
        &self.capabilities
    }
}

/// Messages sent from a client to the code-mode host.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields, tag = "type", rename_all_fields = "camelCase")]
pub enum ClientToHost {
    #[serde(rename = "connection/hello")]
    ClientHello(ClientHello),
    #[serde(rename = "session/open")]
    OpenSession { session_id: SessionId },
    #[serde(rename = "session/close")]
    CloseSession { session_id: SessionId },
}

/// Messages sent from the code-mode host to a client.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields, tag = "type", rename_all_fields = "camelCase")]
pub enum HostToClient {
    #[serde(rename = "connection/ready")]
    HostHello(HostHello),
    #[serde(rename = "connection/rejected")]
    HandshakeRejected { reason: HandshakeRejectReason },
    #[serde(rename = "session/ready")]
    SessionReady { session_id: SessionId },
    #[serde(rename = "session/closed")]
    SessionClosed { session_id: SessionId },
}

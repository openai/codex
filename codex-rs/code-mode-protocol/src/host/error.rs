use serde::Deserialize;
use serde::Serialize;

use super::Capability;
use super::SupportedProtocolVersions;

/// Explains why connection negotiation was rejected before any session opened.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, tag = "type", rename_all_fields = "camelCase")]
pub enum HandshakeRejectReason {
    #[serde(rename = "noCompatibleVersion")]
    NoCompatibleVersion {
        supported_versions: SupportedProtocolVersions,
    },
    #[serde(rename = "missingRequiredCapability")]
    MissingRequiredCapability { capability: Capability },
    #[serde(rename = "invalidHello")]
    InvalidHello { message: String },
}

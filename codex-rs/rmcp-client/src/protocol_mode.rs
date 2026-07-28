use std::ffi::OsStr;
use std::io;

use rmcp::model::ProtocolVersion;
use rmcp::service::ClientLifecycleMode;

/// MCP compatibility policy selected once when a Codex session is created.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum McpProtocolMode {
    /// Preserve the existing MCP initialization and OAuth behavior.
    #[default]
    Legacy,
    /// Allow the MCP 2026-07-28 discovery and request lifecycle.
    V20260728,
}

impl McpProtocolMode {
    /// Returns the newest protocol version this compatibility policy can use.
    pub fn preferred_protocol_version(self) -> ProtocolVersion {
        match self {
            Self::Legacy => ProtocolVersion::V_2025_06_18,
            Self::V20260728 => ProtocolVersion::V_2026_07_28,
        }
    }

    pub(crate) fn client_lifecycle(self) -> ClientLifecycleMode {
        match self {
            Self::Legacy => ClientLifecycleMode::Initialize,
            Self::V20260728 => ClientLifecycleMode::Auto {
                preferred_versions: vec![ProtocolVersion::V_2026_07_28],
                legacy_version: Some(ProtocolVersion::V_2025_06_18),
            },
        }
    }

    pub(crate) fn stdio_mode(self, requested_version: Option<&OsStr>) -> io::Result<Self> {
        match (self, requested_version) {
            (Self::Legacy, _) => Ok(Self::Legacy),
            (_, None) => Ok(Self::Legacy),
            (Self::V20260728, Some(version)) if version == OsStr::new("2026-07-28") => {
                Ok(Self::V20260728)
            }
            (_, Some(version)) => Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "unsupported CODEX_MCP_PROTOCOL_VERSION `{}` for stdio MCP server; expected `2026-07-28`",
                    version.to_string_lossy()
                ),
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn protocol_modes_select_compatible_sdk_lifecycles() {
        assert_eq!(
            McpProtocolMode::Legacy.preferred_protocol_version(),
            ProtocolVersion::V_2025_06_18
        );
        assert_eq!(
            McpProtocolMode::Legacy.client_lifecycle(),
            ClientLifecycleMode::Initialize
        );
        assert_eq!(
            McpProtocolMode::V20260728.preferred_protocol_version(),
            ProtocolVersion::V_2026_07_28
        );
        assert_eq!(
            McpProtocolMode::V20260728.client_lifecycle(),
            ClientLifecycleMode::Auto {
                preferred_versions: vec![ProtocolVersion::V_2026_07_28],
                legacy_version: Some(ProtocolVersion::V_2025_06_18),
            }
        );
    }

    #[test]
    fn stdio_requires_both_the_modern_feature_and_a_server_opt_in() {
        let modern_version = Some(OsStr::new("2026-07-28"));

        assert_eq!(
            McpProtocolMode::Legacy
                .stdio_mode(/*requested_version*/ None)
                .unwrap(),
            McpProtocolMode::Legacy
        );
        assert_eq!(
            McpProtocolMode::Legacy.stdio_mode(modern_version).unwrap(),
            McpProtocolMode::Legacy
        );
        assert_eq!(
            McpProtocolMode::V20260728
                .stdio_mode(/*requested_version*/ None)
                .unwrap(),
            McpProtocolMode::Legacy
        );
        assert_eq!(
            McpProtocolMode::V20260728
                .stdio_mode(modern_version)
                .unwrap(),
            McpProtocolMode::V20260728
        );
    }

    #[test]
    fn stdio_rejects_unknown_protocol_markers() {
        let error = McpProtocolMode::V20260728
            .stdio_mode(Some(OsStr::new("1999-01-01")))
            .expect_err("an unknown protocol marker must fail");

        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
        assert!(error.to_string().contains("1999-01-01"));
    }

    #[test]
    fn legacy_stdio_does_not_interpret_existing_protocol_markers() {
        assert_eq!(
            McpProtocolMode::Legacy
                .stdio_mode(Some(OsStr::new("1999-01-01")))
                .unwrap(),
            McpProtocolMode::Legacy
        );
    }
}

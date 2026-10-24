//! Selects the handshake for a fresh local helper when automatic discovery cannot finish in place.
//! A late discovery response can still negotiate the modern protocol after rmcp sent initialize.

use std::io;

use rmcp::model::ErrorCode;
use rmcp::model::ErrorData;
use rmcp::model::JsonRpcMessage;
use rmcp::model::ProtocolVersion;
use rmcp::model::ServerResult;
use rmcp::service::ClientInitializeError;

use super::HandshakeError;
use crate::McpProtocolMode;
use crate::http_client_adapter::is_known_legacy_protocol_version;
use crate::local_stdio_transport::DiscoveryOutcome;

pub(super) fn restart_mode(
    error: &ClientInitializeError,
    discovery: Option<DiscoveryOutcome>,
) -> anyhow::Result<Option<McpProtocolMode>> {
    let Some(DiscoveryOutcome::Late {
        reply,
        after_reoffer,
    }) = discovery
    else {
        return Ok(should_restart_legacy(error, discovery).then_some(McpProtocolMode::Legacy));
    };
    match *reply {
        JsonRpcMessage::Response(response) => match response.result {
            ServerResult::DiscoverResult(result) => {
                let supported = result.supported_versions;
                restart_for_versions(&supported).map(Some).ok_or_else(|| {
                    ClientInitializeError::NoCompatibleProtocolVersion {
                        client_supported: vec![ProtocolVersion::V_2026_07_28],
                        server_supported: supported,
                    }
                })
            }
            ServerResult::InitializeResult(result)
                if is_known_legacy_protocol_version(&result.protocol_version) =>
            {
                Ok(Some(McpProtocolMode::Legacy))
            }
            result => Err(ClientInitializeError::ExpectedInitResult(Some(result))),
        },
        JsonRpcMessage::Error(response) => {
            let mode = offered_restart_mode(&response.error);
            match mode {
                // rmcp permits one re-offer. A fresh discover verifies it even without an ID.
                Some(McpProtocolMode::DiscoverLocalStdio) if !after_reoffer => Ok(mode),
                // Without an ID the error may have rejected initialize itself.
                Some(McpProtocolMode::Legacy) if response.id.is_some() => Ok(mode),
                _ => Err(ClientInitializeError::JsonRpcError(response.error)),
            }
        }
        _ => unreachable!("a late discovery reply is a response or error"),
    }
    .map_err(|source| HandshakeError { source }.into())
}

fn offered_restart_mode(error: &ErrorData) -> Option<McpProtocolMode> {
    if error.code != ErrorCode::UNSUPPORTED_PROTOCOL_VERSION {
        return None;
    }
    let supported = error.data.as_ref()?.get("supported")?;
    let versions = serde_json::from_value::<Vec<ProtocolVersion>>(supported.clone()).ok()?;
    restart_for_versions(&versions)
}

fn restart_for_versions(supported: &[ProtocolVersion]) -> Option<McpProtocolMode> {
    if supported.contains(&ProtocolVersion::V_2026_07_28) {
        Some(McpProtocolMode::DiscoverLocalStdio)
    } else if supported.iter().any(is_known_legacy_protocol_version) {
        Some(McpProtocolMode::Legacy)
    } else {
        None
    }
}

fn should_restart_legacy(
    error: &ClientInitializeError,
    discovery: Option<DiscoveryOutcome>,
) -> bool {
    match error {
        // A helper that assumes its first request is initialize can answer discover with this.
        ClientInitializeError::ExpectedInitResult(Some(ServerResult::InitializeResult(result))) => {
            !matches!(discovery, Some(DiscoveryOutcome::Reply))
                && is_known_legacy_protocol_version(&result.protocol_version)
        }
        ClientInitializeError::NoCompatibleProtocolVersion {
            server_supported, ..
        } => restart_for_versions(server_supported) == Some(McpProtocolMode::Legacy),
        ClientInitializeError::LegacyFallbackFailed { discover, fallback } => {
            match fallback.as_ref() {
                ClientInitializeError::ConnectionClosed(_) => true,
                // A legacy helper can reject initialize if discover already took its first request.
                ClientInitializeError::JsonRpcError(_) => matches!(
                    discover.as_ref(),
                    ClientInitializeError::JsonRpcError(error) if error.code == ErrorCode::METHOD_NOT_FOUND
                ),
                ClientInitializeError::TransportError { error, .. } => {
                    error.error.downcast_ref::<io::Error>().is_some_and(|e| {
                        matches!(
                            e.kind(),
                            io::ErrorKind::BrokenPipe | io::ErrorKind::ConnectionReset
                        )
                    })
                }
                _ => false,
            }
        }
        _ => matches!(discovery, Some(DiscoveryOutcome::RetryLegacy)),
    }
}

use super::invalid_request;
use codex_app_server_protocol::JSONRPCErrorError;
use codex_core::CodexThread;
use codex_protocol::protocol::MultiAgentVersion;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::SubAgentSource;

pub(super) const DIRECT_INPUT_TO_MULTI_AGENT_V2_SUBAGENT_ERROR: &str =
    "direct app-server input is not allowed for multi-agent v2 sub-agents";

/// Mirrors the ownership policy in request validation and thread capability responses.
pub(super) fn can_accept_direct_input(
    multi_agent_version: Option<MultiAgentVersion>,
    session_source: &SessionSource,
) -> bool {
    multi_agent_version != Some(MultiAgentVersion::V2)
        || !matches!(
            session_source,
            SessionSource::SubAgent(SubAgentSource::ThreadSpawn { .. })
        )
}

pub(super) async fn ensure_direct_input_allowed(
    thread: &CodexThread,
) -> Result<(), JSONRPCErrorError> {
    let config_snapshot = thread.config_snapshot().await;
    if !can_accept_direct_input(
        thread.multi_agent_version(),
        &config_snapshot.session_source,
    ) {
        return Err(invalid_request(
            DIRECT_INPUT_TO_MULTI_AGENT_V2_SUBAGENT_ERROR,
        ));
    }

    Ok(())
}

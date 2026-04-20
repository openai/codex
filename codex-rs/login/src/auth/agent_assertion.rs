use anyhow::Result;
use codex_agent_identity::AgentIdentityKey;
pub use codex_agent_identity::AgentTaskAuthorizationTarget;

use super::storage::AgentIdentityAuthRecord;

pub(super) fn authorization_header_for_agent_task(
    record: &AgentIdentityAuthRecord,
    target: AgentTaskAuthorizationTarget<'_>,
) -> Result<String> {
    codex_agent_identity::authorization_header_for_agent_task(
        AgentIdentityKey {
            agent_runtime_id: &record.agent_runtime_id,
            private_key_pkcs8_base64: &record.agent_private_key,
        },
        target,
    )
}

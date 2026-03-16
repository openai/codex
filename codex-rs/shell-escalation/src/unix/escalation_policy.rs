use codex_utils_absolute_path::AbsolutePathBuf;

use crate::unix::escalate_protocol::EscalationDecision;

/// Decides what action to take in response to an execve request from a client.
#[async_trait::async_trait]
pub trait EscalationPolicy: Send + Sync {
    async fn determine_action(
        &self,
        file: &AbsolutePathBuf,
        argv: &[String],
        workdir: &AbsolutePathBuf,
    ) -> anyhow::Result<EscalationDecision>;
}

use std::path::Path;

use crate::posix::escalate_protocol::EscalateAction;

#[async_trait::async_trait]
pub(crate) trait EscalationPolicy: Send + Sync {
    async fn determine_action(
        &self,
        file: &Path,
        argv: &[String],
        workdir: &Path,
    ) -> Result<EscalateAction, rmcp::ErrorData>;
}

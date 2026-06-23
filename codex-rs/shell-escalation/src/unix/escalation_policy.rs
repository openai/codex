use std::future::Future;
use std::pin::Pin;

use codex_utils_absolute_path::AbsolutePathBuf;

use crate::unix::escalate_protocol::EscalationDecision;

/// Decides what action to take in response to an execve request from a client.
pub trait EscalationPolicy: Send + Sync {
    fn determine_action<'a>(
        &'a self,
        file: &'a AbsolutePathBuf,
        argv: &'a [String],
        workdir: &'a AbsolutePathBuf,
    ) -> EscalationPolicyFuture<'a>;
}

pub type EscalationPolicyFuture<'a> =
    Pin<Box<dyn Future<Output = anyhow::Result<EscalationDecision>> + Send + 'a>>;

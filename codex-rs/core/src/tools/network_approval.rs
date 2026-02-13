use crate::codex::NetworkApprovalOutcome;
use crate::codex::Session;
use crate::network_policy_decision::denied_network_policy_message;
use crate::tools::sandboxing::ToolError;
use codex_network_proxy::NetworkProxy;
use std::path::PathBuf;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum NetworkApprovalMode {
    Immediate,
    Deferred,
}

#[derive(Clone, Debug)]
pub(crate) struct NetworkApprovalSpec {
    pub command: Vec<String>,
    pub cwd: PathBuf,
    pub network: Option<NetworkProxy>,
    pub mode: NetworkApprovalMode,
}

#[derive(Clone, Debug)]
pub(crate) struct DeferredNetworkApproval {
    attempt_id: String,
    network: Option<NetworkProxy>,
}

impl DeferredNetworkApproval {
    pub(crate) fn attempt_id(&self) -> &str {
        &self.attempt_id
    }
}

#[derive(Debug)]
pub(crate) struct ActiveNetworkApproval {
    attempt_id: Option<String>,
    network: Option<NetworkProxy>,
    mode: NetworkApprovalMode,
}

impl ActiveNetworkApproval {
    pub(crate) fn attempt_id(&self) -> Option<&str> {
        self.attempt_id.as_deref()
    }

    pub(crate) fn mode(&self) -> NetworkApprovalMode {
        self.mode
    }

    pub(crate) fn into_deferred(self) -> Option<DeferredNetworkApproval> {
        match (self.mode, self.attempt_id) {
            (NetworkApprovalMode::Deferred, Some(attempt_id)) => Some(DeferredNetworkApproval {
                attempt_id,
                network: self.network,
            }),
            _ => None,
        }
    }
}

pub(crate) async fn begin_network_approval(
    session: &Session,
    turn_id: &str,
    call_id: &str,
    has_managed_network_requirements: bool,
    spec: Option<NetworkApprovalSpec>,
) -> Option<ActiveNetworkApproval> {
    let spec = spec?;
    if !has_managed_network_requirements || spec.network.is_none() {
        return None;
    }

    let attempt_id = Uuid::new_v4().to_string();
    session
        .register_network_approval_attempt(
            attempt_id.clone(),
            turn_id.to_string(),
            call_id.to_string(),
            spec.command,
            spec.cwd,
        )
        .await;

    Some(ActiveNetworkApproval {
        attempt_id: Some(attempt_id),
        network: spec.network,
        mode: spec.mode,
    })
}

pub(crate) async fn finish_immediate_network_approval(
    session: &Session,
    active: ActiveNetworkApproval,
) -> Result<(), ToolError> {
    let Some(attempt_id) = active.attempt_id.as_deref() else {
        return Ok(());
    };

    let approval_outcome = session.take_network_approval_outcome(attempt_id).await;
    let denied_message =
        blocked_message_for_attempt(active.network.as_ref(), Some(attempt_id)).await;

    session
        .unregister_network_approval_attempt(attempt_id)
        .await;

    if approval_outcome == Some(NetworkApprovalOutcome::DeniedByUser) {
        return Err(ToolError::Rejected("rejected by user".to_string()));
    }
    if let Some(message) = denied_message {
        return Err(ToolError::Rejected(message));
    }

    Ok(())
}

pub(crate) async fn blocked_message_for_attempt(
    network: Option<&NetworkProxy>,
    attempt_id: Option<&str>,
) -> Option<String> {
    let (Some(network), Some(attempt_id)) = (network, attempt_id) else {
        return None;
    };

    match network.latest_blocked_request_for_attempt(attempt_id).await {
        Ok(Some(blocked)) => denied_network_policy_message(&blocked),
        Ok(None) => None,
        Err(err) => {
            tracing::debug!(
                "failed to read blocked network telemetry for attempt {attempt_id}: {err}"
            );
            None
        }
    }
}

pub(crate) async fn deferred_rejection_message(
    session: &Session,
    deferred: &DeferredNetworkApproval,
) -> Option<String> {
    if session
        .take_network_approval_outcome(deferred.attempt_id())
        .await
        == Some(NetworkApprovalOutcome::DeniedByUser)
    {
        return Some("rejected by user".to_string());
    }

    blocked_message_for_attempt(deferred.network.as_ref(), Some(deferred.attempt_id())).await
}

pub(crate) async fn finish_deferred_network_approval(
    session: &Session,
    deferred: Option<DeferredNetworkApproval>,
) {
    let Some(deferred) = deferred else {
        return;
    };
    session
        .unregister_network_approval_attempt(deferred.attempt_id())
        .await;
}

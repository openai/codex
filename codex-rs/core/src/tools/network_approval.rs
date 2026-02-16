use crate::codex::Session;
use crate::network_policy_decision::denied_network_policy_message;
use crate::tools::sandboxing::ToolError;
use codex_network_proxy::BlockedRequest;
use codex_network_proxy::BlockedRequestObserver;
use codex_network_proxy::NetworkDecision;
use codex_network_proxy::NetworkPolicyDecider;
use codex_network_proxy::NetworkPolicyRequest;
use codex_network_proxy::NetworkProtocol;
use codex_network_proxy::NetworkProxy;
use codex_protocol::approvals::NetworkApprovalContext;
use codex_protocol::approvals::NetworkApprovalProtocol;
use codex_protocol::protocol::ReviewDecision;
use std::collections::HashMap;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::sync::RwLock;
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
}

impl DeferredNetworkApproval {
    pub(crate) fn attempt_id(&self) -> &str {
        &self.attempt_id
    }
}

#[derive(Debug)]
pub(crate) struct ActiveNetworkApproval {
    attempt_id: Option<String>,
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
            (NetworkApprovalMode::Deferred, Some(attempt_id)) => {
                Some(DeferredNetworkApproval { attempt_id })
            }
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum NetworkApprovalOutcome {
    DeniedByUser,
    DeniedByPolicy(String),
}

struct NetworkApprovalAttempt {
    turn_id: String,
    call_id: String,
    command: Vec<String>,
    cwd: PathBuf,
    approved_hosts: Mutex<HashSet<String>>,
    outcome: Mutex<Option<NetworkApprovalOutcome>>,
}

pub(crate) struct NetworkApprovalService {
    attempts: Mutex<HashMap<String, Arc<NetworkApprovalAttempt>>>,
    session_approved_hosts: Mutex<HashSet<String>>,
}

impl Default for NetworkApprovalService {
    fn default() -> Self {
        Self {
            attempts: Mutex::new(HashMap::new()),
            session_approved_hosts: Mutex::new(HashSet::new()),
        }
    }
}

impl NetworkApprovalService {
    pub(crate) async fn register_attempt(
        &self,
        attempt_id: String,
        turn_id: String,
        call_id: String,
        command: Vec<String>,
        cwd: PathBuf,
    ) {
        let mut attempts = self.attempts.lock().await;
        attempts.insert(
            attempt_id,
            Arc::new(NetworkApprovalAttempt {
                turn_id,
                call_id,
                command,
                cwd,
                approved_hosts: Mutex::new(HashSet::new()),
                outcome: Mutex::new(None),
            }),
        );
    }

    pub(crate) async fn unregister_attempt(&self, attempt_id: &str) {
        let mut attempts = self.attempts.lock().await;
        attempts.remove(attempt_id);
    }

    pub(crate) async fn take_outcome(&self, attempt_id: &str) -> Option<NetworkApprovalOutcome> {
        let attempt = {
            let attempts = self.attempts.lock().await;
            attempts.get(attempt_id).cloned()
        }?;
        let mut outcome = attempt.outcome.lock().await;
        outcome.take()
    }

    pub(crate) async fn take_user_denial_outcome(&self, attempt_id: &str) -> bool {
        let attempt = {
            let attempts = self.attempts.lock().await;
            attempts.get(attempt_id).cloned()
        };
        let Some(attempt) = attempt else {
            return false;
        };
        let mut outcome = attempt.outcome.lock().await;
        if matches!(outcome.as_ref(), Some(NetworkApprovalOutcome::DeniedByUser)) {
            outcome.take();
            return true;
        }
        false
    }

    async fn resolve_attempt_for_request(
        &self,
        request: &NetworkPolicyRequest,
    ) -> Option<Arc<NetworkApprovalAttempt>> {
        let attempts = self.attempts.lock().await;

        if let Some(attempt_id) = request.attempt_id.as_deref() {
            if let Some(attempt) = attempts.get(attempt_id).cloned() {
                return Some(attempt);
            }
            return None;
        }

        if attempts.len() == 1 {
            return attempts.values().next().cloned();
        }

        None
    }

    async fn resolve_attempt_for_blocked_request(
        &self,
        blocked: &BlockedRequest,
    ) -> Option<Arc<NetworkApprovalAttempt>> {
        let attempts = self.attempts.lock().await;

        if let Some(attempt_id) = blocked.attempt_id.as_deref() {
            if let Some(attempt) = attempts.get(attempt_id).cloned() {
                return Some(attempt);
            }
            return None;
        }

        if attempts.len() == 1 {
            return attempts.values().next().cloned();
        }

        None
    }

    pub(crate) async fn record_blocked_request(&self, blocked: BlockedRequest) {
        let Some(message) = denied_network_policy_message(&blocked) else {
            return;
        };

        let Some(attempt) = self.resolve_attempt_for_blocked_request(&blocked).await else {
            return;
        };

        let mut outcome = attempt.outcome.lock().await;
        if matches!(outcome.as_ref(), Some(NetworkApprovalOutcome::DeniedByUser)) {
            return;
        }
        *outcome = Some(NetworkApprovalOutcome::DeniedByPolicy(message));
    }

    pub(crate) async fn handle_inline_policy_request(
        &self,
        session: &Session,
        request: NetworkPolicyRequest,
    ) -> NetworkDecision {
        const REASON_NOT_ALLOWED: &str = "not_allowed";

        {
            let approved_hosts = self.session_approved_hosts.lock().await;
            if approved_hosts.contains(request.host.as_str()) {
                return NetworkDecision::Allow;
            }
        }

        let Some(attempt) = self.resolve_attempt_for_request(&request).await else {
            return NetworkDecision::deny(REASON_NOT_ALLOWED);
        };

        {
            let approved_hosts = attempt.approved_hosts.lock().await;
            if approved_hosts.contains(request.host.as_str()) {
                return NetworkDecision::Allow;
            }
        }

        let protocol = match request.protocol {
            NetworkProtocol::Http => NetworkApprovalProtocol::Http,
            NetworkProtocol::HttpsConnect => NetworkApprovalProtocol::Https,
            NetworkProtocol::Socks5Tcp => NetworkApprovalProtocol::Socks5Tcp,
            NetworkProtocol::Socks5Udp => NetworkApprovalProtocol::Socks5Udp,
        };

        let Some(turn_context) = session.turn_context_for_sub_id(&attempt.turn_id).await else {
            return NetworkDecision::deny(REASON_NOT_ALLOWED);
        };

        let approval_decision = session
            .request_command_approval(
                turn_context.as_ref(),
                attempt.call_id.clone(),
                attempt.command.clone(),
                attempt.cwd.clone(),
                Some(format!(
                    "Network access to \"{}\" is blocked by policy.",
                    request.host
                )),
                Some(NetworkApprovalContext {
                    host: request.host.clone(),
                    protocol,
                }),
                None,
                None,
            )
            .await;

        match approval_decision {
            ReviewDecision::Approved | ReviewDecision::ApprovedExecpolicyAmendment { .. } => {
                let mut approved_hosts = attempt.approved_hosts.lock().await;
                approved_hosts.insert(request.host);
                NetworkDecision::Allow
            }
            ReviewDecision::ApprovedForSession => {
                let mut approved_hosts = self.session_approved_hosts.lock().await;
                approved_hosts.insert(request.host);
                NetworkDecision::Allow
            }
            ReviewDecision::Denied | ReviewDecision::Abort => {
                let mut outcome = attempt.outcome.lock().await;
                *outcome = Some(NetworkApprovalOutcome::DeniedByUser);
                NetworkDecision::deny(REASON_NOT_ALLOWED)
            }
        }
    }
}

pub(crate) fn build_blocked_request_observer(
    network_approval: Arc<NetworkApprovalService>,
) -> Arc<dyn BlockedRequestObserver> {
    Arc::new(move |blocked: BlockedRequest| {
        let network_approval = Arc::clone(&network_approval);
        async move {
            network_approval.record_blocked_request(blocked).await;
        }
    })
}

pub(crate) fn build_network_policy_decider(
    network_approval: Arc<NetworkApprovalService>,
    network_policy_decider_session: Arc<RwLock<std::sync::Weak<Session>>>,
) -> Arc<dyn NetworkPolicyDecider> {
    Arc::new(move |request: NetworkPolicyRequest| {
        let network_approval = Arc::clone(&network_approval);
        let network_policy_decider_session = Arc::clone(&network_policy_decider_session);
        async move {
            let Some(session) = network_policy_decider_session.read().await.upgrade() else {
                return NetworkDecision::ask("not_allowed");
            };
            network_approval
                .handle_inline_policy_request(session.as_ref(), request)
                .await
        }
    })
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
        .services
        .network_approval
        .register_attempt(
            attempt_id.clone(),
            turn_id.to_string(),
            call_id.to_string(),
            spec.command,
            spec.cwd,
        )
        .await;

    Some(ActiveNetworkApproval {
        attempt_id: Some(attempt_id),
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

    let approval_outcome = session
        .services
        .network_approval
        .take_outcome(attempt_id)
        .await;

    session
        .services
        .network_approval
        .unregister_attempt(attempt_id)
        .await;

    match approval_outcome {
        Some(NetworkApprovalOutcome::DeniedByUser) => {
            Err(ToolError::Rejected("rejected by user".to_string()))
        }
        Some(NetworkApprovalOutcome::DeniedByPolicy(message)) => Err(ToolError::Rejected(message)),
        None => Ok(()),
    }
}

pub(crate) async fn deferred_rejection_message(
    session: &Session,
    deferred: &DeferredNetworkApproval,
) -> Option<String> {
    match session
        .services
        .network_approval
        .take_outcome(deferred.attempt_id())
        .await
    {
        Some(NetworkApprovalOutcome::DeniedByUser) => Some("rejected by user".to_string()),
        Some(NetworkApprovalOutcome::DeniedByPolicy(message)) => Some(message),
        None => None,
    }
}

pub(crate) async fn finish_deferred_network_approval(
    session: &Session,
    deferred: Option<DeferredNetworkApproval>,
) {
    let Some(deferred) = deferred else {
        return;
    };
    session
        .services
        .network_approval
        .unregister_attempt(deferred.attempt_id())
        .await;
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_network_proxy::BlockedRequestArgs;
    use codex_network_proxy::NetworkPolicyRequestArgs;
    use pretty_assertions::assert_eq;

    fn http_request(host: &str, attempt_id: Option<&str>) -> NetworkPolicyRequest {
        NetworkPolicyRequest::new(NetworkPolicyRequestArgs {
            protocol: NetworkProtocol::Http,
            host: host.to_string(),
            port: 80,
            client_addr: None,
            method: Some("GET".to_string()),
            command: None,
            exec_policy_hint: None,
            attempt_id: attempt_id.map(ToString::to_string),
        })
    }

    #[tokio::test]
    async fn resolve_attempt_for_request_falls_back_to_single_active_attempt() {
        let service = NetworkApprovalService::default();
        service
            .register_attempt(
                "attempt-1".to_string(),
                "turn-1".to_string(),
                "call-1".to_string(),
                vec!["curl".to_string(), "example.com".to_string()],
                std::env::temp_dir(),
            )
            .await;

        let resolved = service
            .resolve_attempt_for_request(&http_request("example.com", None))
            .await
            .expect("single active attempt should be used as fallback");
        assert_eq!(resolved.call_id, "call-1");
    }

    #[tokio::test]
    async fn resolve_attempt_for_request_returns_exact_attempt_match() {
        let service = NetworkApprovalService::default();
        service
            .register_attempt(
                "attempt-1".to_string(),
                "turn-1".to_string(),
                "call-1".to_string(),
                vec!["curl".to_string(), "example.com".to_string()],
                std::env::temp_dir(),
            )
            .await;
        service
            .register_attempt(
                "attempt-2".to_string(),
                "turn-2".to_string(),
                "call-2".to_string(),
                vec!["curl".to_string(), "openai.com".to_string()],
                std::env::temp_dir(),
            )
            .await;

        let resolved = service
            .resolve_attempt_for_request(&http_request("openai.com", Some("attempt-2")))
            .await
            .expect("attempt-2 should resolve");
        assert_eq!(resolved.call_id, "call-2");
    }

    #[tokio::test]
    async fn resolve_attempt_for_request_returns_none_for_unknown_attempt_id() {
        let service = NetworkApprovalService::default();
        service
            .register_attempt(
                "attempt-1".to_string(),
                "turn-1".to_string(),
                "call-1".to_string(),
                vec!["curl".to_string(), "example.com".to_string()],
                std::env::temp_dir(),
            )
            .await;

        let resolved = service
            .resolve_attempt_for_request(&http_request("example.com", Some("attempt-unknown")))
            .await;
        assert!(resolved.is_none());
    }

    #[tokio::test]
    async fn resolve_attempt_for_request_returns_none_when_ambiguous() {
        let service = NetworkApprovalService::default();
        service
            .register_attempt(
                "attempt-1".to_string(),
                "turn-1".to_string(),
                "call-1".to_string(),
                vec!["curl".to_string(), "example.com".to_string()],
                std::env::temp_dir(),
            )
            .await;
        service
            .register_attempt(
                "attempt-2".to_string(),
                "turn-2".to_string(),
                "call-2".to_string(),
                vec!["curl".to_string(), "robinhood.com".to_string()],
                std::env::temp_dir(),
            )
            .await;

        let resolved = service
            .resolve_attempt_for_request(&http_request("example.com", None))
            .await;
        assert!(resolved.is_none());
    }

    #[tokio::test]
    async fn take_outcome_clears_stored_value() {
        let service = NetworkApprovalService::default();
        service
            .register_attempt(
                "attempt-1".to_string(),
                "turn-1".to_string(),
                "call-1".to_string(),
                vec!["curl".to_string(), "example.com".to_string()],
                std::env::temp_dir(),
            )
            .await;

        let attempt = {
            let attempts = service.attempts.lock().await;
            attempts
                .get("attempt-1")
                .cloned()
                .expect("attempt should exist")
        };
        {
            let mut outcome = attempt.outcome.lock().await;
            *outcome = Some(NetworkApprovalOutcome::DeniedByUser);
        }

        assert_eq!(
            service.take_outcome("attempt-1").await,
            Some(NetworkApprovalOutcome::DeniedByUser)
        );
        assert_eq!(service.take_outcome("attempt-1").await, None);
    }

    #[tokio::test]
    async fn take_user_denial_outcome_preserves_policy_denial() {
        let service = NetworkApprovalService::default();
        service
            .register_attempt(
                "attempt-1".to_string(),
                "turn-1".to_string(),
                "call-1".to_string(),
                vec!["curl".to_string(), "example.com".to_string()],
                std::env::temp_dir(),
            )
            .await;

        let attempt = {
            let attempts = service.attempts.lock().await;
            attempts
                .get("attempt-1")
                .cloned()
                .expect("attempt should exist")
        };
        {
            let mut outcome = attempt.outcome.lock().await;
            *outcome = Some(NetworkApprovalOutcome::DeniedByPolicy(
                "policy denied".to_string(),
            ));
        }

        assert!(!service.take_user_denial_outcome("attempt-1").await);
        assert_eq!(
            service.take_outcome("attempt-1").await,
            Some(NetworkApprovalOutcome::DeniedByPolicy(
                "policy denied".to_string(),
            ))
        );
    }

    #[tokio::test]
    async fn record_blocked_request_stores_policy_denial_outcome() {
        let service = NetworkApprovalService::default();
        service
            .register_attempt(
                "attempt-1".to_string(),
                "turn-1".to_string(),
                "call-1".to_string(),
                vec!["curl".to_string(), "example.com".to_string()],
                std::env::temp_dir(),
            )
            .await;

        service
            .record_blocked_request(BlockedRequest::new(BlockedRequestArgs {
                host: "example.com".to_string(),
                reason: "denied".to_string(),
                client: None,
                method: Some("GET".to_string()),
                mode: None,
                protocol: "http".to_string(),
                attempt_id: Some("attempt-1".to_string()),
                decision: Some("deny".to_string()),
                source: Some("baseline_policy".to_string()),
                port: Some(80),
            }))
            .await;

        let outcome = service
            .take_outcome("attempt-1")
            .await
            .expect("outcome should be recorded");
        match outcome {
            NetworkApprovalOutcome::DeniedByPolicy(message) => {
                assert_eq!(
                    message,
                    "Network access to \"example.com\" was blocked: domain is explicitly denied by policy and cannot be approved from this prompt.".to_string()
                );
            }
            NetworkApprovalOutcome::DeniedByUser => panic!("expected policy denial"),
        }
    }

    #[tokio::test]
    async fn record_blocked_request_does_not_override_user_denial() {
        let service = NetworkApprovalService::default();
        service
            .register_attempt(
                "attempt-1".to_string(),
                "turn-1".to_string(),
                "call-1".to_string(),
                vec!["curl".to_string(), "example.com".to_string()],
                std::env::temp_dir(),
            )
            .await;

        let attempt = {
            let attempts = service.attempts.lock().await;
            attempts
                .get("attempt-1")
                .cloned()
                .expect("attempt should exist")
        };
        {
            let mut outcome = attempt.outcome.lock().await;
            *outcome = Some(NetworkApprovalOutcome::DeniedByUser);
        }

        service
            .record_blocked_request(BlockedRequest::new(BlockedRequestArgs {
                host: "example.com".to_string(),
                reason: "denied".to_string(),
                client: None,
                method: Some("GET".to_string()),
                mode: None,
                protocol: "http".to_string(),
                attempt_id: Some("attempt-1".to_string()),
                decision: Some("deny".to_string()),
                source: Some("baseline_policy".to_string()),
                port: Some(80),
            }))
            .await;

        assert_eq!(
            service.take_outcome("attempt-1").await,
            Some(NetworkApprovalOutcome::DeniedByUser)
        );
    }
}

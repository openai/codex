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
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::ReviewDecision;
use indexmap::IndexMap;
use std::collections::HashMap;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::sync::Notify;
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
    registration_id: String,
}

impl DeferredNetworkApproval {
    pub(crate) fn registration_id(&self) -> &str {
        &self.registration_id
    }
}

#[derive(Debug)]
pub(crate) struct ActiveNetworkApproval {
    registration_id: Option<String>,
    mode: NetworkApprovalMode,
}

impl ActiveNetworkApproval {
    pub(crate) fn mode(&self) -> NetworkApprovalMode {
        self.mode
    }

    pub(crate) fn into_deferred(self) -> Option<DeferredNetworkApproval> {
        match (self.mode, self.registration_id) {
            (NetworkApprovalMode::Deferred, Some(registration_id)) => {
                Some(DeferredNetworkApproval { registration_id })
            }
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct HostApprovalKey {
    host: String,
    protocol: &'static str,
    port: u16,
}

impl HostApprovalKey {
    fn from_request(request: &NetworkPolicyRequest, protocol: NetworkApprovalProtocol) -> Self {
        Self {
            host: request.host.to_ascii_lowercase(),
            protocol: protocol_key_label(protocol),
            port: request.port,
        }
    }
}

fn protocol_key_label(protocol: NetworkApprovalProtocol) -> &'static str {
    match protocol {
        NetworkApprovalProtocol::Http => "http",
        NetworkApprovalProtocol::Https => "https",
        NetworkApprovalProtocol::Socks5Tcp => "socks5-tcp",
        NetworkApprovalProtocol::Socks5Udp => "socks5-udp",
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PendingApprovalDecision {
    AllowOnce,
    AllowForSession,
    Deny,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum NetworkApprovalOutcome {
    DeniedByUser,
    DeniedByPolicy(String),
}

fn allows_network_prompt(policy: AskForApproval) -> bool {
    !matches!(policy, AskForApproval::Never)
}

impl PendingApprovalDecision {
    fn to_network_decision(self) -> NetworkDecision {
        match self {
            Self::AllowOnce | Self::AllowForSession => NetworkDecision::Allow,
            Self::Deny => NetworkDecision::deny("not_allowed"),
        }
    }
}

struct PendingHostApproval {
    owner_call_id: String,
    decision: Mutex<Option<PendingApprovalDecision>>,
    notify: Notify,
}

impl PendingHostApproval {
    fn new(owner_call_id: String) -> Self {
        Self {
            owner_call_id,
            decision: Mutex::new(None),
            notify: Notify::new(),
        }
    }

    fn owner_call_id(&self) -> &str {
        &self.owner_call_id
    }

    async fn wait_for_decision(&self) -> PendingApprovalDecision {
        loop {
            if let Some(decision) = *self.decision.lock().await {
                return decision;
            }
            self.notify.notified().await;
        }
    }

    async fn set_decision(&self, decision: PendingApprovalDecision) {
        {
            let mut current = self.decision.lock().await;
            *current = Some(decision);
        }
        self.notify.notify_waiters();
    }
}

struct ActiveNetworkApprovalCall {
    registration_id: String,
    turn_id: String,
    call_id: String,
    command: Vec<String>,
    cwd: PathBuf,
}

pub(crate) struct NetworkApprovalService {
    active_calls: Mutex<IndexMap<String, Arc<ActiveNetworkApprovalCall>>>,
    call_outcomes: Mutex<HashMap<String, NetworkApprovalOutcome>>,
    pending_host_approvals: Mutex<HashMap<HostApprovalKey, Arc<PendingHostApproval>>>,
    session_approved_hosts: Mutex<HashSet<HostApprovalKey>>,
}

impl Default for NetworkApprovalService {
    fn default() -> Self {
        Self {
            active_calls: Mutex::new(IndexMap::new()),
            call_outcomes: Mutex::new(HashMap::new()),
            pending_host_approvals: Mutex::new(HashMap::new()),
            session_approved_hosts: Mutex::new(HashSet::new()),
        }
    }
}

impl NetworkApprovalService {
    async fn register_call(
        &self,
        registration_id: String,
        turn_id: String,
        call_id: String,
        command: Vec<String>,
        cwd: PathBuf,
    ) {
        let mut active_calls = self.active_calls.lock().await;
        let key = registration_id.clone();
        active_calls.insert(
            key,
            Arc::new(ActiveNetworkApprovalCall {
                registration_id,
                turn_id,
                call_id,
                command,
                cwd,
            }),
        );
    }

    pub(crate) async fn unregister_call(&self, registration_id: &str) {
        let mut active_calls = self.active_calls.lock().await;
        active_calls.shift_remove(registration_id);
        let mut call_outcomes = self.call_outcomes.lock().await;
        call_outcomes.remove(registration_id);
    }

    async fn resolve_call_context(&self) -> Option<Arc<ActiveNetworkApprovalCall>> {
        let active_calls = self.active_calls.lock().await;
        if active_calls.len() == 1 {
            return active_calls.values().next().cloned();
        }

        None
    }

    async fn get_or_create_pending_approval(
        &self,
        key: HostApprovalKey,
        owner_call_id: &str,
    ) -> (Arc<PendingHostApproval>, bool) {
        let mut pending = self.pending_host_approvals.lock().await;
        if let Some(existing) = pending.get(&key).cloned() {
            return (existing, false);
        }

        let created = Arc::new(PendingHostApproval::new(owner_call_id.to_string()));
        pending.insert(key, Arc::clone(&created));
        (created, true)
    }

    async fn take_call_outcome(&self, registration_id: &str) -> Option<NetworkApprovalOutcome> {
        let mut call_outcomes = self.call_outcomes.lock().await;
        call_outcomes.remove(registration_id)
    }

    async fn record_call_outcome(&self, registration_id: &str, outcome: NetworkApprovalOutcome) {
        let mut call_outcomes = self.call_outcomes.lock().await;
        if matches!(
            call_outcomes.get(registration_id),
            Some(NetworkApprovalOutcome::DeniedByUser)
        ) {
            return;
        }
        call_outcomes.insert(registration_id.to_string(), outcome);
    }

    pub(crate) async fn record_blocked_request(&self, blocked: BlockedRequest) {
        let Some(message) = denied_network_policy_message(&blocked) else {
            return;
        };

        let Some(owner_call) = self.resolve_call_context().await else {
            return;
        };

        self.record_call_outcome(
            &owner_call.registration_id,
            NetworkApprovalOutcome::DeniedByPolicy(message),
        )
        .await;
    }

    pub(crate) async fn handle_inline_policy_request(
        &self,
        session: &Session,
        request: NetworkPolicyRequest,
    ) -> NetworkDecision {
        const REASON_NOT_ALLOWED: &str = "not_allowed";

        let protocol = match request.protocol {
            NetworkProtocol::Http => NetworkApprovalProtocol::Http,
            NetworkProtocol::HttpsConnect => NetworkApprovalProtocol::Https,
            NetworkProtocol::Socks5Tcp => NetworkApprovalProtocol::Socks5Tcp,
            NetworkProtocol::Socks5Udp => NetworkApprovalProtocol::Socks5Udp,
        };
        let key = HostApprovalKey::from_request(&request, protocol);

        {
            let approved_hosts = self.session_approved_hosts.lock().await;
            if approved_hosts.contains(&key) {
                return NetworkDecision::Allow;
            }
        }

        let Some(owner_call) = self.resolve_call_context().await else {
            return NetworkDecision::deny(REASON_NOT_ALLOWED);
        };

        let (pending, is_owner) = self
            .get_or_create_pending_approval(key.clone(), &owner_call.call_id)
            .await;
        if !is_owner {
            return pending.wait_for_decision().await.to_network_decision();
        }

        let Some(turn_context) = session.turn_context_for_sub_id(&owner_call.turn_id).await else {
            pending.set_decision(PendingApprovalDecision::Deny).await;
            let mut pending_approvals = self.pending_host_approvals.lock().await;
            pending_approvals.remove(&key);
            self.record_call_outcome(
                &owner_call.registration_id,
                NetworkApprovalOutcome::DeniedByPolicy(format!(
                    "Network access to \"{}\" was blocked by policy.",
                    request.host
                )),
            )
            .await;
            return NetworkDecision::deny(REASON_NOT_ALLOWED);
        };
        if !allows_network_prompt(turn_context.approval_policy) {
            pending.set_decision(PendingApprovalDecision::Deny).await;
            let mut pending_approvals = self.pending_host_approvals.lock().await;
            pending_approvals.remove(&key);
            self.record_call_outcome(
                &owner_call.registration_id,
                NetworkApprovalOutcome::DeniedByPolicy(format!(
                    "Network access to \"{}\" was blocked by policy.",
                    request.host
                )),
            )
            .await;
            return NetworkDecision::deny(REASON_NOT_ALLOWED);
        }

        let host = key.host.clone();
        let approval_id = format!(
            "{}#network#{}#{}#{}",
            pending.owner_call_id(),
            key.protocol,
            host,
            key.port
        );

        let approval_decision = session
            .request_command_approval(
                turn_context.as_ref(),
                approval_id,
                owner_call.command.clone(),
                owner_call.cwd.clone(),
                Some(format!(
                    "Network access to \"{}\" is blocked by policy.",
                    request.host
                )),
                Some(NetworkApprovalContext {
                    host: request.host.clone(),
                    protocol,
                }),
                None,
            )
            .await;

        let resolved = match approval_decision {
            ReviewDecision::Approved | ReviewDecision::ApprovedExecpolicyAmendment { .. } => {
                PendingApprovalDecision::AllowOnce
            }
            ReviewDecision::ApprovedForSession => PendingApprovalDecision::AllowForSession,
            ReviewDecision::Denied | ReviewDecision::Abort => {
                self.record_call_outcome(
                    &owner_call.registration_id,
                    NetworkApprovalOutcome::DeniedByUser,
                )
                .await;
                PendingApprovalDecision::Deny
            }
        };

        if matches!(resolved, PendingApprovalDecision::AllowForSession) {
            let mut approved_hosts = self.session_approved_hosts.lock().await;
            approved_hosts.insert(key.clone());
        }

        pending.set_decision(resolved).await;
        let mut pending_approvals = self.pending_host_approvals.lock().await;
        pending_approvals.remove(&key);

        resolved.to_network_decision()
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

    let registration_id = Uuid::new_v4().to_string();
    session
        .services
        .network_approval
        .register_call(
            registration_id.clone(),
            turn_id.to_string(),
            call_id.to_string(),
            spec.command,
            spec.cwd,
        )
        .await;

    Some(ActiveNetworkApproval {
        registration_id: Some(registration_id),
        mode: spec.mode,
    })
}

pub(crate) async fn finish_immediate_network_approval(
    session: &Session,
    active: ActiveNetworkApproval,
) -> Result<(), ToolError> {
    let Some(registration_id) = active.registration_id.as_deref() else {
        return Ok(());
    };

    let approval_outcome = session
        .services
        .network_approval
        .take_call_outcome(registration_id)
        .await;

    session
        .services
        .network_approval
        .unregister_call(registration_id)
        .await;

    match approval_outcome {
        Some(NetworkApprovalOutcome::DeniedByUser) => {
            Err(ToolError::Rejected("rejected by user".to_string()))
        }
        Some(NetworkApprovalOutcome::DeniedByPolicy(message)) => Err(ToolError::Rejected(message)),
        None => Ok(()),
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
        .unregister_call(deferred.registration_id())
        .await;
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_network_proxy::BlockedRequestArgs;
    use codex_protocol::protocol::AskForApproval;
    use pretty_assertions::assert_eq;

    #[tokio::test]
    async fn pending_approvals_are_deduped_per_host_protocol_and_port() {
        let service = NetworkApprovalService::default();
        let key = HostApprovalKey {
            host: "example.com".to_string(),
            protocol: "http",
            port: 443,
        };

        let (first, first_is_owner) = service
            .get_or_create_pending_approval(key.clone(), "call-a")
            .await;
        let (second, second_is_owner) = service.get_or_create_pending_approval(key, "call-b").await;

        assert!(first_is_owner);
        assert!(!second_is_owner);
        assert_eq!(first.owner_call_id(), "call-a");
        assert!(Arc::ptr_eq(&first, &second));
    }

    #[tokio::test]
    async fn pending_approvals_do_not_dedupe_across_ports() {
        let service = NetworkApprovalService::default();
        let first_key = HostApprovalKey {
            host: "example.com".to_string(),
            protocol: "https",
            port: 443,
        };
        let second_key = HostApprovalKey {
            host: "example.com".to_string(),
            protocol: "https",
            port: 8443,
        };

        let (first, first_is_owner) = service
            .get_or_create_pending_approval(first_key, "call-a")
            .await;
        let (second, second_is_owner) = service
            .get_or_create_pending_approval(second_key, "call-b")
            .await;

        assert!(first_is_owner);
        assert!(second_is_owner);
        assert!(!Arc::ptr_eq(&first, &second));
    }

    #[tokio::test]
    async fn pending_waiters_receive_owner_decision() {
        let pending = Arc::new(PendingHostApproval::new("call-a".to_string()));

        let waiter = {
            let pending = Arc::clone(&pending);
            tokio::spawn(async move { pending.wait_for_decision().await })
        };

        pending
            .set_decision(PendingApprovalDecision::AllowOnce)
            .await;

        let decision = waiter.await.expect("waiter should complete");
        assert_eq!(decision, PendingApprovalDecision::AllowOnce);
    }

    #[test]
    fn allow_once_and_allow_for_session_both_allow_network() {
        assert_eq!(
            PendingApprovalDecision::AllowOnce.to_network_decision(),
            NetworkDecision::Allow
        );
        assert_eq!(
            PendingApprovalDecision::AllowForSession.to_network_decision(),
            NetworkDecision::Allow
        );
    }

    #[test]
    fn never_policy_disables_network_prompts() {
        assert!(!allows_network_prompt(AskForApproval::Never));
        assert!(allows_network_prompt(AskForApproval::OnRequest));
        assert!(allows_network_prompt(AskForApproval::OnFailure));
        assert!(allows_network_prompt(AskForApproval::UnlessTrusted));
    }

    fn denied_blocked_request(host: &str) -> BlockedRequest {
        BlockedRequest::new(BlockedRequestArgs {
            host: host.to_string(),
            reason: "not_allowed".to_string(),
            client: None,
            method: None,
            mode: None,
            protocol: "http".to_string(),
            decision: Some("deny".to_string()),
            source: Some("decider".to_string()),
            port: Some(80),
        })
    }

    #[tokio::test]
    async fn record_blocked_request_sets_policy_outcome_for_owner_call() {
        let service = NetworkApprovalService::default();
        service
            .register_call(
                "registration-1".to_string(),
                "turn-1".to_string(),
                "call-1".to_string(),
                vec!["curl".to_string(), "example.com".to_string()],
                PathBuf::from("/tmp"),
            )
            .await;

        service
            .record_blocked_request(denied_blocked_request("example.com"))
            .await;

        assert_eq!(
            service.take_call_outcome("registration-1").await,
            Some(NetworkApprovalOutcome::DeniedByPolicy(
                "Network access to \"example.com\" was blocked: domain is not on the allowlist for the current sandbox mode.".to_string()
            ))
        );
    }

    #[tokio::test]
    async fn blocked_request_policy_does_not_override_user_denial_outcome() {
        let service = NetworkApprovalService::default();
        service
            .register_call(
                "registration-1".to_string(),
                "turn-1".to_string(),
                "call-1".to_string(),
                vec!["curl".to_string(), "example.com".to_string()],
                PathBuf::from("/tmp"),
            )
            .await;

        service
            .record_call_outcome("registration-1", NetworkApprovalOutcome::DeniedByUser)
            .await;
        service
            .record_blocked_request(denied_blocked_request("example.com"))
            .await;

        assert_eq!(
            service.take_call_outcome("registration-1").await,
            Some(NetworkApprovalOutcome::DeniedByUser)
        );
    }

    #[tokio::test]
    async fn record_blocked_request_ignores_ambiguous_unattributed_blocked_requests() {
        let service = NetworkApprovalService::default();
        service
            .register_call(
                "registration-1".to_string(),
                "turn-1".to_string(),
                "call-1".to_string(),
                vec!["curl".to_string(), "example.com".to_string()],
                PathBuf::from("/tmp"),
            )
            .await;
        service
            .register_call(
                "registration-2".to_string(),
                "turn-2".to_string(),
                "call-2".to_string(),
                vec!["curl".to_string(), "example.org".to_string()],
                PathBuf::from("/tmp"),
            )
            .await;

        service
            .record_blocked_request(denied_blocked_request("example.com"))
            .await;

        assert_eq!(service.take_call_outcome("registration-1").await, None);
        assert_eq!(service.take_call_outcome("registration-2").await, None);
    }
}

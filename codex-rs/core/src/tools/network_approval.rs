use crate::codex::Session;
use crate::guardian::GUARDIAN_REJECTION_MESSAGE;
use crate::guardian::GuardianApprovalRequest;
use crate::guardian::review_approval_request;
use crate::guardian::routes_approval_to_guardian;
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
use codex_protocol::approvals::NetworkPolicyRuleAction;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::Event;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::ReviewDecision;
use codex_protocol::protocol::WarningEvent;
use indexmap::IndexMap;
use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::sync::Notify;
use tokio::sync::RwLock;
use tracing::warn;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum NetworkApprovalMode {
    Immediate,
    Deferred,
}

#[derive(Clone, Debug)]
pub(crate) struct NetworkApprovalSpec {
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

/// Whether an allowlist miss may be reviewed instead of hard-denied.
fn allows_network_approval_flow(policy: AskForApproval) -> bool {
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
    decision: Mutex<Option<PendingApprovalDecision>>,
    notify: Notify,
}

impl PendingHostApproval {
    fn new() -> Self {
        Self {
            decision: Mutex::new(None),
            notify: Notify::new(),
        }
    }

    async fn wait_for_decision(&self) -> PendingApprovalDecision {
        loop {
            let notified = self.notify.notified();
            if let Some(decision) = *self.decision.lock().await {
                return decision;
            }
            notified.await;
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
}

pub(crate) struct NetworkApprovalService {
    active_calls: Mutex<IndexMap<String, Arc<ActiveNetworkApprovalCall>>>,
    call_outcomes: Mutex<HashMap<String, NetworkApprovalOutcome>>,
    pending_host_approvals: Mutex<HashMap<HostApprovalKey, Arc<PendingHostApproval>>>,
    session_approved_hosts: Mutex<HashSet<HostApprovalKey>>,
    session_denied_hosts: Mutex<HashSet<HostApprovalKey>>,
}

impl Default for NetworkApprovalService {
    fn default() -> Self {
        Self {
            active_calls: Mutex::new(IndexMap::new()),
            call_outcomes: Mutex::new(HashMap::new()),
            pending_host_approvals: Mutex::new(HashMap::new()),
            session_approved_hosts: Mutex::new(HashSet::new()),
            session_denied_hosts: Mutex::new(HashSet::new()),
        }
    }
}

impl NetworkApprovalService {
    pub(crate) async fn copy_session_approved_hosts_to(&self, other: &Self) {
        let approved_hosts = self.session_approved_hosts.lock().await;
        let mut other_approved_hosts = other.session_approved_hosts.lock().await;
        other_approved_hosts.extend(approved_hosts.iter().cloned());
    }

    async fn register_call(&self, registration_id: String, turn_id: String) {
        let mut active_calls = self.active_calls.lock().await;
        let key = registration_id.clone();
        active_calls.insert(
            key,
            Arc::new(ActiveNetworkApprovalCall {
                registration_id,
                turn_id,
            }),
        );
    }

    pub(crate) async fn unregister_call(&self, registration_id: &str) {
        let mut active_calls = self.active_calls.lock().await;
        active_calls.shift_remove(registration_id);
        let mut call_outcomes = self.call_outcomes.lock().await;
        call_outcomes.remove(registration_id);
    }

    async fn resolve_single_active_call(&self) -> Option<Arc<ActiveNetworkApprovalCall>> {
        let active_calls = self.active_calls.lock().await;
        if active_calls.len() == 1 {
            return active_calls.values().next().cloned();
        }

        None
    }

    async fn get_or_create_pending_approval(
        &self,
        key: HostApprovalKey,
    ) -> (Arc<PendingHostApproval>, bool) {
        let mut pending = self.pending_host_approvals.lock().await;
        if let Some(existing) = pending.get(&key).cloned() {
            return (existing, false);
        }

        let created = Arc::new(PendingHostApproval::new());
        pending.insert(key, Arc::clone(&created));
        (created, true)
    }

    async fn record_outcome_for_single_active_call(&self, outcome: NetworkApprovalOutcome) {
        let Some(owner_call) = self.resolve_single_active_call().await else {
            return;
        };
        self.record_call_outcome(&owner_call.registration_id, outcome)
            .await;
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

        self.record_outcome_for_single_active_call(NetworkApprovalOutcome::DeniedByPolicy(message))
            .await;
    }

    async fn active_turn_context(session: &Session) -> Option<Arc<crate::codex::TurnContext>> {
        let active_turn = session.active_turn.lock().await;
        active_turn
            .as_ref()
            .and_then(|turn| turn.tasks.first())
            .map(|(_, task)| Arc::clone(&task.turn_context))
    }

    fn format_network_target(protocol: &str, host: &str, port: u16) -> String {
        format!("{protocol}://{host}:{port}")
    }

    fn approval_id_for_key(key: &HostApprovalKey) -> String {
        format!("network#{}#{}#{}", key.protocol, key.host, key.port)
    }

    pub(crate) async fn handle_inline_policy_request(
        &self,
        session: Arc<Session>,
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
            let denied_hosts = self.session_denied_hosts.lock().await;
            if denied_hosts.contains(&key) {
                return NetworkDecision::deny(REASON_NOT_ALLOWED);
            }
        }

        {
            let approved_hosts = self.session_approved_hosts.lock().await;
            if approved_hosts.contains(&key) {
                return NetworkDecision::Allow;
            }
        }

        let (pending, is_owner) = self.get_or_create_pending_approval(key.clone()).await;
        if !is_owner {
            return pending.wait_for_decision().await.to_network_decision();
        }

        let target = Self::format_network_target(key.protocol, request.host.as_str(), key.port);
        let policy_denial_message =
            format!("Network access to \"{target}\" was blocked by policy.");
        let prompt_reason = format!("{} is not in the allowed_domains", request.host);

        let Some(turn_context) = Self::active_turn_context(session.as_ref()).await else {
            pending.set_decision(PendingApprovalDecision::Deny).await;
            let mut pending_approvals = self.pending_host_approvals.lock().await;
            pending_approvals.remove(&key);
            self.record_outcome_for_single_active_call(NetworkApprovalOutcome::DeniedByPolicy(
                policy_denial_message,
            ))
            .await;
            return NetworkDecision::deny(REASON_NOT_ALLOWED);
        };
        if !allows_network_approval_flow(turn_context.approval_policy.value()) {
            pending.set_decision(PendingApprovalDecision::Deny).await;
            let mut pending_approvals = self.pending_host_approvals.lock().await;
            pending_approvals.remove(&key);
            self.record_outcome_for_single_active_call(NetworkApprovalOutcome::DeniedByPolicy(
                policy_denial_message,
            ))
            .await;
            return NetworkDecision::deny(REASON_NOT_ALLOWED);
        }

        let network_approval_context = NetworkApprovalContext {
            host: request.host.clone(),
            protocol,
        };
        let owner_call = self.resolve_single_active_call().await;
        let approval_decision = if routes_approval_to_guardian(&turn_context) {
            // TODO(ccunningham): Attach guardian network reviews to the reviewed tool item
            // lifecycle instead of this temporary standalone network approval id.
            review_approval_request(
                &session,
                &turn_context,
                GuardianApprovalRequest::NetworkAccess {
                    id: Self::approval_id_for_key(&key),
                    turn_id: owner_call
                        .as_ref()
                        .map_or_else(|| turn_context.sub_id.clone(), |call| call.turn_id.clone()),
                    target,
                    host: request.host,
                    protocol,
                    port: key.port,
                },
                Some(policy_denial_message.clone()),
            )
            .await
        } else {
            let approval_id = Self::approval_id_for_key(&key);
            let prompt_command = vec!["network-access".to_string(), target.clone()];
            let available_decisions = None;
            session
                .request_command_approval(
                    turn_context.as_ref(),
                    approval_id,
                    None,
                    prompt_command,
                    turn_context.cwd.clone(),
                    Some(prompt_reason),
                    Some(network_approval_context.clone()),
                    None,
                    None,
                    None,
                    available_decisions,
                )
                .await
        };

        let mut cache_session_deny = false;
        let resolved = match approval_decision {
            ReviewDecision::Approved
            | ReviewDecision::ApprovedOverrideCommand { .. }
            | ReviewDecision::ApprovedExecpolicyAmendment { .. } => {
                PendingApprovalDecision::AllowOnce
            }
            ReviewDecision::ApprovedForSession => PendingApprovalDecision::AllowForSession,
            ReviewDecision::NetworkPolicyAmendment {
                network_policy_amendment,
            } => match network_policy_amendment.action {
                NetworkPolicyRuleAction::Allow => {
                    match session
                        .persist_network_policy_amendment(
                            &network_policy_amendment,
                            &network_approval_context,
                        )
                        .await
                    {
                        Ok(()) => {
                            session
                                .record_network_policy_amendment_message(
                                    &turn_context.sub_id,
                                    &network_policy_amendment,
                                )
                                .await;
                        }
                        Err(err) => {
                            let message =
                                format!("Failed to apply network policy amendment: {err}");
                            warn!("{message}");
                            session
                                .send_event_raw(Event {
                                    id: turn_context.sub_id.clone(),
                                    msg: EventMsg::Warning(WarningEvent { message }),
                                })
                                .await;
                        }
                    }
                    PendingApprovalDecision::AllowForSession
                }
                NetworkPolicyRuleAction::Deny => {
                    match session
                        .persist_network_policy_amendment(
                            &network_policy_amendment,
                            &network_approval_context,
                        )
                        .await
                    {
                        Ok(()) => {
                            session
                                .record_network_policy_amendment_message(
                                    &turn_context.sub_id,
                                    &network_policy_amendment,
                                )
                                .await;
                        }
                        Err(err) => {
                            let message =
                                format!("Failed to apply network policy amendment: {err}");
                            warn!("{message}");
                            session
                                .send_event_raw(Event {
                                    id: turn_context.sub_id.clone(),
                                    msg: EventMsg::Warning(WarningEvent { message }),
                                })
                                .await;
                        }
                    }
                    if let Some(owner_call) = owner_call.as_ref() {
                        self.record_call_outcome(
                            &owner_call.registration_id,
                            NetworkApprovalOutcome::DeniedByUser,
                        )
                        .await;
                    }
                    cache_session_deny = true;
                    PendingApprovalDecision::Deny
                }
            },
            ReviewDecision::Denied | ReviewDecision::Abort => {
                if routes_approval_to_guardian(&turn_context) {
                    if let Some(owner_call) = owner_call.as_ref() {
                        self.record_call_outcome(
                            &owner_call.registration_id,
                            NetworkApprovalOutcome::DeniedByPolicy(
                                GUARDIAN_REJECTION_MESSAGE.to_string(),
                            ),
                        )
                        .await;
                    }
                } else if let Some(owner_call) = owner_call.as_ref() {
                    self.record_call_outcome(
                        &owner_call.registration_id,
                        NetworkApprovalOutcome::DeniedByUser,
                    )
                    .await;
                }
                PendingApprovalDecision::Deny
            }
        };

        if matches!(resolved, PendingApprovalDecision::AllowForSession) {
            {
                let mut denied_hosts = self.session_denied_hosts.lock().await;
                denied_hosts.remove(&key);
            }
            let mut approved_hosts = self.session_approved_hosts.lock().await;
            approved_hosts.insert(key.clone());
        }

        if cache_session_deny {
            {
                let mut approved_hosts = self.session_approved_hosts.lock().await;
                approved_hosts.remove(&key);
            }
            let mut denied_hosts = self.session_denied_hosts.lock().await;
            denied_hosts.insert(key.clone());
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
                .handle_inline_policy_request(session, request)
                .await
        }
    })
}

pub(crate) async fn begin_network_approval(
    session: &Session,
    turn_id: &str,
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
        .register_call(registration_id.clone(), turn_id.to_string())
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
#[path = "network_approval_tests.rs"]
mod tests;

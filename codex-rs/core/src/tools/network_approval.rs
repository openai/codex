use crate::guardian::GuardianApprovalRequest;
use crate::guardian::GuardianNetworkAccessTrigger;
use crate::guardian::GuardianReviewOptions;
use crate::guardian::new_guardian_review_id;
use crate::guardian::review_approval_request_with_cancel;
use crate::guardian::routes_approval_to_guardian;
use crate::hook_runtime::run_permission_request_hooks;
use crate::network_policy_decision::denied_network_policy_message;
use crate::session::session::Session;
use crate::session::turn_context::TurnEnvironment;
use crate::tools::events::truncate_rejection_message;
use crate::tools::sandboxing::PermissionRequestPayload;
use crate::tools::sandboxing::ToolError;
use codex_analytics::GuardianApprovalRequestSource;
use codex_hooks::PermissionRequestDecision;
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
use codex_protocol::models::PermissionProfile;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::Event;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::ReviewDecision;
use codex_protocol::protocol::WarningEvent;
use codex_sandboxing::record_network_sandbox_violation;
use indexmap::IndexMap;
use std::collections::HashMap;
use std::collections::HashSet;
use std::io;
use std::sync::Arc;
use std::sync::Mutex as SyncMutex;
use std::sync::OnceLock as SyncOnceLock;
use tokio::sync::Mutex;
use tokio::sync::Notify;
use tokio::sync::OnceCell;
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;
use tracing::error;
use tracing::warn;
use uuid::Uuid;

const ABANDONED_NETWORK_APPROVAL_MESSAGE: &str =
    "network approval was cancelled before a decision was returned";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum NetworkApprovalMode {
    Immediate,
    Deferred,
}

#[derive(Clone, Debug)]
pub(crate) struct NetworkApprovalSpec {
    pub network: Option<NetworkProxy>,
    pub mode: NetworkApprovalMode,
    pub trigger: GuardianNetworkAccessTrigger,
    pub command: String,
    pub environment_id: String,
    pub permission_profile: PermissionProfile,
}

#[derive(Clone, Debug)]
pub(crate) struct DeferredNetworkApproval {
    registration_id: String,
    cancellation_token: CancellationToken,
    finish_outcome: Arc<OnceCell<Option<NetworkApprovalOutcome>>>,
    _execution_proxy: Option<NetworkProxy>,
}

impl DeferredNetworkApproval {
    pub(crate) fn registration_id(&self) -> &str {
        &self.registration_id
    }

    pub(crate) fn cancellation_token(&self) -> CancellationToken {
        self.cancellation_token.clone()
    }

    pub(crate) fn is_cancelled(&self) -> bool {
        self.cancellation_token.is_cancelled()
    }

    async fn finish(&self, service: &NetworkApprovalService) -> Result<(), ToolError> {
        let outcome = self
            .finish_outcome
            .get_or_init(|| async { service.finish_call_outcome(&self.registration_id).await })
            .await
            .clone();
        let outcome =
            outcome.or_else(|| abandoned_network_approval_outcome(&self.cancellation_token));
        network_approval_outcome_to_result(outcome)
    }
}

#[derive(Debug)]
pub(crate) struct ActiveNetworkApproval {
    registration_id: Option<String>,
    mode: NetworkApprovalMode,
    cancellation_token: CancellationToken,
    execution_proxy: NetworkProxy,
}

impl ActiveNetworkApproval {
    pub(crate) fn mode(&self) -> NetworkApprovalMode {
        self.mode
    }

    pub(crate) fn cancellation_token(&self) -> CancellationToken {
        self.cancellation_token.clone()
    }

    pub(crate) fn execution_proxy(&self) -> &NetworkProxy {
        &self.execution_proxy
    }

    pub(crate) fn into_deferred(self) -> Option<DeferredNetworkApproval> {
        let ActiveNetworkApproval {
            registration_id,
            mode,
            cancellation_token,
            execution_proxy,
        } = self;
        match (mode, registration_id) {
            (NetworkApprovalMode::Deferred, Some(registration_id)) => {
                Some(DeferredNetworkApproval {
                    registration_id,
                    cancellation_token,
                    finish_outcome: Arc::new(OnceCell::new()),
                    _execution_proxy: Some(execution_proxy),
                })
            }
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct HostApprovalKey {
    environment_id: String,
    host: String,
    protocol: &'static str,
    port: u16,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct PendingHostApprovalKey {
    host: HostApprovalKey,
    turn_id: String,
    execution_id: Option<String>,
}

impl HostApprovalKey {
    fn from_request(
        request: &NetworkPolicyRequest,
        protocol: NetworkApprovalProtocol,
        environment_id: String,
    ) -> Self {
        Self {
            environment_id,
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
    DeniedByApproval(String),
    DeniedByPolicy(String),
}

fn network_approval_outcome_to_result(
    outcome: Option<NetworkApprovalOutcome>,
) -> Result<(), ToolError> {
    match outcome {
        Some(NetworkApprovalOutcome::DeniedByApproval(rejection)) => {
            Err(ToolError::Rejected(truncate_rejection_message(&rejection)))
        }
        Some(NetworkApprovalOutcome::DeniedByPolicy(message)) => {
            Err(ToolError::Rejected(truncate_rejection_message(&message)))
        }
        None => Ok(()),
    }
}

fn abandoned_network_approval_outcome(
    cancellation_token: &CancellationToken,
) -> Option<NetworkApprovalOutcome> {
    cancellation_token.is_cancelled().then(|| {
        NetworkApprovalOutcome::DeniedByPolicy(ABANDONED_NETWORK_APPROVAL_MESSAGE.to_string())
    })
}

/// Whether an allowlist miss may be reviewed instead of hard-denied.
fn allows_network_approval_flow(policy: AskForApproval) -> bool {
    !matches!(policy, AskForApproval::Never)
}

fn permission_profile_allows_network_approval_flow(permission_profile: &PermissionProfile) -> bool {
    matches!(permission_profile, PermissionProfile::Managed { .. })
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
    decision: SyncOnceLock<PendingApprovalDecision>,
    notify: Notify,
}

impl PendingHostApproval {
    fn new() -> Self {
        Self {
            decision: SyncOnceLock::new(),
            notify: Notify::new(),
        }
    }

    async fn wait_for_decision(&self) -> PendingApprovalDecision {
        loop {
            let notified = self.notify.notified();
            if let Some(decision) = self.decision.get() {
                return *decision;
            }
            notified.await;
        }
    }

    fn set_decision(&self, decision: PendingApprovalDecision) {
        if self.decision.set(decision).is_ok() {
            self.notify.notify_waiters();
        }
    }
}

struct ActiveNetworkApprovalCall {
    registration_id: String,
    turn_id: String,
    trigger: GuardianNetworkAccessTrigger,
    command: String,
    environment_id: String,
    permission_profile: PermissionProfile,
    cancellation_token: CancellationToken,
}

enum ActiveNetworkApprovalAttribution {
    None,
    Single(Arc<ActiveNetworkApprovalCall>),
    Ambiguous,
}

struct NetworkRequestAttribution {
    owner_call: Option<Arc<ActiveNetworkApprovalCall>>,
    environment_id: Option<String>,
}

#[derive(Default)]
struct NetworkApprovalCallState {
    active_calls: IndexMap<String, Arc<ActiveNetworkApprovalCall>>,
    call_outcomes: HashMap<String, NetworkApprovalOutcome>,
}

pub(crate) struct NetworkApprovalService {
    calls: Mutex<NetworkApprovalCallState>,
    // Owner cleanup runs from Drop, so this lock cannot require an async task.
    pending_host_approvals: SyncMutex<HashMap<PendingHostApprovalKey, Arc<PendingHostApproval>>>,
    // Keep persisted session policy and the in-memory approval caches in the same order.
    session_policy_commit_lock: Mutex<()>,
    session_approved_hosts: Mutex<HashSet<HostApprovalKey>>,
    session_denied_hosts: Mutex<HashSet<HostApprovalKey>>,
}

/// Removes and resolves the exact pending generation created by one request.
///
/// Dropping an unfinished owner fails its existing waiters closed without
/// deleting a newer same-host request.
struct PendingHostApprovalOwner<'a> {
    service: &'a NetworkApprovalService,
    key: PendingHostApprovalKey,
    pending: Arc<PendingHostApproval>,
    execution_cancellation: Option<CancellationToken>,
    decision_on_drop: PendingApprovalDecision,
    completed: bool,
}

impl<'a> PendingHostApprovalOwner<'a> {
    fn new(
        service: &'a NetworkApprovalService,
        key: PendingHostApprovalKey,
        pending: Arc<PendingHostApproval>,
        execution_cancellation: Option<CancellationToken>,
    ) -> Self {
        Self {
            service,
            key,
            pending,
            execution_cancellation,
            decision_on_drop: PendingApprovalDecision::Deny,
            completed: false,
        }
    }

    fn set_decision_on_drop(&mut self, decision: PendingApprovalDecision) {
        self.decision_on_drop = decision;
    }

    fn complete(mut self, decision: PendingApprovalDecision) {
        self.cancel_execution_if_denied(decision);
        self.publish_and_remove(decision);
        self.completed = true;
    }

    fn cancel_execution_if_denied(&self, decision: PendingApprovalDecision) {
        if matches!(decision, PendingApprovalDecision::Deny)
            && let Some(execution_cancellation) = &self.execution_cancellation
        {
            execution_cancellation.cancel();
        }
    }

    fn publish_and_remove(&self, decision: PendingApprovalDecision) {
        // Remove this generation before waking its waiters so a concurrent retry
        // cannot attach to an already-completed approval.
        let mut approvals = self
            .service
            .pending_host_approvals
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if approvals
            .get(&self.key)
            .is_some_and(|current| Arc::ptr_eq(current, &self.pending))
        {
            approvals.remove(&self.key);
        }
        drop(approvals);
        self.pending.set_decision(decision);
    }
}

impl Drop for PendingHostApprovalOwner<'_> {
    fn drop(&mut self) {
        if !self.completed {
            self.cancel_execution_if_denied(self.decision_on_drop);
            self.publish_and_remove(self.decision_on_drop);
        }
    }
}

impl Default for NetworkApprovalService {
    fn default() -> Self {
        Self {
            calls: Mutex::new(NetworkApprovalCallState::default()),
            pending_host_approvals: SyncMutex::new(HashMap::new()),
            session_policy_commit_lock: Mutex::new(()),
            session_approved_hosts: Mutex::new(HashSet::new()),
            session_denied_hosts: Mutex::new(HashSet::new()),
        }
    }
}

impl NetworkApprovalService {
    /// Replace the target session's approval cache with the source session's
    /// currently approved hosts.
    #[expect(
        clippy::await_holding_invalid_type,
        reason = "the approval snapshot must not interleave with a session policy commit"
    )]
    pub(crate) async fn sync_session_approved_hosts_to(&self, other: &Self) {
        let _commit_guard = self.session_policy_commit_lock.lock().await;
        let approved_hosts = self.session_approved_hosts.lock().await.clone();
        let mut other_approved_hosts = other.session_approved_hosts.lock().await;
        other_approved_hosts.clear();
        other_approved_hosts.extend(approved_hosts.iter().cloned());
    }

    async fn register_call(&self, call: ActiveNetworkApprovalCall) {
        let mut calls = self.calls.lock().await;
        calls
            .active_calls
            .insert(call.registration_id.clone(), Arc::new(call));
    }

    pub(crate) async fn unregister_call(&self, registration_id: &str) {
        self.remove_call(registration_id).await;
    }

    async fn resolve_single_active_call(&self) -> Option<Arc<ActiveNetworkApprovalCall>> {
        let calls = self.calls.lock().await;
        // Shared proxy requests can still arrive without an execution ID. Only pick an owner when
        // there is exactly one candidate; with concurrent calls, canceling one would be a guess.
        if calls.active_calls.len() == 1 {
            return calls.active_calls.values().next().cloned();
        }

        None
    }

    async fn resolve_active_call_by_execution_id(
        &self,
        execution_id: &str,
    ) -> Option<Arc<ActiveNetworkApprovalCall>> {
        self.calls
            .lock()
            .await
            .active_calls
            .get(execution_id)
            .cloned()
    }

    async fn resolve_active_call_attribution(&self) -> ActiveNetworkApprovalAttribution {
        let calls = self.calls.lock().await;
        match calls.active_calls.len() {
            0 => ActiveNetworkApprovalAttribution::None,
            1 => calls.active_calls.values().next().cloned().map_or(
                ActiveNetworkApprovalAttribution::None,
                ActiveNetworkApprovalAttribution::Single,
            ),
            _ => ActiveNetworkApprovalAttribution::Ambiguous,
        }
    }

    async fn resolve_request_attribution(
        &self,
        request: &NetworkPolicyRequest,
    ) -> Option<NetworkRequestAttribution> {
        if let Some(execution_id) = request.execution_id.as_deref() {
            let call = self
                .resolve_active_call_by_execution_id(execution_id)
                .await?;
            let environment_id = request
                .environment_id
                .clone()
                .unwrap_or_else(|| call.environment_id.clone());
            return (call.environment_id == environment_id).then_some(NetworkRequestAttribution {
                owner_call: Some(call),
                environment_id: Some(environment_id),
            });
        }

        if let Some(environment_id) = request.environment_id.clone() {
            let owner_call = match self.resolve_active_call_attribution().await {
                ActiveNetworkApprovalAttribution::Single(call) => {
                    (call.environment_id == environment_id).then_some(call)
                }
                ActiveNetworkApprovalAttribution::None
                | ActiveNetworkApprovalAttribution::Ambiguous => None,
            };
            return Some(NetworkRequestAttribution {
                owner_call,
                environment_id: Some(environment_id),
            });
        }

        match self.resolve_active_call_attribution().await {
            ActiveNetworkApprovalAttribution::None => Some(NetworkRequestAttribution {
                owner_call: None,
                environment_id: None,
            }),
            ActiveNetworkApprovalAttribution::Single(call) => {
                let environment_id = call.environment_id.clone();
                Some(NetworkRequestAttribution {
                    owner_call: Some(call),
                    environment_id: Some(environment_id),
                })
            }
            ActiveNetworkApprovalAttribution::Ambiguous => None,
        }
    }

    fn get_or_create_pending_approval(
        &self,
        key: PendingHostApprovalKey,
    ) -> (Arc<PendingHostApproval>, bool) {
        let mut pending = self
            .pending_host_approvals
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(existing) = pending.get(&key).cloned() {
            return (existing, false);
        }

        let created = Arc::new(PendingHostApproval::new());
        pending.insert(key, Arc::clone(&created));
        (created, true)
    }

    #[cfg(test)]
    async fn take_call_outcome(&self, registration_id: &str) -> Option<NetworkApprovalOutcome> {
        let mut calls = self.calls.lock().await;
        calls.call_outcomes.remove(registration_id)
    }

    async fn record_call_outcome(&self, registration_id: &str, outcome: NetworkApprovalOutcome) {
        let mut calls = self.calls.lock().await;
        let Some(call) = calls.active_calls.get(registration_id).cloned() else {
            return;
        };
        if matches!(
            calls.call_outcomes.get(registration_id),
            Some(NetworkApprovalOutcome::DeniedByApproval(_))
        ) {
            return;
        }
        calls
            .call_outcomes
            .insert(registration_id.to_string(), outcome);

        drop(calls);
        call.cancellation_token.cancel();
    }

    async fn remove_call(&self, registration_id: &str) -> Option<NetworkApprovalOutcome> {
        let mut calls = self.calls.lock().await;
        calls.active_calls.shift_remove(registration_id);
        calls.call_outcomes.remove(registration_id)
    }

    async fn finish_call_outcome(&self, registration_id: &str) -> Option<NetworkApprovalOutcome> {
        self.remove_call(registration_id).await
    }

    async fn finish_call(
        &self,
        registration_id: &str,
        cancellation_token: &CancellationToken,
    ) -> Result<(), ToolError> {
        let outcome = self
            .finish_call_outcome(registration_id)
            .await
            .or_else(|| abandoned_network_approval_outcome(cancellation_token));
        network_approval_outcome_to_result(outcome)
    }

    pub(crate) async fn record_blocked_request(&self, blocked: BlockedRequest) {
        let Some(message) = denied_network_policy_message(&blocked) else {
            return;
        };

        let owner_call = if let Some(execution_id) = blocked.execution_id.as_deref() {
            self.resolve_active_call_by_execution_id(execution_id).await
        } else {
            self.resolve_single_active_call().await
        };
        let Some(owner_call) = owner_call else {
            return;
        };

        let mut calls = self.calls.lock().await;
        if calls
            .call_outcomes
            .contains_key(&owner_call.registration_id)
        {
            return;
        }
        calls.call_outcomes.insert(
            owner_call.registration_id.clone(),
            NetworkApprovalOutcome::DeniedByPolicy(message),
        );

        drop(calls);
        owner_call.cancellation_token.cancel();
    }

    async fn active_turn_context(
        session: &Session,
    ) -> Option<Arc<crate::session::turn_context::TurnContext>> {
        let active_turn = session.active_turn.lock().await;
        active_turn
            .as_ref()
            .and_then(|turn| turn.task.as_ref())
            .map(|task| Arc::clone(&task.turn_context))
    }

    fn format_network_target(protocol: &str, host: &str, port: u16) -> String {
        format!("{protocol}://{host}:{port}")
    }

    fn approval_id_for_key(key: &HostApprovalKey) -> String {
        format!(
            "network#{}#{}#{}#{}",
            key.environment_id, key.protocol, key.host, key.port
        )
    }

    #[expect(
        clippy::await_holding_invalid_type,
        reason = "persisted session policy and its in-memory caches must commit atomically"
    )]
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
        let Some(NetworkRequestAttribution {
            owner_call,
            environment_id: active_environment_id,
        }) = self.resolve_request_attribution(&request).await
        else {
            return NetworkDecision::deny(REASON_NOT_ALLOWED);
        };
        let turn_context = Self::active_turn_context(session.as_ref()).await;
        let Some(environment_id) = active_environment_id.or_else(|| {
            turn_context
                .as_ref()
                .and_then(|turn_context| turn_context.environments.primary())
                .map(|environment| environment.environment_id.clone())
        }) else {
            return NetworkDecision::deny(REASON_NOT_ALLOWED);
        };
        let key = HostApprovalKey::from_request(&request, protocol, environment_id.clone());

        {
            let _commit_guard = self.session_policy_commit_lock.lock().await;
            {
                let denied_hosts = self.session_denied_hosts.lock().await;
                if denied_hosts.contains(&key) {
                    return NetworkDecision::deny(REASON_NOT_ALLOWED);
                }
            }

            let approved_hosts = self.session_approved_hosts.lock().await;
            if approved_hosts.contains(&key) {
                return NetworkDecision::Allow;
            }
        }

        let target = Self::format_network_target(key.protocol, request.host.as_str(), key.port);
        let policy_denial_message =
            format!("Network access to \"{target}\" was blocked by policy.");
        let prompt_reason = format!("{} is not in the allowed_domains", request.host);

        let Some(turn_context) = turn_context else {
            if let Some(owner_call) = owner_call.as_ref() {
                self.record_call_outcome(
                    &owner_call.registration_id,
                    NetworkApprovalOutcome::DeniedByPolicy(policy_denial_message),
                )
                .await;
            }
            return NetworkDecision::deny(REASON_NOT_ALLOWED);
        };
        let pending_key = PendingHostApprovalKey {
            host: key.clone(),
            turn_id: owner_call
                .as_ref()
                .map_or_else(|| turn_context.sub_id.clone(), |call| call.turn_id.clone()),
            execution_id: owner_call
                .as_ref()
                .map(|owner_call| owner_call.registration_id.clone()),
        };
        let (pending, is_owner) = self.get_or_create_pending_approval(pending_key.clone());
        if !is_owner {
            return pending.wait_for_decision().await.to_network_decision();
        }
        let mut pending_owner = PendingHostApprovalOwner::new(
            self,
            pending_key,
            Arc::clone(&pending),
            owner_call
                .as_ref()
                .map(|call| call.cancellation_token.clone()),
        );

        let permission_profile = owner_call
            .as_ref()
            .map(|call| &call.permission_profile)
            .or_else(|| {
                turn_context
                    .environments
                    .turn_environments()
                    .find(|environment| environment.environment_id == environment_id)
                    .map(TurnEnvironment::permission_profile)
            });
        if !permission_profile.is_some_and(permission_profile_allows_network_approval_flow) {
            if let Some(owner_call) = owner_call.as_ref() {
                self.record_call_outcome(
                    &owner_call.registration_id,
                    NetworkApprovalOutcome::DeniedByPolicy(policy_denial_message),
                )
                .await;
            }
            pending_owner.complete(PendingApprovalDecision::Deny);
            return NetworkDecision::deny(REASON_NOT_ALLOWED);
        }
        if !allows_network_approval_flow(turn_context.approval_policy()) {
            if let Some(owner_call) = owner_call.as_ref() {
                self.record_call_outcome(
                    &owner_call.registration_id,
                    NetworkApprovalOutcome::DeniedByPolicy(policy_denial_message),
                )
                .await;
            }
            pending_owner.complete(PendingApprovalDecision::Deny);
            return NetworkDecision::deny(REASON_NOT_ALLOWED);
        }

        let network_approval_context = NetworkApprovalContext {
            host: request.host.clone(),
            protocol,
        };
        let guardian_approval_id = Self::approval_id_for_key(&key);
        let hook_run_id_suffix = owner_call.as_ref().map_or_else(
            || guardian_approval_id.clone(),
            |call| format!("{guardian_approval_id}#{}", call.registration_id),
        );
        let prompt_command = vec!["network-access".to_string(), target.clone()];
        let command = owner_call
            .as_ref()
            .map_or_else(|| prompt_command.join(" "), |call| call.command.clone());
        let hook_approval_decision = match run_permission_request_hooks(
            &session,
            &turn_context,
            &hook_run_id_suffix,
            PermissionRequestPayload::bash(command, Some(format!("network-access {target}"))),
        )
        .await
        {
            Some(PermissionRequestDecision::Allow) => Some(ReviewDecision::Approved),
            Some(PermissionRequestDecision::Deny { message }) => {
                if let Some(owner_call) = owner_call.as_ref() {
                    self.record_call_outcome(
                        &owner_call.registration_id,
                        NetworkApprovalOutcome::DeniedByPolicy(message),
                    )
                    .await;
                }
                pending_owner.complete(PendingApprovalDecision::Deny);
                return NetworkDecision::deny(REASON_NOT_ALLOWED);
            }
            None => None,
        };
        let use_guardian = routes_approval_to_guardian(&turn_context);
        let guardian_review_id = use_guardian.then(new_guardian_review_id);
        let approval_decision = if let Some(hook_approval_decision) = hook_approval_decision {
            hook_approval_decision
        } else if let Some(review_id) = guardian_review_id.clone() {
            let review_cancel = CancellationToken::new();
            let review_cancel_guard = review_cancel.clone().drop_guard();
            let review_session = Arc::clone(&session);
            let review_turn = Arc::clone(&turn_context);
            let review_request = GuardianApprovalRequest::NetworkAccess {
                id: guardian_approval_id.clone(),
                turn_id: owner_call
                    .as_ref()
                    .map_or_else(|| turn_context.sub_id.clone(), |call| call.turn_id.clone()),
                target: target.clone(),
                host: request.host.clone(),
                protocol,
                port: key.port,
                trigger: owner_call.as_ref().map(|call| call.trigger.clone()),
            };
            let retry_reason = Some(policy_denial_message.clone());
            let review = tokio::spawn(async move {
                review_approval_request_with_cancel(
                    &review_session,
                    &review_turn,
                    review_id,
                    review_request,
                    retry_reason,
                    GuardianReviewOptions {
                        plugin_attribution_override: None,
                        approval_request_source: GuardianApprovalRequestSource::MainTurn,
                        external_cancel: Some(review_cancel),
                    },
                )
                .await
            });
            let decision = review.await.unwrap_or_else(|err| {
                warn!("network Guardian review task failed: {err}");
                ReviewDecision::denied("automatic approval review could not complete")
            });
            drop(review_cancel_guard.disarm());
            decision
        } else {
            let available_decisions = None;
            let cwd = if let Some(owner_call) = owner_call.as_ref() {
                owner_call.trigger.cwd.clone()
            } else {
                turn_context
                    .environments
                    .turn_environments()
                    .find(|environment| environment.environment_id == environment_id)
                    .and_then(|environment| environment.cwd().to_abs_path().ok())
                    .unwrap_or_else(|| {
                        #[allow(deprecated)]
                        turn_context.cwd.clone()
                    })
            };
            let approval_call_id = format!("{guardian_approval_id}#{}", Uuid::new_v4());
            session
                .request_command_approval(
                    turn_context.as_ref(),
                    approval_call_id,
                    /*approval_id*/ None,
                    Some(environment_id),
                    prompt_command,
                    cwd,
                    Some(prompt_reason),
                    Some(network_approval_context.clone()),
                    /*proposed_execpolicy_amendment*/ None,
                    /*additional_permissions*/ None,
                    available_decisions,
                    /*plugin_attribution_override*/ None,
                )
                .await
        };

        let _session_policy_commit_guard = if matches!(
            &approval_decision,
            ReviewDecision::Approved
                | ReviewDecision::ApprovedExecpolicyAmendment { .. }
                | ReviewDecision::ApprovedForSession
                | ReviewDecision::NetworkPolicyAmendment { .. }
        ) {
            Some(self.session_policy_commit_lock.lock().await)
        } else {
            None
        };
        let resolved = match approval_decision {
            ReviewDecision::Approved | ReviewDecision::ApprovedExecpolicyAmendment { .. } => {
                if self.session_denied_hosts.lock().await.contains(&key) {
                    if let Some(owner_call) = owner_call.as_ref() {
                        self.record_call_outcome(
                            &owner_call.registration_id,
                            NetworkApprovalOutcome::DeniedByPolicy(policy_denial_message.clone()),
                        )
                        .await;
                    }
                    PendingApprovalDecision::Deny
                } else {
                    PendingApprovalDecision::AllowOnce
                }
            }
            ReviewDecision::ApprovedForSession => {
                if self.session_denied_hosts.lock().await.contains(&key) {
                    if let Some(owner_call) = owner_call.as_ref() {
                        self.record_call_outcome(
                            &owner_call.registration_id,
                            NetworkApprovalOutcome::DeniedByPolicy(policy_denial_message.clone()),
                        )
                        .await;
                    }
                    PendingApprovalDecision::Deny
                } else {
                    self.session_approved_hosts.lock().await.insert(key.clone());
                    PendingApprovalDecision::AllowForSession
                }
            }
            ReviewDecision::NetworkPolicyAmendment {
                network_policy_amendment,
            } => match network_policy_amendment.action {
                NetworkPolicyRuleAction::Allow => {
                    match session
                        .persist_network_policy_amendment(
                            &network_policy_amendment,
                            &network_approval_context,
                            || {
                                pending_owner
                                    .set_decision_on_drop(PendingApprovalDecision::AllowForSession);
                            },
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
                    if pending_owner.decision_on_drop == PendingApprovalDecision::AllowForSession {
                        {
                            let mut denied_hosts = self.session_denied_hosts.lock().await;
                            denied_hosts.remove(&key);
                        }
                        self.session_approved_hosts.lock().await.insert(key.clone());
                        PendingApprovalDecision::AllowForSession
                    } else {
                        if let Some(owner_call) = owner_call.as_ref() {
                            self.record_call_outcome(
                                &owner_call.registration_id,
                                NetworkApprovalOutcome::DeniedByPolicy(
                                    policy_denial_message.clone(),
                                ),
                            )
                            .await;
                        }
                        PendingApprovalDecision::Deny
                    }
                }
                NetworkPolicyRuleAction::Deny => {
                    match session
                        .persist_network_policy_amendment(
                            &network_policy_amendment,
                            &network_approval_context,
                            || {
                                pending_owner.set_decision_on_drop(PendingApprovalDecision::Deny);
                            },
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
                            NetworkApprovalOutcome::DeniedByApproval(
                                "rejected by user".to_string(),
                            ),
                        )
                        .await;
                    }
                    {
                        let mut approved_hosts = self.session_approved_hosts.lock().await;
                        approved_hosts.remove(&key);
                    }
                    self.session_denied_hosts.lock().await.insert(key.clone());
                    PendingApprovalDecision::Deny
                }
            },
            ReviewDecision::ApprovedMcpPolicyAmendment => {
                error!("Network approval received ApprovedMcpPolicyAmendment");
                if let Some(owner_call) = owner_call.as_ref() {
                    let rejection = "Error while requesting approval".to_string();
                    let outcome = if use_guardian {
                        NetworkApprovalOutcome::DeniedByPolicy(rejection)
                    } else {
                        NetworkApprovalOutcome::DeniedByApproval(rejection)
                    };
                    self.record_call_outcome(&owner_call.registration_id, outcome)
                        .await;
                }
                PendingApprovalDecision::Deny
            }
            ReviewDecision::Denied { rejection } => {
                if let Some(owner_call) = owner_call.as_ref() {
                    let outcome = if use_guardian {
                        NetworkApprovalOutcome::DeniedByPolicy(rejection)
                    } else {
                        NetworkApprovalOutcome::DeniedByApproval(rejection)
                    };
                    self.record_call_outcome(&owner_call.registration_id, outcome)
                        .await;
                }
                PendingApprovalDecision::Deny
            }
            ReviewDecision::TimedOut => {
                if let Some(owner_call) = owner_call.as_ref() {
                    self.record_call_outcome(
                        &owner_call.registration_id,
                        NetworkApprovalOutcome::DeniedByPolicy(
                            crate::guardian::guardian_timeout_message(),
                        ),
                    )
                    .await;
                }
                PendingApprovalDecision::Deny
            }
            ReviewDecision::Abort => {
                if use_guardian {
                    if let Some(owner_call) = owner_call.as_ref() {
                        self.record_call_outcome(
                            &owner_call.registration_id,
                            NetworkApprovalOutcome::DeniedByPolicy(
                                "automatic approval review was cancelled".to_string(),
                            ),
                        )
                        .await;
                    }
                } else if let Some(owner_call) = owner_call.as_ref() {
                    self.record_call_outcome(
                        &owner_call.registration_id,
                        NetworkApprovalOutcome::DeniedByApproval("rejected by user".to_string()),
                    )
                    .await;
                }
                PendingApprovalDecision::Deny
            }
        };
        pending_owner.set_decision_on_drop(resolved);

        pending_owner.complete(resolved);

        resolved.to_network_decision()
    }
}

pub(crate) fn build_blocked_request_observer(
    network_approval: Arc<NetworkApprovalService>,
) -> Arc<dyn BlockedRequestObserver> {
    Arc::new(move |blocked: BlockedRequest| {
        let network_approval = Arc::clone(&network_approval);
        async move {
            record_network_sandbox_violation(&blocked);
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
    managed_network_active: bool,
    spec: Option<NetworkApprovalSpec>,
) -> Result<Option<ActiveNetworkApproval>, ToolError> {
    let NetworkApprovalSpec {
        network,
        mode,
        trigger,
        command,
        environment_id,
        permission_profile,
    } = match spec {
        Some(spec) => spec,
        None => return Ok(None),
    };
    let Some(network) = network else {
        return Ok(None);
    };
    if !managed_network_active {
        return Ok(None);
    }

    let registration_id = Uuid::new_v4().to_string();
    let attribution_token = Uuid::new_v4().to_string();
    let execution_proxy = network
        .for_execution(&environment_id, &registration_id, attribution_token)
        .map_err(|err| {
            ToolError::Codex(codex_protocol::error::CodexErr::Io(io::Error::other(
                format!("failed to create execution-scoped network proxy: {err}"),
            )))
        })?;
    let cancellation_token = CancellationToken::new();
    session
        .services
        .network_approval
        .register_call(ActiveNetworkApprovalCall {
            registration_id: registration_id.clone(),
            turn_id: turn_id.to_string(),
            trigger,
            command,
            environment_id,
            permission_profile,
            cancellation_token: cancellation_token.clone(),
        })
        .await;

    Ok(Some(ActiveNetworkApproval {
        registration_id: Some(registration_id),
        mode,
        cancellation_token,
        execution_proxy,
    }))
}

pub(crate) async fn finish_immediate_network_approval(
    session: &Session,
    active: ActiveNetworkApproval,
) -> Result<(), ToolError> {
    let Some(registration_id) = active.registration_id.as_deref() else {
        return Ok(());
    };

    session
        .services
        .network_approval
        .finish_call(registration_id, &active.cancellation_token)
        .await
}

pub(crate) async fn finish_deferred_network_approval(
    session: &Session,
    deferred: Option<DeferredNetworkApproval>,
) -> Result<(), ToolError> {
    let Some(deferred) = deferred else {
        return Ok(());
    };
    deferred.finish(&session.services.network_approval).await
}

#[cfg(test)]
#[path = "network_approval_tests.rs"]
mod tests;

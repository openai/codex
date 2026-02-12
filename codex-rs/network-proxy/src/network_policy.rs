use crate::reasons::REASON_POLICY_DENIED;
use crate::runtime::HostBlockDecision;
use crate::runtime::HostBlockReason;
use crate::state::NetworkProxyAuditMetadata;
use crate::state::NetworkProxyState;
use anyhow::Result;
use async_trait::async_trait;
use chrono::SecondsFormat;
use chrono::Utc;
use std::future::Future;
use std::sync::Arc;

const OTEL_NETWORK_PROXY_TARGET: &str = "codex_otel.network_proxy";
const OTEL_DOMAIN_POLICY_EVENT_NAME: &str = "codex.network_proxy.domain_policy_decision";
const OTEL_BLOCK_POLICY_EVENT_NAME: &str = "codex.network_proxy.block_decision";
const DOMAIN_POLICY_SCOPE: &str = "domain_rule";
const POLICY_DECISION_DENY: &str = "deny";
const DOMAIN_POLICY_DECISION_ALLOW: &str = "allow";
const DOMAIN_POLICY_REASON_ALLOWED: &str = "allowed";
const DOMAIN_POLICY_METHOD_NONE: &str = "none";
const DOMAIN_POLICY_CLIENT_UNKNOWN: &str = "unknown";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NetworkProtocol {
    Http,
    HttpsConnect,
    Socks5Tcp,
    Socks5Udp,
}

impl NetworkProtocol {
    pub const fn as_policy_protocol(self) -> &'static str {
        match self {
            Self::Http => "http",
            Self::HttpsConnect => "https_connect",
            Self::Socks5Tcp => "socks5_tcp",
            Self::Socks5Udp => "socks5_udp",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NetworkPolicyDecision {
    Deny,
    Ask,
}

impl NetworkPolicyDecision {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Deny => "deny",
            Self::Ask => "ask",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NetworkDecisionSource {
    BaselinePolicy,
    ModeGuard,
    ProxyState,
    Decider,
}

impl NetworkDecisionSource {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::BaselinePolicy => "baseline_policy",
            Self::ModeGuard => "mode_guard",
            Self::ProxyState => "proxy_state",
            Self::Decider => "decider",
        }
    }
}

#[derive(Clone, Debug)]
pub struct NetworkPolicyRequest {
    pub protocol: NetworkProtocol,
    pub host: String,
    pub port: u16,
    pub client_addr: Option<String>,
    pub method: Option<String>,
    pub command: Option<String>,
    pub exec_policy_hint: Option<String>,
}

pub struct NetworkPolicyRequestArgs {
    pub protocol: NetworkProtocol,
    pub host: String,
    pub port: u16,
    pub client_addr: Option<String>,
    pub method: Option<String>,
    pub command: Option<String>,
    pub exec_policy_hint: Option<String>,
}

impl NetworkPolicyRequest {
    pub fn new(args: NetworkPolicyRequestArgs) -> Self {
        let NetworkPolicyRequestArgs {
            protocol,
            host,
            port,
            client_addr,
            method,
            command,
            exec_policy_hint,
        } = args;
        Self {
            protocol,
            host,
            port,
            client_addr,
            method,
            command,
            exec_policy_hint,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NetworkDecision {
    Allow,
    Deny {
        reason: String,
        source: NetworkDecisionSource,
        decision: NetworkPolicyDecision,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BaselinePolicyOutcome {
    Allowed,
    Blocked(HostBlockReason),
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct DomainPolicyAuditEvent {
    decision: &'static str,
    source: &'static str,
    reason: String,
    protocol: &'static str,
    domain: String,
    port: u16,
    method: String,
    client_addr: String,
    policy_override: bool,
    metadata: NetworkProxyAuditMetadata,
}

pub(crate) struct NonDomainDenyAuditEventArgs<'a> {
    pub source: NetworkDecisionSource,
    pub reason: &'a str,
    pub protocol: NetworkProtocol,
    pub host: &'a str,
    pub port: u16,
    pub method: Option<&'a str>,
    pub client_addr: Option<&'a str>,
    pub metadata: &'a NetworkProxyAuditMetadata,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct NonDomainDenyAuditEvent {
    source: &'static str,
    reason: String,
    protocol: &'static str,
    domain: String,
    port: u16,
    method: String,
    client_addr: String,
    metadata: NetworkProxyAuditMetadata,
}

impl NetworkDecision {
    pub fn deny(reason: impl Into<String>) -> Self {
        Self::deny_with_source(reason, NetworkDecisionSource::Decider)
    }

    pub fn deny_with_source(reason: impl Into<String>, source: NetworkDecisionSource) -> Self {
        let reason = reason.into();
        let reason = if reason.is_empty() {
            REASON_POLICY_DENIED.to_string()
        } else {
            reason
        };
        Self::Deny {
            reason,
            source,
            decision: NetworkPolicyDecision::Deny,
        }
    }

    pub fn ask_with_source(reason: impl Into<String>, source: NetworkDecisionSource) -> Self {
        let reason = reason.into();
        let reason = if reason.is_empty() {
            REASON_POLICY_DENIED.to_string()
        } else {
            reason
        };
        Self::Deny {
            reason,
            source,
            decision: NetworkPolicyDecision::Ask,
        }
    }
}

/// Decide whether a network request should be allowed.
///
/// If `command` or `exec_policy_hint` is provided, callers can map exec-policy
/// approvals to network access (e.g., allow all requests for commands matching
/// approved prefixes like `curl *`).
#[async_trait]
pub trait NetworkPolicyDecider: Send + Sync + 'static {
    async fn decide(&self, req: NetworkPolicyRequest) -> NetworkDecision;
}

#[async_trait]
impl<D: NetworkPolicyDecider + ?Sized> NetworkPolicyDecider for Arc<D> {
    async fn decide(&self, req: NetworkPolicyRequest) -> NetworkDecision {
        (**self).decide(req).await
    }
}

#[async_trait]
impl<F, Fut> NetworkPolicyDecider for F
where
    F: Fn(NetworkPolicyRequest) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = NetworkDecision> + Send,
{
    async fn decide(&self, req: NetworkPolicyRequest) -> NetworkDecision {
        (self)(req).await
    }
}

pub(crate) async fn evaluate_host_policy(
    state: &NetworkProxyState,
    decider: Option<&Arc<dyn NetworkPolicyDecider>>,
    request: &NetworkPolicyRequest,
) -> Result<NetworkDecision> {
    let baseline_outcome = match state.host_blocked(&request.host, request.port).await? {
        HostBlockDecision::Allowed => BaselinePolicyOutcome::Allowed,
        HostBlockDecision::Blocked(reason) => BaselinePolicyOutcome::Blocked(reason),
    };

    let decision = match baseline_outcome {
        BaselinePolicyOutcome::Allowed => NetworkDecision::Allow,
        BaselinePolicyOutcome::Blocked(HostBlockReason::NotAllowed) => {
            if let Some(decider) = decider {
                map_decider_decision(decider.decide(request.clone()).await)
            } else {
                NetworkDecision::deny_with_source(
                    HostBlockReason::NotAllowed.as_str(),
                    NetworkDecisionSource::BaselinePolicy,
                )
            }
        }
        BaselinePolicyOutcome::Blocked(reason) => NetworkDecision::deny_with_source(
            reason.as_str(),
            NetworkDecisionSource::BaselinePolicy,
        ),
    };

    let audit_event =
        domain_policy_audit_event(request, baseline_outcome, &decision, state.audit_metadata());
    emit_domain_policy_audit_event(&audit_event);

    Ok(decision)
}

fn map_decider_decision(decision: NetworkDecision) -> NetworkDecision {
    match decision {
        NetworkDecision::Allow => NetworkDecision::Allow,
        NetworkDecision::Deny {
            reason, decision, ..
        } => NetworkDecision::Deny {
            reason,
            source: NetworkDecisionSource::Decider,
            decision,
        },
    }
}

fn domain_policy_audit_event(
    request: &NetworkPolicyRequest,
    baseline_outcome: BaselinePolicyOutcome,
    decision: &NetworkDecision,
    metadata: &NetworkProxyAuditMetadata,
) -> DomainPolicyAuditEvent {
    let method = request
        .method
        .clone()
        .unwrap_or_else(|| DOMAIN_POLICY_METHOD_NONE.to_string());
    let client_addr = request
        .client_addr
        .clone()
        .unwrap_or_else(|| DOMAIN_POLICY_CLIENT_UNKNOWN.to_string());

    match decision {
        NetworkDecision::Allow => {
            let (source, reason, policy_override) = match baseline_outcome {
                BaselinePolicyOutcome::Allowed => (
                    NetworkDecisionSource::BaselinePolicy.as_str(),
                    DOMAIN_POLICY_REASON_ALLOWED.to_string(),
                    false,
                ),
                BaselinePolicyOutcome::Blocked(HostBlockReason::NotAllowed) => (
                    NetworkDecisionSource::Decider.as_str(),
                    HostBlockReason::NotAllowed.as_str().to_string(),
                    true,
                ),
                BaselinePolicyOutcome::Blocked(reason) => (
                    NetworkDecisionSource::Decider.as_str(),
                    reason.as_str().to_string(),
                    false,
                ),
            };

            DomainPolicyAuditEvent {
                decision: DOMAIN_POLICY_DECISION_ALLOW,
                source,
                reason,
                protocol: request.protocol.as_policy_protocol(),
                domain: request.host.clone(),
                port: request.port,
                method,
                client_addr,
                policy_override,
                metadata: metadata.clone(),
            }
        }
        NetworkDecision::Deny {
            reason,
            source,
            decision,
        } => DomainPolicyAuditEvent {
            decision: decision.as_str(),
            source: source.as_str(),
            reason: reason.clone(),
            protocol: request.protocol.as_policy_protocol(),
            domain: request.host.clone(),
            port: request.port,
            method,
            client_addr,
            policy_override: false,
            metadata: metadata.clone(),
        },
    }
}

fn emit_domain_policy_audit_event(event: &DomainPolicyAuditEvent) {
    tracing::event!(
        target: OTEL_NETWORK_PROXY_TARGET,
        tracing::Level::INFO,
        event.name = OTEL_DOMAIN_POLICY_EVENT_NAME,
        event.timestamp = %audit_event_timestamp(),
        conversation.id = event.metadata.conversation_id.as_deref(),
        app.version = event.metadata.app_version.as_deref(),
        auth_mode = event.metadata.auth_mode.as_deref(),
        originator = event.metadata.originator.as_deref(),
        user.account_id = event.metadata.account_id.as_deref(),
        user.email = event.metadata.account_email.as_deref(),
        terminal.type = event.metadata.terminal_type.as_deref(),
        model = event.metadata.model.as_deref(),
        slug = event.metadata.slug.as_deref(),
        network.policy.scope = DOMAIN_POLICY_SCOPE,
        network.policy.decision = event.decision,
        network.policy.source = event.source,
        network.policy.reason = event.reason.as_str(),
        network.transport.protocol = event.protocol,
        server.address = event.domain.as_str(),
        server.port = event.port,
        http.request.method = event.method.as_str(),
        client.address = event.client_addr.as_str(),
        network.policy.override = event.policy_override,
    );
}

pub(crate) fn emit_non_domain_deny_audit_event(args: NonDomainDenyAuditEventArgs<'_>) {
    let event = non_domain_deny_audit_event(args);

    tracing::event!(
        target: OTEL_NETWORK_PROXY_TARGET,
        tracing::Level::INFO,
        event.name = OTEL_BLOCK_POLICY_EVENT_NAME,
        event.timestamp = %audit_event_timestamp(),
        conversation.id = event.metadata.conversation_id.as_deref(),
        app.version = event.metadata.app_version.as_deref(),
        auth_mode = event.metadata.auth_mode.as_deref(),
        originator = event.metadata.originator.as_deref(),
        user.account_id = event.metadata.account_id.as_deref(),
        user.email = event.metadata.account_email.as_deref(),
        terminal.type = event.metadata.terminal_type.as_deref(),
        model = event.metadata.model.as_deref(),
        slug = event.metadata.slug.as_deref(),
        network.policy.scope = event.source,
        network.policy.decision = POLICY_DECISION_DENY,
        network.policy.source = event.source,
        network.policy.reason = event.reason.as_str(),
        network.transport.protocol = event.protocol,
        server.address = event.domain.as_str(),
        server.port = event.port,
        http.request.method = event.method.as_str(),
        client.address = event.client_addr.as_str(),
        network.policy.override = false,
    );
}

fn non_domain_deny_audit_event(args: NonDomainDenyAuditEventArgs<'_>) -> NonDomainDenyAuditEvent {
    debug_assert!(matches!(
        args.source,
        NetworkDecisionSource::ModeGuard | NetworkDecisionSource::ProxyState
    ));

    NonDomainDenyAuditEvent {
        source: args.source.as_str(),
        reason: args.reason.to_string(),
        protocol: args.protocol.as_policy_protocol(),
        domain: args.host.to_string(),
        port: args.port,
        method: args.method.unwrap_or(DOMAIN_POLICY_METHOD_NONE).to_string(),
        client_addr: args
            .client_addr
            .unwrap_or(DOMAIN_POLICY_CLIENT_UNKNOWN)
            .to_string(),
        metadata: args.metadata.clone(),
    }
}

fn audit_event_timestamp() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::config::NetworkProxySettings;
    use crate::reasons::REASON_DENIED;
    use crate::reasons::REASON_METHOD_NOT_ALLOWED;
    use crate::reasons::REASON_NOT_ALLOWED;
    use crate::reasons::REASON_NOT_ALLOWED_LOCAL;
    use crate::reasons::REASON_PROXY_DISABLED;
    use crate::state::network_proxy_state_for_policy;
    use chrono::DateTime;
    use pretty_assertions::assert_eq;
    use std::sync::Arc;
    use std::sync::atomic::AtomicUsize;
    use std::sync::atomic::Ordering;

    fn sample_request() -> NetworkPolicyRequest {
        NetworkPolicyRequest::new(NetworkPolicyRequestArgs {
            protocol: NetworkProtocol::Http,
            host: "api.example.com".to_string(),
            port: 443,
            client_addr: Some("127.0.0.1:9999".to_string()),
            method: Some("GET".to_string()),
            command: None,
            exec_policy_hint: None,
        })
    }

    fn sample_metadata() -> NetworkProxyAuditMetadata {
        NetworkProxyAuditMetadata {
            conversation_id: Some("019c4a22-679d-7eb2-aa9c-b95114e5ee8d".to_string()),
            app_version: Some("0.0.0".to_string()),
            auth_mode: Some("Chatgpt".to_string()),
            originator: Some("codex_cli_rs".to_string()),
            account_id: Some("f7f33107-5fb9-4ee1-8922-3eae76b5b5a0".to_string()),
            account_email: Some("test@example.com".to_string()),
            terminal_type: Some("iTerm.app/3.6.5".to_string()),
            model: Some("gpt-5.3-codex".to_string()),
            slug: Some("gpt-5.3-codex".to_string()),
        }
    }

    #[test]
    fn domain_policy_audit_event_reports_baseline_allow() {
        let request = sample_request();

        let event = domain_policy_audit_event(
            &request,
            BaselinePolicyOutcome::Allowed,
            &NetworkDecision::Allow,
            &NetworkProxyAuditMetadata::default(),
        );

        assert_eq!(
            event,
            DomainPolicyAuditEvent {
                decision: "allow",
                source: "baseline_policy",
                reason: "allowed".to_string(),
                protocol: "http",
                domain: "api.example.com".to_string(),
                port: 443,
                method: "GET".to_string(),
                client_addr: "127.0.0.1:9999".to_string(),
                policy_override: false,
                metadata: NetworkProxyAuditMetadata::default(),
            }
        );
    }

    #[test]
    fn domain_policy_audit_event_reports_baseline_deny_denied() {
        let request = sample_request();
        let decision =
            NetworkDecision::deny_with_source(REASON_DENIED, NetworkDecisionSource::BaselinePolicy);

        let event = domain_policy_audit_event(
            &request,
            BaselinePolicyOutcome::Blocked(HostBlockReason::Denied),
            &decision,
            &NetworkProxyAuditMetadata::default(),
        );

        assert_eq!(
            event,
            DomainPolicyAuditEvent {
                decision: "deny",
                source: "baseline_policy",
                reason: REASON_DENIED.to_string(),
                protocol: "http",
                domain: "api.example.com".to_string(),
                port: 443,
                method: "GET".to_string(),
                client_addr: "127.0.0.1:9999".to_string(),
                policy_override: false,
                metadata: NetworkProxyAuditMetadata::default(),
            }
        );
    }

    #[test]
    fn domain_policy_audit_event_reports_baseline_deny_not_allowed_local() {
        let request = sample_request();
        let decision = NetworkDecision::deny_with_source(
            REASON_NOT_ALLOWED_LOCAL,
            NetworkDecisionSource::BaselinePolicy,
        );

        let event = domain_policy_audit_event(
            &request,
            BaselinePolicyOutcome::Blocked(HostBlockReason::NotAllowedLocal),
            &decision,
            &NetworkProxyAuditMetadata::default(),
        );

        assert_eq!(
            event,
            DomainPolicyAuditEvent {
                decision: "deny",
                source: "baseline_policy",
                reason: REASON_NOT_ALLOWED_LOCAL.to_string(),
                protocol: "http",
                domain: "api.example.com".to_string(),
                port: 443,
                method: "GET".to_string(),
                client_addr: "127.0.0.1:9999".to_string(),
                policy_override: false,
                metadata: NetworkProxyAuditMetadata::default(),
            }
        );
    }

    #[test]
    fn domain_policy_audit_event_reports_decider_override_allow() {
        let request = sample_request();

        let event = domain_policy_audit_event(
            &request,
            BaselinePolicyOutcome::Blocked(HostBlockReason::NotAllowed),
            &NetworkDecision::Allow,
            &NetworkProxyAuditMetadata::default(),
        );

        assert_eq!(
            event,
            DomainPolicyAuditEvent {
                decision: "allow",
                source: "decider",
                reason: REASON_NOT_ALLOWED.to_string(),
                protocol: "http",
                domain: "api.example.com".to_string(),
                port: 443,
                method: "GET".to_string(),
                client_addr: "127.0.0.1:9999".to_string(),
                policy_override: true,
                metadata: NetworkProxyAuditMetadata::default(),
            }
        );
    }

    #[test]
    fn domain_policy_audit_event_reports_decider_ask() {
        let request = sample_request();
        let decision = NetworkDecision::ask_with_source(
            "requires_user_approval",
            NetworkDecisionSource::Decider,
        );

        let event = domain_policy_audit_event(
            &request,
            BaselinePolicyOutcome::Blocked(HostBlockReason::NotAllowed),
            &decision,
            &NetworkProxyAuditMetadata::default(),
        );

        assert_eq!(
            event,
            DomainPolicyAuditEvent {
                decision: "ask",
                source: "decider",
                reason: "requires_user_approval".to_string(),
                protocol: "http",
                domain: "api.example.com".to_string(),
                port: 443,
                method: "GET".to_string(),
                client_addr: "127.0.0.1:9999".to_string(),
                policy_override: false,
                metadata: NetworkProxyAuditMetadata::default(),
            }
        );
    }

    #[test]
    fn domain_policy_audit_event_includes_metadata() {
        let request = sample_request();
        let metadata = sample_metadata();

        let event = domain_policy_audit_event(
            &request,
            BaselinePolicyOutcome::Allowed,
            &NetworkDecision::Allow,
            &metadata,
        );

        assert_eq!(event.metadata, metadata);
    }

    #[test]
    fn non_domain_deny_audit_event_reports_mode_guard_method_block() {
        let event = non_domain_deny_audit_event(NonDomainDenyAuditEventArgs {
            source: NetworkDecisionSource::ModeGuard,
            reason: REASON_METHOD_NOT_ALLOWED,
            protocol: NetworkProtocol::Http,
            host: "api.example.com",
            port: 443,
            method: Some("POST"),
            client_addr: Some("127.0.0.1:9999"),
            metadata: &NetworkProxyAuditMetadata::default(),
        });

        assert_eq!(
            event,
            NonDomainDenyAuditEvent {
                source: "mode_guard",
                reason: REASON_METHOD_NOT_ALLOWED.to_string(),
                protocol: "http",
                domain: "api.example.com".to_string(),
                port: 443,
                method: "POST".to_string(),
                client_addr: "127.0.0.1:9999".to_string(),
                metadata: NetworkProxyAuditMetadata::default(),
            }
        );
    }

    #[test]
    fn non_domain_deny_audit_event_reports_proxy_state_proxy_disabled() {
        let event = non_domain_deny_audit_event(NonDomainDenyAuditEventArgs {
            source: NetworkDecisionSource::ProxyState,
            reason: REASON_PROXY_DISABLED,
            protocol: NetworkProtocol::Socks5Tcp,
            host: "api.example.com",
            port: 443,
            method: None,
            client_addr: None,
            metadata: &NetworkProxyAuditMetadata::default(),
        });

        assert_eq!(
            event,
            NonDomainDenyAuditEvent {
                source: "proxy_state",
                reason: REASON_PROXY_DISABLED.to_string(),
                protocol: "socks5_tcp",
                domain: "api.example.com".to_string(),
                port: 443,
                method: "none".to_string(),
                client_addr: "unknown".to_string(),
                metadata: NetworkProxyAuditMetadata::default(),
            }
        );
    }

    #[test]
    fn non_domain_deny_audit_event_uses_none_and_unknown_fallbacks() {
        let event = non_domain_deny_audit_event(NonDomainDenyAuditEventArgs {
            source: NetworkDecisionSource::ModeGuard,
            reason: REASON_METHOD_NOT_ALLOWED,
            protocol: NetworkProtocol::Http,
            host: "api.example.com",
            port: 80,
            method: None,
            client_addr: None,
            metadata: &NetworkProxyAuditMetadata::default(),
        });

        assert_eq!(event.method, "none");
        assert_eq!(event.client_addr, "unknown");
    }

    #[test]
    fn non_domain_deny_audit_event_supports_unix_socket_sentinel() {
        let event = non_domain_deny_audit_event(NonDomainDenyAuditEventArgs {
            source: NetworkDecisionSource::ModeGuard,
            reason: REASON_METHOD_NOT_ALLOWED,
            protocol: NetworkProtocol::Http,
            host: "unix-socket",
            port: 0,
            method: Some("POST"),
            client_addr: Some("127.0.0.1:9999"),
            metadata: &NetworkProxyAuditMetadata::default(),
        });

        assert_eq!(event.domain, "unix-socket");
        assert_eq!(event.port, 0);
    }

    #[test]
    fn non_domain_deny_audit_event_includes_metadata() {
        let metadata = sample_metadata();

        let event = non_domain_deny_audit_event(NonDomainDenyAuditEventArgs {
            source: NetworkDecisionSource::ModeGuard,
            reason: REASON_METHOD_NOT_ALLOWED,
            protocol: NetworkProtocol::Http,
            host: "api.example.com",
            port: 80,
            method: Some("GET"),
            client_addr: Some("127.0.0.1:9999"),
            metadata: &metadata,
        });

        assert_eq!(event.metadata, metadata);
    }

    #[test]
    fn domain_policy_audit_target_is_exportable_by_otel_filter() {
        assert!(OTEL_NETWORK_PROXY_TARGET.starts_with("codex_otel"));
    }

    #[test]
    fn audit_event_timestamp_is_rfc3339_utc_with_millisecond_precision() {
        let timestamp = audit_event_timestamp();
        let parsed = DateTime::parse_from_rfc3339(&timestamp).unwrap();

        assert_eq!(parsed.offset().local_minus_utc(), 0);
        assert!(timestamp.ends_with('Z'));
        assert_eq!(timestamp.matches('.').count(), 1);
        assert_eq!(timestamp.split('.').nth(1).unwrap().len(), 4);
    }

    #[tokio::test]
    async fn evaluate_host_policy_invokes_decider_for_not_allowed() {
        let state = network_proxy_state_for_policy(NetworkProxySettings::default());
        let calls = Arc::new(AtomicUsize::new(0));
        let decider: Arc<dyn NetworkPolicyDecider> = Arc::new({
            let calls = calls.clone();
            move |_req| {
                calls.fetch_add(1, Ordering::SeqCst);
                // The default policy denies all; the decider is consulted for not_allowed
                // requests and can override that decision.
                async { NetworkDecision::Allow }
            }
        });

        let request = NetworkPolicyRequest::new(NetworkPolicyRequestArgs {
            protocol: NetworkProtocol::Http,
            host: "example.com".to_string(),
            port: 80,
            client_addr: None,
            method: Some("GET".to_string()),
            command: None,
            exec_policy_hint: None,
        });

        let decision = evaluate_host_policy(&state, Some(&decider), &request)
            .await
            .unwrap();
        assert_eq!(decision, NetworkDecision::Allow);
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn evaluate_host_policy_skips_decider_for_denied() {
        let state = network_proxy_state_for_policy(NetworkProxySettings {
            allowed_domains: vec!["example.com".to_string()],
            denied_domains: vec!["blocked.com".to_string()],
            ..NetworkProxySettings::default()
        });
        let calls = Arc::new(AtomicUsize::new(0));
        let decider: Arc<dyn NetworkPolicyDecider> = Arc::new({
            let calls = calls.clone();
            move |_req| {
                calls.fetch_add(1, Ordering::SeqCst);
                async { NetworkDecision::Allow }
            }
        });

        let request = NetworkPolicyRequest::new(NetworkPolicyRequestArgs {
            protocol: NetworkProtocol::Http,
            host: "blocked.com".to_string(),
            port: 80,
            client_addr: None,
            method: Some("GET".to_string()),
            command: None,
            exec_policy_hint: None,
        });

        let decision = evaluate_host_policy(&state, Some(&decider), &request)
            .await
            .unwrap();
        assert_eq!(
            decision,
            NetworkDecision::Deny {
                reason: REASON_DENIED.to_string(),
                source: NetworkDecisionSource::BaselinePolicy,
                decision: NetworkPolicyDecision::Deny,
            }
        );
        assert_eq!(calls.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn evaluate_host_policy_skips_decider_for_not_allowed_local() {
        let state = network_proxy_state_for_policy(NetworkProxySettings {
            allowed_domains: vec!["example.com".to_string()],
            allow_local_binding: false,
            ..NetworkProxySettings::default()
        });
        let calls = Arc::new(AtomicUsize::new(0));
        let decider: Arc<dyn NetworkPolicyDecider> = Arc::new({
            let calls = calls.clone();
            move |_req| {
                calls.fetch_add(1, Ordering::SeqCst);
                async { NetworkDecision::Allow }
            }
        });

        let request = NetworkPolicyRequest::new(NetworkPolicyRequestArgs {
            protocol: NetworkProtocol::Http,
            host: "127.0.0.1".to_string(),
            port: 80,
            client_addr: None,
            method: Some("GET".to_string()),
            command: None,
            exec_policy_hint: None,
        });

        let decision = evaluate_host_policy(&state, Some(&decider), &request)
            .await
            .unwrap();
        assert_eq!(
            decision,
            NetworkDecision::Deny {
                reason: REASON_NOT_ALLOWED_LOCAL.to_string(),
                source: NetworkDecisionSource::BaselinePolicy,
                decision: NetworkPolicyDecision::Deny,
            }
        );
        assert_eq!(calls.load(Ordering::SeqCst), 0);
    }
}

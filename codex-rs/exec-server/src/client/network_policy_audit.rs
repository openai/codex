use super::NetworkPolicyAuditContext;
use crate::protocol::ExecServerNetworkProtocol;
use crate::protocol::MAX_NETWORK_POLICY_HOST_BYTES;
use crate::protocol::MAX_NETWORK_POLICY_PROCESS_ID_BYTES;
use crate::protocol::MAX_NETWORK_POLICY_REASON_BYTES;
use crate::protocol::NetworkPolicyDecisionNotification;

const MAX_NETWORK_POLICY_METHOD_BYTES: usize = 32;
const MAX_NETWORK_POLICY_CLIENT_BYTES: usize = 256;
const MAX_NETWORK_POLICY_TIMESTAMP_BYTES: usize = 64;

pub(super) fn emit_network_policy_decision(
    context: &NetworkPolicyAuditContext,
    decision: &NetworkPolicyDecisionNotification,
) -> bool {
    if decision.process_id.is_empty()
        || decision.process_id.len() > MAX_NETWORK_POLICY_PROCESS_ID_BYTES
        || decision.host.is_empty()
        || decision.host.len() > MAX_NETWORK_POLICY_HOST_BYTES
        || decision.host.chars().any(char::is_control)
        || decision.host.chars().any(char::is_whitespace)
        || decision.reason.len() > MAX_NETWORK_POLICY_REASON_BYTES
        || decision.reason.chars().any(char::is_control)
        || !matches!(decision.scope.as_str(), "domain" | "non_domain")
        || !matches!(decision.decision.as_str(), "allow" | "deny" | "ask")
        || !matches!(
            decision.source.as_str(),
            "baseline_policy" | "mode_guard" | "proxy_state" | "decider"
        )
        || decision.timestamp.is_empty()
        || decision.timestamp.len() > MAX_NETWORK_POLICY_TIMESTAMP_BYTES
        || decision.timestamp.chars().any(char::is_control)
        || decision.method.as_ref().is_some_and(|method| {
            method.len() > MAX_NETWORK_POLICY_METHOD_BYTES
                || method.chars().any(char::is_control)
                || method.chars().any(char::is_whitespace)
        })
        || decision.client.as_ref().is_some_and(|client| {
            client.len() > MAX_NETWORK_POLICY_CLIENT_BYTES
                || client.chars().any(char::is_control)
                || client.chars().any(char::is_whitespace)
        })
    {
        return false;
    }

    let protocol = match decision.protocol {
        ExecServerNetworkProtocol::Http => "http",
        ExecServerNetworkProtocol::HttpsConnect => "https_connect",
        ExecServerNetworkProtocol::Socks5Tcp => "socks5_tcp",
        ExecServerNetworkProtocol::Socks5Udp => "socks5_udp",
    };
    let metadata = &context.metadata;
    tracing::event!(
        target: "codex_otel.log_only",
        tracing::Level::INFO,
        event.name = "codex.network_proxy.policy_decision",
        event.timestamp = decision.timestamp,
        conversation.id = metadata.conversation_id.as_deref(),
        app.version = metadata.app_version.as_deref(),
        auth_mode = metadata.auth_mode.as_deref(),
        originator = metadata.originator.as_deref(),
        user.account_id = metadata.user_account_id.as_deref(),
        user.email = metadata.user_email.as_deref(),
        terminal.type = metadata.terminal_type.as_deref(),
        model = metadata.model.as_deref(),
        slug = metadata.slug.as_deref(),
        network.policy.scope = decision.scope,
        network.policy.decision = decision.decision,
        network.policy.source = decision.source,
        network.policy.reason = decision.reason,
        network.transport.protocol = protocol,
        server.address = decision.host,
        server.port = decision.port,
        http.request.method = decision.method.as_deref().unwrap_or("none"),
        client.address = decision.client.as_deref().unwrap_or("unknown"),
        execution.id = context.execution_id.as_deref(),
        network.policy.override = decision.policy_override,
    );
    true
}

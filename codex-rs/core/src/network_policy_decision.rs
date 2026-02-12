use codex_protocol::approvals::NetworkApprovalContext;
use codex_protocol::approvals::NetworkApprovalProtocol;
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct NetworkPolicyDecisionPayload {
    pub decision: String,
    pub source: String,
    pub protocol: Option<String>,
    pub host: Option<String>,
    pub reason: Option<String>,
    pub port: Option<u16>,
}

impl NetworkPolicyDecisionPayload {
    pub(crate) fn is_ask_from_decider(&self) -> bool {
        self.decision.eq_ignore_ascii_case("ask") && self.source.eq_ignore_ascii_case("decider")
    }
}

pub(crate) fn network_approval_context_from_payload(
    payload: &NetworkPolicyDecisionPayload,
) -> Option<NetworkApprovalContext> {
    if !payload.is_ask_from_decider() {
        return None;
    }

    let protocol = match payload.protocol.as_deref() {
        Some("http") => NetworkApprovalProtocol::Http,
        Some("https") | Some("https_connect") => NetworkApprovalProtocol::Https,
        _ => return None,
    };

    let host = payload.host.as_deref()?.trim();
    if host.is_empty() {
        return None;
    }

    Some(NetworkApprovalContext {
        host: host.to_string(),
        protocol,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn network_approval_context_requires_ask_from_decider() {
        let payload = NetworkPolicyDecisionPayload {
            decision: "deny".to_string(),
            source: "decider".to_string(),
            protocol: Some("https_connect".to_string()),
            host: Some("example.com".to_string()),
            reason: Some("not_allowed".to_string()),
            port: Some(443),
        };

        assert_eq!(network_approval_context_from_payload(&payload), None);
    }

    #[test]
    fn network_approval_context_maps_http_and_https_protocols() {
        let http_payload = NetworkPolicyDecisionPayload {
            decision: "ask".to_string(),
            source: "decider".to_string(),
            protocol: Some("http".to_string()),
            host: Some("example.com".to_string()),
            reason: Some("not_allowed".to_string()),
            port: Some(80),
        };
        assert_eq!(
            network_approval_context_from_payload(&http_payload),
            Some(NetworkApprovalContext {
                host: "example.com".to_string(),
                protocol: NetworkApprovalProtocol::Http,
            })
        );

        let https_payload = NetworkPolicyDecisionPayload {
            decision: "ask".to_string(),
            source: "decider".to_string(),
            protocol: Some("https_connect".to_string()),
            host: Some("example.com".to_string()),
            reason: Some("not_allowed".to_string()),
            port: Some(443),
        };
        assert_eq!(
            network_approval_context_from_payload(&https_payload),
            Some(NetworkApprovalContext {
                host: "example.com".to_string(),
                protocol: NetworkApprovalProtocol::Https,
            })
        );
    }
}

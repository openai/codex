use serde::Deserialize;
use serde_json::Value;

pub(crate) const NETWORK_POLICY_DECISION_PREFIX: &str = "CODEX_NETWORK_POLICY_DECISION ";

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct NetworkPolicyDecisionPayload {
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

    pub(crate) fn is_blocking_decision(&self) -> bool {
        !self.decision.eq_ignore_ascii_case("allow")
    }
}

pub(crate) fn extract_network_policy_decisions(text: &str) -> Vec<NetworkPolicyDecisionPayload> {
    text.lines()
        .flat_map(extract_policy_decisions_from_fragment)
        .collect()
}

fn extract_policy_decisions_from_fragment(fragment: &str) -> Vec<NetworkPolicyDecisionPayload> {
    let mut payloads = Vec::new();

    if let Some(payload) = parse_prefixed_payload(fragment) {
        payloads.push(payload);
    }

    if let Ok(value) = serde_json::from_str::<Value>(fragment) {
        extract_policy_decisions_from_json_value(&value, &mut payloads);
    }

    payloads
}

fn extract_policy_decisions_from_json_value(
    value: &Value,
    payloads: &mut Vec<NetworkPolicyDecisionPayload>,
) {
    match value {
        Value::String(text) => {
            payloads.extend(text.lines().filter_map(parse_prefixed_payload));
        }
        Value::Array(values) => {
            for value in values {
                extract_policy_decisions_from_json_value(value, payloads);
            }
        }
        Value::Object(map) => {
            for value in map.values() {
                extract_policy_decisions_from_json_value(value, payloads);
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) => {}
    }
}

fn parse_prefixed_payload(text: &str) -> Option<NetworkPolicyDecisionPayload> {
    let payload = text.strip_prefix(NETWORK_POLICY_DECISION_PREFIX)?;
    serde_json::from_str::<NetworkPolicyDecisionPayload>(payload).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn extracts_payload_from_prefixed_line() {
        let text = r#"CODEX_NETWORK_POLICY_DECISION {"decision":"ask","source":"decider","protocol":"http","host":"example.com","port":80}"#;

        let payloads = extract_network_policy_decisions(text);
        assert_eq!(
            payloads,
            vec![NetworkPolicyDecisionPayload {
                decision: "ask".to_string(),
                source: "decider".to_string(),
                protocol: Some("http".to_string()),
                host: Some("example.com".to_string()),
                reason: None,
                port: Some(80),
            }]
        );
    }

    #[test]
    fn extracts_payload_from_generic_json_string_field() {
        let text = r#"{"unexpected":"CODEX_NETWORK_POLICY_DECISION {\"decision\":\"deny\",\"source\":\"baseline_policy\",\"protocol\":\"https_connect\",\"host\":\"google.com\",\"port\":443}"}"#;

        let payloads = extract_network_policy_decisions(text);
        assert_eq!(payloads.len(), 1);
        assert_eq!(payloads[0].decision, "deny");
        assert_eq!(payloads[0].source, "baseline_policy");
        assert_eq!(payloads[0].host.as_deref(), Some("google.com"));
    }

    #[test]
    fn extracts_payload_from_nested_json_values() {
        let text = r#"{"data":[{"meta":{"message":"CODEX_NETWORK_POLICY_DECISION {\"decision\":\"ask\",\"source\":\"decider\",\"protocol\":\"https_connect\",\"host\":\"api.example.com\",\"port\":443}\nblocked"}}]}"#;

        let payloads = extract_network_policy_decisions(text);
        assert_eq!(payloads.len(), 1);
        assert_eq!(payloads[0].decision, "ask");
        assert_eq!(payloads[0].host.as_deref(), Some("api.example.com"));
    }

    #[test]
    fn ignores_lines_without_policy_prefix() {
        let text = r#"{"status":"blocked","message":"domain not in allowlist"}"#;
        assert!(extract_network_policy_decisions(text).is_empty());
    }
}

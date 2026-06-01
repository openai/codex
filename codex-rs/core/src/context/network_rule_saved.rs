use super::ContextualUserFragment;
use codex_protocol::approvals::NetworkPolicyAmendment;
use codex_protocol::approvals::NetworkPolicyRuleAction;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct NetworkRuleSaved {
    action: NetworkPolicyRuleAction,
    host: String,
}

impl NetworkRuleSaved {
    pub(crate) fn new(amendment: &NetworkPolicyAmendment) -> Self {
        Self {
            action: amendment.action,
            host: amendment.host.clone(),
        }
    }
}

impl ContextualUserFragment for NetworkRuleSaved {
    fn role() -> &'static str {
        "developer"
    }

    fn markers(&self) -> (&'static str, &'static str) {
        Self::type_markers()
    }

    fn type_markers() -> (&'static str, &'static str) {
        ("", "")
    }

    fn body(&self) -> String {
        let (action, list_name) = match self.action {
            NetworkPolicyRuleAction::Allow => ("Allowed", "allowlist"),
            NetworkPolicyRuleAction::Deny => ("Denied", "denylist"),
        };
        format!(
            "{action} network rule saved in execpolicy ({list_name}): {}",
            self.host
        )
    }
}

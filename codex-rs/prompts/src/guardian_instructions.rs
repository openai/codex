//! Composes Guardian prompts and rejection feedback from selected text and runtime inputs.
//! Callers select policy overrides, output contracts, calibration, and truncation limits.

use codex_context_fragments::ContextualUserFragment;
use codex_guardian_context::truncate_text;
use codex_protocol::models::ContentItemKind;

const TENANT_POLICY_CONFIG_PLACEHOLDER: &str = "{{ tenant_policy_config }}";

/// Reviewer base instructions composed from the effective policy and caller-owned output contract.
#[derive(Debug, Clone, Copy)]
pub struct GuardianPolicyInstructions<'a> {
    tenant_policy_config: &'a str,
    policy_template: &'a str,
    output_contract: &'a str,
}

impl<'a> GuardianPolicyInstructions<'a> {
    pub fn new(
        tenant_policy_config: &'a str,
        policy_template: &'a str,
        output_contract: &'a str,
    ) -> Self {
        Self {
            tenant_policy_config,
            policy_template,
            output_contract,
        }
    }
}

impl ContextualUserFragment for GuardianPolicyInstructions<'_> {
    fn role(&self) -> &'static str {
        "developer"
    }

    fn content_kind(&self) -> ContentItemKind {
        ContentItemKind("guardian.policy_instructions".to_owned())
    }

    fn markers(&self) -> (&'static str, &'static str) {
        Self::type_markers()
    }

    fn type_markers() -> (&'static str, &'static str) {
        ("", "")
    }

    fn body(&self) -> String {
        let Self {
            tenant_policy_config,
            policy_template,
            output_contract,
        } = *self;
        let prompt = policy_template.trim_end().replace(
            TENANT_POLICY_CONFIG_PLACEHOLDER,
            tenant_policy_config.trim(),
        );
        format!("{prompt}\n\n{output_contract}\n")
    }
}

/// Classifier instructions composed from selected text, policy, and the caller-owned output contract.
/// A supplied token limit applies after the complete instructions are composed.
#[derive(Debug, Clone, Copy)]
pub struct GuardianClassifierInstructions<'a> {
    classifier_instructions: &'a str,
    policy: &'a str,
    output_contract: &'a str,
    max_tokens: Option<usize>,
}

impl<'a> GuardianClassifierInstructions<'a> {
    pub fn new(
        classifier_instructions: &'a str,
        policy: &'a str,
        output_contract: &'a str,
        max_tokens: Option<usize>,
    ) -> Self {
        Self {
            classifier_instructions,
            policy,
            output_contract,
            max_tokens,
        }
    }
}

impl ContextualUserFragment for GuardianClassifierInstructions<'_> {
    fn role(&self) -> &'static str {
        "developer"
    }

    fn content_kind(&self) -> ContentItemKind {
        ContentItemKind("guardian.classifier_instructions".to_owned())
    }

    fn markers(&self) -> (&'static str, &'static str) {
        Self::type_markers()
    }

    fn type_markers() -> (&'static str, &'static str) {
        ("", "")
    }

    fn body(&self) -> String {
        let Self {
            classifier_instructions,
            policy,
            output_contract,
            max_tokens,
        } = *self;
        let instructions = if classifier_instructions.contains(TENANT_POLICY_CONFIG_PLACEHOLDER) {
            classifier_instructions.replace(TENANT_POLICY_CONFIG_PLACEHOLDER, policy)
        } else {
            format!("{classifier_instructions}\n\n# Security Policy\n{policy}")
        };
        let instructions = if instructions.trim_end().ends_with(output_contract) {
            instructions
        } else {
            format!("{instructions}\n\n{output_contract}")
        };
        match max_tokens {
            Some(max_tokens) => truncate_text(&instructions, max_tokens),
            None => instructions,
        }
    }
}

/// Renders feedback for an action the caller has already decided to reject.
pub fn render_guardian_rejection(rationale: &str, rejection_instructions: &str) -> String {
    let rationale = if rationale.trim().is_empty() {
        "Auto-reviewer denied the action without a specific rationale."
    } else {
        rationale.trim()
    };
    format!(
        "This action was rejected due to unacceptable risk.\nReason: {rationale}\n{rejection_instructions}"
    )
}

#[cfg(test)]
#[path = "guardian_instructions_tests.rs"]
mod tests;

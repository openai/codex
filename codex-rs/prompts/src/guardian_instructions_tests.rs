//! Covers policy substitution, terminal output contracts, truncation, and rejection-text overrides.

use codex_context_fragments::ContextualUserFragment;
use codex_guardian_context::truncate_text;
use pretty_assertions::assert_eq;

use super::GuardianClassifierInstructions;
use super::GuardianPolicyInstructions;
use super::render_guardian_rejection;

#[test]
fn reviewer_policy_substitution_preserves_template_layout_and_appends_contract() {
    for (template, policy, expected) in [
        (
            " Review:\n{{ tenant_policy_config }}\nAgain: {{ tenant_policy_config }}\n \t",
            " \nTenant policy.\n ",
            " Review:\nTenant policy.\nAgain: Tenant policy.\n\nReview contract.\n",
        ),
        (
            "Policy: {{ tenant_policy_config }}",
            "",
            "Policy: \n\nReview contract.\n",
        ),
        ("", "Tenant policy.", "\n\nReview contract.\n"),
    ] {
        assert_eq!(
            GuardianPolicyInstructions::new(policy, template, "Review contract.").render(),
            expected,
        );
    }
}

#[test]
fn classifier_policy_substitution_and_legacy_append_keep_one_terminal_contract_before_truncation() {
    for (instructions, policy, expected) in [
        (
            "Classify: {{ tenant_policy_config }}",
            " Tenant policy.\n",
            "Classify:  Tenant policy.\n\n\nClassifier contract.",
        ),
        (
            "Classify: {{ tenant_policy_config }}\n\nClassifier contract.\n \t",
            "Tenant policy.",
            "Classify: Tenant policy.\n\nClassifier contract.\n \t",
        ),
        (
            "Legacy classifier.",
            "Tenant policy.",
            "Legacy classifier.\n\n# Security Policy\nTenant policy.\n\nClassifier contract.",
        ),
    ] {
        for max_tokens in [None, Some(20)] {
            assert_eq!(
                GuardianClassifierInstructions::new(
                    instructions,
                    policy,
                    "Classifier contract.",
                    max_tokens,
                )
                .render(),
                match max_tokens {
                    Some(max_tokens) => truncate_text(expected, max_tokens),
                    None => expected.to_owned(),
                },
            );
        }
    }
}

#[test]
fn rejection_feedback_trims_rationale_and_preserves_instruction_overrides() {
    for (rationale, instructions, expected) in [
        (
            " \nSensitive data would leave the workspace.\t ",
            " Ask for approval.\n",
            "This action was rejected due to unacceptable risk.\nReason: Sensitive data would leave the workspace.\n Ask for approval.\n",
        ),
        (
            " \n\t",
            "",
            "This action was rejected due to unacceptable risk.\nReason: Auto-reviewer denied the action without a specific rationale.\n",
        ),
    ] {
        assert_eq!(render_guardian_rejection(rationale, instructions), expected);
    }
}

//! Resolves Guardian policy, feedback, and classifier text from catalog values or bundled defaults.
//! Configuration overlays and prompt composition remain with consumers.

use codex_protocol::openai_models::AutoReviewMessages;
use codex_protocol::openai_models::ModelMessages;

const POLICY: &str = include_str!("../../templates/guardian/policy.md");
const POLICY_TEMPLATE: &str = include_str!("../../templates/guardian/policy_template.md");
const NODE_REPL_POLICY: &str = include_str!("../../templates/guardian/node_repl_policy.md");
const REJECTION_INSTRUCTIONS: &str = concat!(
    "The agent must not attempt to achieve the same outcome via workaround, ",
    "indirect execution, or policy circumvention. ",
    "Proceed only with a materially safer alternative, ",
    "or if the user explicitly approves the action after being informed of the risk. ",
    "Otherwise, stop and request user input.",
);
const TIMEOUT_INSTRUCTIONS: &str = concat!(
    "The automatic permission approval review did not finish before its deadline. ",
    "Do not assume the action is unsafe based on the timeout alone. ",
    "You may retry once, or ask the user for guidance or explicit approval.",
);
const CLASSIFIER_INSTRUCTIONS: &str =
    include_str!("../../templates/guardian/classifier_instructions.md");

/// Auto-review policy and feedback text before runtime configuration overlays and rendering.
#[derive(Debug, Clone, Copy)]
pub struct ResolvedAutoReviewMessages<'a> {
    pub policy: &'a str,
    pub policy_template: &'a str,
    pub node_repl_policy: &'a str,
    pub rejection_instructions: &'a str,
    pub timeout_instructions: &'a str,
}

impl<'a> ResolvedAutoReviewMessages<'a> {
    pub(crate) fn new(messages: Option<&'a AutoReviewMessages>) -> Self {
        Self {
            policy: messages
                .and_then(|messages| messages.policy.as_deref())
                .unwrap_or(POLICY),
            policy_template: messages
                .and_then(|messages| messages.policy_template.as_deref())
                .unwrap_or(POLICY_TEMPLATE),
            node_repl_policy: messages
                .and_then(|messages| messages.node_repl_policy.as_deref())
                .unwrap_or(NODE_REPL_POLICY),
            rejection_instructions: messages
                .and_then(|messages| messages.rejection_instructions.as_deref())
                .unwrap_or(REJECTION_INSTRUCTIONS),
            timeout_instructions: messages
                .and_then(|messages| messages.timeout_instructions.as_deref())
                .unwrap_or(TIMEOUT_INSTRUCTIONS),
        }
    }
}

pub(crate) fn classifier_instructions(messages: Option<&ModelMessages>) -> &str {
    messages
        .and_then(|messages| messages.guardian_v2.as_ref())
        .and_then(|messages| messages.classifier_instructions.as_deref())
        .unwrap_or(CLASSIFIER_INSTRUCTIONS)
}

//! Covers role composition and attribution for captured model instructions.

use super::*;
use codex_context_fragments::AnnotatedContent;
use codex_context_fragments::RenderedFragment;
use codex_protocol::models::ContentItemMetadata;
use pretty_assertions::assert_eq;

#[test]
fn role_segment_filters_base_and_appends_bundled_guidance() {
    let shared = DEFAULT_MULTI_AGENT_V2_SHARED_USAGE_HINT_TEXT;
    let wait = DEFAULT_MULTI_AGENT_V2_WAIT_AGENT_USAGE_HINT_TEXT;
    let model_override = DEFAULT_MULTI_AGENT_V2_MODEL_OVERRIDE_USAGE_HINT_TEXT;
    let expected_body = format!(
        "Role.\n## Work\nContinue.\n{shared}\n{wait}\n\nThere are 2 available concurrency slots, meaning that up to 2 agents can be active at once, including you.\n\n{model_override}"
    );
    let instructions = MultiAgentRoleInstructions::Composed {
        base: "Role.\n## Plan tool\nOmit role checklist guidance.\n## Work\nContinue.".to_string(),
        omit_update_plan_instructions: true,
        max_concurrency: 2,
        wait_agent_enabled: true,
        expose_model_overrides: true,
    };
    assert_eq!(
        instructions.render_fragment(),
        RenderedFragment::new(
            "developer",
            AnnotatedContent::text(
                format!("<multi_agent_role>{expected_body}</multi_agent_role>"),
                ContentItemKind("multi_agent.role_instructions".to_string()),
                ContentItemMetadata::harness(),
            ),
        ),
    );
}

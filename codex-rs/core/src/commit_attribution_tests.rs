use super::build_commit_message_trailer;
use super::commit_message_trailer_instruction;
use super::resolve_attribution_value;

#[test]
fn blank_attribution_disables_trailer_prompt() {
    assert_eq!(build_commit_message_trailer(Some("")), None);
    assert_eq!(commit_message_trailer_instruction(Some("   ")), None);
}

#[test]
fn default_attribution_uses_codex_trailer() {
    assert_eq!(
        build_commit_message_trailer(None).as_deref(),
        Some("Co-authored-by: Codex <noreply@openai.com>")
    );
}

#[test]
fn resolve_value_handles_default_custom_and_blank() {
    assert_eq!(
        resolve_attribution_value(None),
        Some("Codex <noreply@openai.com>".to_string())
    );
    assert_eq!(
        resolve_attribution_value(Some("MyAgent <me@example.com>")),
        Some("MyAgent <me@example.com>".to_string())
    );
    assert_eq!(
        resolve_attribution_value(Some("MyAgent")),
        Some("MyAgent".to_string())
    );
    assert_eq!(resolve_attribution_value(Some("   ")), None);
}

#[test]
fn instruction_mentions_trailer_and_omits_generated_with() {
    let instruction = commit_message_trailer_instruction(Some("AgentX <agent@example.com>"))
        .expect("instruction expected");
    assert!(instruction.contains("Co-authored-by: AgentX <agent@example.com>"));
    assert!(instruction.contains("exactly once"));
    assert!(!instruction.contains("Generated-with"));
}

//! Complete action rendering preserves nested arguments and rejects overflow.

use super::*;
use anyhow::Result;
use pretty_assertions::assert_eq;
use serde_json::Value;

#[test]
fn guardian_action_preserves_complete_arguments_at_budget_boundary() -> Result<()> {
    let arguments = json!({
        "call_id": "genuine-call",
        "tool": "untrusted-tool",
        "attachments": [{"content": "🦀\"\\\n".repeat(/*n*/ 40) + "required suffix"}],
        "metadata": {"description": "required argument", "values": [1, false, null]},
    });
    let render = |max_action_tokens| {
        GuardianAction {
            tool_name: ToolName::plain("inspect_values"),
            payload: ToolPayload::Function {
                arguments: arguments.to_string(),
            },
        }
        .render(max_action_tokens)
    };
    let mut expected = arguments.clone();
    expected["tool"] = json!("inspect_values");
    let bytes = serde_json::to_string_pretty(&expected)?.len();
    let budget = TruncationPolicy::Bytes(bytes.saturating_add(1)).token_budget();
    let rendered = render(budget)?;
    assert_eq!(serde_json::from_str::<Value>(&rendered)?, expected);
    assert_eq!(rendered.len(), bytes);
    assert!(matches!(
        render(budget - 1),
        Err(ActionRenderError::TooLarge { .. })
    ));
    Ok(())
}

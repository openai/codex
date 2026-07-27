use super::*;
use pretty_assertions::assert_eq;

#[test]
fn converts_tool_use_blocks_to_bounded_external_agent_tags() {
    let block = serde_json::json!({
        "type": "tool_use",
        "name": "Bash",
        "input": {
            "description": "Check repo status",
            "command": "git status --short"
        }
    });

    assert_eq!(
        tool_call_note(&block),
        "[external_agent_tool_call: Bash]\n\
         description: Check repo status\n\
         command: git status --short\n\
         [/external_agent_tool_call]"
    );
}

#[test]
fn converts_tool_result_blocks_to_bounded_external_agent_tags() {
    let block = serde_json::json!({
        "type": "tool_result",
        "content": "codex-rs/external-agent-migration/src/sessions/records_common.rs"
    });

    assert_eq!(
        tool_result_note(&block),
        "[external_agent_tool_result]\n\
         codex-rs/external-agent-migration/src/sessions/records_common.rs\n\
         [/external_agent_tool_result]"
    );
}

#[test]
fn converts_error_tool_result_blocks_to_bounded_external_agent_tags() {
    let block = serde_json::json!({
        "type": "tool_result",
        "is_error": true,
        "content": "command failed"
    });

    assert_eq!(
        tool_result_note(&block),
        "[external_agent_tool_result: error]\n\
         command failed\n\
         [/external_agent_tool_result]"
    );
}

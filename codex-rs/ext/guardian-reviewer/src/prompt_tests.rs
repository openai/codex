#![cfg(test)]

use super::GuardianPromptItems;
use super::GuardianPromptMode;
use super::GuardianTranscriptCursor;
use super::compose_guardian_prompt;
use super::guardian_truncate_text;
use crate::GuardianApprovalRequest;
use crate::GuardianMcpAnnotations;
use crate::build_guardian_planned_action;
use codex_guardian_context::ContextProfile;
use codex_guardian_context::SectionHistory;
use codex_guardian_context::SectionInput;
use codex_guardian_context::default_registry;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;
use codex_protocol::models::SandboxPermissions;
use codex_utils_path_uri::LegacyAppPathString;
use codex_utils_path_uri::PathUri;
use pretty_assertions::assert_eq;

#[test]
fn guardian_truncate_text_keeps_prefix_suffix_and_xml_marker() {
    let content = "prefix ".repeat(200) + &" suffix".repeat(200);

    let (truncated, was_truncated) = guardian_truncate_text(&content, /*token_cap*/ 20);

    assert!(truncated.starts_with("prefix"));
    assert!(truncated.contains("<truncated omitted_approx_tokens=\""));
    assert!(truncated.ends_with("suffix"));
    assert!(was_truncated);
}

fn guardian_parent_history() -> Vec<ResponseItem> {
    vec![
        ResponseItem::Message {
            status: None,
            encrypted_content: None,
            id: None,
            role: "user".to_string(),
            content: vec![ContentItem::InputText {
                text: "Please check the repo visibility and push the docs fix if needed."
                    .to_string(),
            }],
            phase: None,
            internal_chat_message_metadata_passthrough: None,
        },
        ResponseItem::FunctionCall {
            status: None,
            encrypted_content: None,
            id: None,
            name: "gh_repo_view".to_string(),
            namespace: None,
            arguments: "{\"repo\":\"openai/codex\"}".to_string(),
            call_id: "call-1".to_string(),
            encrypted_function_args: None,
            internal_chat_message_metadata_passthrough: None,
        },
        ResponseItem::FunctionCallOutput {
            id: None,
            call_id: Some("call-1".to_string()),
            name: None,
            namespace: None,
            output: codex_protocol::models::FunctionCallOutputPayload::from_text(
                "repo visibility: public".to_string(),
            ),
            internal_chat_message_metadata_passthrough: None,
        },
        ResponseItem::Message {
            status: None,
            encrypted_content: None,
            id: None,
            role: "assistant".to_string(),
            content: vec![ContentItem::OutputText {
                annotations: None,
                logprobs: None,
                text: "The repo is public; I now need approval to push the docs fix.".to_string(),
            }],
            phase: None,
            internal_chat_message_metadata_passthrough: None,
        },
    ]
}

fn guardian_mcp_request(server: &str, tool_name: &str) -> GuardianApprovalRequest {
    GuardianApprovalRequest::McpToolCall {
        id: "mcp-1".to_string(),
        server: server.to_string(),
        tool_name: tool_name.to_string(),
        arguments: Some(serde_json::json!({
            "code": "await browser.open('https://example.com')",
        })),
        connector_id: Some("connector-1".to_string()),
        connector_name: Some("Connected tools".to_string()),
        connector_description: None,
        connected_account_email: None,
        tool_title: Some("Execute JavaScript".to_string()),
        tool_description: None,
        annotations: Some(GuardianMcpAnnotations {
            destructive_hint: None,
            open_world_hint: Some(true),
            read_only_hint: None,
        }),
        prompt_permissions: None,
    }
}

fn guardian_prompt_text(items: &[codex_protocol::user_input::UserInput]) -> String {
    items
        .iter()
        .map(|item| match item {
            codex_protocol::user_input::UserInput::Text { text, .. } => text.as_str(),
            _ => "",
        })
        .collect::<String>()
}

fn build_guardian_prompt_items(
    history: &dyn SectionHistory,
    request: GuardianApprovalRequest,
    mode: GuardianPromptMode,
) -> anyhow::Result<GuardianPromptItems> {
    let profile = ContextProfile::synchronous();
    let action = build_guardian_planned_action(&request, /*reason*/ None)?;
    let sections = default_registry().prepare(&SectionInput {
        target: profile.target,
        history,
        transcript: &profile.transcript,
        is_selected_exec_output: None,
        root_conversation: &[],
        root_conversation_candidates: None,
        trusted_user_answers: &[],
        planned_action: Some(&action),
        permissions: None,
        previous_reviews: None,
        trusted_tool: None,
        trusted_skill_paths: &[],
        images: None,
        node_repl: None,
    })?;
    compose_guardian_prompt(
        sections,
        &profile,
        mode,
        /*history_version*/ 0,
        "11111111-1111-4111-8111-111111111111",
        /*node_repl_evidence_sequence*/ 0,
    )
}

#[test]
fn build_guardian_prompt_delta_mode_preserves_original_numbering() -> anyhow::Result<()> {
    let mut history = guardian_parent_history();
    history.extend([
        ResponseItem::Message {
            status: None,
            encrypted_content: None,
            id: None,
            role: "user".to_string(),
            content: vec![ContentItem::InputText {
                text: "Please also push the second docs fix.".to_string(),
            }],
            phase: None,
            internal_chat_message_metadata_passthrough: None,
        },
        ResponseItem::Message {
            status: None,
            encrypted_content: None,
            id: None,
            role: "assistant".to_string(),
            content: vec![ContentItem::OutputText {
                annotations: None,
                logprobs: None,
                text: "I need approval for the second push.".to_string(),
            }],
            phase: None,
            internal_chat_message_metadata_passthrough: None,
        },
    ]);

    let prompt = build_guardian_prompt_items(
        &history,
        GuardianApprovalRequest::ExecCommand {
            id: "shell-2".to_string(),
            environment_id: "local".to_string(),
            command: vec!["git".to_string(), "push".to_string()],
            cwd: PathUri::parse("file:///repo/codex-rs/core").expect("valid cwd"),
            guardian_cwd: LegacyAppPathString::from_string("/repo/codex-rs/core"),
            sandbox_permissions: SandboxPermissions::UseDefault,
            additional_permissions: None,
            justification: Some("Need to push the second docs fix.".to_string()),
            tty: false,
        },
        GuardianPromptMode::Delta {
            cursor: GuardianTranscriptCursor {
                parent_history_version: 0,
                transcript_entry_count: 4,
            },
        },
    )?;

    let text = guardian_prompt_text(&prompt.context.into_user_inputs()?);
    assert!(text.contains("added since your last approval assessment"));
    assert!(text.contains(">>> TRANSCRIPT DELTA START\n"));
    assert!(text.contains("[5] user: Please also push the second docs fix."));
    assert!(text.contains("[6] assistant: I need approval for the second push."));
    assert!(text.contains(">>> TRANSCRIPT DELTA END\n"));
    assert!(text.contains("The Codex agent has requested the following next action:\n"));
    assert!(!text.contains("[1] user: Please check the repo visibility"));
    assert_eq!(prompt.transcript_cursor.transcript_entry_count, 6);

    Ok(())
}

#[test]
fn build_guardian_prompt_delta_mode_handles_empty_delta() -> anyhow::Result<()> {
    let history = guardian_parent_history();

    let prompt = build_guardian_prompt_items(
        &history,
        GuardianApprovalRequest::ExecCommand {
            id: "shell-2".to_string(),
            environment_id: "local".to_string(),
            command: vec!["git".to_string(), "push".to_string()],
            cwd: PathUri::parse("file:///repo/codex-rs/core").expect("valid cwd"),
            guardian_cwd: LegacyAppPathString::from_string("/repo/codex-rs/core"),
            sandbox_permissions: SandboxPermissions::UseDefault,
            additional_permissions: None,
            justification: Some("Need to push the second docs fix.".to_string()),
            tty: false,
        },
        GuardianPromptMode::Delta {
            cursor: GuardianTranscriptCursor {
                parent_history_version: 0,
                transcript_entry_count: 4,
            },
        },
    )?;

    let text = guardian_prompt_text(&prompt.context.into_user_inputs()?);
    assert!(text.contains(">>> TRANSCRIPT DELTA START\n"));
    assert!(text.contains("<no retained transcript delta entries>"));
    assert!(text.contains(">>> TRANSCRIPT DELTA END\n"));
    assert_eq!(prompt.transcript_cursor.transcript_entry_count, 4);

    Ok(())
}

#[test]
fn build_guardian_prompt_stale_delta_cursor_falls_back_to_full_prompt() -> anyhow::Result<()> {
    let history = guardian_parent_history();

    let prompt = build_guardian_prompt_items(
        &history,
        GuardianApprovalRequest::ExecCommand {
            id: "shell-3".to_string(),
            environment_id: "local".to_string(),
            command: vec!["git".to_string(), "push".to_string()],
            cwd: PathUri::parse("file:///repo/codex-rs/core").expect("valid cwd"),
            guardian_cwd: LegacyAppPathString::from_string("/repo/codex-rs/core"),
            sandbox_permissions: SandboxPermissions::UseDefault,
            additional_permissions: None,
            justification: Some("Need to push the docs fix.".to_string()),
            tty: false,
        },
        GuardianPromptMode::Delta {
            cursor: GuardianTranscriptCursor {
                parent_history_version: 0,
                transcript_entry_count: 99,
            },
        },
    )?;

    let text = guardian_prompt_text(&prompt.context.into_user_inputs()?);
    assert!(text.contains("whose request action you are assessing"));
    assert!(text.contains(">>> TRANSCRIPT START\n"));
    assert!(!text.contains("TRANSCRIPT DELTA"));
    assert_eq!(prompt.transcript_cursor.transcript_entry_count, 4);

    Ok(())
}

#[test]
fn build_guardian_prompt_items_keeps_other_requests_generic() -> anyhow::Result<()> {
    let history = guardian_parent_history();

    for request in [
        guardian_mcp_request("node_repl", "js"),
        guardian_mcp_request("node_repl", "inspect"),
        guardian_mcp_request("another_server", "js"),
        GuardianApprovalRequest::ExecCommand {
            id: "shell-1".to_string(),
            environment_id: "local".to_string(),
            command: vec!["git".to_string(), "push".to_string()],
            cwd: PathUri::parse("file:///repo/codex-rs/core").expect("valid cwd"),
            guardian_cwd: LegacyAppPathString::from_string("/repo/codex-rs/core"),
            sandbox_permissions: SandboxPermissions::UseDefault,
            additional_permissions: None,
            justification: Some("Need to push the reviewed docs fix.".to_string()),
            tty: false,
        },
    ] {
        let prompt = build_guardian_prompt_items(&history, request, GuardianPromptMode::Full)?;

        let text = guardian_prompt_text(&prompt.context.into_user_inputs()?);
        assert!(text.contains("Assess the exact planned action below."));
        assert!(text.contains("Planned action JSON:"));
        assert!(!text.contains("Node REPL action JSON:"));
        assert!(!text.contains("Distinguish preparation"));
    }

    Ok(())
}

//! Regression coverage for opt-in failure blocking through managed hook discovery and execution.

use super::*;
use crate::PermissionRequestDecision;
use crate::PermissionRequestRequest;
use crate::PostCompactRequest;
use crate::PostToolUseRequest;
use crate::PreCompactRequest;
use crate::SessionStartRequest;
use crate::SessionStartSource;
use crate::StartHookTarget;
use crate::StopHookTarget;
use crate::UserPromptSubmitRequest;
use codex_protocol::protocol::HookCompletedEvent;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;

const MCP_EVENTS: [HookEventName; 11] = [
    HookEventName::PreToolUse,
    HookEventName::PermissionRequest,
    HookEventName::PostToolUse,
    HookEventName::PreCompact,
    HookEventName::PostCompact,
    HookEventName::SessionStart,
    HookEventName::UserPromptSubmit,
    HookEventName::SubagentStart,
    HookEventName::SubagentStop,
    HookEventName::Stop,
    HookEventName::Interrupt,
];

#[derive(Clone, Copy)]
struct McpResult(Result<&'static str, &'static str>);

impl HookMcpExecutor for McpResult {
    fn execute(&self, _call: HookMcpCall) -> BoxFuture<'_, anyhow::Result<HookMcpOutput>> {
        async move {
            match self.0 {
                Ok(text) => Ok(HookMcpOutput {
                    result: codex_protocol::mcp::CallToolResult {
                        content: vec![json!({"type": "text", "text": text})],
                        structured_content: None,
                        meta: None,
                        is_error: None,
                    },
                    source_tool_namespace: None,
                }),
                Err(error) => anyhow::bail!(error),
            }
        }
        .boxed()
    }
}

fn engine(event: HookEventName, handlers: Vec<Value>, output: McpResult) -> ClaudeHooksEngine {
    let hooks = serde_json::from_value(json!({
        format!("{event:?}"): [{ "hooks": handlers }],
    }))
    .expect("managed hooks");
    let stack = required_hooks_stack(
        ManagedHooksRequirementsToml {
            hooks,
            ..Default::default()
        },
        RequirementSource::LegacyManagedConfigTomlFromMdm,
    );
    ClaudeHooksEngine::new(
        /*enabled*/ true,
        /*bypass_hook_trust*/ false,
        Some(&stack),
        Vec::new(),
        Vec::new(),
        command_runtime(CommandShell {
            program: "missing-hook-shell-for-test".to_string(),
            args: Vec::new(),
        }),
        Arc::new(output),
    )
}

async fn run(engine: &ClaudeHooksEngine, event: HookEventName) -> (bool, Vec<HookCompletedEvent>) {
    match event {
        HookEventName::PreToolUse => {
            let outcome = engine
                .run_pre_tool_use(PreToolUseRequest {
                    session_id: ThreadId::new(),
                    turn_id: "turn".to_string(),
                    subagent: None,
                    cwd: cwd(),
                    tool_cwd: None,
                    transcript_path: None,
                    model: "test".to_string(),
                    permission_mode: "default".to_string(),
                    tool_name: "Bash".to_string(),
                    matcher_aliases: Vec::new(),
                    tool_use_id: "call".to_string(),
                    tool_input: json!({ "command": "echo hello" }),
                })
                .await;
            assert_eq!(outcome.updated_input, None);
            (outcome.should_block, outcome.hook_events)
        }
        HookEventName::PermissionRequest => {
            let outcome = engine
                .run_permission_request(PermissionRequestRequest {
                    session_id: ThreadId::new(),
                    turn_id: "turn".to_string(),
                    subagent: None,
                    cwd: cwd().to_path_buf(),
                    tool_cwd: None,
                    transcript_path: None,
                    model: "test".to_string(),
                    permission_mode: "default".to_string(),
                    tool_name: "Bash".to_string(),
                    matcher_aliases: Vec::new(),
                    run_id_suffix: "call".to_string(),
                    tool_input: json!({ "command": "echo hello" }),
                })
                .await;
            (
                matches!(
                    outcome.decision,
                    Some(PermissionRequestDecision::Deny { .. })
                ),
                outcome.hook_events,
            )
        }
        HookEventName::PostToolUse => {
            let outcome = engine
                .run_post_tool_use(PostToolUseRequest {
                    session_id: ThreadId::new(),
                    turn_id: "turn".to_string(),
                    subagent: None,
                    cwd: cwd(),
                    tool_cwd: None,
                    transcript_path: None,
                    model: "test".to_string(),
                    permission_mode: "default".to_string(),
                    tool_name: "Bash".to_string(),
                    matcher_aliases: Vec::new(),
                    tool_use_id: "call".to_string(),
                    tool_input: json!({"command": "echo hello"}),
                    tool_response: json!("hello"),
                })
                .await;
            assert_eq!(outcome.should_block, outcome.feedback_message.is_some());
            (outcome.should_block, outcome.hook_events)
        }
        HookEventName::UserPromptSubmit => {
            let outcome = engine
                .run_user_prompt_submit(UserPromptSubmitRequest {
                    session_id: ThreadId::new(),
                    turn_id: "turn".to_string(),
                    subagent: None,
                    cwd: cwd(),
                    transcript_path: None,
                    model: "test".to_string(),
                    permission_mode: "default".to_string(),
                    prompt: "hello".to_string(),
                })
                .await;
            assert_eq!(outcome.should_stop, outcome.stop_reason.is_some());
            (outcome.should_stop, outcome.hook_events)
        }
        HookEventName::SessionStart | HookEventName::SubagentStart => {
            let target = if event == HookEventName::SessionStart {
                StartHookTarget::SessionStart {
                    source: SessionStartSource::Startup,
                }
            } else {
                StartHookTarget::SubagentStart {
                    turn_id: "turn".to_string(),
                    agent_id: "agent".to_string(),
                    agent_type: "default".to_string(),
                }
            };
            let outcome = engine
                .run_session_start(
                    SessionStartRequest {
                        session_id: ThreadId::new(),
                        cwd: cwd(),
                        transcript_path: None,
                        model: "test".to_string(),
                        permission_mode: "default".to_string(),
                        target,
                    },
                    Some("turn".to_string()),
                )
                .await;
            assert_eq!(outcome.should_stop, outcome.stop_reason.is_some());
            (outcome.should_stop, outcome.hook_events)
        }
        HookEventName::PreCompact => {
            let outcome = engine
                .run_pre_compact(PreCompactRequest {
                    session_id: ThreadId::new(),
                    turn_id: "turn".to_string(),
                    subagent: None,
                    cwd: cwd(),
                    transcript_path: None,
                    model: "test".to_string(),
                    trigger: "manual".to_string(),
                })
                .await;
            assert_eq!(outcome.should_stop, outcome.stop_reason.is_some());
            (outcome.should_stop, outcome.hook_events)
        }
        HookEventName::PostCompact => {
            let outcome = engine
                .run_post_compact(PostCompactRequest {
                    session_id: ThreadId::new(),
                    turn_id: "turn".to_string(),
                    subagent: None,
                    cwd: cwd(),
                    transcript_path: None,
                    model: "test".to_string(),
                    trigger: "manual".to_string(),
                })
                .await;
            assert_eq!(outcome.should_stop, outcome.stop_reason.is_some());
            (outcome.should_stop, outcome.hook_events)
        }
        HookEventName::Stop | HookEventName::SubagentStop => {
            let target = if event == HookEventName::Stop {
                StopHookTarget::Stop
            } else {
                StopHookTarget::SubagentStop {
                    agent_id: "agent".to_string(),
                    agent_type: "default".to_string(),
                    agent_transcript_path: None,
                }
            };
            let outcome = engine
                .run_stop(StopRequest {
                    session_id: ThreadId::new(),
                    turn_id: "turn".to_string(),
                    cwd: cwd(),
                    transcript_path: None,
                    model: "test".to_string(),
                    permission_mode: "default".to_string(),
                    request_metadata: None,
                    stop_hook_active: false,
                    last_assistant_message: None,
                    target,
                })
                .await;
            assert_eq!(
                outcome.should_block,
                !outcome.continuation_fragments.is_empty()
            );
            (outcome.should_block, outcome.hook_events)
        }
        HookEventName::Interrupt => {
            let outcome = engine
                .run_interrupt(InterruptRequest {
                    session_id: ThreadId::new(),
                    turn_id: "turn".to_string(),
                    cwd: cwd(),
                    transcript_path: None,
                    model: "test".to_string(),
                    permission_mode: "default".to_string(),
                    request_metadata: None,
                })
                .await;
            // Cancellation has already happened; only the hook lifecycle status changes.
            (
                outcome
                    .hook_events
                    .iter()
                    .any(|event| event.run.status == HookRunStatus::Blocked),
                outcome.hook_events,
            )
        }
        HookEventName::SessionEnd
        | HookEventName::PreModelRequest
        | HookEventName::PostModelRequest => {
            unreachable!("event does not support blocking MCP hooks")
        }
    }
}

#[tokio::test]
async fn mcp_failures_block_only_when_opted_in() {
    for event in MCP_EVENTS {
        for policy in [None, Some("continue"), Some("block")] {
            for output in [
                McpResult(Err("MCP tool returned an error")),
                McpResult(Err("MCP server disconnected")),
                McpResult(Err("MCP request timed out")),
                McpResult(Ok("{invalid json")),
                McpResult(Ok(r#"{"continue":false}"#)),
                McpResult(Ok("{}")),
                McpResult(Ok("")),
                McpResult(Ok("hook finished")),
            ] {
                if output.0 == Ok(r#"{"continue":false}"#)
                    && !matches!(
                        event,
                        HookEventName::PreToolUse | HookEventName::PermissionRequest
                    )
                {
                    continue;
                }
                let mut handler = json!({"type": "mcp_tool", "server": "scanner", "tool": "check"});
                if let Some(policy) = policy {
                    handler["onFailure"] = json!(policy);
                }
                let engine = engine(event, vec![handler], output);
                assert!(engine.required_load_errors().is_empty());
                let (blocked, events) = run(&engine, event).await;
                let failed = match output.0 {
                    Ok("{}" | "") => false,
                    Ok("hook finished") => matches!(
                        event,
                        HookEventName::Stop
                            | HookEventName::SubagentStop
                            | HookEventName::Interrupt
                    ),
                    _ => true,
                };
                let expected_block = failed && policy == Some("block");
                assert_eq!(
                    blocked, expected_block,
                    "{event:?} {policy:?} {:?}",
                    output.0
                );
                assert_eq!(events.len(), 1);
                assert_eq!(
                    events[0].run.status,
                    if expected_block {
                        HookRunStatus::Blocked
                    } else if failed {
                        HookRunStatus::Failed
                    } else {
                        HookRunStatus::Completed
                    }
                );
                assert_eq!(
                    events[0]
                        .run
                        .entries
                        .iter()
                        .any(|entry| entry.kind == HookOutputEntryKind::Error),
                    failed
                );
            }
        }
    }
}

#[tokio::test]
async fn command_start_failure_obeys_policy() {
    for event in MCP_EVENTS {
        for policy in ["continue", "block"] {
            let engine = engine(
                event,
                vec![json!({
                    "type": "command", "command": "echo hello", "onFailure": policy,
                })],
                McpResult(Ok("{}")),
            );
            assert!(engine.required_load_errors().is_empty());
            let (blocked, events) = run(&engine, event).await;
            assert_eq!(blocked, policy == "block");
            assert!(
                events[0]
                    .run
                    .entries
                    .iter()
                    .any(|entry| entry.kind == HookOutputEntryKind::Error)
            );
        }
    }
}

#[cfg(unix)]
#[tokio::test]
async fn command_exit_timeout_and_invalid_output_obey_policy() {
    for event in MCP_EVENTS {
        for policy in ["continue", "block"] {
            for (command, failed) in [
                ("exit 1", true),
                ("exit 2", true),
                ("printf '{invalid'", true),
                ("sleep 10", true),
                ("exit 0", false),
            ] {
                let mut engine = engine(
                    event,
                    vec![json!({
                        "type": "command", "command": command, "onFailure": policy, "timeout": 1,
                    })],
                    McpResult(Ok("{}")),
                );
                engine.command_runtime = command_runtime(CommandShell {
                    program: "sh".to_string(),
                    args: vec!["-c".to_string()],
                });
                let (blocked, events) = run(&engine, event).await;
                assert_eq!(
                    blocked,
                    failed && policy == "block",
                    "{event:?} {policy} {command}"
                );
                assert_eq!(
                    events[0]
                        .run
                        .entries
                        .iter()
                        .any(|entry| entry.kind == HookOutputEntryKind::Error),
                    failed
                );
            }
        }
    }
}

#[tokio::test]
async fn failure_block_cannot_be_overridden_by_a_successful_hook() {
    for event in MCP_EVENTS {
        let engine = engine(
            event,
            vec![
                json!({ "type": "command", "command": "echo hello", "onFailure": "block" }),
                json!({ "type": "mcp_tool", "server": "scanner", "tool": "check" }),
            ],
            McpResult(Ok(if event == HookEventName::PermissionRequest {
                r#"{"hookSpecificOutput":{"hookEventName":"PermissionRequest","decision":{"behavior":"allow"}}}"#
            } else if event == HookEventName::PreToolUse {
                r#"{"hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"allow","updatedInput":{"command":"echo rewritten"}}}"#
            } else {
                "{}"
            })),
        );
        let (blocked, events) = run(&engine, event).await;
        assert!(blocked);
        assert_eq!(events.len(), 2);
        assert_eq!(events[1].run.status, HookRunStatus::Completed);
    }
}

#[test]
fn managed_hooks_reject_failure_blocking_that_cannot_be_enforced() {
    for (event, mut handler) in MCP_EVENTS
        .into_iter()
        .map(|event| {
            (
                event,
                json!({"type": "command", "command": "echo hello", "async": true}),
            )
        })
        .chain([
            (
                HookEventName::SessionEnd,
                json!({"type": "command", "command": "echo hello"}),
            ),
            (
                HookEventName::SessionEnd,
                json!({"type": "mcp_tool", "server": "scanner", "tool": "check"}),
            ),
        ])
        .chain(
            [
                HookEventName::PreModelRequest,
                HookEventName::PostModelRequest,
            ]
            .into_iter()
            .flat_map(|event| {
                [
                    json!({"type": "command", "command": "echo hello"}),
                    json!({"type": "mcp_tool", "server": "scanner", "tool": "check"}),
                ]
                .map(|handler| (event, handler))
            }),
        )
    {
        handler["onFailure"] = json!("block");
        let engine = engine(event, vec![handler], McpResult(Ok("{}")));
        assert!(engine.handlers.is_empty());
        assert_eq!(engine.required_load_errors().len(), 1);
        assert!(
            engine.required_load_errors()[0].contains("onFailure:block requires a synchronous")
        );
    }
}

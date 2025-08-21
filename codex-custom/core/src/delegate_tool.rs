use futures::StreamExt;
use serde::Deserialize;

use crate::client_common::EnvironmentContext;
use crate::client_common::Prompt;
use crate::exec::ExecParams;
use crate::models::ContentItem;
use crate::models::FunctionCallOutputPayload;
use crate::models::ResponseInputItem;
use crate::models::ResponseItem;
use crate::models::ShellToolCallParams;
use crate::openai_tools::JsonSchema;
use crate::openai_tools::OpenAiTool;
use crate::openai_tools::ResponsesApiTool;
use crate::protocol::BackgroundEventEvent;
use crate::protocol::Event;
use crate::protocol::EventMsg;
use crate::protocol::TokenUsage;

use crate::codex::Session;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct DelegateTaskArgs {
    instructions: String,
    input: String,
    #[serde(default)]
    allow_shell: bool,
    #[serde(default)]
    timeout_ms: Option<u64>,
}

fn parse_delegate_args(
    arguments: String,
    call_id: &str,
) -> Result<DelegateTaskArgs, Box<ResponseInputItem>> {
    match serde_json::from_str::<DelegateTaskArgs>(&arguments) {
        Ok(args) => Ok(args),
        Err(e) => {
            let output = ResponseInputItem::FunctionCallOutput {
                call_id: call_id.to_string(),
                output: FunctionCallOutputPayload {
                    content: format!("failed to parse function arguments: {e}"),
                    success: None,
                },
            };
            Err(Box::new(output))
        }
    }
}

/// Handle the `delegate_task` tool call by running a one‑shot sub‑agent with
/// its own instructions and returning the final assistant message as a string.
///
/// MVP: tools are disabled even if `allow_shell=true`. This keeps the initial
/// implementation safe and avoids cross‑contamination of the main conversation
/// history. A future revision can optionally expose a read‑only shell to the
/// sub‑agent and handle tool calls inside this loop.
pub(crate) async fn handle_delegate_task(
    sess: &Session,
    sub_id: String,
    call_id: String,
    arguments: String,
) -> ResponseInputItem {
    let args = match parse_delegate_args(arguments, &call_id) {
        Ok(a) => a,
        Err(output) => return *output,
    };

    // Surface a lightweight progress hint to any front-end.
    let _ = sess
        .tx_event
        .send(Event {
            id: sub_id.clone(),
            msg: EventMsg::BackgroundEvent(BackgroundEventEvent {
                message: format!(
                    "sub-agent started (allow_shell={}): {}",
                    args.allow_shell,
                    args.instructions.chars().take(64).collect::<String>()
                ),
            }),
        })
        .await;

    // Build a minimal input transcript for the sub-agent: a single user message.
    let mut transcript: Vec<ResponseItem> = vec![ResponseItem::Message {
        id: None,
        role: "user".to_string(),
        content: vec![ContentItem::InputText {
            text: args.input.clone(),
        }],
    }];
    let mut report = String::new();
    let mut last_usage: Option<TokenUsage> = None;

    // Iterate turns, handling only `shell` tool calls when enabled.
    // Stop when the model returns a pure assistant message with no pending tool calls.
    loop {
        let tools: Vec<OpenAiTool> = if args.allow_shell {
            vec![make_readonly_shell_tool()]
        } else {
            Vec::new()
        };
        // Preserve the validated base instructions used by the main agent and provide
        // the delegate guidance via `user_instructions`. Some providers reject arbitrary
        // overrides of the base system prompt, returning 400 ("Instructions are not valid").
        let prompt = Prompt {
            input: transcript.clone(),
            user_instructions: Some(args.instructions.clone()),
            store: false,
            environment_context: Some(sess.environment_context()),
            tools,
            base_instructions_override: None,
        };

        let mut stream = match sess.model_stream(&prompt).await {
            Ok(s) => s,
            Err(e) => {
                return ResponseInputItem::FunctionCallOutput {
                    call_id,
                    output: FunctionCallOutputPayload {
                        content: format!("delegate_task stream error: {e}"),
                        success: Some(false),
                    },
                };
            }
        };

        // Results of this turn
        let mut turn_assistant = String::new();
        let mut new_messages: Vec<ResponseItem> = Vec::new();
        let mut saw_tool_call = false;

        loop {
            let next = stream.next().await;
            let Some(ev) = next else { break };
            match ev {
                Ok(crate::client_common::ResponseEvent::Created) => {}
                Ok(crate::client_common::ResponseEvent::OutputItemDone(item)) => {
                    match &item {
                        ResponseItem::Message { role, content, .. } if role == "assistant" => {
                            for c in content {
                                if let ContentItem::OutputText { text } = c {
                                    turn_assistant.push_str(text);
                                }
                            }
                        }
                        ResponseItem::FunctionCall {
                            name,
                            arguments,
                            call_id: fc_id,
                            ..
                        } => {
                            if name == "shell" && args.allow_shell {
                                saw_tool_call = true;
                                // Append the function call to transcript
                                new_messages.push(item.clone());
                                // Execute under read-only sandbox
                                let exec_output_item = match exec_shell_call_readonly(
                                    sess,
                                    &sub_id,
                                    args.timeout_ms,
                                    arguments.clone(),
                                    fc_id,
                                )
                                .await
                                {
                                    Ok(output_item) => output_item,
                                    Err(e) => ResponseItem::FunctionCallOutput {
                                        call_id: fc_id.clone(),
                                        output: FunctionCallOutputPayload {
                                            content: e,
                                            success: Some(false),
                                        },
                                    },
                                };
                                new_messages.push(exec_output_item);
                            } else {
                                // Unsupported function/tool – reply with structured failure so the model can adapt.
                                new_messages.push(ResponseItem::FunctionCallOutput {
                                    call_id: fc_id.clone(),
                                    output: FunctionCallOutputPayload {
                                        content: format!("unsupported call: {name}"),
                                        success: Some(false),
                                    },
                                });
                            }
                        }
                        _ => {}
                    }
                }
                Ok(crate::client_common::ResponseEvent::Completed { token_usage, .. }) => {
                    last_usage = token_usage;
                    break;
                }
                Ok(crate::client_common::ResponseEvent::OutputTextDelta(delta)) => {
                    // Stream sub-agent deltas to the UI as BackgroundEvent in a dim style.
                    let _ = sess
                        .tx_event
                        .send(Event {
                            id: sub_id.clone(),
                            msg: EventMsg::BackgroundEvent(BackgroundEventEvent {
                                message: format!("sub-agent: {}", delta),
                            }),
                        })
                        .await;
                }
                Ok(crate::client_common::ResponseEvent::ReasoningSummaryDelta(_)) => {}
                Ok(crate::client_common::ResponseEvent::ReasoningContentDelta(_)) => {}
                Err(e) => {
                    return ResponseInputItem::FunctionCallOutput {
                        call_id,
                        output: FunctionCallOutputPayload {
                            content: format!("delegate_task error: {e}"),
                            success: Some(false),
                        },
                    };
                }
            }
        }

        if saw_tool_call {
            // Feed function call + outputs back in and continue another turn.
            transcript.extend(new_messages);
            continue;
        }

        // No tool call; append any final assistant text and stop.
        report.push_str(&turn_assistant);
        break;
    }

    // Append a short token usage footer to aid the caller.
    if let Some(u) = last_usage {
        use std::fmt::Write as _;
        let _ = write!(
            &mut report,
            "\n\n[delegate_task tokens: input={}, output={}, total={}]",
            u.input_tokens, u.output_tokens, u.total_tokens
        );
    }

    let _ = sess
        .tx_event
        .send(Event {
            id: sub_id,
            msg: EventMsg::BackgroundEvent(BackgroundEventEvent {
                message: "sub-agent completed".to_string(),
            }),
        })
        .await;

    ResponseInputItem::FunctionCallOutput {
        call_id,
        output: FunctionCallOutputPayload {
            content: report,
            success: Some(true),
        },
    }
}

fn make_readonly_shell_tool() -> OpenAiTool {
    use std::collections::BTreeMap;
    let mut properties = BTreeMap::new();
    properties.insert(
        "command".to_string(),
        JsonSchema::Array {
            items: Box::new(JsonSchema::String { description: None }),
            description: Some("The command to execute as an argv array".to_string()),
        },
    );
    properties.insert(
        "workdir".to_string(),
        JsonSchema::String {
            description: Some("Working directory to run the command in".to_string()),
        },
    );
    properties.insert(
        "timeout".to_string(),
        JsonSchema::Number {
            description: Some("Timeout for the command in milliseconds".to_string()),
        },
    );

    OpenAiTool::Function(ResponsesApiTool {
        name: "shell".to_string(),
        description: "Runs a shell command in a read-only sandbox and returns its output"
            .to_string(),
        strict: false,
        parameters: JsonSchema::Object {
            properties,
            required: Some(vec!["command".to_string()]),
            additional_properties: Some(false),
        },
    })
}

async fn exec_shell_call_readonly(
    sess: &Session,
    sub_id: &str,
    default_timeout_ms: Option<u64>,
    arguments: String,
    call_id: &str,
) -> Result<ResponseItem, String> {
    // Parse arguments → ShellToolCallParams
    let p: ShellToolCallParams = serde_json::from_str(&arguments)
        .map_err(|e| format!("failed to parse function arguments: {e}"))?;

    // Notify UI about the sub-agent shell command being run.
    let preview_cmd = join_shell_command(&p.command);
    let _ = sess
        .tx_event
        .send(Event {
            id: sub_id.to_string(),
            msg: EventMsg::BackgroundEvent(BackgroundEventEvent {
                message: format!("sub-agent[shell] $ {}", preview_cmd),
            }),
        })
        .await;

    let params = ExecParams {
        command: p.command,
        cwd: sess.resolve_path(p.workdir.clone()),
        timeout_ms: p.timeout_ms.or(default_timeout_ms),
        env: sess.default_exec_env(),
        // Always enforce read-only; do not honor escalation in delegate.
        with_escalated_permissions: Some(false),
        justification: None,
    };

    let output = if sess.delegate_enforce_read_only() {
        sess.exec_read_only(params)
            .await
            .map_err(|e| e.to_string())?
    } else {
        sess.exec_with_policy(params, sess.sandbox_policy_ref())
            .await
            .map_err(|e| e.to_string())?
    };
    let exit_code = output.exit_code;
    let formatted = format_exec_output(&output);

    // Notify UI about the command completion with exit status and duration.
    let _ = sess
        .tx_event
        .send(Event {
            id: sub_id.to_string(),
            msg: EventMsg::BackgroundEvent(BackgroundEventEvent {
                message: format!(
                    "sub-agent[shell] exit {} ({}s)",
                    output.exit_code,
                    ((output.duration.as_secs_f32()) * 10.0).round() / 10.0
                ),
            }),
        })
        .await;

    Ok(ResponseItem::FunctionCallOutput {
        call_id: call_id.to_string(),
        output: FunctionCallOutputPayload {
            content: formatted,
            success: Some(exit_code == 0),
        },
    })
}

fn join_shell_command(cmd: &[String]) -> String {
    // Simple shell-ish join for display; avoid heavy deps.
    cmd.iter()
        .map(|s| {
            if s.chars().any(|c| c.is_whitespace()) {
                format!("\"{}\"", s.replace('"', "\\\""))
            } else {
                s.clone()
            }
        })
        .collect::<Vec<String>>()
        .join(" ")
}

fn format_exec_output(exec_output: &crate::exec::ExecToolCallOutput) -> String {
    use serde::Serialize;
    #[derive(Serialize)]
    struct ExecMetadata {
        exit_code: i32,
        duration_seconds: f32,
    }
    #[derive(Serialize)]
    struct ExecOutput<'a> {
        output: &'a str,
        metadata: ExecMetadata,
    }
    let is_success = exec_output.exit_code == 0;
    let text = if is_success {
        &exec_output.stdout
    } else {
        &exec_output.stderr
    };
    let mut formatted_output = text.text.clone();
    if let Some(lines) = text.truncated_after_lines {
        formatted_output.push_str(&format!(
            "\n\n[Output truncated after {lines} lines: too many lines or bytes.]"
        ));
    }
    let payload = ExecOutput {
        output: &formatted_output,
        metadata: ExecMetadata {
            exit_code: exec_output.exit_code,
            duration_seconds: ((exec_output.duration.as_secs_f32()) * 10.0).round() / 10.0,
        },
    };
    serde_json::to_string(&payload).unwrap_or_else(|_| formatted_output)
}

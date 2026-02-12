use std::io::BufRead;
use std::io::BufReader;
use std::process::ChildStdout;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::mpsc::Sender;
use std::thread;
use std::thread::JoinHandle;

use anyhow::Context;
use codex_app_server_protocol::CommandExecutionApprovalDecision;
use codex_app_server_protocol::CommandExecutionRequestApprovalResponse;
use codex_app_server_protocol::FileChangeApprovalDecision;
use codex_app_server_protocol::FileChangeRequestApprovalResponse;
use codex_app_server_protocol::JSONRPCMessage;
use codex_app_server_protocol::JSONRPCNotification;
use codex_app_server_protocol::JSONRPCRequest;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::ServerNotification;
use codex_app_server_protocol::ServerRequest;
use codex_app_server_protocol::ThreadItem;
use codex_app_server_protocol::ThreadListResponse;
use codex_app_server_protocol::ThreadResumeResponse;
use codex_app_server_protocol::ThreadStartResponse;
use serde::Serialize;
use std::io::Write;

use crate::output::LabelColor;
use crate::output::Output;
use crate::state::PendingRequest;
use crate::state::ReaderEvent;
use crate::state::State;

pub fn start_reader(
    mut stdout: BufReader<ChildStdout>,
    stdin: Arc<Mutex<Option<std::process::ChildStdin>>>,
    state: Arc<Mutex<State>>,
    events: Sender<ReaderEvent>,
    output: Output,
    auto_approve: bool,
    filtered_output: bool,
) -> JoinHandle<()> {
    thread::spawn(move || {
        let command_decision = if auto_approve {
            CommandExecutionApprovalDecision::Accept
        } else {
            CommandExecutionApprovalDecision::Decline
        };
        let file_decision = if auto_approve {
            FileChangeApprovalDecision::Accept
        } else {
            FileChangeApprovalDecision::Decline
        };

        let mut buffer = String::new();

        loop {
            buffer.clear();
            match stdout.read_line(&mut buffer) {
                Ok(0) => break,
                Ok(_) => {}
                Err(err) => {
                    let _ = output.client_line(&format!("failed to read from server: {err}"));
                    break;
                }
            }

            let line = buffer.trim_end_matches(['\n', '\r']);
            if !line.is_empty() && !filtered_output {
                let _ = output.server_line(line);
            }

            let Ok(message) = serde_json::from_str::<JSONRPCMessage>(line) else {
                continue;
            };

            match message {
                JSONRPCMessage::Request(request) => {
                    if let Err(err) = handle_server_request(
                        request,
                        &command_decision,
                        &file_decision,
                        &stdin,
                        &output,
                    ) {
                        let _ =
                            output.client_line(&format!("failed to handle server request: {err}"));
                    }
                }
                JSONRPCMessage::Response(response) => {
                    if let Err(err) = handle_response(response, &state, &events) {
                        let _ = output.client_line(&format!("failed to handle response: {err}"));
                    }
                }
                JSONRPCMessage::Notification(notification) => {
                    if filtered_output
                        && let Err(err) = handle_filtered_notification(notification, &output)
                    {
                        let _ =
                            output.client_line(&format!("failed to filter notification: {err}"));
                    }
                }
                _ => {}
            }
        }
    })
}

fn handle_server_request(
    request: JSONRPCRequest,
    command_decision: &CommandExecutionApprovalDecision,
    file_decision: &FileChangeApprovalDecision,
    stdin: &Arc<Mutex<Option<std::process::ChildStdin>>>,
    output: &Output,
) -> anyhow::Result<()> {
    let server_request = match ServerRequest::try_from(request.clone()) {
        Ok(server_request) => server_request,
        Err(_) => return Ok(()),
    };

    match server_request {
        ServerRequest::CommandExecutionRequestApproval { request_id, params } => {
            let response = CommandExecutionRequestApprovalResponse {
                decision: command_decision.clone(),
            };
            output.client_line(&format!(
                "auto-response for command approval {request_id:?}: {command_decision:?} ({params:?})"
            ))?;
            send_response(stdin, request_id, response)
        }
        ServerRequest::FileChangeRequestApproval { request_id, params } => {
            let response = FileChangeRequestApprovalResponse {
                decision: file_decision.clone(),
            };
            output.client_line(&format!(
                "auto-response for file change approval {request_id:?}: {file_decision:?} ({params:?})"
            ))?;
            send_response(stdin, request_id, response)
        }
        _ => Ok(()),
    }
}

fn handle_response(
    response: JSONRPCResponse,
    state: &Arc<Mutex<State>>,
    events: &Sender<ReaderEvent>,
) -> anyhow::Result<()> {
    let pending = {
        let mut state = state.lock().expect("state lock poisoned");
        state.pending.remove(&response.id)
    };

    let Some(pending) = pending else {
        return Ok(());
    };

    match pending {
        PendingRequest::Start => {
            let parsed = serde_json::from_value::<ThreadStartResponse>(response.result)
                .context("decode thread/start response")?;
            let thread_id = parsed.thread.id;
            {
                let mut state = state.lock().expect("state lock poisoned");
                state.thread_id = Some(thread_id.clone());
                if !state.known_threads.iter().any(|id| id == &thread_id) {
                    state.known_threads.push(thread_id.clone());
                }
            }
            events.send(ReaderEvent::ThreadReady { thread_id }).ok();
        }
        PendingRequest::Resume => {
            let parsed = serde_json::from_value::<ThreadResumeResponse>(response.result)
                .context("decode thread/resume response")?;
            let thread_id = parsed.thread.id;
            {
                let mut state = state.lock().expect("state lock poisoned");
                state.thread_id = Some(thread_id.clone());
                if !state.known_threads.iter().any(|id| id == &thread_id) {
                    state.known_threads.push(thread_id.clone());
                }
            }
            events.send(ReaderEvent::ThreadReady { thread_id }).ok();
        }
        PendingRequest::List => {
            let parsed = serde_json::from_value::<ThreadListResponse>(response.result)
                .context("decode thread/list response")?;
            let thread_ids: Vec<String> = parsed.data.into_iter().map(|thread| thread.id).collect();
            {
                let mut state = state.lock().expect("state lock poisoned");
                for thread_id in &thread_ids {
                    if !state.known_threads.iter().any(|id| id == thread_id) {
                        state.known_threads.push(thread_id.clone());
                    }
                }
            }
            events
                .send(ReaderEvent::ThreadList {
                    thread_ids,
                    next_cursor: parsed.next_cursor,
                })
                .ok();
        }
    }

    Ok(())
}

fn handle_filtered_notification(
    notification: JSONRPCNotification,
    output: &Output,
) -> anyhow::Result<()> {
    let Ok(server_notification) = ServerNotification::try_from(notification) else {
        return Ok(());
    };

    match server_notification {
        ServerNotification::ItemCompleted(payload) => {
            emit_filtered_item(payload.item, &payload.thread_id, output)
        }
        _ => Ok(()),
    }
}

fn emit_filtered_item(item: ThreadItem, thread_id: &str, output: &Output) -> anyhow::Result<()> {
    let thread_label = output.format_label(thread_id, LabelColor::Thread);
    match item {
        ThreadItem::AgentMessage { text, .. } => {
            let label = output.format_label("assistant", LabelColor::Assistant);
            output.server_line(&format!("{thread_label} {label}: {text}"))?;
        }
        ThreadItem::Plan { text, .. } => {
            let label = output.format_label("assistant", LabelColor::Assistant);
            output.server_line(&format!("{thread_label} {label}: plan"))?;
            write_multiline(output, &thread_label, &format!("{label}:"), &text)?;
        }
        ThreadItem::CommandExecution {
            command,
            status,
            exit_code,
            aggregated_output,
            ..
        } => {
            let label = output.format_label("tool", LabelColor::Tool);
            output.server_line(&format!(
                "{thread_label} {label}: command {command} ({status:?})"
            ))?;
            if let Some(exit_code) = exit_code {
                let label = output.format_label("tool exit", LabelColor::ToolMeta);
                output.server_line(&format!("{thread_label} {label}: {exit_code}"))?;
            }
            if let Some(aggregated_output) = aggregated_output {
                let label = output.format_label("tool output", LabelColor::ToolMeta);
                write_multiline(
                    output,
                    &thread_label,
                    &format!("{label}:"),
                    &aggregated_output,
                )?;
            }
        }
        ThreadItem::FileChange {
            changes, status, ..
        } => {
            let label = output.format_label("tool", LabelColor::Tool);
            output.server_line(&format!(
                "{thread_label} {label}: file change ({status:?}, {} files)",
                changes.len()
            ))?;
        }
        ThreadItem::McpToolCall {
            server,
            tool,
            status,
            arguments,
            result,
            error,
            ..
        } => {
            let label = output.format_label("tool", LabelColor::Tool);
            output.server_line(&format!(
                "{thread_label} {label}: {server}.{tool} ({status:?})"
            ))?;
            if !arguments.is_null() {
                let label = output.format_label("tool args", LabelColor::ToolMeta);
                output.server_line(&format!("{thread_label} {label}: {arguments}"))?;
            }
            if let Some(result) = result {
                let label = output.format_label("tool result", LabelColor::ToolMeta);
                output.server_line(&format!("{thread_label} {label}: {result:?}"))?;
            }
            if let Some(error) = error {
                let label = output.format_label("tool error", LabelColor::ToolMeta);
                output.server_line(&format!("{thread_label} {label}: {error:?}"))?;
            }
        }
        _ => {}
    }

    Ok(())
}

fn write_multiline(
    output: &Output,
    thread_label: &str,
    header: &str,
    text: &str,
) -> anyhow::Result<()> {
    output.server_line(&format!("{thread_label} {header}"))?;
    for line in text.lines() {
        output.server_line(&format!("{thread_label}   {line}"))?;
    }
    Ok(())
}

fn send_response<T: Serialize>(
    stdin: &Arc<Mutex<Option<std::process::ChildStdin>>>,
    request_id: codex_app_server_protocol::RequestId,
    response: T,
) -> anyhow::Result<()> {
    let result = serde_json::to_value(response).context("serialize response")?;
    let message = JSONRPCResponse {
        id: request_id,
        result,
    };
    let json = serde_json::to_string(&message).context("serialize response message")?;
    let mut line = json;
    line.push('\n');

    let mut stdin = stdin.lock().expect("stdin lock poisoned");
    let Some(stdin) = stdin.as_mut() else {
        anyhow::bail!("stdin already closed");
    };
    stdin.write_all(line.as_bytes()).context("write response")?;
    stdin.flush().context("flush response")?;
    Ok(())
}

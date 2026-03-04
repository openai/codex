use std::process::Stdio;
use std::time::Duration;
use std::time::Instant;

use anyhow::Context;
use anyhow::Result;
use anyhow::anyhow;
use codex_hooks::command_from_argv;
use codex_protocol::ThreadId;
use codex_protocol::approvals::ElicitationRequestEvent;
use codex_protocol::protocol::ApplyPatchApprovalRequestEvent;
use codex_protocol::protocol::EXTERNAL_APPROVAL_HANDLER_WARNING_PREFIX;
use codex_protocol::protocol::ElicitationAction;
use codex_protocol::protocol::ExecApprovalRequestEvent;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::ReviewDecision;
use serde::Serialize;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tracing::info;

use crate::config::types::ApprovalHandlerConfig;

#[derive(Debug, Serialize)]
struct ApprovalCommandRequest<'a, T> {
    thread_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    thread_label: Option<&'a str>,
    #[serde(flatten)]
    event: &'a T,
}

#[derive(Debug)]
struct CommandOutput {
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}

pub(crate) async fn request_exec_approval(
    config: &ApprovalHandlerConfig,
    thread_id: ThreadId,
    thread_label: Option<&str>,
    event: &ExecApprovalRequestEvent,
) -> Result<ReviewDecision> {
    let request = ApprovalCommandRequest {
        thread_id: thread_id.to_string(),
        thread_label,
        event,
    };
    let op = invoke_approval_handler(config, &request).await?;
    match op {
        Op::ExecApproval {
            id,
            turn_id,
            decision,
        } => {
            let expected_id = event.effective_approval_id();
            if id != expected_id {
                return Err(anyhow!(
                    "approval handler returned exec approval for unexpected id `{id}`; expected `{expected_id}`"
                ));
            }
            if let Some(turn_id) = turn_id
                && turn_id != event.turn_id
            {
                return Err(anyhow!(
                    "approval handler returned exec approval for unexpected turn_id `{turn_id}`; expected `{}`",
                    event.turn_id
                ));
            }
            Ok(decision)
        }
        other => Err(anyhow!(
            "approval handler returned wrong op for exec approval request: {other:?}"
        )),
    }
}

pub(crate) fn fallback_warning_message(dialog_kind: &str, err: &anyhow::Error) -> String {
    format!(
        "{EXTERNAL_APPROVAL_HANDLER_WARNING_PREFIX}: {dialog_kind} approval dialog failed; falling back to the built-in prompt. {err:#}"
    )
}

pub(crate) fn deny_warning_message(
    dialog_kind: &str,
    deny_verb: &str,
    err: &anyhow::Error,
) -> String {
    format!(
        "{EXTERNAL_APPROVAL_HANDLER_WARNING_PREFIX}: {dialog_kind} approval dialog failed; {deny_verb} the request. {err:#}"
    )
}

pub(crate) async fn request_patch_approval(
    config: &ApprovalHandlerConfig,
    thread_id: ThreadId,
    thread_label: Option<&str>,
    event: &ApplyPatchApprovalRequestEvent,
) -> Result<ReviewDecision> {
    let request = ApprovalCommandRequest {
        thread_id: thread_id.to_string(),
        thread_label,
        event,
    };
    let op = invoke_approval_handler(config, &request).await?;
    match op {
        Op::PatchApproval { id, decision } => {
            if id != event.call_id {
                return Err(anyhow!(
                    "approval handler returned patch approval for unexpected id `{id}`; expected `{}`",
                    event.call_id
                ));
            }
            Ok(decision)
        }
        other => Err(anyhow!(
            "approval handler returned wrong op for patch approval request: {other:?}"
        )),
    }
}

pub(crate) async fn request_elicitation_approval(
    config: &ApprovalHandlerConfig,
    thread_id: ThreadId,
    thread_label: Option<&str>,
    event: &ElicitationRequestEvent,
) -> Result<ElicitationAction> {
    let request = ApprovalCommandRequest {
        thread_id: thread_id.to_string(),
        thread_label,
        event,
    };
    let op = invoke_approval_handler(config, &request).await?;
    match op {
        Op::ResolveElicitation {
            server_name,
            request_id,
            decision,
        } => {
            if server_name != event.server_name {
                return Err(anyhow!(
                    "approval handler returned elicitation approval for unexpected server `{server_name}`; expected `{}`",
                    event.server_name
                ));
            }
            if request_id != event.id {
                return Err(anyhow!(
                    "approval handler returned elicitation approval for unexpected request_id `{request_id}`; expected `{}`",
                    event.id
                ));
            }
            Ok(decision)
        }
        other => Err(anyhow!(
            "approval handler returned wrong op for elicitation request: {other:?}"
        )),
    }
}

async fn invoke_approval_handler<T: Serialize>(
    config: &ApprovalHandlerConfig,
    request: &T,
) -> Result<Op> {
    let mut command = command_from_argv(&config.command)
        .ok_or_else(|| anyhow!("approval_handler.command must not be empty"))?;
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let input = serde_json::to_vec(request).context("failed to serialize approval request")?;
    let output = run_command(command, input, Duration::from_millis(config.timeout_ms)).await?;
    if output.stdout.is_empty() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow!(
            "approval handler returned no stdout{}",
            format_stderr_suffix(&stderr)
        ));
    }

    serde_json::from_slice::<Op>(&output.stdout).with_context(|| {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        format!(
            "failed to parse approval handler stdout as Op; stdout=`{}`{}",
            stdout.trim(),
            format_stderr_suffix(&stderr)
        )
    })
}

async fn run_command(
    mut command: tokio::process::Command,
    input: Vec<u8>,
    timeout: Duration,
) -> Result<CommandOutput> {
    let start = Instant::now();
    let mut child = command
        .spawn()
        .context("failed to spawn approval handler")?;
    let child_id = child.id();
    info!(
        "approval handler: spawned pid={child_id:?} input_bytes={} timeout_ms={}",
        input.len(),
        timeout.as_millis()
    );
    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| anyhow!("approval handler stdin was not piped"))?;
    let mut stdout = child
        .stdout
        .take()
        .ok_or_else(|| anyhow!("approval handler stdout was not piped"))?;
    let mut stderr = child
        .stderr
        .take()
        .ok_or_else(|| anyhow!("approval handler stderr was not piped"))?;

    stdin
        .write_all(&input)
        .await
        .context("failed to write approval request to handler stdin")?;
    info!(
        "approval handler: stdin write complete pid={child_id:?} elapsed_ms={}",
        start.elapsed().as_millis()
    );
    stdin
        .shutdown()
        .await
        .context("failed to close approval handler stdin")?;
    info!(
        "approval handler: stdin shutdown complete pid={child_id:?} elapsed_ms={}",
        start.elapsed().as_millis()
    );
    drop(stdin);
    info!(
        "approval handler: stdin dropped pid={child_id:?} elapsed_ms={}",
        start.elapsed().as_millis()
    );

    let stdout_task = tokio::spawn(async move {
        let mut bytes = Vec::new();
        stdout.read_to_end(&mut bytes).await.map(|_| bytes)
    });
    let stderr_task = tokio::spawn(async move {
        let mut bytes = Vec::new();
        stderr.read_to_end(&mut bytes).await.map(|_| bytes)
    });

    info!(
        "approval handler: waiting for child pid={child_id:?} elapsed_ms={}",
        start.elapsed().as_millis()
    );
    let status = match tokio::time::timeout(timeout, child.wait()).await {
        Ok(status) => {
            let status = status.context("failed to wait for approval handler")?;
            info!(
                "approval handler: child exited pid={child_id:?} status={status} elapsed_ms={}",
                start.elapsed().as_millis()
            );
            status
        }
        Err(_) => {
            info!(
                "approval handler: timeout waiting for child pid={child_id:?} elapsed_ms={}",
                start.elapsed().as_millis()
            );
            let _ = child.kill().await;
            let _ = child.wait().await;
            return Err(anyhow!(
                "approval handler timed out after {} ms",
                timeout.as_millis()
            ));
        }
    };

    let stdout = stdout_task
        .await
        .context("approval handler stdout task join failed")?
        .context("failed to read approval handler stdout")?;
    let stderr = stderr_task
        .await
        .context("approval handler stderr task join failed")?
        .context("failed to read approval handler stderr")?;

    if !status.success() {
        let stderr_text = String::from_utf8_lossy(&stderr);
        return Err(anyhow!(
            "approval handler exited with status {status}{}",
            format_stderr_suffix(&stderr_text)
        ));
    }

    Ok(CommandOutput { stdout, stderr })
}

fn format_stderr_suffix(stderr: &str) -> String {
    let trimmed = stderr.trim();
    if trimmed.is_empty() {
        String::new()
    } else {
        format!("; stderr=`{trimmed}`")
    }
}

#[cfg(test)]
mod tests {
    use codex_protocol::approvals::NetworkPolicyAmendment;
    use codex_protocol::mcp::RequestId;
    use codex_protocol::protocol::ExecPolicyAmendment;
    use codex_protocol::protocol::NetworkPolicyRuleAction;
    use pretty_assertions::assert_eq;
    use serde_json::json;

    use super::*;

    #[tokio::test]
    async fn exec_validation_accepts_matching_op() {
        let event = ExecApprovalRequestEvent {
            call_id: "call-1".to_string(),
            approval_id: Some("approval-1".to_string()),
            turn_id: "turn-1".to_string(),
            command: vec!["echo".to_string(), "hi".to_string()],
            cwd: std::env::temp_dir(),
            reason: None,
            network_approval_context: None,
            proposed_execpolicy_amendment: None,
            proposed_network_policy_amendments: None,
            additional_permissions: None,
            available_decisions: None,
            parsed_cmd: Vec::new(),
        };

        let op = Op::ExecApproval {
            id: "approval-1".to_string(),
            turn_id: Some("turn-1".to_string()),
            decision: ReviewDecision::Approved,
        };

        let decision = match op {
            Op::ExecApproval {
                id,
                turn_id,
                decision,
            } => {
                assert_eq!(id, event.effective_approval_id());
                assert_eq!(turn_id.as_deref(), Some(event.turn_id.as_str()));
                decision
            }
            _ => unreachable!(),
        };

        assert_eq!(decision, ReviewDecision::Approved);
    }

    #[test]
    fn stderr_suffix_omits_empty_values() {
        assert_eq!(format_stderr_suffix(""), "");
        assert_eq!(format_stderr_suffix("  \n"), "");
        assert_eq!(format_stderr_suffix("oops\n"), "; stderr=`oops`");
    }

    #[test]
    fn approval_command_request_omits_thread_label_when_absent() {
        let event = ExecApprovalRequestEvent {
            call_id: "call-123".to_string(),
            approval_id: Some("approval-123".to_string()),
            turn_id: "turn-123".to_string(),
            command: vec!["echo".to_string(), "hello".to_string()],
            cwd: "/tmp".into(),
            reason: Some("because".to_string()),
            network_approval_context: None,
            proposed_execpolicy_amendment: Some(ExecPolicyAmendment {
                command: vec!["echo".to_string()],
            }),
            proposed_network_policy_amendments: Some(vec![NetworkPolicyAmendment {
                host: "example.com".to_string(),
                action: NetworkPolicyRuleAction::Allow,
            }]),
            additional_permissions: None,
            available_decisions: Some(vec![ReviewDecision::Approved]),
            parsed_cmd: Vec::new(),
        };
        let request = ApprovalCommandRequest {
            thread_id: "thread-123".to_string(),
            thread_label: None,
            event: &event,
        };

        let value = serde_json::to_value(&request).expect("request should serialize");

        assert_eq!(value["thread_id"], json!("thread-123"));
        assert!(value.get("thread_label").is_none());
        assert_eq!(value["call_id"], json!("call-123"));
        assert_eq!(value["turn_id"], json!("turn-123"));
        assert_eq!(value["command"], json!(["echo", "hello"]));
        assert_eq!(value["reason"], json!("because"));
    }

    #[test]
    fn approval_command_request_includes_thread_label_when_present() {
        let event = ElicitationRequestEvent {
            server_name: "server".to_string(),
            id: RequestId::Integer(7),
            message: "need info".to_string(),
        };
        let request = ApprovalCommandRequest {
            thread_id: "thread-456".to_string(),
            thread_label: Some("Scout [worker]"),
            event: &event,
        };

        let value = serde_json::to_value(&request).expect("request should serialize");

        assert_eq!(value["thread_id"], json!("thread-456"));
        assert_eq!(value["thread_label"], json!("Scout [worker]"));
        assert_eq!(value["server_name"], json!("server"));
        assert_eq!(value["id"], json!(7));
        assert_eq!(value["message"], json!("need info"));
    }

    #[test]
    fn elicitation_request_id_equality_matches_both_variants() {
        assert_eq!(
            RequestId::String("abc".to_string()),
            RequestId::String("abc".to_string())
        );
        assert_eq!(RequestId::Integer(7), RequestId::Integer(7));
    }

    #[test]
    fn fallback_warning_message_uses_red_warning_prefix() {
        let err = anyhow!("approval handler timed out after 1000 ms");

        let message = fallback_warning_message("exec", &err);

        assert_eq!(
            message,
            "External approval handler failed: exec approval dialog failed; falling back to the built-in prompt. approval handler timed out after 1000 ms"
        );
    }

    #[test]
    fn deny_warning_message_uses_requested_verb() {
        let err = anyhow!("approval handler exited with status 1");

        let message = deny_warning_message("elicitation", "declining", &err);

        assert_eq!(
            message,
            "External approval handler failed: elicitation approval dialog failed; declining the request. approval handler exited with status 1"
        );
    }
}

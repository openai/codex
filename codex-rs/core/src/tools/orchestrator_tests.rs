use super::*;
use crate::state::ActiveTurn;
use crate::tools::sandboxing::Approvable;
use crate::tools::sandboxing::PermissionRequestPayload;
use crate::tools::sandboxing::Sandboxable;
use codex_hooks::Hooks;
use codex_hooks::HooksConfig;
use codex_protocol::exec_output::ExecToolCallOutput;
use codex_protocol::exec_output::StreamOutput;
use codex_protocol::protocol::EventMsg;
use codex_sandboxing::SandboxablePreference;
use futures::future::BoxFuture;
use pretty_assertions::assert_eq;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::timeout;

#[derive(Default)]
struct OneShotProbe {
    attempts: usize,
}

impl Approvable<()> for OneShotProbe {
    type ApprovalKey = String;

    fn approval_keys(&self, _req: &()) -> Vec<Self::ApprovalKey> {
        Vec::new()
    }

    fn exec_approval_requirement(&self, _req: &()) -> Option<ExecApprovalRequirement> {
        Some(ExecApprovalRequirement::NeedsOneShotApproval {
            reason: Some("one run only".to_string()),
        })
    }

    fn permission_request_payload(&self, _req: &()) -> Option<PermissionRequestPayload> {
        Some(PermissionRequestPayload::bash(
            "probe".to_string(),
            Some("one-shot probe".to_string()),
        ))
    }

    fn start_approval_async<'a>(
        &'a mut self,
        _req: &'a (),
        ctx: ApprovalCtx<'a>,
    ) -> BoxFuture<'a, ReviewDecision> {
        Box::pin(async move {
            let cwd = ctx
                .turn
                .environments
                .primary()
                .expect("primary environment")
                .cwd()
                .to_abs_path()
                .expect("local environment cwd");
            ctx.session
                .request_command_approval(
                    ctx.turn,
                    ctx.call_id.to_string(),
                    ctx.approval_id,
                    Some(ctx.approval_purpose),
                    /*environment_id*/ None,
                    vec!["probe".to_string()],
                    cwd,
                    ctx.retry_reason,
                    ctx.network_approval_context,
                    /*proposed_execpolicy_amendment*/ None,
                    /*additional_permissions*/ None,
                    Some(vec![ReviewDecision::Approved, ReviewDecision::Abort]),
                )
                .await
        })
    }
}

fn install_sequenced_permission_hook(
    session: &Arc<crate::session::session::Session>,
    turn: &Arc<crate::session::turn_context::TurnContext>,
    modes: &[&str],
) -> std::path::PathBuf {
    let home = &turn.config.codex_home;
    std::fs::create_dir_all(home).expect("recreate codex home");
    let script_path = home.join("permission_request_hook.py");
    let log_path = home.join("permission_request_hook_log.jsonl");
    std::fs::write(
        &script_path,
        format!(
            r#"import json
from pathlib import Path
import sys

log_path = Path(r"{log_path}")
modes = {modes}
payload = json.load(sys.stdin)
seen = [] if not log_path.exists() else log_path.read_text(encoding="utf-8").splitlines()
with log_path.open("a", encoding="utf-8") as handle:
    handle.write(json.dumps(payload) + "\n")
mode = modes[min(len(seen), len(modes) - 1)]
decision = {{"behavior": "allow"}} if mode == "allow" else {{"behavior": "deny", "message": "blocked by test hook"}}
print(json.dumps({{"hookSpecificOutput": {{"hookEventName": "PermissionRequest", "decision": decision}}}}))
"#,
            log_path = log_path.display(),
            modes = serde_json::to_string(modes).expect("serialize hook modes"),
        ),
    )
    .expect("write permission hook");
    std::fs::write(
        home.join("hooks.json"),
        serde_json::json!({
            "hooks": {
                "PermissionRequest": [{
                    "matcher": "^Bash$",
                    "hooks": [{
                        "type": "command",
                        "command": format!(
                            "{} \"{}\"",
                            if cfg!(windows) { "python" } else { "python3" },
                            script_path.display(),
                        ),
                    }],
                }],
            },
        })
        .to_string(),
    )
    .expect("write hooks config");

    let mut shell_argv = session
        .user_shell()
        .derive_exec_args("", /*use_login_shell*/ false);
    let shell_program = shell_argv.remove(0);
    let _ = shell_argv.pop();
    session
        .services
        .hooks
        .store(Arc::new(Hooks::new(HooksConfig {
            feature_enabled: true,
            bypass_hook_trust: true,
            config_layer_stack: Some(turn.config.config_layer_stack.clone()),
            shell_program: Some(shell_program),
            shell_args: shell_argv,
            ..HooksConfig::default()
        })));
    log_path.into_path_buf()
}

async fn next_exec_approval(
    events: &async_channel::Receiver<codex_protocol::protocol::Event>,
) -> codex_protocol::protocol::ExecApprovalRequestEvent {
    loop {
        let event = events.recv().await.expect("approval event");
        if let EventMsg::ExecApprovalRequest(approval) = event.msg {
            return approval;
        }
    }
}

impl Sandboxable for OneShotProbe {
    fn sandbox_preference(&self) -> SandboxablePreference {
        SandboxablePreference::Auto
    }
}

impl ToolRuntime<(), ()> for OneShotProbe {
    async fn run(
        &mut self,
        _req: &(),
        _attempt: &SandboxAttempt<'_>,
        _ctx: &ToolCtx,
    ) -> Result<(), ToolError> {
        self.attempts += 1;
        if self.attempts == 1 {
            let output = ExecToolCallOutput {
                exit_code: 1,
                stdout: StreamOutput::new(String::new()),
                stderr: StreamOutput::new("sandbox denied".to_string()),
                aggregated_output: StreamOutput::new("sandbox denied".to_string()),
                duration: Duration::from_millis(1),
                timed_out: false,
            };
            Err(ToolError::Codex(CodexErr::Sandbox(SandboxErr::Denied {
                output: Box::new(output),
                network_policy_decision: None,
            })))
        } else {
            Ok(())
        }
    }
}

#[tokio::test]
async fn one_shot_retry_uses_distinct_waiter_and_ignores_stale_initial_responses() {
    let (session, turn, events) = crate::session::tests::make_session_and_context_with_rx().await;
    *session.active_turn.lock().await = Some(ActiveTurn::default());
    let call_id = "one-shot-probe".to_string();
    let tool_ctx = ToolCtx {
        session: Arc::clone(&session),
        turn: Arc::clone(&turn),
        call_id: call_id.clone(),
        tool_name: codex_tools::ToolName::plain("probe"),
    };
    let mut run = tokio::spawn(async move {
        let mut probe = OneShotProbe::default();
        let result = ToolOrchestrator::new()
            .run(
                &mut probe,
                &(),
                &tool_ctx,
                turn.as_ref(),
                AskForApproval::UnlessTrusted,
            )
            .await;
        (result, probe.attempts)
    });

    let initial = events.recv().await.expect("initial approval event");
    let EventMsg::ExecApprovalRequest(initial) = initial.msg else {
        panic!("expected initial exec approval");
    };
    assert_eq!(initial.call_id, call_id);
    assert_eq!(initial.approval_purpose, Some(ExecApprovalPurpose::Initial));
    let initial_id = initial.approval_id.clone().expect("initial callback ID");
    assert_ne!(initial_id, call_id);
    Uuid::parse_str(&initial_id).expect("initial callback ID should be a UUID");
    assert_eq!(
        initial.effective_available_decisions(),
        vec![ReviewDecision::Approved, ReviewDecision::Abort]
    );
    session
        .notify_exec_approval(&initial_id, ReviewDecision::Approved)
        .await;

    let retry = events.recv().await.expect("retry approval event");
    let EventMsg::ExecApprovalRequest(retry) = retry.msg else {
        panic!("expected retry exec approval");
    };
    assert_eq!(retry.call_id, call_id);
    assert_eq!(
        retry.approval_purpose,
        Some(ExecApprovalPurpose::SandboxRetry)
    );
    let retry_id = retry.approval_id.expect("retry callback ID");
    assert_ne!(retry_id, call_id);
    assert_ne!(retry_id, initial_id);
    Uuid::parse_str(&retry_id).expect("retry callback ID should be a UUID");

    for stale_id in [&call_id, &initial_id] {
        for stale_decision in [ReviewDecision::Approved, ReviewDecision::Abort] {
            session.notify_exec_approval(stale_id, stale_decision).await;
        }
    }
    assert!(
        timeout(Duration::from_millis(50), &mut run).await.is_err(),
        "stale initial responses must not resolve the retry waiter"
    );

    session
        .notify_exec_approval(&retry_id, ReviewDecision::Approved)
        .await;
    let (result, attempts) = timeout(Duration::from_secs(1), &mut run)
        .await
        .expect("orchestrator timed out")
        .expect("orchestrator task failed");
    result.expect("approved retry should succeed");
    assert_eq!(attempts, 2);
}

#[tokio::test]
async fn one_shot_hooks_allow_falls_through_and_deny_blocks_each_phase() {
    for (modes, expected_approvals, expected_attempts, should_succeed) in [
        (&["allow", "allow"][..], 2, 2, true),
        (&["deny"][..], 0, 0, false),
        (&["allow", "deny"][..], 1, 1, false),
    ] {
        let (session, turn, events) =
            crate::session::tests::make_session_and_context_with_rx().await;
        *session.active_turn.lock().await = Some(ActiveTurn::default());
        let log_path = install_sequenced_permission_hook(&session, &turn, modes);
        let tool_ctx = ToolCtx {
            session: Arc::clone(&session),
            turn: Arc::clone(&turn),
            call_id: format!("hook-probe-{}", modes.join("-")),
            tool_name: codex_tools::ToolName::plain("probe"),
        };
        let run = tokio::spawn(async move {
            let mut probe = OneShotProbe::default();
            let result = ToolOrchestrator::new()
                .run(
                    &mut probe,
                    &(),
                    &tool_ctx,
                    turn.as_ref(),
                    AskForApproval::UnlessTrusted,
                )
                .await;
            (result, probe.attempts)
        });

        for index in 0..expected_approvals {
            let approval = next_exec_approval(&events).await;
            assert_eq!(
                approval.approval_purpose,
                Some(if index == 0 {
                    ExecApprovalPurpose::Initial
                } else {
                    ExecApprovalPurpose::SandboxRetry
                })
            );
            session
                .notify_exec_approval(&approval.effective_approval_id(), ReviewDecision::Approved)
                .await;
        }

        let (result, attempts) = timeout(Duration::from_secs(5), run)
            .await
            .expect("hook scenario timed out")
            .expect("hook scenario task failed");
        assert_eq!(result.is_ok(), should_succeed, "hook modes: {modes:?}");
        assert_eq!(attempts, expected_attempts, "hook modes: {modes:?}");
        assert_eq!(
            std::fs::read_to_string(log_path)
                .expect("read permission hook log")
                .lines()
                .count(),
            modes.len(),
        );
    }
}

#[test]
fn one_shot_never_bypasses_retry_approval() {
    let requirement = ExecApprovalRequirement::NeedsOneShotApproval {
        reason: Some("one run only".to_string()),
    };

    assert!(!can_bypass_retry_approval(
        /*strict_auto_review*/ false,
        &requirement,
        /*policy_bypasses_approval*/ true,
        /*has_network_approval_context*/ false,
    ));
    assert_eq!(
        permission_request_hook_mode(/*strict_auto_review*/ false, &requirement),
        PermissionRequestHookMode::DenyOnly,
    );
}

#[test]
fn cacheable_approval_keeps_session_retry_and_hook_behavior() {
    let requirement = ExecApprovalRequirement::NeedsApproval {
        reason: None,
        proposed_execpolicy_amendment: None,
    };

    assert!(can_bypass_retry_approval(
        /*strict_auto_review*/ false,
        &requirement,
        /*policy_bypasses_approval*/ true,
        /*has_network_approval_context*/ false,
    ));
    assert_eq!(
        permission_request_hook_mode(/*strict_auto_review*/ false, &requirement),
        PermissionRequestHookMode::AllowAndDeny,
    );
    assert_eq!(
        permission_request_hook_mode(/*strict_auto_review*/ true, &requirement),
        PermissionRequestHookMode::Skip,
    );
}

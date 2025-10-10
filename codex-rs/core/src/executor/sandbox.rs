use crate::apply_patch::ApplyPatchExec;
use crate::codex::Session;
use crate::exec::SandboxType;
use crate::executor::ExecutionMode;
use crate::executor::ExecutionRequest;
use crate::executor::ExecutorConfig;
use crate::executor::errors::ExecError;
use crate::safety::SafetyCheck;
use crate::safety::assess_command_safety;
use crate::safety::assess_patch_safety;
use codex_otel::otel_event_manager::OtelEventManager;
use codex_otel::otel_event_manager::ToolDecisionSource;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::ReviewDecision;
use std::collections::HashSet;

/// Sandbox placement options selected for an execution run, including whether
/// to escalate after failures and whether approvals should persist.
pub(crate) struct SandboxDecision {
    pub(crate) initial_sandbox: SandboxType,
    pub(crate) escalate_on_failure: bool,
    pub(crate) record_session_approval: bool,
}

impl SandboxDecision {
    fn auto(sandbox: SandboxType, escalate_on_failure: bool) -> Self {
        Self {
            initial_sandbox: sandbox,
            escalate_on_failure,
            record_session_approval: false,
        }
    }

    fn user_override(record_session_approval: bool) -> Self {
        Self {
            initial_sandbox: SandboxType::None,
            escalate_on_failure: false,
            record_session_approval,
        }
    }
}

fn should_escalate_on_failure(approval: AskForApproval, sandbox: SandboxType) -> bool {
    matches!(
        (approval, sandbox),
        (
            AskForApproval::UnlessTrusted | AskForApproval::OnFailure,
            SandboxType::MacosSeatbelt | SandboxType::LinuxSeccomp
        )
    )
}

/// Determines how a command should be sandboxed, prompting the user when
/// policy requires explicit approval.
#[allow(clippy::too_many_arguments)]
pub async fn select_sandbox(
    request: &ExecutionRequest,
    approval_policy: AskForApproval,
    approval_cache: HashSet<Vec<String>>,
    config: &ExecutorConfig,
    session: &Session,
    sub_id: &str,
    call_id: &str,
    otel_event_manager: &OtelEventManager,
) -> Result<SandboxDecision, ExecError> {
    match &request.mode {
        ExecutionMode::Shell => {
            select_shell_sandbox(
                request,
                approval_policy,
                approval_cache,
                config,
                session,
                sub_id,
                call_id,
                otel_event_manager,
            )
            .await
        }
        ExecutionMode::ApplyPatch(exec) => {
            select_apply_patch_sandbox(exec, approval_policy, config)
        }
    }
}

#[allow(clippy::too_many_arguments)]
async fn select_shell_sandbox(
    request: &ExecutionRequest,
    approval_policy: AskForApproval,
    approved_snapshot: HashSet<Vec<String>>,
    config: &ExecutorConfig,
    session: &Session,
    sub_id: &str,
    call_id: &str,
    otel_event_manager: &OtelEventManager,
) -> Result<SandboxDecision, ExecError> {
    let command_for_safety = if request.approval_command.is_empty() {
        request.params.command.clone()
    } else {
        request.approval_command.clone()
    };

    let safety = assess_command_safety(
        &command_for_safety,
        approval_policy,
        &config.sandbox_policy,
        &approved_snapshot,
        request.params.with_escalated_permissions.unwrap_or(false),
    );

    match safety {
        SafetyCheck::AutoApprove {
            sandbox_type,
            user_explicitly_approved,
        } => {
            let mut decision = SandboxDecision::auto(
                sandbox_type,
                should_escalate_on_failure(approval_policy, sandbox_type),
            );
            if user_explicitly_approved {
                decision.record_session_approval = true;
            }
            let (decision_for_event, source) = if user_explicitly_approved {
                (ReviewDecision::ApprovedForSession, ToolDecisionSource::User)
            } else {
                (ReviewDecision::Approved, ToolDecisionSource::Config)
            };
            otel_event_manager.tool_decision("local_shell", call_id, decision_for_event, source);
            Ok(decision)
        }
        SafetyCheck::AskUser => {
            let decision = session
                .request_command_approval(
                    sub_id.to_string(),
                    call_id.to_string(),
                    request.approval_command.clone(),
                    request.params.cwd.clone(),
                    request.params.justification.clone(),
                )
                .await;

            otel_event_manager.tool_decision(
                "local_shell",
                call_id,
                decision,
                ToolDecisionSource::User,
            );
            match decision {
                ReviewDecision::Approved => Ok(SandboxDecision::user_override(false)),
                ReviewDecision::ApprovedForSession => Ok(SandboxDecision::user_override(true)),
                ReviewDecision::Denied | ReviewDecision::Abort => {
                    Err(ExecError::rejection("exec command rejected by user"))
                }
            }
        }
        SafetyCheck::Reject { reason } => Err(ExecError::rejection(format!(
            "exec command rejected: {reason}"
        ))),
    }
}

fn select_apply_patch_sandbox(
    exec: &ApplyPatchExec,
    approval_policy: AskForApproval,
    config: &ExecutorConfig,
) -> Result<SandboxDecision, ExecError> {
    if exec.user_explicitly_approved_this_action {
        return Ok(SandboxDecision::user_override(false));
    }

    match assess_patch_safety(
        &exec.action,
        approval_policy,
        &config.sandbox_policy,
        &config.sandbox_cwd,
    ) {
        SafetyCheck::AutoApprove { sandbox_type, .. } => Ok(SandboxDecision::auto(
            sandbox_type,
            should_escalate_on_failure(approval_policy, sandbox_type),
        )),
        SafetyCheck::AskUser => Err(ExecError::rejection(
            "patch requires approval but none was recorded",
        )),
        SafetyCheck::Reject { reason } => {
            Err(ExecError::rejection(format!("patch rejected: {reason}")))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codex::make_session_and_context;
    use crate::exec::ExecParams;
    use crate::function_tool::FunctionCallError;
    use crate::protocol::SandboxPolicy;
    use codex_apply_patch::ApplyPatchAction;
    use pretty_assertions::assert_eq;

    #[tokio::test]
    async fn select_apply_patch_user_override_when_explicit() {
        let (session, ctx) = make_session_and_context();
        let tmp = tempfile::tempdir().expect("tmp");
        let p = tmp.path().join("a.txt");
        let action = ApplyPatchAction::new_add_for_test(&p, "hello".to_string());
        let exec = ApplyPatchExec {
            action,
            user_explicitly_approved_this_action: true,
        };
        let cfg = ExecutorConfig::new(SandboxPolicy::ReadOnly, std::env::temp_dir(), None);
        let request = ExecutionRequest {
            params: ExecParams {
                command: vec!["apply_patch".into()],
                cwd: std::env::temp_dir(),
                timeout_ms: None,
                env: std::collections::HashMap::new(),
                with_escalated_permissions: None,
                justification: None,
            },
            approval_command: vec!["apply_patch".into()],
            mode: ExecutionMode::ApplyPatch(exec),
            stdout_stream: None,
            use_shell_profile: false,
        };
        let otel_event_manager = ctx.client.get_otel_event_manager();
        let decision = select_sandbox(
            &request,
            AskForApproval::OnRequest,
            Default::default(),
            &cfg,
            &session,
            "sub",
            "call",
            &otel_event_manager,
        )
        .await
        .expect("ok");
        // Explicit user override runs without sandbox
        assert_eq!(decision.initial_sandbox, SandboxType::None);
        assert_eq!(decision.escalate_on_failure, false);
    }

    #[tokio::test]
    async fn select_apply_patch_autoapprove_in_danger() {
        let (session, ctx) = make_session_and_context();
        let tmp = tempfile::tempdir().expect("tmp");
        let p = tmp.path().join("a.txt");
        let action = ApplyPatchAction::new_add_for_test(&p, "hello".to_string());
        let exec = ApplyPatchExec {
            action,
            user_explicitly_approved_this_action: false,
        };
        let cfg = ExecutorConfig::new(SandboxPolicy::DangerFullAccess, std::env::temp_dir(), None);
        let request = ExecutionRequest {
            params: ExecParams {
                command: vec!["apply_patch".into()],
                cwd: std::env::temp_dir(),
                timeout_ms: None,
                env: std::collections::HashMap::new(),
                with_escalated_permissions: None,
                justification: None,
            },
            approval_command: vec!["apply_patch".into()],
            mode: ExecutionMode::ApplyPatch(exec),
            stdout_stream: None,
            use_shell_profile: false,
        };
        let otel_event_manager = ctx.client.get_otel_event_manager();
        let decision = select_sandbox(
            &request,
            AskForApproval::OnRequest,
            Default::default(),
            &cfg,
            &session,
            "sub",
            "call",
            &otel_event_manager,
        )
        .await
        .expect("ok");
        // On platforms with a sandbox, DangerFullAccess still prefers it
        let expected = crate::safety::get_platform_sandbox().unwrap_or(SandboxType::None);
        assert_eq!(decision.initial_sandbox, expected);
        assert_eq!(decision.escalate_on_failure, false);
    }

    #[tokio::test]
    async fn select_apply_patch_requires_approval_on_unless_trusted() {
        let (session, ctx) = make_session_and_context();
        let tempdir = tempfile::tempdir().expect("tmpdir");
        let p = tempdir.path().join("a.txt");
        let action = ApplyPatchAction::new_add_for_test(&p, "hello".to_string());
        let exec = ApplyPatchExec {
            action,
            user_explicitly_approved_this_action: false,
        };
        let cfg = ExecutorConfig::new(SandboxPolicy::ReadOnly, std::env::temp_dir(), None);
        let request = ExecutionRequest {
            params: ExecParams {
                command: vec!["apply_patch".into()],
                cwd: std::env::temp_dir(),
                timeout_ms: None,
                env: std::collections::HashMap::new(),
                with_escalated_permissions: None,
                justification: None,
            },
            approval_command: vec!["apply_patch".into()],
            mode: ExecutionMode::ApplyPatch(exec),
            stdout_stream: None,
            use_shell_profile: false,
        };
        let otel_event_manager = ctx.client.get_otel_event_manager();
        let result = select_sandbox(
            &request,
            AskForApproval::UnlessTrusted,
            Default::default(),
            &cfg,
            &session,
            "sub",
            "call",
            &otel_event_manager,
        )
        .await;
        match result {
            Ok(_) => panic!("expected error"),
            Err(ExecError::Function(FunctionCallError::RespondToModel(msg))) => {
                assert!(msg.contains("requires approval"))
            }
            Err(other) => panic!("unexpected error: {other:?}"),
        }
    }

    #[tokio::test]
    async fn select_shell_autoapprove_in_danger_mode() {
        let (session, ctx) = make_session_and_context();
        let cfg = ExecutorConfig::new(SandboxPolicy::DangerFullAccess, std::env::temp_dir(), None);
        let request = ExecutionRequest {
            params: ExecParams {
                command: vec!["some-unknown".into()],
                cwd: std::env::temp_dir(),
                timeout_ms: None,
                env: std::collections::HashMap::new(),
                with_escalated_permissions: None,
                justification: None,
            },
            approval_command: vec!["some-unknown".into()],
            mode: ExecutionMode::Shell,
            stdout_stream: None,
            use_shell_profile: false,
        };
        let otel_event_manager = ctx.client.get_otel_event_manager();
        let decision = select_sandbox(
            &request,
            AskForApproval::OnRequest,
            Default::default(),
            &cfg,
            &session,
            "sub",
            "call",
            &otel_event_manager,
        )
        .await
        .expect("ok");
        assert_eq!(decision.initial_sandbox, SandboxType::None);
        assert_eq!(decision.escalate_on_failure, false);
    }

    #[cfg(any(target_os = "macos", target_os = "linux"))]
    #[tokio::test]
    async fn select_shell_escalates_on_failure_with_platform_sandbox() {
        let (session, ctx) = make_session_and_context();
        let cfg = ExecutorConfig::new(SandboxPolicy::ReadOnly, std::env::temp_dir(), None);
        let request = ExecutionRequest {
            params: ExecParams {
                // Unknown command => untrusted but not flagged dangerous
                command: vec!["some-unknown".into()],
                cwd: std::env::temp_dir(),
                timeout_ms: None,
                env: std::collections::HashMap::new(),
                with_escalated_permissions: None,
                justification: None,
            },
            approval_command: vec!["some-unknown".into()],
            mode: ExecutionMode::Shell,
            stdout_stream: None,
            use_shell_profile: false,
        };
        let otel_event_manager = ctx.client.get_otel_event_manager();
        let decision = select_sandbox(
            &request,
            AskForApproval::OnFailure,
            Default::default(),
            &cfg,
            &session,
            "sub",
            "call",
            &otel_event_manager,
        )
        .await
        .expect("ok");
        // On macOS/Linux we should have a platform sandbox and escalate on failure
        assert_ne!(decision.initial_sandbox, SandboxType::None);
        assert_eq!(decision.escalate_on_failure, true);
    }
}

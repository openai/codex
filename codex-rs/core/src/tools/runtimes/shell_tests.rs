use super::*;
use crate::state::ActiveTurn;
use codex_exec_server::Environment;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::ExecApprovalPurpose;
use codex_utils_path_uri::PathUri;
use std::sync::Arc;

fn one_shot_request() -> ShellRequest {
    let cwd = AbsolutePathBuf::try_from(std::env::current_dir().expect("read current dir"))
        .expect("current dir is absolute");
    ShellRequest {
        command: vec!["echo".to_string(), "one shot".to_string()],
        turn_environment: TurnEnvironment::new(
            "remote".to_string(),
            Arc::new(Environment::default_for_tests()),
            PathUri::from_abs_path(&cwd),
            /*shell*/ None,
        ),
        shell_type: None,
        hook_command: "echo 'one shot'".to_string(),
        cwd,
        timeout_ms: None,
        cancellation_token: CancellationToken::new(),
        env: HashMap::new(),
        explicit_env_overrides: HashMap::new(),
        network: None,
        sandbox_permissions: SandboxPermissions::UseDefault,
        additional_permissions: None,
        #[cfg(unix)]
        additional_permissions_preapproved: false,
        justification: None,
        exec_approval_requirement: ExecApprovalRequirement::NeedsOneShotApproval {
            reason: Some("one run only".to_string()),
        },
    }
}

#[tokio::test]
async fn approval_key_includes_environment_id() {
    let cwd = AbsolutePathBuf::try_from(std::env::current_dir().expect("read current dir"))
        .expect("current dir is absolute");
    let mut request = ShellRequest {
        command: vec!["echo".to_string(), "hello".to_string()],
        turn_environment: TurnEnvironment::new(
            "remote".to_string(),
            Arc::new(Environment::default_for_tests()),
            PathUri::from_abs_path(&cwd),
            /*shell*/ None,
        ),
        shell_type: None,
        hook_command: "echo hello".to_string(),
        cwd: cwd.clone(),
        timeout_ms: None,
        cancellation_token: CancellationToken::new(),
        env: HashMap::new(),
        explicit_env_overrides: HashMap::new(),
        network: None,
        sandbox_permissions: SandboxPermissions::UseDefault,
        additional_permissions: None,
        #[cfg(unix)]
        additional_permissions_preapproved: false,
        justification: None,
        exec_approval_requirement: ExecApprovalRequirement::Skip {
            bypass_sandbox: false,
            proposed_execpolicy_amendment: None,
        },
    };
    let runtime = ShellRuntime::for_shell_command(ShellRuntimeBackend::ShellCommandClassic);
    let original_key = runtime.approval_keys(&request);
    request.turn_environment.environment_id = "other".to_string();
    let other_key = runtime.approval_keys(&request);

    assert_ne!(original_key, other_key);
}

#[tokio::test]
async fn one_shot_approval_has_no_session_cache_key() {
    let request = one_shot_request();
    let runtime = ShellRuntime::for_shell_command(ShellRuntimeBackend::ShellCommandClassic);

    assert!(runtime.approval_keys(&request).is_empty());
}

#[tokio::test]
async fn one_shot_approval_routes_by_callback_id() {
    let (session, turn, events) = crate::session::tests::make_session_and_context_with_rx().await;
    *session.active_turn.lock().await = Some(ActiveTurn::default());
    let mut runtime = ShellRuntime::for_shell_command(ShellRuntimeBackend::ShellCommandClassic);
    let request = one_shot_request();
    let call_id = "command-item";
    let approval_id = "retry-callback";
    let approval = runtime.start_approval_async(
        &request,
        ApprovalCtx {
            session: &session,
            turn: &turn,
            call_id,
            approval_id: Some(approval_id.to_string()),
            approval_purpose: ExecApprovalPurpose::SandboxRetry,
            guardian_review_id: None,
            retry_reason: Some("sandbox denied".to_string()),
            network_approval_context: None,
        },
    );
    let respond = async {
        let event = events.recv().await.expect("approval event");
        let EventMsg::ExecApprovalRequest(event) = event.msg else {
            panic!("expected exec approval");
        };
        assert_eq!(event.call_id, call_id);
        assert_eq!(event.approval_id.as_deref(), Some(approval_id));
        assert_eq!(
            event.approval_purpose,
            Some(ExecApprovalPurpose::SandboxRetry)
        );
        assert_eq!(
            event.effective_available_decisions(),
            vec![ReviewDecision::Approved, ReviewDecision::Abort]
        );
        session
            .notify_exec_approval(approval_id, ReviewDecision::Approved)
            .await;
    };

    let (decision, ()) = tokio::join!(approval, respond);
    assert_eq!(decision, ReviewDecision::Approved);
}

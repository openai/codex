use anyhow::Result;
use app_test_support::MockResponsesConfig;
use app_test_support::TestAppServer;
use app_test_support::create_mock_responses_server_repeating_assistant;
use codex_app_server_protocol::ApprovalsReviewer;
use codex_app_server_protocol::ApprovalsReviewer::AutoReview;
use codex_app_server_protocol::ApprovalsReviewer::User;
use codex_app_server_protocol::AskForApproval;
use codex_app_server_protocol::AskForApproval::Never;
use codex_app_server_protocol::AskForApproval::OnRequest;
use codex_app_server_protocol::JSONRPCError;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::SandboxMode;
use codex_app_server_protocol::SandboxPolicy;
use codex_app_server_protocol::ThreadForkParams as ForkParams;
use codex_app_server_protocol::ThreadForkResponse as ForkResponse;
use codex_app_server_protocol::ThreadResumeParams as ResumeParams;
use codex_app_server_protocol::ThreadResumeResponse as ResumeResponse;
use codex_app_server_protocol::ThreadSettingsUpdateParams as UpdateParams;
use codex_app_server_protocol::ThreadSettingsUpdateResponse as UpdateResponse;
use codex_app_server_protocol::ThreadSettingsUpdatedNotification as SettingsUpdated;
use codex_app_server_protocol::ThreadStartParams as StartParams;
use codex_app_server_protocol::TurnStartParams as TurnParams;
use codex_app_server_protocol::TurnStartResponse as TurnResponse;
use codex_app_server_protocol::UserInput;
use codex_features::Feature;
use pretty_assertions::assert_eq;
use std::time::Duration;
use tempfile::TempDir;
use tokio::time::timeout;

const TIMEOUT: Duration = Duration::from_secs(10);
const MODEL: &str = "protected-model";
const REQUIREMENTS: &str = "[auto_review]\nrequired_on_models = [\"protected-model\"]\n";
const UNSAFE: [(Option<AskForApproval>, Option<ApprovalsReviewer>); 2] =
    [(Some(Never), None), (None, Some(User))];

macro_rules! params {
    ($ty:ident, $($field:ident $(= $value:expr)?),* $(,)?) => {
        $ty { $($field $( : $value)?,)* ..Default::default() }
    };
}

async fn app_server(
    config: MockResponsesConfig,
    requirements: &str,
) -> Result<(TempDir, TestAppServer)> {
    let home = TempDir::new()?;
    config.write(home.path())?;
    std::fs::write(home.path().join("requirements.toml"), requirements)?;
    let server = TestAppServer::builder()
        .with_codex_home(home.path())
        .build_initialized_with_timeout(TIMEOUT)
        .await?;
    Ok((home, server))
}

async fn managed_server() -> Result<(TempDir, TestAppServer)> {
    app_server(
        MockResponsesConfig::new("http://localhost/unused").with_approval_policy("on-request"),
        REQUIREMENTS,
    )
    .await
}

async fn assert_error(server: &mut TestAppServer, request_id: i64, message: &str) -> Result<()> {
    let error: JSONRPCError = timeout(
        TIMEOUT,
        server.read_stream_until_error_message(RequestId::Integer(request_id)),
    )
    .await??;
    assert_eq!(error.error.code, -32600);
    assert!(error.error.message.contains(message));
    Ok(())
}

async fn assert_protected_update(server: &mut TestAppServer) -> Result<()> {
    let updated: SettingsUpdated =
        timeout(TIMEOUT, server.read_notification("thread/settings/updated")).await??;
    let settings = updated.thread_settings;
    assert_protected(
        &settings.model,
        settings.approval_policy,
        settings.approvals_reviewer,
    );
    Ok(())
}

fn assert_protected(model: &str, policy: AskForApproval, reviewer: ApprovalsReviewer) {
    assert_eq!((model, policy, reviewer), (MODEL, OnRequest, AutoReview));
}

#[tokio::test]
async fn thread_start_enforces_protected_model_auto_review() -> Result<()> {
    let (_home, mut server) = managed_server().await?;
    let started = server
        .start_thread(params!(StartParams, model = Some(MODEL.to_string())))
        .await?;
    assert_protected(
        &started.model,
        started.approval_policy,
        started.approvals_reviewer,
    );
    for (approval_policy, approvals_reviewer) in UNSAFE {
        let started = server
            .start_thread(params!(
                StartParams,
                model = Some(MODEL.to_string()),
                approval_policy,
                approvals_reviewer,
            ))
            .await?;
        assert_protected(
            &started.model,
            started.approval_policy,
            started.approvals_reviewer,
        );
    }
    let started = server
        .start_thread(params!(
            StartParams,
            model = Some(MODEL.to_string()),
            approval_policy = Some(Never),
            approvals_reviewer = Some(User),
            sandbox = Some(SandboxMode::DangerFullAccess),
        ))
        .await?;
    assert_protected(
        &started.model,
        started.approval_policy,
        started.approvals_reviewer,
    );
    assert!(matches!(
        started.sandbox,
        SandboxPolicy::WorkspaceWrite { .. }
    ));
    let (_home, mut disabled) = app_server(
        MockResponsesConfig::new("http://localhost/unused")
            .with_approval_policy("on-request")
            .disable_feature(Feature::GuardianApproval),
        REQUIREMENTS,
    )
    .await?;
    let id = disabled
        .send_thread_start_request_with_auto_env(params!(
            StartParams,
            model = Some(MODEL.to_string())
        ))
        .await?;
    assert_error(&mut disabled, id, "you need to use auto review").await
}

#[tokio::test]
async fn thread_and_turn_settings_enforce_protected_model_auto_review() -> Result<()> {
    let (_home, mut server) = managed_server().await?;
    let thread = server.start_thread(StartParams::default()).await?.thread;
    let id = server
        .send_thread_settings_update_request(params!(
            UpdateParams,
            thread_id = thread.id.clone(),
            model = Some(MODEL.to_string())
        ))
        .await?;
    let _: UpdateResponse = timeout(TIMEOUT, server.read_response(id)).await??;
    assert_protected_update(&mut server).await?;
    for (approval_policy, approvals_reviewer) in UNSAFE {
        let id = server
            .send_thread_settings_update_request(params!(
                UpdateParams,
                thread_id = thread.id.clone(),
                approval_policy,
                approvals_reviewer,
            ))
            .await?;
        assert_error(&mut server, id, "you need to use auto review").await?;
    }
    let id = server
        .send_thread_settings_update_request(params!(
            UpdateParams,
            thread_id = thread.id.clone(),
            sandbox_policy = Some(SandboxPolicy::DangerFullAccess),
        ))
        .await?;
    assert_error(&mut server, id, "you need to use auto review").await?;
    let id = server
        .send_turn_start_request(params!(
            TurnParams,
            thread_id = thread.id,
            approvals_reviewer = Some(User)
        ))
        .await?;
    assert_error(&mut server, id, "you need to use auto review").await?;

    let turn_thread = server.start_thread(StartParams::default()).await?.thread;
    let id = server
        .send_turn_start_request(params!(
            TurnParams,
            thread_id = turn_thread.id.clone(),
            model = Some(MODEL.to_string()),
            approvals_reviewer = Some(User)
        ))
        .await?;
    assert_error(&mut server, id, "you need to use auto review").await?;
    let id = server
        .send_turn_start_request(params!(
            TurnParams,
            thread_id = turn_thread.id,
            model = Some(MODEL.to_string())
        ))
        .await?;
    let _: TurnResponse = timeout(TIMEOUT, server.read_response(id)).await??;
    assert_protected_update(&mut server).await
}

#[tokio::test]
async fn thread_resume_and_fork_upgrade_legacy_protected_model_settings() -> Result<()> {
    let responses = create_mock_responses_server_repeating_assistant("Done").await;
    let (home, mut legacy) = app_server(
        MockResponsesConfig::new(&responses.uri()).with_model(MODEL),
        "",
    )
    .await?;
    let started = legacy.start_thread(StartParams::default()).await?;
    assert_eq!(
        (started.approval_policy, started.approvals_reviewer),
        (Never, User),
    );
    let thread_id = started.thread.id;
    legacy
        .start_turn_and_wait_for_completion(params!(
            TurnParams,
            thread_id = thread_id.clone(),
            input = vec![UserInput::Text {
                text: "Save legacy settings".to_string(),
                text_elements: Vec::new()
            }],
        ))
        .await?;
    drop(legacy);
    MockResponsesConfig::new(&responses.uri())
        .with_model("ordinary-model")
        .with_approval_policy("on-request")
        .write(home.path())?;
    std::fs::write(home.path().join("requirements.toml"), REQUIREMENTS)?;
    let mut server = TestAppServer::builder()
        .with_codex_home(home.path())
        .build_initialized_with_timeout(TIMEOUT)
        .await?;

    let id = server
        .send_thread_fork_request(params!(
            ForkParams,
            thread_id = thread_id.clone(),
            model = Some(MODEL.to_string()),
            approval_policy = Some(Never),
            approvals_reviewer = Some(User),
        ))
        .await?;
    let fork: ForkResponse = timeout(TIMEOUT, server.read_response(id)).await??;
    assert_protected(&fork.model, fork.approval_policy, fork.approvals_reviewer);
    let id = server
        .send_thread_fork_request(params!(
            ForkParams,
            thread_id = thread_id.clone(),
            model = Some(MODEL.to_string())
        ))
        .await?;
    let fork: ForkResponse = timeout(TIMEOUT, server.read_response(id)).await??;
    assert_protected(&fork.model, fork.approval_policy, fork.approvals_reviewer);
    let id = server
        .send_thread_resume_request(params!(
            ResumeParams,
            thread_id = thread_id.clone(),
            approval_policy = Some(Never),
            approvals_reviewer = Some(User),
        ))
        .await?;
    let resumed: ResumeResponse = timeout(TIMEOUT, server.read_response(id)).await??;
    assert_protected(
        &resumed.model,
        resumed.approval_policy,
        resumed.approvals_reviewer,
    );
    let id = server
        .send_thread_resume_request(params!(ResumeParams, thread_id))
        .await?;
    let resumed: ResumeResponse = timeout(TIMEOUT, server.read_response(id)).await??;
    assert_protected(
        &resumed.model,
        resumed.approval_policy,
        resumed.approvals_reviewer,
    );
    Ok(())
}

#[tokio::test]
async fn thread_settings_update_enforces_global_reviewer_requirements() -> Result<()> {
    let (_home, mut server) = app_server(
        MockResponsesConfig::new("http://localhost/unused").with_approval_policy("on-request"),
        "allowed_approvals_reviewers = [\"auto_review\"]\n",
    )
    .await?;
    let thread = server.start_thread(StartParams::default()).await?;
    assert_eq!(thread.approvals_reviewer, AutoReview);
    let id = server
        .send_thread_settings_update_request(params!(
            UpdateParams,
            thread_id = thread.thread.id,
            approvals_reviewer = Some(User)
        ))
        .await?;
    assert_error(&mut server, id, "approvals_reviewer").await
}

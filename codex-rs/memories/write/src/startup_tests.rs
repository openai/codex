use crate::extensions::seed_extension_instructions;
use crate::memory_root;
use crate::phase2;
use crate::runtime::MemoryStartupContext;
use crate::start_memories_startup_task;
use codex_config::types::MemoriesConfig;
use codex_features::Feature;
use codex_git_utils::diff_since_latest_init;
use codex_git_utils::reset_git_repository;
use codex_login::AuthManager;
use codex_login::CodexAuth;
use codex_model_provider::ModelProvider;
use codex_model_provider::ProviderAccountResult;
use codex_model_provider::SharedModelProvider;
use codex_model_provider::create_model_provider;
use codex_model_provider_info::ModelProviderInfo;
use codex_protocol::ThreadId;
use codex_protocol::config_types::ServiceTier;
use codex_protocol::openai_models::ModelsResponse;
use codex_protocol::openai_models::ReasoningEffort;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::SessionSource;
use core_test_support::responses::ResponseMock;
use core_test_support::responses::ResponsesRequest;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_sse_once;
use core_test_support::responses::mount_sse_sequence;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::test_codex::TestCodex;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use pretty_assertions::assert_eq;
use std::future::Future;
use std::path::Path;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;
use tempfile::TempDir;
use tokio::time::Duration;
use tokio::time::Instant;

#[tokio::test]
async fn memories_startup_creates_memory_root() -> anyhow::Result<()> {
    let server = start_mock_server().await;
    let home = Arc::new(TempDir::new()?);
    let memory_root = home.path().join("memories");
    let test = build_test_codex(&server, home).await?;

    assert!(!memory_root.exists());
    trigger_memories_startup(&test).await;
    wait_for_dir(&memory_root).await?;

    shutdown_test_codex(&test).await?;
    Ok(())
}

#[tokio::test]
async fn memories_startup_phase2_tracks_workspace_diff_across_runs() -> anyhow::Result<()> {
    let server = start_mock_server().await;
    let home = Arc::new(TempDir::new()?);
    let db = init_state_db(&home).await?;
    let memory_root = home.path().join("memories");

    let now = chrono::Utc::now();
    let _thread_a = seed_stage1_output(
        db.as_ref(),
        home.path(),
        now - chrono::Duration::hours(2),
        "raw memory A",
        "rollout summary A",
        "rollout-a",
    )
    .await?;

    let rollout_summaries_root = memory_root.join("rollout_summaries");
    tokio::fs::create_dir_all(&rollout_summaries_root).await?;
    tokio::fs::write(
        memory_root.join("raw_memories.md"),
        "# Raw Memories\n\nraw memory A\n",
    )
    .await?;
    tokio::fs::write(
        rollout_summaries_root.join("rollout-a.md"),
        "git_branch: branch-rollout-a\n\nrollout summary A\n",
    )
    .await?;
    reset_git_repository(&memory_root).await?;

    let _thread_b = seed_stage1_output(
        db.as_ref(),
        home.path(),
        now - chrono::Duration::hours(1),
        "raw memory B",
        "rollout summary B",
        "rollout-b",
    )
    .await?;

    let phase2 = mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-phase2"),
            ev_assistant_message("msg-phase2", "phase2 complete"),
            ev_completed("resp-phase2"),
        ]),
    )
    .await;

    let test = build_test_codex(&server, home.clone()).await?;
    trigger_memories_startup(&test).await;

    let request = wait_for_single_request(&phase2).await;
    let prompt = phase2_prompt_text(&request);
    assert!(
        prompt.contains("phase2_workspace_diff.md"),
        "expected workspace diff file in prompt: {prompt}"
    );

    wait_for_phase2_workspace_reset(&memory_root).await?;
    let raw_memories = tokio::fs::read_to_string(memory_root.join("raw_memories.md")).await?;
    assert!(raw_memories.contains("raw memory B"));
    assert!(!raw_memories.contains("raw memory A"));
    let rollout_summaries = read_rollout_summary_bodies(&memory_root).await?;
    assert_eq!(rollout_summaries.len(), 1);
    assert!(
        rollout_summaries
            .iter()
            .any(|summary| summary.contains("rollout summary B"))
    );
    assert!(
        rollout_summaries
            .iter()
            .any(|summary| summary.contains("git_branch: branch-rollout-b"))
    );
    assert!(
        rollout_summaries
            .iter()
            .all(|summary| !summary.contains("rollout summary A"))
    );

    shutdown_test_codex(&test).await?;
    Ok(())
}

#[tokio::test]
async fn memories_startup_phase2_prunes_old_extension_resources() -> anyhow::Result<()> {
    let server = start_mock_server().await;
    let home = Arc::new(TempDir::new()?);
    let db = init_state_db(&home).await?;
    let now = chrono::Utc::now();
    let _thread_id = seed_stage1_output(
        db.as_ref(),
        home.path(),
        now - chrono::Duration::hours(1),
        "raw memory",
        "rollout summary",
        "rollout",
    )
    .await?;

    let chronicle_resources = home.path().join("memories/extensions/chronicle/resources");
    tokio::fs::create_dir_all(&chronicle_resources).await?;
    tokio::fs::write(
        home.path()
            .join("memories/extensions/chronicle/instructions.md"),
        "instructions",
    )
    .await?;
    let old_file = chronicle_resources.join(format!(
        "{}-abcd-10min-old.md",
        (now - chrono::Duration::days(8)).format("%Y-%m-%dT%H-%M-%S")
    ));
    tokio::fs::write(&old_file, "old resource").await?;
    let recent_file = chronicle_resources.join(format!(
        "{}-abcd-10min-recent.md",
        (now - chrono::Duration::days(6)).format("%Y-%m-%dT%H-%M-%S")
    ));
    tokio::fs::write(&recent_file, "recent resource").await?;

    let phase2 = mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-phase2"),
            ev_assistant_message("msg-phase2", "phase2 complete"),
            ev_completed("resp-phase2"),
        ]),
    )
    .await;

    let test = build_test_codex(&server, home.clone()).await?;
    trigger_memories_startup(&test).await;

    let request = wait_for_single_request(&phase2).await;
    let prompt = phase2_prompt_text(&request);
    assert!(
        prompt.contains("phase2_workspace_diff.md"),
        "expected workspace diff file in prompt: {prompt}"
    );

    wait_for_phase2_workspace_reset(&home.path().join("memories")).await?;
    wait_for_file_removed(&old_file).await?;
    assert!(
        !tokio::fs::try_exists(&old_file).await?,
        "old extension resource should be pruned"
    );
    assert!(
        tokio::fs::try_exists(&recent_file).await?,
        "recent extension resource should be retained"
    );

    shutdown_test_codex(&test).await?;
    Ok(())
}

#[tokio::test]
async fn memories_startup_phase2_prunes_old_extension_resources_without_stage1_input()
-> anyhow::Result<()> {
    let server = start_mock_server().await;
    let home = Arc::new(TempDir::new()?);
    let db = init_state_db(&home).await?;
    db.memories()
        .enqueue_global_consolidation(/*input_watermark*/ 1)
        .await?;

    let now = chrono::Utc::now();
    let chronicle_resources = home.path().join("memories/extensions/chronicle/resources");
    tokio::fs::create_dir_all(&chronicle_resources).await?;
    tokio::fs::write(
        home.path()
            .join("memories/extensions/chronicle/instructions.md"),
        "instructions",
    )
    .await?;
    let old_file = chronicle_resources.join(format!(
        "{}-abcd-10min-old.md",
        (now - chrono::Duration::days(8)).format("%Y-%m-%dT%H-%M-%S")
    ));
    tokio::fs::write(&old_file, "old resource").await?;

    let phase2 = mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-phase2-empty"),
            ev_assistant_message("msg-phase2-empty", "phase2 complete"),
            ev_completed("resp-phase2-empty"),
        ]),
    )
    .await;

    let test = build_test_codex(&server, home.clone()).await?;
    trigger_memories_startup(&test).await;

    let request = wait_for_single_request(&phase2).await;
    let prompt = phase2_prompt_text(&request);
    assert!(
        prompt.contains("phase2_workspace_diff.md"),
        "expected workspace diff file in prompt: {prompt}"
    );

    wait_for_file_removed(&old_file).await?;
    wait_for_phase2_workspace_reset(&home.path().join("memories")).await?;

    shutdown_test_codex(&test).await?;
    Ok(())
}

#[tokio::test]
async fn memories_startup_phase1_uses_live_thread_service_tier_and_detached_metadata()
-> anyhow::Result<()> {
    let server = start_mock_server().await;
    let home = Arc::new(TempDir::new()?);
    let test = build_test_codex(&server, home).await?;
    assert_eq!(test.config.service_tier, None);
    reset_git_repository(&test.config.cwd).await?;

    core_test_support::submit_thread_settings(
        &test.codex,
        codex_protocol::protocol::ThreadSettingsOverrides {
            service_tier: Some(Some(ServiceTier::Fast.request_value().to_string())),
            ..Default::default()
        },
    )
    .await?;

    let config_snapshot =
        wait_for_service_tier(&test, Some(ServiceTier::Fast.request_value().to_string())).await?;
    assert_eq!(
        config_snapshot.service_tier,
        Some(ServiceTier::Fast.request_value().to_string())
    );

    let context = crate::runtime::MemoryStartupContext::new(
        Arc::clone(&test.thread_manager),
        test.thread_manager.auth_manager(),
        test.session_configured.thread_id,
        Arc::clone(&test.codex),
        &test.config,
        config_snapshot.session_source.clone(),
    );
    let request_context = context
        .stage_one_request_context(
            &test.config,
            test.config.model.as_deref().unwrap_or("gpt-5.4-mini"),
            ReasoningEffort::Low,
        )
        .await;
    assert_eq!(
        request_context.service_tier,
        Some(ServiceTier::Fast.request_value().to_string())
    );

    let stage_one = mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-phase1"),
            ev_assistant_message("msg-phase1", "phase1 complete"),
            ev_completed("resp-phase1"),
        ]),
    )
    .await;
    context
        .stream_stage_one_prompt(
            &test.config,
            &codex_core::Prompt::default(),
            &request_context,
        )
        .await?;
    let request = wait_for_single_request(&stage_one).await;
    let metadata_header = request
        .header("x-codex-turn-metadata")
        .expect("detached memory request should include workspace metadata");
    let metadata: serde_json::Value =
        serde_json::from_str(&metadata_header).expect("turn metadata json");
    assert_eq!(metadata["request_kind"].as_str(), Some("memory"));
    assert!(metadata.get("session_id").is_none());
    assert!(metadata.get("thread_id").is_none());
    assert!(metadata.get("turn_id").is_none());
    assert!(metadata.get("window_id").is_none());
    assert!(metadata.get("workspaces").is_some());

    shutdown_test_codex(&test).await?;
    Ok(())
}

#[tokio::test]
async fn memories_startup_provider_defaults_drive_phase_requests() -> anyhow::Result<()> {
    let server = start_mock_server().await;
    let home = Arc::new(TempDir::new()?);

    let requests =
        run_memories_startup_model_request_test(&server, home, startup_test_memories_config())
            .await?;

    assert_eq!(
        requests[0].body_json()["model"].as_str(),
        Some(MOCK_PROVIDER_PHASE_ONE_MODEL)
    );
    assert_eq!(
        requests[1].body_json()["model"].as_str(),
        Some(MOCK_PROVIDER_PHASE_TWO_MODEL)
    );

    Ok(())
}

#[tokio::test]
async fn memories_startup_explicit_model_overrides_drive_phase_requests() -> anyhow::Result<()> {
    let server = start_mock_server().await;
    let home = Arc::new(TempDir::new()?);

    let mut memories = startup_test_memories_config();
    memories.extract_model = Some("override.phase-one".to_string());
    memories.consolidation_model = Some("override.phase-two".to_string());
    let requests = run_memories_startup_model_request_test(&server, home, memories).await?;

    assert_eq!(
        requests[0].body_json()["model"].as_str(),
        Some("override.phase-one")
    );
    assert_eq!(
        requests[1].body_json()["model"].as_str(),
        Some("override.phase-two")
    );

    Ok(())
}

async fn run_memories_startup_model_request_test(
    server: &wiremock::MockServer,
    home: Arc<TempDir>,
    memories: MemoriesConfig,
) -> anyhow::Result<Vec<ResponsesRequest>> {
    let phase1_memories = memories.clone();
    let phase1_test =
        build_test_codex_with_memories_config(server, home.clone(), phase1_memories).await?;
    let phase1_provider = Arc::new(MockMemoryModelProvider::new(
        phase1_test.config.model_provider.clone(),
        Some(phase1_test.thread_manager.auth_manager()),
    ));
    let responses = mount_sse_sequence(
        server,
        vec![
            sse(vec![
                ev_response_created("resp-phase1"),
                ev_assistant_message(
                    "msg-phase1",
                    r#"{"raw_memory":"raw memory","rollout_summary":"rollout summary","rollout_slug":"startup-models"}"#,
                ),
                ev_completed("resp-phase1"),
            ]),
            sse(vec![
                ev_response_created("resp-phase2"),
                ev_assistant_message("msg-phase2", "phase2 complete"),
                ev_completed("resp-phase2"),
            ]),
        ],
    )
    .await;

    run_memory_phase_one_request_with_provider(&phase1_test, phase1_provider).await?;
    let _ = wait_for_request(&responses, /*expected_count*/ 1).await;
    shutdown_test_codex(&phase1_test).await?;

    let phase2_home = Arc::new(TempDir::new()?);
    let phase2_test =
        build_test_codex_with_memories_config(server, phase2_home.clone(), memories).await?;
    let phase2_provider = Arc::new(MockMemoryModelProvider::new(
        phase2_test.config.model_provider.clone(),
        Some(phase2_test.thread_manager.auth_manager()),
    ));
    let db = phase2_test
        .codex
        .state_db()
        .ok_or_else(|| anyhow::anyhow!("state db should be enabled for memory startup test"))?;
    seed_stage1_output(
        db.as_ref(),
        phase2_home.path(),
        chrono::Utc::now(),
        "raw memory for phase two",
        "rollout summary for phase two",
        "startup-models-phase-two",
    )
    .await?;

    run_memory_phase_two_with_provider(&phase2_test, phase2_provider).await?;

    let requests = wait_for_request(&responses, /*expected_count*/ 2).await;
    wait_for_phase2_workspace_reset(&phase2_home.path().join("memories")).await?;
    shutdown_test_codex(&phase2_test).await?;
    Ok(requests)
}

fn startup_test_memories_config() -> MemoriesConfig {
    MemoriesConfig {
        max_raw_memories_for_consolidation: 1,
        min_rollout_idle_hours: 0,
        ..MemoriesConfig::default()
    }
}

async fn build_test_codex(
    server: &wiremock::MockServer,
    home: Arc<TempDir>,
) -> anyhow::Result<TestCodex> {
    build_test_codex_with_memories_config(server, home, startup_test_memories_config()).await
}

async fn build_test_codex_with_memories_config(
    server: &wiremock::MockServer,
    home: Arc<TempDir>,
    memories: MemoriesConfig,
) -> anyhow::Result<TestCodex> {
    test_codex()
        .with_home(home)
        .with_config(move |config| {
            config
                .features
                .enable(Feature::Sqlite)
                .expect("test config should allow feature update");
            config.memories = memories;
        })
        .build(server)
        .await
}

async fn init_state_db(home: &Arc<TempDir>) -> anyhow::Result<Arc<codex_state::StateRuntime>> {
    let db =
        codex_state::StateRuntime::init(home.path().to_path_buf(), "test-provider".into()).await?;
    db.mark_backfill_complete(/*last_watermark*/ None).await?;
    Ok(db)
}

async fn trigger_memories_startup(test: &TestCodex) {
    let config_snapshot = test.codex.config_snapshot().await;
    let mut config = test.config.clone();
    config
        .features
        .enable(Feature::MemoryTool)
        .expect("test config should allow feature update");
    start_memories_startup_task(
        Arc::clone(&test.thread_manager),
        test.thread_manager.auth_manager(),
        test.session_configured.thread_id,
        Arc::clone(&test.codex),
        Arc::new(config),
        &config_snapshot.session_source,
    );
}

async fn memory_startup_context_with_provider(
    test: &TestCodex,
    provider: SharedModelProvider,
) -> (Arc<MemoryStartupContext>, Arc<codex_core::config::Config>) {
    let config_snapshot = test.codex.config_snapshot().await;
    let mut config = test.config.clone();
    config
        .features
        .enable(Feature::MemoryTool)
        .expect("test config should allow feature update");
    let config = Arc::new(config);
    let context = Arc::new(MemoryStartupContext::new_for_testing(
        Arc::clone(&test.thread_manager),
        test.thread_manager.auth_manager(),
        test.session_configured.thread_id,
        Arc::clone(&test.codex),
        config.as_ref(),
        config_snapshot.session_source,
        provider,
    ));

    (context, config)
}

async fn run_memory_phase_one_request_with_provider(
    test: &TestCodex,
    provider: SharedModelProvider,
) -> anyhow::Result<()> {
    let (context, config) = memory_startup_context_with_provider(test, provider).await;
    let model = config.memories.extract_model.clone().unwrap_or_else(|| {
        context
            .provider()
            .memory_extraction_preferred_model()
            .to_string()
    });
    let request_context = context
        .stage_one_request_context(&config, &model, crate::stage_one::REASONING_EFFORT)
        .await;
    context
        .stream_stage_one_prompt(&config, &codex_core::Prompt::default(), &request_context)
        .await?;
    Ok(())
}

async fn run_memory_phase_two_with_provider(
    test: &TestCodex,
    provider: SharedModelProvider,
) -> anyhow::Result<()> {
    let (context, config) = memory_startup_context_with_provider(test, provider).await;
    let root = memory_root(&config.codex_home);
    tokio::fs::create_dir_all(&root).await?;
    seed_extension_instructions(&root).await?;
    phase2::run(context, config).await;
    Ok(())
}

const MOCK_PROVIDER_PHASE_ONE_MODEL: &str = "mock.phase-one";
const MOCK_PROVIDER_PHASE_TWO_MODEL: &str = "mock.phase-two";

#[derive(Debug)]
struct MockMemoryModelProvider {
    delegate: SharedModelProvider,
}

impl MockMemoryModelProvider {
    fn new(info: ModelProviderInfo, auth_manager: Option<Arc<AuthManager>>) -> Self {
        Self {
            delegate: create_model_provider(info, auth_manager),
        }
    }
}

impl ModelProvider for MockMemoryModelProvider {
    fn info(&self) -> &ModelProviderInfo {
        self.delegate.info()
    }

    fn memory_extraction_preferred_model(&self) -> &'static str {
        MOCK_PROVIDER_PHASE_ONE_MODEL
    }

    fn memory_consolidation_preferred_model(&self) -> &'static str {
        MOCK_PROVIDER_PHASE_TWO_MODEL
    }

    fn auth_manager(&self) -> Option<Arc<AuthManager>> {
        self.delegate.auth_manager()
    }

    fn auth<'life0, 'async_trait>(
        &'life0 self,
    ) -> Pin<Box<dyn Future<Output = Option<CodexAuth>> + Send + 'async_trait>>
    where
        'life0: 'async_trait,
        Self: 'async_trait,
    {
        let delegate = Arc::clone(&self.delegate);
        Box::pin(async move { delegate.auth().await })
    }

    fn account_state(&self) -> ProviderAccountResult {
        self.delegate.account_state()
    }

    fn models_manager(
        &self,
        codex_home: PathBuf,
        config_model_catalog: Option<ModelsResponse>,
    ) -> codex_models_manager::manager::SharedModelsManager {
        self.delegate
            .models_manager(codex_home, config_model_catalog)
    }
}

async fn seed_stage1_output(
    db: &codex_state::StateRuntime,
    codex_home: &Path,
    updated_at: chrono::DateTime<chrono::Utc>,
    raw_memory: &str,
    rollout_summary: &str,
    rollout_slug: &str,
) -> anyhow::Result<ThreadId> {
    let thread_id = ThreadId::new();
    let mut metadata_builder = codex_state::ThreadMetadataBuilder::new(
        thread_id,
        codex_home.join(format!("rollout-{thread_id}.jsonl")),
        updated_at,
        SessionSource::Cli,
    );
    metadata_builder.cwd = codex_home.join(format!("workspace-{rollout_slug}"));
    metadata_builder.model_provider = Some("test-provider".to_string());
    metadata_builder.git_branch = Some(format!("branch-{rollout_slug}"));
    let metadata = metadata_builder.build("test-provider");
    db.upsert_thread(&metadata).await?;

    seed_stage1_output_for_existing_thread(
        db,
        thread_id,
        updated_at.timestamp(),
        raw_memory,
        rollout_summary,
        Some(rollout_slug),
    )
    .await?;

    Ok(thread_id)
}

async fn wait_for_single_request(mock: &ResponseMock) -> ResponsesRequest {
    wait_for_request(mock, /*expected_count*/ 1).await.remove(0)
}

async fn wait_for_file_removed(path: &Path) -> anyhow::Result<()> {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if !tokio::fs::try_exists(path).await? {
            return Ok(());
        }
        assert!(
            Instant::now() < deadline,
            "timed out waiting for {} to be removed",
            path.display()
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

async fn wait_for_dir(path: &Path) -> anyhow::Result<()> {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if tokio::fs::try_exists(path).await? && path.is_dir() {
            return Ok(());
        }
        assert!(
            Instant::now() < deadline,
            "timed out waiting for {} to be created",
            path.display()
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

async fn wait_for_request(mock: &ResponseMock, expected_count: usize) -> Vec<ResponsesRequest> {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let requests = mock.requests();
        if requests.len() >= expected_count {
            return requests;
        }
        assert!(
            Instant::now() < deadline,
            "timed out waiting for {expected_count} responses requests, got {}",
            requests.len()
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

async fn wait_for_service_tier(
    test: &TestCodex,
    expected_service_tier: Option<String>,
) -> anyhow::Result<codex_core::ThreadConfigSnapshot> {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let config_snapshot = test.codex.config_snapshot().await;
        if config_snapshot.service_tier == expected_service_tier {
            return Ok(config_snapshot);
        }
        anyhow::ensure!(
            Instant::now() < deadline,
            "timed out waiting for service_tier to become {expected_service_tier:?}, current={:?}",
            config_snapshot.service_tier
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

fn phase2_prompt_text(request: &ResponsesRequest) -> String {
    request
        .message_input_texts("user")
        .into_iter()
        .find(|text| text.contains("Memory workspace diff:"))
        .expect("phase2 prompt text")
}

async fn wait_for_phase2_workspace_reset(memory_root: &Path) -> anyhow::Result<()> {
    wait_for_file_removed(&memory_root.join("phase2_workspace_diff.md")).await?;
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if let Ok(diff) = diff_since_latest_init(memory_root).await
            && !diff.has_changes()
        {
            return Ok(());
        }
        assert!(
            Instant::now() < deadline,
            "timed out waiting for clean memory workspace baseline"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

async fn seed_stage1_output_for_existing_thread(
    db: &codex_state::StateRuntime,
    thread_id: ThreadId,
    updated_at: i64,
    raw_memory: &str,
    rollout_summary: &str,
    rollout_slug: Option<&str>,
) -> anyhow::Result<()> {
    let owner = ThreadId::new();
    let claim = db
        .memories()
        .try_claim_stage1_job(
            thread_id, owner, updated_at, /*lease_seconds*/ 3_600,
            /*max_running_jobs*/ 64,
        )
        .await?;
    let ownership_token = match claim {
        codex_state::Stage1JobClaimOutcome::Claimed { ownership_token } => ownership_token,
        other => panic!("unexpected stage-1 claim outcome: {other:?}"),
    };

    assert!(
        db.memories()
            .mark_stage1_job_succeeded(
                thread_id,
                &ownership_token,
                updated_at,
                raw_memory,
                rollout_summary,
                rollout_slug,
            )
            .await?,
        "stage-1 success should enqueue global consolidation"
    );

    Ok(())
}

async fn read_rollout_summary_bodies(memory_root: &Path) -> anyhow::Result<Vec<String>> {
    let mut dir = tokio::fs::read_dir(memory_root.join("rollout_summaries")).await?;
    let mut summaries = Vec::new();
    while let Some(entry) = dir.next_entry().await? {
        summaries.push(tokio::fs::read_to_string(entry.path()).await?);
    }
    summaries.sort();
    Ok(summaries)
}

async fn shutdown_test_codex(test: &TestCodex) -> anyhow::Result<()> {
    test.codex.submit(Op::Shutdown {}).await?;
    wait_for_event(&test.codex, |ev| matches!(ev, EventMsg::ShutdownComplete)).await;
    Ok(())
}

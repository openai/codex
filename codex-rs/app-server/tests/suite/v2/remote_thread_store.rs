//! Regression coverage for app-server thread operations backed by a non-local
//! `ThreadStore`.
//!
//! The app-server startup path should honor `experimental_thread_store_endpoint`
//! by routing all thread persistence through the configured store. This suite
//! registers an in-memory store for a synthetic endpoint, which exercises the
//! same config-driven selection path as a remote store without requiring the
//! real gRPC service.
//!
//! The important failure mode is accidentally materializing local persistence
//! while a non-local store is configured. After `thread/start` and a simple turn,
//! the temporary `codex_home` must not contain rollout session files or sqlite
//! state files.

use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use app_test_support::create_mock_responses_server_repeating_assistant;
use async_trait::async_trait;
use chrono::Utc;
use codex_app_server::in_process;
use codex_app_server::in_process::InProcessServerEvent;
use codex_app_server::in_process::InProcessStartArgs;
use codex_app_server_protocol::ClientInfo;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::InitializeParams;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ServerNotification;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::UserInput as V2UserInput;
use codex_arg0::Arg0DispatchPaths;
use codex_config::NoopThreadConfigLoader;
use codex_core::config::ConfigBuilder;
use codex_core::config_loader::CloudRequirementsLoader;
use codex_core::config_loader::LoaderOverrides;
use codex_exec_server::EnvironmentManager;
use codex_feedback::CodexFeedback;
use codex_protocol::ThreadId;
use codex_protocol::protocol::AskForApproval as CoreAskForApproval;
use codex_protocol::protocol::RolloutItem;
use codex_protocol::protocol::SandboxPolicy;
use codex_protocol::protocol::SessionSource;
use codex_thread_store::AppendThreadItemsParams;
use codex_thread_store::ArchiveThreadParams;
use codex_thread_store::CreateThreadParams;
use codex_thread_store::ListThreadsParams;
use codex_thread_store::LoadThreadHistoryParams;
use codex_thread_store::ReadThreadParams;
use codex_thread_store::ResumeThreadParams;
use codex_thread_store::StoredThread;
use codex_thread_store::StoredThreadHistory;
use codex_thread_store::ThreadPage;
use codex_thread_store::ThreadStore;
use codex_thread_store::ThreadStoreError;
use codex_thread_store::ThreadStoreResult;
use codex_thread_store::UpdateThreadMetadataParams;
use pretty_assertions::assert_eq;
use tempfile::TempDir;
use tokio::sync::Mutex;
use tokio::time::timeout;
use uuid::Uuid;

const DEFAULT_READ_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

#[tokio::test]
async fn thread_start_with_non_local_thread_store_does_not_create_local_persistence() -> Result<()>
{
    let server = create_mock_responses_server_repeating_assistant("Done").await;
    let codex_home = TempDir::new()?;
    let endpoint = format!("test://thread-store/{}", Uuid::new_v4());
    create_config_toml_with_thread_store_endpoint(codex_home.path(), &server.uri(), &endpoint)?;

    let loader_overrides = LoaderOverrides::without_managed_config_for_tests();
    let config = ConfigBuilder::default()
        .codex_home(codex_home.path().to_path_buf())
        .fallback_cwd(Some(codex_home.path().to_path_buf()))
        .loader_overrides(loader_overrides.clone())
        .build()
        .await?;

    let thread_store = Arc::new(InMemoryThreadStore::default());
    let registered_store: Arc<dyn ThreadStore> = thread_store.clone();
    codex_thread_store::register_test_thread_store(endpoint.clone(), registered_store);
    let _registered_store = RegisteredTestThreadStore { endpoint };

    let mut client = in_process::start(InProcessStartArgs {
        arg0_paths: Arg0DispatchPaths::default(),
        config: Arc::new(config),
        cli_overrides: Vec::new(),
        loader_overrides,
        cloud_requirements: CloudRequirementsLoader::default(),
        thread_config_loader: Arc::new(NoopThreadConfigLoader),
        feedback: CodexFeedback::new(),
        log_db: None,
        environment_manager: Arc::new(EnvironmentManager::default_for_tests()),
        config_warnings: Vec::new(),
        session_source: SessionSource::Cli,
        enable_codex_api_key_env: false,
        initialize: InitializeParams {
            client_info: ClientInfo {
                name: "codex-app-server-tests".to_string(),
                title: None,
                version: "0.1.0".to_string(),
            },
            capabilities: None,
        },
        channel_capacity: in_process::DEFAULT_IN_PROCESS_CHANNEL_CAPACITY,
    })
    .await?;

    let response = client
        .request(ClientRequest::ThreadStart {
            request_id: RequestId::Integer(1),
            params: ThreadStartParams::default(),
        })
        .await?
        .expect("thread/start should succeed");
    let ThreadStartResponse { thread, .. } =
        serde_json::from_value(response).expect("thread/start response should parse");
    assert_eq!(thread.path, None);

    client
        .request(ClientRequest::TurnStart {
            request_id: RequestId::Integer(2),
            params: TurnStartParams {
                thread_id: thread.id.clone(),
                input: vec![V2UserInput::Text {
                    text: "Hello".to_string(),
                    text_elements: Vec::new(),
                }],
                ..Default::default()
            },
        })
        .await?
        .expect("turn/start should succeed");

    timeout(DEFAULT_READ_TIMEOUT, async {
        loop {
            let Some(event) = client.next_event().await else {
                anyhow::bail!("in-process app-server stopped before turn/completed");
            };
            if let InProcessServerEvent::ServerNotification(ServerNotification::TurnCompleted(
                completed,
            )) = event
                && completed.thread_id == thread.id
            {
                return Ok::<(), anyhow::Error>(());
            }
        }
    })
    .await??;

    client.shutdown().await?;

    let calls = thread_store.calls().await;
    assert_eq!(calls.create_thread, 1);
    assert!(
        calls.append_items > 0,
        "turn/start should append rollout items through the injected store"
    );
    assert!(
        calls.flush_thread > 0,
        "turn completion should flush through the injected store"
    );

    assert_no_local_persistence_artifacts(codex_home.path())?;

    Ok(())
}

fn assert_no_local_persistence_artifacts(codex_home: &Path) -> Result<()> {
    // These are the observable tripwires for accidental local persistence. If a
    // future code path constructs a local rollout/session store or opens the
    // local thread sqlite database, it should leave one of these artifacts in
    // the isolated test codex_home.
    assert!(
        !codex_home.join("sessions").exists(),
        "non-local thread persistence should not create local rollout sessions"
    );
    assert!(
        !codex_home.join("archived_sessions").exists(),
        "non-local thread persistence should not create archived rollout sessions"
    );
    assert!(
        !codex_state::state_db_path(codex_home).exists(),
        "non-local thread persistence should not create local thread sqlite"
    );

    let sqlite_artifacts = std::fs::read_dir(codex_home)?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| {
                    name.ends_with(".sqlite")
                        || name.ends_with(".sqlite-shm")
                        || name.ends_with(".sqlite-wal")
                })
        })
        .collect::<Vec<_>>();

    assert!(
        sqlite_artifacts.is_empty(),
        "non-local thread persistence should not create sqlite artifacts: {sqlite_artifacts:?}"
    );

    Ok(())
}

struct RegisteredTestThreadStore {
    endpoint: String,
}

impl Drop for RegisteredTestThreadStore {
    fn drop(&mut self) {
        codex_thread_store::remove_test_thread_store(&self.endpoint);
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct InMemoryThreadStoreCalls {
    create_thread: usize,
    resume_thread: usize,
    append_items: usize,
    persist_thread: usize,
    flush_thread: usize,
    shutdown_thread: usize,
    discard_thread: usize,
    load_history: usize,
    read_thread: usize,
    list_threads: usize,
    update_thread_metadata: usize,
    archive_thread: usize,
    unarchive_thread: usize,
}

#[derive(Default)]
struct InMemoryThreadStore {
    state: Mutex<InMemoryThreadStoreState>,
}

#[derive(Default)]
struct InMemoryThreadStoreState {
    calls: InMemoryThreadStoreCalls,
    created_threads: HashMap<ThreadId, CreateThreadParams>,
    histories: HashMap<ThreadId, Vec<RolloutItem>>,
    names: HashMap<ThreadId, Option<String>>,
}

impl InMemoryThreadStore {
    async fn calls(&self) -> InMemoryThreadStoreCalls {
        self.state.lock().await.calls.clone()
    }
}

#[async_trait]
impl ThreadStore for InMemoryThreadStore {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    async fn create_thread(&self, params: CreateThreadParams) -> ThreadStoreResult<()> {
        let mut state = self.state.lock().await;
        state.calls.create_thread += 1;
        state.histories.entry(params.thread_id).or_default();
        state.created_threads.insert(params.thread_id, params);
        Ok(())
    }

    async fn resume_thread(&self, params: ResumeThreadParams) -> ThreadStoreResult<()> {
        let mut state = self.state.lock().await;
        state.calls.resume_thread += 1;
        state.histories.entry(params.thread_id).or_default();
        Ok(())
    }

    async fn append_items(&self, params: AppendThreadItemsParams) -> ThreadStoreResult<()> {
        let mut state = self.state.lock().await;
        state.calls.append_items += 1;
        state
            .histories
            .entry(params.thread_id)
            .or_default()
            .extend(params.items);
        Ok(())
    }

    async fn persist_thread(&self, _thread_id: ThreadId) -> ThreadStoreResult<()> {
        self.state.lock().await.calls.persist_thread += 1;
        Ok(())
    }

    async fn flush_thread(&self, _thread_id: ThreadId) -> ThreadStoreResult<()> {
        self.state.lock().await.calls.flush_thread += 1;
        Ok(())
    }

    async fn shutdown_thread(&self, _thread_id: ThreadId) -> ThreadStoreResult<()> {
        self.state.lock().await.calls.shutdown_thread += 1;
        Ok(())
    }

    async fn discard_thread(&self, _thread_id: ThreadId) -> ThreadStoreResult<()> {
        self.state.lock().await.calls.discard_thread += 1;
        Ok(())
    }

    async fn load_history(
        &self,
        params: LoadThreadHistoryParams,
    ) -> ThreadStoreResult<StoredThreadHistory> {
        let mut state = self.state.lock().await;
        state.calls.load_history += 1;
        let items = state.histories.get(&params.thread_id).cloned().ok_or(
            ThreadStoreError::ThreadNotFound {
                thread_id: params.thread_id,
            },
        )?;
        Ok(StoredThreadHistory {
            thread_id: params.thread_id,
            items,
        })
    }

    async fn read_thread(&self, params: ReadThreadParams) -> ThreadStoreResult<StoredThread> {
        let mut state = self.state.lock().await;
        state.calls.read_thread += 1;
        stored_thread_from_state(&state, params.thread_id, params.include_history)
    }

    async fn list_threads(&self, _params: ListThreadsParams) -> ThreadStoreResult<ThreadPage> {
        let mut state = self.state.lock().await;
        state.calls.list_threads += 1;
        let mut items = state
            .created_threads
            .keys()
            .map(|thread_id| stored_thread_from_state(&state, *thread_id, false))
            .collect::<ThreadStoreResult<Vec<_>>>()?;
        items.sort_by_key(|item| item.thread_id.to_string());
        Ok(ThreadPage {
            items,
            next_cursor: None,
        })
    }

    async fn update_thread_metadata(
        &self,
        params: UpdateThreadMetadataParams,
    ) -> ThreadStoreResult<StoredThread> {
        let mut state = self.state.lock().await;
        state.calls.update_thread_metadata += 1;
        if let Some(name) = params.patch.name {
            state.names.insert(params.thread_id, Some(name));
        }
        stored_thread_from_state(&state, params.thread_id, false)
    }

    async fn archive_thread(&self, _params: ArchiveThreadParams) -> ThreadStoreResult<()> {
        self.state.lock().await.calls.archive_thread += 1;
        Ok(())
    }

    async fn unarchive_thread(
        &self,
        params: ArchiveThreadParams,
    ) -> ThreadStoreResult<StoredThread> {
        let mut state = self.state.lock().await;
        state.calls.unarchive_thread += 1;
        stored_thread_from_state(&state, params.thread_id, false)
    }
}

fn stored_thread_from_state(
    state: &InMemoryThreadStoreState,
    thread_id: ThreadId,
    include_history: bool,
) -> ThreadStoreResult<StoredThread> {
    let created = state
        .created_threads
        .get(&thread_id)
        .ok_or(ThreadStoreError::ThreadNotFound { thread_id })?;
    let history_items = state.histories.get(&thread_id).cloned().unwrap_or_default();
    let history = include_history.then(|| StoredThreadHistory {
        thread_id,
        items: history_items.clone(),
    });
    let name = state.names.get(&thread_id).cloned().flatten();

    Ok(StoredThread {
        thread_id,
        rollout_path: None,
        forked_from_id: created.forked_from_id,
        preview: String::new(),
        name,
        model_provider: "test".to_string(),
        model: None,
        reasoning_effort: None,
        created_at: Utc::now(),
        updated_at: Utc::now(),
        archived_at: None,
        cwd: PathBuf::new(),
        cli_version: "test".to_string(),
        source: created.source.clone(),
        agent_nickname: None,
        agent_role: None,
        agent_path: None,
        git_info: None,
        approval_mode: CoreAskForApproval::Never,
        sandbox_policy: SandboxPolicy::new_read_only_policy(),
        token_usage: None,
        first_user_message: None,
        history,
    })
}

fn create_config_toml_with_thread_store_endpoint(
    codex_home: &Path,
    server_uri: &str,
    endpoint: &str,
) -> std::io::Result<()> {
    std::fs::write(
        codex_home.join("config.toml"),
        format!(
            r#"
model = "mock-model"
approval_policy = "never"
sandbox_mode = "read-only"
experimental_thread_store_endpoint = "{endpoint}"

model_provider = "mock_provider"

[model_providers.mock_provider]
name = "Mock provider for test"
base_url = "{server_uri}/v1"
wire_api = "responses"
request_max_retries = 0
stream_max_retries = 0
"#
        ),
    )
}

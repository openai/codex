use anyhow::Result;
use anyhow::anyhow;
use codex_app_server::in_process::InProcessClientHandle;
use codex_app_server::in_process::InProcessServerEvent;
use codex_app_server::in_process::InProcessStartArgs;
use codex_app_server::in_process::PendingInProcessRequest;
use codex_app_server::in_process::start as start_in_process;
use codex_app_server_protocol::ClientInfo;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::FuzzyFileSearchParams;
use codex_app_server_protocol::FuzzyFileSearchSessionCompletedNotification;
use codex_app_server_protocol::FuzzyFileSearchSessionStartParams;
use codex_app_server_protocol::FuzzyFileSearchSessionStopParams;
use codex_app_server_protocol::FuzzyFileSearchSessionUpdateParams;
use codex_app_server_protocol::FuzzyFileSearchSessionUpdatedNotification;
use codex_app_server_protocol::InitializeCapabilities;
use codex_app_server_protocol::InitializeParams;
use codex_app_server_protocol::JSONRPCError;
use codex_app_server_protocol::JSONRPCNotification;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ServerNotification;
use codex_arg0::Arg0DispatchPaths;
use codex_core::config::ConfigBuilder;
use codex_core::config_loader::CloudRequirementsLoader;
use codex_core::config_loader::LoaderOverrides;
use codex_feedback::CodexFeedback;
use codex_protocol::protocol::SessionSource;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::collections::HashMap;
use std::collections::VecDeque;
use std::path::Path;
use std::sync::Arc;
use tempfile::TempDir;
use tokio::time::timeout;

type PendingRequest = PendingInProcessRequest;

const DEFAULT_READ_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);
const SHORT_READ_TIMEOUT: std::time::Duration = std::time::Duration::from_millis(500);
const STOP_GRACE_PERIOD: std::time::Duration = std::time::Duration::from_millis(250);
const SESSION_UPDATED_METHOD: &str = "fuzzyFileSearch/sessionUpdated";
const SESSION_COMPLETED_METHOD: &str = "fuzzyFileSearch/sessionCompleted";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FileExpectation {
    Any,
    Empty,
    NonEmpty,
}

fn create_config_toml(codex_home: &Path) -> std::io::Result<()> {
    let config_toml = codex_home.join("config.toml");
    std::fs::write(
        config_toml,
        r#"
model = "mock-model"
approval_policy = "never"
sandbox_mode = "danger-full-access"

[features]
shell_snapshot = false
"#,
    )
}

struct McpProcess {
    client: Option<InProcessClientHandle>,
    next_request_id: i64,
    pending_requests: HashMap<i64, PendingRequest>,
    buffered_notifications: VecDeque<JSONRPCNotification>,
    start_args: Option<InProcessStartArgs>,
}

impl McpProcess {
    async fn new(codex_home: &Path) -> Result<Self> {
        let config = Arc::new(
            ConfigBuilder::default()
                .codex_home(codex_home.to_path_buf())
                .fallback_cwd(Some(codex_home.to_path_buf()))
                .build()
                .await?,
        );

        Ok(Self {
            client: None,
            next_request_id: 1,
            pending_requests: HashMap::new(),
            buffered_notifications: VecDeque::new(),
            start_args: Some(InProcessStartArgs {
                arg0_paths: Arg0DispatchPaths::default(),
                config,
                cli_overrides: Vec::new(),
                loader_overrides: LoaderOverrides::default(),
                cloud_requirements: CloudRequirementsLoader::default(),
                auth_manager: None,
                thread_manager: None,
                feedback: CodexFeedback::new(),
                config_warnings: Vec::new(),
                session_source: SessionSource::Cli,
                enable_codex_api_key_env: false,
                initialize: InitializeParams {
                    client_info: ClientInfo {
                        name: "codex-app-server-tests".to_string(),
                        title: None,
                        version: "0.1.0".to_string(),
                    },
                    capabilities: Some(InitializeCapabilities {
                        experimental_api: true,
                        ..Default::default()
                    }),
                },
                channel_capacity: 64,
            }),
        })
    }

    async fn initialize(&mut self) -> Result<()> {
        if self.client.is_some() {
            return Ok(());
        }

        let start_args = self
            .start_args
            .take()
            .ok_or_else(|| anyhow!("MCP already initialized"))?;
        self.client = Some(start_in_process(start_args).await?);
        Ok(())
    }

    async fn send_fuzzy_file_search_request(
        &mut self,
        query: &str,
        roots: Vec<String>,
        cancellation_token: Option<String>,
    ) -> Result<i64> {
        let request_id = self.next_request_id();
        self.enqueue_request(
            request_id,
            ClientRequest::FuzzyFileSearch {
                request_id: RequestId::Integer(request_id),
                params: FuzzyFileSearchParams {
                    query: query.to_string(),
                    roots,
                    cancellation_token,
                },
            },
        )?;
        Ok(request_id)
    }

    async fn send_fuzzy_file_search_session_start_request(
        &mut self,
        session_id: &str,
        roots: Vec<String>,
    ) -> Result<i64> {
        let request_id = self.next_request_id();
        self.enqueue_request(
            request_id,
            ClientRequest::FuzzyFileSearchSessionStart {
                request_id: RequestId::Integer(request_id),
                params: FuzzyFileSearchSessionStartParams {
                    session_id: session_id.to_string(),
                    roots,
                },
            },
        )?;
        Ok(request_id)
    }

    async fn start_fuzzy_file_search_session(
        &mut self,
        session_id: &str,
        roots: Vec<String>,
    ) -> Result<JSONRPCResponse> {
        let request_id = self
            .send_fuzzy_file_search_session_start_request(session_id, roots)
            .await?;
        self.read_stream_until_response_message(RequestId::Integer(request_id))
            .await
    }

    async fn send_fuzzy_file_search_session_update_request(
        &mut self,
        session_id: &str,
        query: &str,
    ) -> Result<i64> {
        let request_id = self.next_request_id();
        self.enqueue_request(
            request_id,
            ClientRequest::FuzzyFileSearchSessionUpdate {
                request_id: RequestId::Integer(request_id),
                params: FuzzyFileSearchSessionUpdateParams {
                    session_id: session_id.to_string(),
                    query: query.to_string(),
                },
            },
        )?;
        Ok(request_id)
    }

    async fn update_fuzzy_file_search_session(
        &mut self,
        session_id: &str,
        query: &str,
    ) -> Result<JSONRPCResponse> {
        let request_id = self
            .send_fuzzy_file_search_session_update_request(session_id, query)
            .await?;
        self.read_stream_until_response_message(RequestId::Integer(request_id))
            .await
    }

    async fn send_fuzzy_file_search_session_stop_request(
        &mut self,
        session_id: &str,
    ) -> Result<i64> {
        let request_id = self.next_request_id();
        self.enqueue_request(
            request_id,
            ClientRequest::FuzzyFileSearchSessionStop {
                request_id: RequestId::Integer(request_id),
                params: FuzzyFileSearchSessionStopParams {
                    session_id: session_id.to_string(),
                },
            },
        )?;
        Ok(request_id)
    }

    async fn stop_fuzzy_file_search_session(
        &mut self,
        session_id: &str,
    ) -> Result<JSONRPCResponse> {
        let request_id = self
            .send_fuzzy_file_search_session_stop_request(session_id)
            .await?;
        self.read_stream_until_response_message(RequestId::Integer(request_id))
            .await
    }

    async fn read_stream_until_response_message(
        &mut self,
        request_id: RequestId,
    ) -> Result<JSONRPCResponse> {
        let pending = self.take_pending_request(&request_id)?;
        match pending.recv().await? {
            Ok(result) => Ok(JSONRPCResponse {
                id: request_id,
                result,
            }),
            Err(error) => {
                anyhow::bail!("expected response for {request_id:?}, got error: {error:?}")
            }
        }
    }

    async fn read_stream_until_error_message(
        &mut self,
        request_id: RequestId,
    ) -> Result<JSONRPCError> {
        let pending = self.take_pending_request(&request_id)?;
        match pending.recv().await? {
            Ok(result) => {
                anyhow::bail!("expected error for {request_id:?}, got response: {result:?}")
            }
            Err(error) => Ok(JSONRPCError {
                id: request_id,
                error,
            }),
        }
    }

    async fn read_stream_until_notification_message(
        &mut self,
        method: &str,
    ) -> Result<JSONRPCNotification> {
        self.read_stream_until_matching_notification(method, |notification| {
            Ok(notification.method == method)
        })
        .await
    }

    async fn read_stream_until_matching_notification<P>(
        &mut self,
        description: &str,
        mut matches: P,
    ) -> Result<JSONRPCNotification>
    where
        P: FnMut(&JSONRPCNotification) -> Result<bool>,
    {
        let mut index = 0;
        while index < self.buffered_notifications.len() {
            if matches(&self.buffered_notifications[index])? {
                if let Some(notification) = self.buffered_notifications.remove(index) {
                    return Ok(notification);
                }
                anyhow::bail!("buffered notification disappeared while waiting for {description}");
            }
            index += 1;
        }

        loop {
            let event = self
                .client_mut()?
                .next_event()
                .await
                .ok_or_else(|| anyhow!("app-server closed before emitting {description}"))?;

            let notification = match event {
                InProcessServerEvent::ServerNotification(notification) => {
                    jsonrpc_notification_from_server(notification)?
                }
                InProcessServerEvent::LegacyNotification(notification) => notification,
                InProcessServerEvent::Lagged { skipped } => {
                    anyhow::bail!(
                        "missed {skipped} app-server events while waiting for {description}"
                    )
                }
                InProcessServerEvent::ServerRequest(request) => {
                    anyhow::bail!(
                        "unexpected server request while waiting for {description}: {request:?}"
                    )
                }
            };

            if matches(&notification)? {
                return Ok(notification);
            }
            self.buffered_notifications.push_back(notification);
        }
    }

    fn buffered_notification_methods(&self) -> Vec<String> {
        self.buffered_notifications
            .iter()
            .map(|notification| notification.method.clone())
            .collect()
    }

    fn client_mut(&mut self) -> Result<&mut InProcessClientHandle> {
        self.client
            .as_mut()
            .ok_or_else(|| anyhow!("MCP client not initialized"))
    }

    fn enqueue_request(&mut self, request_id: i64, request: ClientRequest) -> Result<()> {
        let sender = self
            .client
            .as_ref()
            .ok_or_else(|| anyhow!("MCP client not initialized"))?
            .sender();
        let pending_request = sender.start_request(request)?;
        if self
            .pending_requests
            .insert(request_id, pending_request)
            .is_some()
        {
            anyhow::bail!("duplicate pending request id: {request_id}");
        }
        Ok(())
    }

    fn next_request_id(&mut self) -> i64 {
        let request_id = self.next_request_id;
        self.next_request_id += 1;
        request_id
    }

    fn take_pending_request(&mut self, request_id: &RequestId) -> Result<PendingRequest> {
        let RequestId::Integer(request_id) = request_id else {
            anyhow::bail!("unsupported request id: {request_id:?}");
        };
        self.pending_requests
            .remove(request_id)
            .ok_or_else(|| anyhow!("missing pending request for id {request_id}"))
    }
}

impl Drop for McpProcess {
    fn drop(&mut self) {
        self.pending_requests.clear();
        if let Some(client) = self.client.take()
            && let Ok(runtime) = tokio::runtime::Handle::try_current()
        {
            runtime.spawn(async move {
                let _ = client.shutdown().await;
            });
        }
    }
}

fn jsonrpc_notification_from_server(
    notification: ServerNotification,
) -> Result<JSONRPCNotification> {
    Ok(serde_json::from_value(serde_json::to_value(notification)?)?)
}

async fn initialized_mcp(codex_home: &TempDir) -> Result<McpProcess> {
    create_config_toml(codex_home.path())?;
    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;
    Ok(mcp)
}

async fn wait_for_session_updated(
    mcp: &mut McpProcess,
    session_id: &str,
    query: &str,
    file_expectation: FileExpectation,
) -> Result<FuzzyFileSearchSessionUpdatedNotification> {
    let description = format!("session update for sessionId={session_id}, query={query}");
    let notification = match timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_matching_notification(&description, |notification| {
            if notification.method != SESSION_UPDATED_METHOD {
                return Ok(false);
            }
            let params = notification
                .params
                .clone()
                .ok_or_else(|| anyhow!("missing notification params"))?;
            let payload =
                serde_json::from_value::<FuzzyFileSearchSessionUpdatedNotification>(params)?;
            let files_match = match file_expectation {
                FileExpectation::Any => true,
                FileExpectation::Empty => payload.files.is_empty(),
                FileExpectation::NonEmpty => !payload.files.is_empty(),
            };
            Ok(payload.session_id == session_id && payload.query == query && files_match)
        }),
    )
    .await
    {
        Ok(result) => result?,
        Err(_) => {
            anyhow::bail!(
                "timed out waiting for {description}; buffered notifications={:?}",
                mcp.buffered_notification_methods()
            )
        }
    };
    let params = notification
        .params
        .ok_or_else(|| anyhow!("missing notification params"))?;
    Ok(serde_json::from_value::<
        FuzzyFileSearchSessionUpdatedNotification,
    >(params)?)
}

async fn wait_for_session_completed(
    mcp: &mut McpProcess,
    session_id: &str,
) -> Result<FuzzyFileSearchSessionCompletedNotification> {
    let description = format!("session completion for sessionId={session_id}");
    let notification = match timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_matching_notification(&description, |notification| {
            if notification.method != SESSION_COMPLETED_METHOD {
                return Ok(false);
            }
            let params = notification
                .params
                .clone()
                .ok_or_else(|| anyhow!("missing notification params"))?;
            let payload =
                serde_json::from_value::<FuzzyFileSearchSessionCompletedNotification>(params)?;
            Ok(payload.session_id == session_id)
        }),
    )
    .await
    {
        Ok(result) => result?,
        Err(_) => {
            anyhow::bail!(
                "timed out waiting for {description}; buffered notifications={:?}",
                mcp.buffered_notification_methods()
            )
        }
    };
    let params = notification
        .params
        .ok_or_else(|| anyhow!("missing notification params"))?;
    Ok(serde_json::from_value::<
        FuzzyFileSearchSessionCompletedNotification,
    >(params)?)
}

async fn assert_update_request_fails_for_missing_session(
    mcp: &mut McpProcess,
    session_id: &str,
    query: &str,
) -> Result<()> {
    let request_id = mcp
        .send_fuzzy_file_search_session_update_request(session_id, query)
        .await?;
    let err = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_error_message(RequestId::Integer(request_id)),
    )
    .await??;
    assert_eq!(err.error.code, -32600);
    assert_eq!(
        err.error.message,
        format!("fuzzy file search session not found: {session_id}")
    );
    Ok(())
}

async fn assert_no_session_updates_for(
    mcp: &mut McpProcess,
    session_id: &str,
    grace_period: std::time::Duration,
    duration: std::time::Duration,
) -> Result<()> {
    let grace_deadline = tokio::time::Instant::now() + grace_period;
    loop {
        let now = tokio::time::Instant::now();
        if now >= grace_deadline {
            break;
        }
        let remaining = grace_deadline - now;
        match timeout(
            remaining,
            mcp.read_stream_until_notification_message(SESSION_UPDATED_METHOD),
        )
        .await
        {
            Err(_) => break,
            Ok(Err(err)) => return Err(err),
            Ok(Ok(_)) => {}
        }
    }

    let deadline = tokio::time::Instant::now() + duration;
    loop {
        let now = tokio::time::Instant::now();
        if now >= deadline {
            return Ok(());
        }
        let remaining = deadline - now;
        match timeout(
            remaining,
            mcp.read_stream_until_notification_message(SESSION_UPDATED_METHOD),
        )
        .await
        {
            Err(_) => return Ok(()),
            Ok(Err(err)) => return Err(err),
            Ok(Ok(notification)) => {
                let params = notification
                    .params
                    .ok_or_else(|| anyhow!("missing notification params"))?;
                let payload =
                    serde_json::from_value::<FuzzyFileSearchSessionUpdatedNotification>(params)?;
                if payload.session_id == session_id {
                    anyhow::bail!("received unexpected session update after stop: {payload:?}");
                }
            }
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_fuzzy_file_search_sorts_and_includes_indices() -> Result<()> {
    // Prepare a temporary Codex home and a separate root with test files.
    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path())?;
    let root = TempDir::new()?;

    // Create files designed to have deterministic ordering for query "abe".
    std::fs::write(root.path().join("abc"), "x")?;
    std::fs::write(root.path().join("abcde"), "x")?;
    std::fs::write(root.path().join("abexy"), "x")?;
    std::fs::write(root.path().join("zzz.txt"), "x")?;
    let sub_dir = root.path().join("sub");
    std::fs::create_dir_all(&sub_dir)?;
    let sub_abce_path = sub_dir.join("abce");
    std::fs::write(&sub_abce_path, "x")?;
    let sub_abce_rel = sub_abce_path
        .strip_prefix(root.path())?
        .to_string_lossy()
        .to_string();

    // Start MCP server and initialize.
    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let root_path = root.path().to_string_lossy().to_string();
    // Send fuzzyFileSearch request.
    let request_id = mcp
        .send_fuzzy_file_search_request("abe", vec![root_path.clone()], None)
        .await?;

    // Read response and verify shape and ordering.
    let resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;

    let value = resp.result;
    let expected_score = 72;

    assert_eq!(
        value,
        json!({
            "files": [
                {
                    "root": root_path.clone(),
                    "path": "abexy",
                    "file_name": "abexy",
                    "score": 84,
                    "indices": [0, 1, 2],
                },
                {
                    "root": root_path.clone(),
                    "path": sub_abce_rel,
                    "file_name": "abce",
                    "score": expected_score,
                    "indices": [4, 5, 7],
                },
                {
                    "root": root_path.clone(),
                    "path": "abcde",
                    "file_name": "abcde",
                    "score": 71,
                    "indices": [0, 1, 4],
                },
            ]
        })
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_fuzzy_file_search_accepts_cancellation_token() -> Result<()> {
    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path())?;
    let root = TempDir::new()?;

    std::fs::write(root.path().join("alpha.txt"), "contents")?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let root_path = root.path().to_string_lossy().to_string();
    let request_id = mcp
        .send_fuzzy_file_search_request("alp", vec![root_path.clone()], None)
        .await?;

    let request_id_2 = mcp
        .send_fuzzy_file_search_request(
            "alp",
            vec![root_path.clone()],
            Some(request_id.to_string()),
        )
        .await?;

    let resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id_2)),
    )
    .await??;

    let files = resp
        .result
        .get("files")
        .ok_or_else(|| anyhow!("files key missing"))?
        .as_array()
        .ok_or_else(|| anyhow!("files not array"))?
        .clone();

    assert_eq!(files.len(), 1);
    assert_eq!(files[0]["root"], root_path);
    assert_eq!(files[0]["path"], "alpha.txt");

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_fuzzy_file_search_session_streams_updates() -> Result<()> {
    let codex_home = TempDir::new()?;
    let root = TempDir::new()?;
    std::fs::write(root.path().join("alpha.txt"), "contents")?;
    let mut mcp = initialized_mcp(&codex_home).await?;

    let root_path = root.path().to_string_lossy().to_string();
    let session_id = "session-1";

    mcp.start_fuzzy_file_search_session(session_id, vec![root_path.clone()])
        .await?;
    mcp.update_fuzzy_file_search_session(session_id, "alp")
        .await?;

    let payload =
        wait_for_session_updated(&mut mcp, session_id, "alp", FileExpectation::NonEmpty).await?;
    assert_eq!(payload.files.len(), 1);
    assert_eq!(payload.files[0].root, root_path);
    assert_eq!(payload.files[0].path, "alpha.txt");
    let completed = wait_for_session_completed(&mut mcp, session_id).await?;
    assert_eq!(completed.session_id, session_id);

    mcp.stop_fuzzy_file_search_session(session_id).await?;

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_fuzzy_file_search_session_no_updates_after_complete_until_query_edited() -> Result<()>
{
    let codex_home = TempDir::new()?;
    let root = TempDir::new()?;
    std::fs::write(root.path().join("alpha.txt"), "contents")?;
    let mut mcp = initialized_mcp(&codex_home).await?;

    let root_path = root.path().to_string_lossy().to_string();
    let session_id = "session-complete-invariant";
    mcp.start_fuzzy_file_search_session(session_id, vec![root_path])
        .await?;

    mcp.update_fuzzy_file_search_session(session_id, "alp")
        .await?;
    wait_for_session_updated(&mut mcp, session_id, "alp", FileExpectation::NonEmpty).await?;
    wait_for_session_completed(&mut mcp, session_id).await?;
    assert_no_session_updates_for(&mut mcp, session_id, STOP_GRACE_PERIOD, SHORT_READ_TIMEOUT)
        .await?;

    mcp.update_fuzzy_file_search_session(session_id, "alpha")
        .await?;
    wait_for_session_updated(&mut mcp, session_id, "alpha", FileExpectation::NonEmpty).await?;

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_fuzzy_file_search_session_update_before_start_errors() -> Result<()> {
    let codex_home = TempDir::new()?;
    let mut mcp = initialized_mcp(&codex_home).await?;
    assert_update_request_fails_for_missing_session(&mut mcp, "missing", "alp").await?;

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_fuzzy_file_search_session_update_works_without_waiting_for_start_response()
-> Result<()> {
    let codex_home = TempDir::new()?;
    let root = TempDir::new()?;
    std::fs::write(root.path().join("alpha.txt"), "contents")?;
    let mut mcp = initialized_mcp(&codex_home).await?;

    let root_path = root.path().to_string_lossy().to_string();
    let session_id = "session-no-wait";

    let start_request_id = mcp
        .send_fuzzy_file_search_session_start_request(session_id, vec![root_path.clone()])
        .await?;
    let update_request_id = mcp
        .send_fuzzy_file_search_session_update_request(session_id, "alp")
        .await?;

    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(update_request_id)),
    )
    .await??;
    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(start_request_id)),
    )
    .await??;

    let payload =
        wait_for_session_updated(&mut mcp, session_id, "alp", FileExpectation::NonEmpty).await?;
    assert_eq!(payload.files.len(), 1);
    assert_eq!(payload.files[0].root, root_path);
    assert_eq!(payload.files[0].path, "alpha.txt");

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_fuzzy_file_search_session_multiple_query_updates_work() -> Result<()> {
    let codex_home = TempDir::new()?;
    let root = TempDir::new()?;
    std::fs::write(root.path().join("alpha.txt"), "contents")?;
    std::fs::write(root.path().join("alphabet.txt"), "contents")?;
    let mut mcp = initialized_mcp(&codex_home).await?;

    let root_path = root.path().to_string_lossy().to_string();
    let session_id = "session-multi-update";
    mcp.start_fuzzy_file_search_session(session_id, vec![root_path.clone()])
        .await?;

    mcp.update_fuzzy_file_search_session(session_id, "alp")
        .await?;
    let alp_payload =
        wait_for_session_updated(&mut mcp, session_id, "alp", FileExpectation::NonEmpty).await?;
    assert_eq!(
        alp_payload.files.iter().all(|file| file.root == root_path),
        true
    );
    wait_for_session_completed(&mut mcp, session_id).await?;

    mcp.update_fuzzy_file_search_session(session_id, "zzzz")
        .await?;
    let zzzz_payload =
        wait_for_session_updated(&mut mcp, session_id, "zzzz", FileExpectation::Any).await?;
    assert_eq!(zzzz_payload.query, "zzzz");
    assert_eq!(zzzz_payload.files.is_empty(), true);
    wait_for_session_completed(&mut mcp, session_id).await?;

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_fuzzy_file_search_session_update_after_stop_fails() -> Result<()> {
    let codex_home = TempDir::new()?;
    let root = TempDir::new()?;
    std::fs::write(root.path().join("alpha.txt"), "contents")?;
    let mut mcp = initialized_mcp(&codex_home).await?;

    let session_id = "session-stop-fail";
    let root_path = root.path().to_string_lossy().to_string();
    mcp.start_fuzzy_file_search_session(session_id, vec![root_path])
        .await?;
    mcp.stop_fuzzy_file_search_session(session_id).await?;

    assert_update_request_fails_for_missing_session(&mut mcp, session_id, "alp").await?;

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_fuzzy_file_search_session_stops_sending_updates_after_stop() -> Result<()> {
    let codex_home = TempDir::new()?;
    let root = TempDir::new()?;
    for i in 0..512 {
        let file_path = root.path().join(format!("file-{i:04}.txt"));
        std::fs::write(file_path, "contents")?;
    }
    let mut mcp = initialized_mcp(&codex_home).await?;

    let root_path = root.path().to_string_lossy().to_string();
    let session_id = "session-stop-no-updates";
    mcp.start_fuzzy_file_search_session(session_id, vec![root_path])
        .await?;
    mcp.update_fuzzy_file_search_session(session_id, "file-")
        .await?;
    wait_for_session_updated(&mut mcp, session_id, "file-", FileExpectation::NonEmpty).await?;

    mcp.stop_fuzzy_file_search_session(session_id).await?;

    assert_no_session_updates_for(&mut mcp, session_id, STOP_GRACE_PERIOD, SHORT_READ_TIMEOUT)
        .await?;

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_fuzzy_file_search_two_sessions_are_independent() -> Result<()> {
    let codex_home = TempDir::new()?;
    let root_a = TempDir::new()?;
    let root_b = TempDir::new()?;
    std::fs::write(root_a.path().join("alpha.txt"), "contents")?;
    std::fs::write(root_b.path().join("beta.txt"), "contents")?;
    let mut mcp = initialized_mcp(&codex_home).await?;

    let root_a_path = root_a.path().to_string_lossy().to_string();
    let root_b_path = root_b.path().to_string_lossy().to_string();
    let session_a = "session-a";
    let session_b = "session-b";

    mcp.start_fuzzy_file_search_session(session_a, vec![root_a_path.clone()])
        .await?;
    mcp.start_fuzzy_file_search_session(session_b, vec![root_b_path.clone()])
        .await?;

    mcp.update_fuzzy_file_search_session(session_a, "alp")
        .await?;

    let session_a_update =
        wait_for_session_updated(&mut mcp, session_a, "alp", FileExpectation::NonEmpty).await?;
    assert_eq!(session_a_update.files.len(), 1);
    assert_eq!(session_a_update.files[0].root, root_a_path);
    assert_eq!(session_a_update.files[0].path, "alpha.txt");

    mcp.update_fuzzy_file_search_session(session_b, "bet")
        .await?;
    let session_b_update =
        wait_for_session_updated(&mut mcp, session_b, "bet", FileExpectation::NonEmpty).await?;
    assert_eq!(session_b_update.files.len(), 1);
    assert_eq!(session_b_update.files[0].root, root_b_path);
    assert_eq!(session_b_update.files[0].path, "beta.txt");

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_fuzzy_file_search_query_cleared_sends_blank_snapshot() -> Result<()> {
    let codex_home = TempDir::new()?;
    let root = TempDir::new()?;
    std::fs::write(root.path().join("alpha.txt"), "contents")?;
    let mut mcp = initialized_mcp(&codex_home).await?;

    let root_path = root.path().to_string_lossy().to_string();
    let session_id = "session-clear-query";
    mcp.start_fuzzy_file_search_session(session_id, vec![root_path])
        .await?;

    mcp.update_fuzzy_file_search_session(session_id, "alp")
        .await?;
    wait_for_session_updated(&mut mcp, session_id, "alp", FileExpectation::NonEmpty).await?;

    mcp.update_fuzzy_file_search_session(session_id, "").await?;
    let payload =
        wait_for_session_updated(&mut mcp, session_id, "", FileExpectation::Empty).await?;
    assert_eq!(payload.files.is_empty(), true);

    Ok(())
}

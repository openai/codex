use std::path::PathBuf;
use std::sync::Arc;

use codex_app_server_client::RemoteAppServerClient;
use codex_app_server_client::RemoteAppServerConnectArgs;
use codex_app_server_client::RemoteAppServerEndpoint;
use codex_app_server_protocol::JSONRPCMessage;
use codex_protocol::ThreadId;
use codex_protocol::protocol::SessionMeta;
use codex_protocol::protocol::SessionMetaLine;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::ThreadHistoryMode;
use codex_state::ThreadMetadataBuilder;
use codex_utils_absolute_path::test_support::PathExt;
use futures::SinkExt;
use futures::StreamExt;
use pretty_assertions::assert_eq;
use serde_json::json;
use tempfile::TempDir;
use tokio_tungstenite::tungstenite::Message;

use super::SessionCollection;
use super::lookup;
use crate::app_server_session::AppServerSession;
use crate::app_server_session::ThreadParamsMode;
use crate::legacy_core::config::Config;
use crate::legacy_core::config::ConfigBuilder;
use crate::resume_source_kinds;
use crate::tests::start_test_embedded_app_server;

async fn build_config(temp_dir: &TempDir) -> std::io::Result<Config> {
    ConfigBuilder::default()
        .codex_home(temp_dir.path().to_path_buf())
        .build()
        .await
}

async fn state_runtime(config: &Config) -> std::io::Result<Arc<codex_state::StateRuntime>> {
    let runtime = codex_state::StateRuntime::init(
        codex_state::SqliteConfig::new_for_testing(config.codex_home.as_path().abs()),
        config.model_provider_id.clone(),
    )
    .await
    .map_err(std::io::Error::other)?;
    runtime
        .mark_backfill_complete(/*last_watermark*/ None)
        .await
        .map_err(std::io::Error::other)?;
    Ok(runtime)
}

async fn lookup_name(
    config: &Config,
    name: &str,
    collections: &[SessionCollection],
    mode: ThreadParamsMode,
    model_provider: Option<&str>,
) -> color_eyre::Result<Option<codex_app_server_protocol::Thread>> {
    let mut app_server = AppServerSession::new(
        codex_app_server_client::AppServerClient::InProcess(
            start_test_embedded_app_server(config.clone()).await?,
        ),
        mode,
    );
    let target = lookup(
        &mut app_server,
        config.codex_home.as_path(),
        name,
        collections,
        &[resume_source_kinds(/*include_non_interactive*/ false)],
        model_provider,
    )
    .await?;
    app_server.shutdown().await?;
    Ok(target)
}

async fn upsert_thread(
    runtime: &codex_state::StateRuntime,
    metadata: codex_state::ThreadMetadata,
) -> std::io::Result<()> {
    runtime
        .upsert_thread(&metadata)
        .await
        .map_err(std::io::Error::other)
}

fn thread_metadata(
    config: &Config,
    thread_id: ThreadId,
    rollout_path: PathBuf,
    title: &str,
) -> codex_state::ThreadMetadata {
    let created_at = chrono::DateTime::parse_from_rfc3339("2025-02-01T10:00:00Z")
        .expect("timestamp should parse")
        .with_timezone(&chrono::Utc);
    let mut builder = ThreadMetadataBuilder::new(
        thread_id,
        rollout_path,
        created_at,
        serde_json::from_value(serde_json::json!("cli"))
            .expect("cli session source should deserialize"),
    );
    builder.cwd = config.codex_home.join("project").to_path_buf();
    let mut metadata = builder.build(config.model_provider_id.as_str());
    metadata.title = title.to_string();
    metadata.first_user_message = Some("preview text".to_string());
    metadata.preview = metadata.first_user_message.clone();
    metadata
}

fn write_rollout(
    config: &Config,
    thread_id: ThreadId,
    timestamp: &str,
    preview: &str,
    source: SessionSource,
    history_mode: ThreadHistoryMode,
) -> color_eyre::Result<PathBuf> {
    let rollout_path = config
        .codex_home
        .join("sessions/2025/02/01")
        .join(format!("rollout-2025-02-01T10-00-00-{thread_id}.jsonl"));
    std::fs::create_dir_all(rollout_path.parent().expect("rollout parent"))?;
    let session_meta = SessionMetaLine {
        meta: SessionMeta {
            session_id: thread_id.into(),
            id: thread_id,
            timestamp: timestamp.to_string(),
            cwd: config.codex_home.join("project").to_path_buf(),
            originator: "codex".to_string(),
            cli_version: "0.0.0".to_string(),
            source,
            model_provider: Some(config.model_provider_id.clone()),
            history_mode,
            ..Default::default()
        },
        git: None,
    };
    let lines = [
        serde_json::json!({
            "timestamp": timestamp,
            "type": "session_meta",
            "payload": session_meta,
        }),
        serde_json::json!({
            "timestamp": timestamp,
            "type": "response_item",
            "payload": {
                "type": "message",
                "role": "user",
                "content": [{"type": "input_text", "text": preview}],
            },
        }),
        serde_json::json!({
            "timestamp": timestamp,
            "type": "event_msg",
            "payload": {
                "type": "user_message",
                "message": preview,
                "kind": "plain",
            },
        }),
    ];
    std::fs::write(
        &rollout_path,
        lines
            .iter()
            .map(serde_json::Value::to_string)
            .collect::<Vec<_>>()
            .join("\n")
            + "\n",
    )?;
    Ok(rollout_path.to_path_buf())
}

#[tokio::test]
async fn resolves_name_and_preview_from_server_list() -> color_eyre::Result<()> {
    let temp_dir = TempDir::new()?;
    let config = build_config(&temp_dir).await?;
    let runtime = state_runtime(&config).await?;
    let named_id = ThreadId::new();
    let named_path = write_rollout(
        &config,
        named_id,
        "2025-02-01T10:00:00Z",
        "preview text",
        SessionSource::Cli,
        ThreadHistoryMode::Legacy,
    )?;
    upsert_thread(
        &runtime,
        thread_metadata(&config, named_id, named_path, "saved-session"),
    )
    .await?;
    let preview_id = ThreadId::new();
    let preview_path = write_rollout(
        &config,
        preview_id,
        "2025-02-02T10:00:00Z",
        "preview text",
        SessionSource::Cli,
        ThreadHistoryMode::Legacy,
    )?;
    upsert_thread(
        &runtime,
        thread_metadata(&config, preview_id, preview_path, "preview text"),
    )
    .await?;
    let other_id = ThreadId::new();
    let mut other_config = config.clone();
    other_config.model_provider_id = "another-provider".to_string();
    let other_path = write_rollout(
        &other_config,
        other_id,
        "2025-02-03T10:00:00Z",
        "preview text",
        SessionSource::Cli,
        ThreadHistoryMode::Legacy,
    )?;
    upsert_thread(
        &runtime,
        thread_metadata(&other_config, other_id, other_path, "saved-session"),
    )
    .await?;

    let named = lookup_name(
        &config,
        "saved-session",
        &[SessionCollection::Active],
        ThreadParamsMode::Embedded,
        Some(&config.model_provider_id),
    )
    .await?;
    let preview = lookup_name(
        &config,
        "preview text",
        &[SessionCollection::Active],
        ThreadParamsMode::Embedded,
        Some(&config.model_provider_id),
    )
    .await?;
    let switched_provider = lookup_name(
        &config,
        "saved-session",
        &[SessionCollection::Active],
        ThreadParamsMode::Embedded,
        Some(&other_config.model_provider_id),
    )
    .await?;
    let remote = lookup_name(
        &other_config,
        "saved-session",
        &[SessionCollection::Active],
        ThreadParamsMode::Remote,
        /*model_provider*/ None,
    )
    .await?;
    assert_eq!(
        (
            named.map(|thread| thread.id),
            preview.map(|thread| thread.id),
            switched_provider.map(|thread| thread.id),
            remote.map(|thread| thread.id),
        ),
        (
            Some(named_id.to_string()),
            Some(preview_id.to_string()),
            Some(other_id.to_string()),
            Some(other_id.to_string()),
        ),
    );
    Ok(())
}

#[tokio::test]
async fn rejects_duplicate_labels_across_server_pages() -> color_eyre::Result<()> {
    let temp_dir = TempDir::new()?;
    let config = build_config(&temp_dir).await?;
    let runtime = state_runtime(&config).await?;
    let mut relevant_ids = Vec::new();
    for index in 0..102 {
        let thread_id = ThreadId::from_string(&format!("00000000-0000-0000-0000-{index:012}"))?;
        if matches!(index, 0 | 1 | 101) {
            relevant_ids.push(thread_id);
        }
        let rollout_path = write_rollout(
            &config,
            thread_id,
            "2025-02-01T10:00:00Z",
            "preview text",
            SessionSource::Cli,
            ThreadHistoryMode::Legacy,
        )?;
        let name = if index == 0 || index == 101 {
            "same-label".to_string()
        } else {
            format!("other-{index}")
        };
        let mut metadata = thread_metadata(&config, thread_id, rollout_path, &name);
        metadata.recency_at += chrono::Duration::seconds(index);
        metadata.updated_at += chrono::Duration::seconds(index);
        upsert_thread(&runtime, metadata).await?;
    }

    let mut app_server = AppServerSession::new(
        codex_app_server_client::AppServerClient::InProcess(
            start_test_embedded_app_server(config.clone()).await?,
        ),
        ThreadParamsMode::Embedded,
    );
    let error = lookup(
        &mut app_server,
        config.codex_home.as_path(),
        "same-label",
        &[SessionCollection::Active],
        &[resume_source_kinds(/*include_non_interactive*/ false)],
        Some(&config.model_provider_id),
    )
    .await
    .expect_err("duplicate labels should require an ID");
    let unique = lookup(
        &mut app_server,
        config.codex_home.as_path(),
        "other-1",
        &[SessionCollection::Active],
        &[resume_source_kinds(/*include_non_interactive*/ false)],
        Some(&config.model_provider_id),
    )
    .await?;
    app_server.shutdown().await?;
    assert_eq!(
        error.to_string(),
        format!(
            "Multiple sessions match 'same-label' (including {} and {}); use a session UUID to disambiguate.",
            relevant_ids[2], relevant_ids[0]
        )
    );
    assert_eq!(
        unique.map(|thread| thread.id),
        Some(relevant_ids[1].to_string())
    );
    Ok(())
}

#[tokio::test]
async fn skips_stale_listed_thread_before_valid_label() -> color_eyre::Result<()> {
    let temp_dir = TempDir::new()?;
    let config = build_config(&temp_dir).await?;
    let runtime = state_runtime(&config).await?;
    let stale_id = ThreadId::new();
    let stale_path = write_rollout(
        &config,
        stale_id,
        "2025-02-01T10:00:00Z",
        "same-label",
        SessionSource::Cli,
        ThreadHistoryMode::Legacy,
    )?;
    upsert_thread(
        &runtime,
        thread_metadata(&config, stale_id, stale_path.clone(), "same-label"),
    )
    .await?;
    std::fs::remove_file(stale_path)?;

    let corrupt_id = ThreadId::new();
    let corrupt_path = write_rollout(
        &config,
        corrupt_id,
        "2025-02-01T10:00:00Z",
        "same-label",
        SessionSource::Cli,
        ThreadHistoryMode::Legacy,
    )?;
    upsert_thread(
        &runtime,
        thread_metadata(&config, corrupt_id, corrupt_path.clone(), "same-label"),
    )
    .await?;
    std::fs::write(corrupt_path, "invalid session header\n")?;

    let valid_id = ThreadId::new();
    let valid_path = write_rollout(
        &config,
        valid_id,
        "2025-02-01T10:00:01Z",
        "same-label",
        SessionSource::Cli,
        ThreadHistoryMode::Legacy,
    )?;
    upsert_thread(
        &runtime,
        thread_metadata(&config, valid_id, valid_path.clone(), "same-label"),
    )
    .await?;
    let wrong_id = ThreadId::new();
    upsert_thread(
        &runtime,
        thread_metadata(&config, wrong_id, valid_path, "same-label"),
    )
    .await?;

    let found = lookup_name(
        &config,
        "same-label",
        &[SessionCollection::Active],
        ThreadParamsMode::Embedded,
        Some(&config.model_provider_id),
    )
    .await?;
    assert_eq!(found.map(|thread| thread.id), Some(valid_id.to_string()));
    Ok(())
}

#[tokio::test]
async fn uses_listed_thread_when_older_server_cannot_read_it() -> color_eyre::Result<()> {
    let temp_dir = TempDir::new()?;
    let config = build_config(&temp_dir).await?;
    let runtime = state_runtime(&config).await?;
    let thread_id = ThreadId::new();
    let path = write_rollout(
        &config,
        thread_id,
        "2025-02-01T10:00:00Z",
        "preview text",
        SessionSource::Cli,
        ThreadHistoryMode::Legacy,
    )?;
    upsert_thread(
        &runtime,
        thread_metadata(&config, thread_id, path, "saved-session"),
    )
    .await?;
    let mut embedded = AppServerSession::new(
        codex_app_server_client::AppServerClient::InProcess(
            start_test_embedded_app_server(config.clone()).await?,
        ),
        ThreadParamsMode::Embedded,
    );
    let listed = embedded
        .thread_read(thread_id, /*include_turns*/ false)
        .await?;
    embedded.shutdown().await?;

    for (mode, next_cursor) in [
        (ThreadParamsMode::Remote, None),
        (ThreadParamsMode::Embedded, None),
        (
            ThreadParamsMode::Remote,
            Some("2025-02-01T10:00:00Z".to_string()),
        ),
        (
            ThreadParamsMode::Remote,
            Some(format!("2025-02-01T10:00:00Z|{thread_id}")),
        ),
    ] {
        let unverified_pagination = next_cursor
            .as_ref()
            .is_some_and(|cursor| !cursor.contains('|'));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
        let url = format!("ws://{}", listener.local_addr()?);
        let listed_for_server = listed.clone();
        let paginated = next_cursor.is_some();
        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let mut socket = tokio_tungstenite::accept_async(stream).await.unwrap();
            while let Some(Ok(Message::Text(text))) = socket.next().await {
                let JSONRPCMessage::Request(request) = serde_json::from_str(&text).unwrap() else {
                    continue;
                };
                let last_page = request.method == "thread/list"
                    && request
                        .params
                        .as_ref()
                        .is_some_and(|params| params["cursor"].is_string());
                let response = match request.method.as_str() {
                    "initialize" => {
                        json!({"id": request.id, "result": {"userAgent": "legacy-test"}})
                    }
                    "thread/list" => {
                        assert_eq!(
                            request.params.as_ref().unwrap()["sortKey"],
                            json!(codex_app_server_protocol::ThreadSortKey::CreatedAt)
                        );
                        assert_eq!(
                            request.params.as_ref().unwrap()["useStateDbOnly"],
                            json!(true)
                        );
                        let data = if last_page {
                            Vec::new()
                        } else {
                            vec![listed_for_server.clone()]
                        };
                        let cursor = if last_page { None } else { next_cursor.clone() };
                        json!({"id": request.id, "result": {"data": data, "nextCursor": cursor}})
                    }
                    "thread/read" => json!({
                        "id": request.id,
                        "error": {
                            "code": -32603,
                            "message": format!("thread not loaded: {thread_id}")
                        }
                    }),
                    other => panic!("unexpected request: {other}"),
                };
                socket
                    .send(Message::Text(response.to_string().into()))
                    .await
                    .unwrap();
                if last_page || (!paginated && request.method == "thread/read") {
                    break;
                }
            }
        });
        let client = RemoteAppServerClient::connect(RemoteAppServerConnectArgs {
            endpoint: RemoteAppServerEndpoint::WebSocket {
                websocket_url: url,
                auth_token: None,
            },
            client_name: "legacy-test".to_string(),
            client_version: "0.0.0".to_string(),
            experimental_api: true,
            mcp_server_openai_form_elicitation: false,
            opt_out_notification_methods: Vec::new(),
            channel_capacity: 8,
        })
        .await?;
        let mut remote = AppServerSession::new(
            codex_app_server_client::AppServerClient::Remote(client),
            mode,
        );
        let found = lookup(
            &mut remote,
            config.codex_home.as_path(),
            "saved-session",
            &[SessionCollection::Active],
            &[resume_source_kinds(/*include_non_interactive*/ false)],
            /*model_provider*/ None,
        )
        .await;
        if unverified_pagination {
            assert_eq!(
                found
                    .expect_err("timestamp-only cursors cannot prove uniqueness")
                    .to_string(),
                format!(
                    "Cannot verify a unique session label across server pages; matching session UUID: {thread_id}. Use it only if this is the session you want."
                )
            );
        } else {
            assert_eq!(found?, Some(listed.clone()));
        }
        server.await?;
    }
    Ok(())
}

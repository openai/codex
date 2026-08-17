use std::sync::Arc;
use std::time::Duration;

use codex_protocol::ThreadId;
use codex_protocol::models::BaseInstructions;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::ThreadHistoryMode;
use codex_protocol::protocol::ThreadMemoryMode;
use codex_protocol::protocol::UserMessageEvent;
use codex_rollout::RolloutItem;
use codex_rollout::RolloutRecorder;
use pretty_assertions::assert_eq;
use tempfile::TempDir;

use super::*;
use crate::CreateThreadParams;
use crate::LiveThread;
use crate::ThreadPersistenceMetadata;
use crate::UpdateThreadMetadataParams;
use crate::local::test_support::test_config;

#[tokio::test]
async fn pending_thread_metadata_is_consumed_by_first_metadata_update() {
    let (_home, store, runtime) = store_with_runtime().await;
    let thread_id = ThreadId::new();
    let pending_patch = staged_model_patch();

    store
        .stage_pending_thread_metadata(thread_id, pending_patch.clone())
        .await
        .expect("stage pending metadata");
    let live_thread = LiveThread::create(store.clone(), create_thread_params(thread_id))
        .await
        .expect("create live thread");
    live_thread
        .append_items(&[user_message_item("materialize pending metadata")])
        .await
        .expect("append observed item");

    let metadata = runtime
        .get_thread(thread_id)
        .await
        .expect("sqlite metadata read")
        .expect("sqlite metadata");
    assert_eq!(metadata.model.as_deref(), Some("staged-model"));
    assert_eq!(metadata.model_provider, "test-provider");
    assert_eq!(metadata.source, "exec");
    assert_eq!(
        metadata.cwd.canonicalize().expect("canonical metadata cwd"),
        std::env::current_dir()
            .expect("current directory")
            .canonicalize()
            .expect("canonical current directory")
    );
    assert_pending_metadata_consumed(&store, thread_id).await;
}

#[tokio::test]
async fn pending_rollout_compatible_metadata_does_not_deadlock() {
    let (_home, store, runtime) = store_with_runtime().await;
    let thread_id = ThreadId::new();

    store
        .stage_pending_thread_metadata(
            thread_id,
            ThreadMetadataPatch {
                name: Some(Some("staged-name".to_string())),
                model_provider: Some("staged-provider".to_string()),
                ..Default::default()
            },
        )
        .await
        .expect("stage pending metadata");
    let live_thread = LiveThread::create(store.clone(), create_thread_params(thread_id))
        .await
        .expect("create live thread");
    tokio::time::timeout(
        Duration::from_secs(5),
        live_thread.update_metadata(
            ThreadMetadataPatch {
                model: Some("updated-model".to_string()),
                ..Default::default()
            },
            /*include_archived*/ false,
        ),
    )
    .await
    .expect("pending metadata update should not deadlock")
    .expect("update metadata before rollout materialization");

    let metadata = runtime
        .get_thread(thread_id)
        .await
        .expect("sqlite metadata read")
        .expect("sqlite metadata");
    assert_eq!(metadata.title, "staged-name");
    assert_eq!(metadata.model_provider, "test-provider");
    assert_eq!(metadata.model.as_deref(), Some("updated-model"));
    assert_pending_metadata_consumed(&store, thread_id).await;
}

#[tokio::test]
async fn pending_rollout_compatible_metadata_is_written_with_observed_update() {
    let (_home, store, _runtime) = store_with_runtime().await;
    let thread_id = ThreadId::new();

    store
        .stage_pending_thread_metadata(
            thread_id,
            ThreadMetadataPatch {
                memory_mode: Some(ThreadMemoryMode::Disabled),
                ..Default::default()
            },
        )
        .await
        .expect("stage pending metadata");
    let _live_thread = LiveThread::create(store.clone(), create_thread_params(thread_id))
        .await
        .expect("create live thread");
    store
        .update_thread_metadata(UpdateThreadMetadataParams {
            thread_id,
            patch: ThreadMetadataPatch {
                model: Some("observed-model".to_string()),
                ..Default::default()
            },
            include_archived: false,
        })
        .await
        .expect("apply observed metadata");

    let rollout_path = store
        .live_rollout_path(thread_id)
        .await
        .expect("rollout path");
    let (items, _, _) = RolloutRecorder::load_rollout_items(rollout_path.as_path())
        .await
        .expect("load durable rollout");
    let memory_mode = items.iter().rev().find_map(|item| match item {
        RolloutItem::SessionMeta(session_meta) => session_meta.meta.memory_mode.as_deref(),
        _ => None,
    });

    assert_eq!(memory_mode, Some("disabled"));
    assert_pending_metadata_consumed(&store, thread_id).await;
}

#[tokio::test]
async fn idle_shutdown_clears_pending_thread_metadata_without_materializing() {
    let (_home, store, runtime) = store_with_runtime().await;
    let thread_id = ThreadId::new();

    store
        .stage_pending_thread_metadata(thread_id, staged_model_patch())
        .await
        .expect("stage pending metadata");
    let live_thread = LiveThread::create(store.clone(), create_thread_params(thread_id))
        .await
        .expect("create live thread");
    live_thread.shutdown().await.expect("shutdown idle thread");

    assert_eq!(
        runtime
            .get_thread(thread_id)
            .await
            .expect("sqlite metadata read"),
        None
    );
    assert_pending_metadata_consumed(&store, thread_id).await;
}

#[tokio::test]
async fn discarding_live_thread_clears_pending_metadata() {
    let (_home, store, _runtime) = store_with_runtime().await;
    let thread_id = ThreadId::new();

    store
        .stage_pending_thread_metadata(thread_id, staged_model_patch())
        .await
        .expect("stage pending metadata");
    let live_thread = LiveThread::create(store.clone(), create_thread_params(thread_id))
        .await
        .expect("create live thread");
    live_thread.discard().await.expect("discard live thread");

    assert_pending_metadata_consumed(&store, thread_id).await;
}

#[tokio::test]
async fn pending_thread_metadata_requires_state_db() {
    let home = TempDir::new().expect("temp dir");
    let store = LocalThreadStore::new(test_config(home.path()), /*state_db*/ None);

    let err = store
        .stage_pending_thread_metadata(ThreadId::new(), staged_model_patch())
        .await
        .expect_err("reject pending metadata without state db");
    assert!(matches!(
        err,
        ThreadStoreError::InvalidRequest { message }
            if message == "pending thread metadata requires a state db"
    ));
}

#[tokio::test]
async fn pending_thread_metadata_rejects_rollout_path() {
    let (home, store, _runtime) = store_with_runtime().await;

    let err = store
        .stage_pending_thread_metadata(
            ThreadId::new(),
            ThreadMetadataPatch {
                rollout_path: Some(home.path().join("wrong-rollout.jsonl")),
                ..Default::default()
            },
        )
        .await
        .expect_err("reject host-owned rollout path");
    assert!(matches!(
        err,
        ThreadStoreError::InvalidRequest { message }
            if message == "pending thread metadata cannot set rollout_path"
    ));
}

async fn store_with_runtime() -> (
    TempDir,
    Arc<LocalThreadStore>,
    Arc<codex_state::StateRuntime>,
) {
    let home = TempDir::new().expect("temp dir");
    let config = test_config(home.path());
    let runtime = codex_state::StateRuntime::init(
        config.sqlite.clone(),
        config.default_model_provider_id.clone(),
    )
    .await
    .expect("state db should initialize");
    let store = Arc::new(LocalThreadStore::new(config, Some(runtime.clone())));
    (home, store, runtime)
}

async fn assert_pending_metadata_consumed(store: &LocalThreadStore, thread_id: ThreadId) {
    store
        .stage_pending_thread_metadata(thread_id, staged_model_patch())
        .await
        .expect("restage metadata after consumption");
    store
        .remove_pending_thread_metadata(thread_id)
        .await
        .expect("remove restaged metadata");
}

fn staged_model_patch() -> ThreadMetadataPatch {
    ThreadMetadataPatch {
        model: Some("staged-model".to_string()),
        ..Default::default()
    }
}

fn create_thread_params(thread_id: ThreadId) -> CreateThreadParams {
    CreateThreadParams {
        session_id: thread_id.into(),
        thread_id,
        extra_config: None,
        forked_from_id: None,
        parent_thread_id: None,
        source: SessionSource::Exec,
        thread_source: None,
        originator: "test_originator".to_string(),
        base_instructions: BaseInstructions::default(),
        dynamic_tools: Vec::new(),
        selected_capability_roots: Vec::new(),
        multi_agent_version: None,
        history_mode: ThreadHistoryMode::Legacy,
        history_base: None,
        subagent_history_start_ordinal: None,
        initial_window_id: uuid::Uuid::now_v7().to_string(),
        metadata: ThreadPersistenceMetadata {
            cwd: Some(std::env::current_dir().expect("cwd")),
            model_provider: "test-provider".to_string(),
            memory_mode: ThreadMemoryMode::Enabled,
        },
    }
}

fn user_message_item(message: &str) -> RolloutItem {
    RolloutItem::EventMsg(EventMsg::UserMessage(UserMessageEvent {
        client_id: None,
        message: message.to_string(),
        images: None,
        local_images: Vec::new(),
        text_elements: Vec::new(),
        ..Default::default()
    }))
}

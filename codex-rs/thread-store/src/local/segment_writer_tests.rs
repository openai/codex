use std::sync::Arc;

use codex_protocol::ThreadId;
use codex_protocol::models::BaseInstructions;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::RolloutItem;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::ThreadMemoryMode;
use codex_protocol::protocol::UserMessageEvent;
use codex_rollout::RolloutRecorder;
use tempfile::TempDir;

use super::*;
use crate::CreateThreadParams;
use crate::DeleteThreadParams;
use crate::LiveThread;
use crate::ResumeThreadParams;
use crate::ThreadPersistenceMetadata;
use crate::ThreadStore;
use crate::local::LocalThreadStore;
use crate::local::RotationTestHook;
use crate::local::test_support::test_config;
use crate::local::test_support::write_session_file;

#[tokio::test]
async fn rotation_keeps_the_canonical_thread_active() {
    let home = TempDir::new().expect("temp dir");
    let config = test_config(home.path());
    let runtime = codex_state::StateRuntime::init(
        config.sqlite_home.clone(),
        config.default_model_provider_id.clone(),
    )
    .await
    .expect("state db should initialize");
    let store = Arc::new(LocalThreadStore::new(config, Some(runtime.clone())));
    let thread_id = ThreadId::default();
    let live_thread = create_live_thread(store.clone(), thread_id).await;
    live_thread
        .append_items(&[user_message_item("before rotation")])
        .await
        .expect("append before rotation");
    let canonical_path = store
        .live_rollout_path(thread_id)
        .await
        .expect("live rollout path");

    let reference = store
        .rotate_thread_segment(thread_id, rotation_params())
        .await
        .expect("rotate thread segment");

    assert!(canonical_path.exists());
    assert!(reference.rollout_path.exists());
    assert!(
        reference.rollout_path.starts_with(
            home.path()
                .join(codex_rollout::ROTATED_ROLLOUT_SEGMENTS_SUBDIR)
                .join(thread_id.to_string())
        )
    );
    assert!(
        !home
            .path()
            .join(codex_rollout::ARCHIVED_SESSIONS_SUBDIR)
            .join(canonical_path.file_name().expect("live rollout file name"))
            .exists()
    );
    let metadata = runtime
        .get_thread(thread_id)
        .await
        .expect("sqlite metadata read")
        .expect("sqlite metadata");
    assert_eq!(metadata.rollout_path, canonical_path);
    assert_eq!(metadata.archived_at, None);
}

#[tokio::test]
async fn deletion_collects_segments_after_the_last_reference() {
    let home = TempDir::new().expect("temp dir");
    let store = Arc::new(LocalThreadStore::new(
        test_config(home.path()),
        /*state_db*/ None,
    ));
    let parent_thread_id = ThreadId::default();
    let parent = create_live_thread(store.clone(), parent_thread_id).await;
    parent
        .append_items(&[user_message_item("parent history")])
        .await
        .expect("append parent history");
    let reference = store
        .rotate_thread_segment(parent_thread_id, rotation_params())
        .await
        .expect("rotate parent segment");
    let immutable_path = reference.rollout_path.clone();

    let child_thread_id = ThreadId::default();
    let child = create_live_thread(store.clone(), child_thread_id).await;
    child
        .append_items(&[RolloutItem::RolloutReference(reference)])
        .await
        .expect("persist child reference");

    store
        .delete_thread(DeleteThreadParams {
            thread_id: parent_thread_id,
        })
        .await
        .expect("delete parent thread");
    assert!(immutable_path.exists());

    store
        .delete_thread(DeleteThreadParams {
            thread_id: child_thread_id,
        })
        .await
        .expect("delete child thread");
    assert!(!immutable_path.exists());
}

#[tokio::test]
async fn failed_rotation_before_staging_keeps_the_writer_open() {
    let home = TempDir::new().expect("temp dir");
    let store = Arc::new(LocalThreadStore::new(
        test_config(home.path()),
        /*state_db*/ None,
    ));
    let thread_id = ThreadId::default();
    let live_thread = create_live_thread(store.clone(), thread_id).await;
    live_thread
        .append_items(&[user_message_item("before failed rotation")])
        .await
        .expect("append before failed rotation");
    tokio::fs::write(
        home.path()
            .join(codex_rollout::ROTATED_ROLLOUT_SEGMENTS_SUBDIR),
        "block directory creation",
    )
    .await
    .expect("create rotation blocker");

    store
        .rotate_thread_segment(thread_id, rotation_params())
        .await
        .expect_err("rotation should fail before replacing the live recorder");
    live_thread
        .append_items(&[user_message_item("after failed rotation")])
        .await
        .expect("live recorder should remain writable");
    let rollout_path = store
        .live_rollout_path(thread_id)
        .await
        .expect("live rollout path");
    assert_rollout_contains_message(rollout_path.as_path(), "after failed rotation").await;
}

#[tokio::test]
async fn legacy_segment_without_id_is_sealed_at_an_immutable_path() {
    let home = TempDir::new().expect("temp dir");
    let store = Arc::new(LocalThreadStore::new(
        test_config(home.path()),
        /*state_db*/ None,
    ));
    let uuid = uuid::Uuid::from_u128(401);
    let thread_id = ThreadId::from_string(&uuid.to_string()).expect("thread id");
    let canonical_path =
        write_session_file(home.path(), "2025-01-03T12-00-00", uuid).expect("legacy rollout");
    let live_thread = LiveThread::resume(
        store.clone(),
        ResumeThreadParams {
            thread_id,
            rollout_path: Some(canonical_path.clone()),
            history: None,
            include_archived: true,
            metadata: thread_metadata(),
        },
    )
    .await
    .expect("resume legacy rollout");

    let reference = store
        .rotate_thread_segment(thread_id, rotation_params())
        .await
        .expect("seal legacy segment");
    assert_eq!(reference.segment_id, None);
    assert_ne!(reference.rollout_path, canonical_path);
    assert!(
        reference.rollout_path.starts_with(
            home.path()
                .join(codex_rollout::ROTATED_ROLLOUT_SEGMENTS_SUBDIR)
                .join(thread_id.to_string())
                .join("initial")
        )
    );

    live_thread
        .append_items(&[user_message_item("new live segment")])
        .await
        .expect("append to successor segment");
    let immutable_json = rollout_json(reference.rollout_path.as_path()).await;
    assert!(!immutable_json.contains("new live segment"));
}

#[tokio::test]
async fn append_waits_for_atomic_rotation() {
    let home = TempDir::new().expect("temp dir");
    let store = Arc::new(LocalThreadStore::new(
        test_config(home.path()),
        /*state_db*/ None,
    ));
    let thread_id = ThreadId::default();
    let live_thread = create_live_thread(store.clone(), thread_id).await;
    live_thread
        .append_items(&[user_message_item("before rotation")])
        .await
        .expect("append before rotation");
    let canonical_path = store
        .live_rollout_path(thread_id)
        .await
        .expect("live rollout path");

    let reached = Arc::new(tokio::sync::Barrier::new(2));
    let release = Arc::new(tokio::sync::Barrier::new(2));
    *store.rotation_test_hook.lock().await = Some(RotationTestHook {
        reached: Arc::clone(&reached),
        release: Arc::clone(&release),
        fail_before_install: false,
    });
    let rotation_store = Arc::clone(&store);
    let rotation = tokio::spawn(async move {
        rotation_store
            .rotate_thread_segment(thread_id, rotation_params())
            .await
    });
    reached.wait().await;
    assert!(canonical_path.exists());
    assert_rollout_contains_message(canonical_path.as_path(), "before rotation").await;

    let append_thread = live_thread.clone();
    let mut append = tokio::spawn(async move {
        append_thread
            .append_items(&[user_message_item("after rotation")])
            .await
    });
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(50), &mut append)
            .await
            .is_err()
    );
    release.wait().await;
    let reference = rotation
        .await
        .expect("rotation task")
        .expect("rotate segment");
    append
        .await
        .expect("append task")
        .expect("append after rotation");

    assert_rollout_contains_message(canonical_path.as_path(), "after rotation").await;
    let immutable_json = rollout_json(reference.rollout_path.as_path()).await;
    assert!(immutable_json.contains("before rotation"));
    assert!(!immutable_json.contains("after rotation"));
}

#[tokio::test]
async fn failed_atomic_install_restores_the_previous_writer() {
    let home = TempDir::new().expect("temp dir");
    let store = Arc::new(LocalThreadStore::new(
        test_config(home.path()),
        /*state_db*/ None,
    ));
    let thread_id = ThreadId::default();
    let live_thread = create_live_thread(store.clone(), thread_id).await;
    live_thread
        .append_items(&[user_message_item("before failed install")])
        .await
        .expect("append before failed install");

    let reached = Arc::new(tokio::sync::Barrier::new(2));
    let release = Arc::new(tokio::sync::Barrier::new(2));
    *store.rotation_test_hook.lock().await = Some(RotationTestHook {
        reached: Arc::clone(&reached),
        release: Arc::clone(&release),
        fail_before_install: true,
    });
    let rotation_store = Arc::clone(&store);
    let rotation = tokio::spawn(async move {
        rotation_store
            .rotate_thread_segment(thread_id, rotation_params())
            .await
    });
    reached.wait().await;
    release.wait().await;
    rotation
        .await
        .expect("rotation task")
        .expect_err("injected install failure");

    live_thread
        .append_items(&[user_message_item("after failed install")])
        .await
        .expect("restored writer should accept appends");
    let rollout_path = store
        .live_rollout_path(thread_id)
        .await
        .expect("live rollout path");
    assert_rollout_contains_message(rollout_path.as_path(), "before failed install").await;
    assert_rollout_contains_message(rollout_path.as_path(), "after failed install").await;
}

#[tokio::test]
async fn rotation_recovers_unreferenced_immutable_left_by_crash() {
    let home = TempDir::new().expect("temp dir");
    let store = Arc::new(LocalThreadStore::new(
        test_config(home.path()),
        /*state_db*/ None,
    ));
    let thread_id = ThreadId::default();
    let live_thread = create_live_thread(store.clone(), thread_id).await;
    live_thread
        .append_items(&[user_message_item("before interrupted rotation")])
        .await
        .expect("append before interrupted rotation");
    let canonical_path = store
        .live_rollout_path(thread_id)
        .await
        .expect("live rollout path");
    let meta = codex_rollout::read_session_meta_line(canonical_path.as_path())
        .await
        .expect("read live rollout metadata");
    let immutable_path = rotated_segment_path(
        home.path(),
        thread_id,
        meta.meta.segment_id,
        canonical_path.as_path(),
    )
    .expect("immutable path");
    tokio::fs::create_dir_all(immutable_path.parent().expect("immutable parent"))
        .await
        .expect("create immutable parent");
    tokio::fs::write(immutable_path.as_path(), "orphan from interrupted rotation")
        .await
        .expect("write orphaned immutable segment");

    let reference = store
        .rotate_thread_segment(thread_id, rotation_params())
        .await
        .expect("retry rotation after interrupted publication");

    assert_eq!(reference.rollout_path, immutable_path);
    assert_rollout_contains_message(
        reference.rollout_path.as_path(),
        "before interrupted rotation",
    )
    .await;
}

#[tokio::test]
async fn snapshot_seals_mark_non_consuming_reference_boundaries() {
    let home = TempDir::new().expect("temp dir");
    let store = Arc::new(LocalThreadStore::new(
        test_config(home.path()),
        /*state_db*/ None,
    ));
    let thread_id = ThreadId::default();
    let live_thread = create_live_thread(store.clone(), thread_id).await;
    live_thread
        .append_items(&[user_message_item("history retained across snapshots")])
        .await
        .expect("append before snapshots");
    let canonical_path = store
        .live_rollout_path(thread_id)
        .await
        .expect("live rollout path");

    for _ in 0..4 {
        let reference = store
            .seal_thread_segment(thread_id, canonical_path.clone(), rotation_params())
            .await
            .expect("seal snapshot segment");
        assert_eq!(reference.nth_user_message, Some(usize::MAX));
        let (successor_items, _, _) = RolloutRecorder::load_rollout_items(&canonical_path)
            .await
            .expect("load snapshot successor");
        assert!(successor_items.iter().any(|item| {
            matches!(
                item,
                RolloutItem::RolloutReference(successor_reference)
                    if successor_reference.nth_user_message == Some(usize::MAX)
            )
        }));
    }
}

async fn create_live_thread(store: Arc<LocalThreadStore>, thread_id: ThreadId) -> LiveThread {
    LiveThread::create(store, create_thread_params(thread_id))
        .await
        .expect("create live thread")
}

fn create_thread_params(thread_id: ThreadId) -> CreateThreadParams {
    CreateThreadParams {
        thread_id,
        extra_config: None,
        forked_from_id: None,
        parent_thread_id: None,
        source: SessionSource::Exec,
        thread_source: None,
        base_instructions: BaseInstructions::default(),
        dynamic_tools: Vec::new(),
        multi_agent_version: None,
        metadata: thread_metadata(),
    }
}

fn thread_metadata() -> ThreadPersistenceMetadata {
    ThreadPersistenceMetadata {
        cwd: Some(std::env::current_dir().expect("cwd")),
        model_provider: "test-provider".to_string(),
        memory_mode: ThreadMemoryMode::Enabled,
    }
}

fn rotation_params() -> RotateThreadSegmentParams {
    RotateThreadSegmentParams {
        initial_items: Vec::new(),
        previous_segment_reference_depth: 1,
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

async fn assert_rollout_contains_message(path: &std::path::Path, expected: &str) {
    let (items, _, _) = RolloutRecorder::load_rollout_items(path)
        .await
        .expect("load rollout items");
    assert!(items.iter().any(|item| {
        matches!(
            item,
            RolloutItem::EventMsg(EventMsg::UserMessage(event)) if event.message == expected
        )
    }));
}

async fn rollout_json(path: &std::path::Path) -> String {
    let (items, _, _) = RolloutRecorder::load_rollout_items(path)
        .await
        .expect("load rollout items");
    serde_json::to_string(&items).expect("serialize rollout items")
}

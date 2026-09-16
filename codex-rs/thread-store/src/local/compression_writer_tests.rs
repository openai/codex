//! Exercises compression through local-store ownership, including detached recorder work.

use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use std::time::SystemTime;

use codex_protocol::ThreadId;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::ThreadMemoryMode;
use codex_protocol::protocol::UserMessageEvent;
use codex_rollout::RolloutItem;
use codex_rollout::RolloutRecorder;
use codex_rollout::WriterLockCoordinator;
use pretty_assertions::assert_eq;
use serde_json::json;
use tempfile::TempDir;
use uuid::Uuid;

use super::LocalThreadStore;
use super::test_support::test_config;
use super::test_support::write_session_file;
use crate::ResumeThreadParams;
use crate::ThreadMetadataPatch;
use crate::ThreadPersistenceMetadata;
use crate::ThreadStore;
use crate::UpdateThreadMetadataParams;

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

fn fixture() -> TestResult<(TempDir, ThreadId, PathBuf)> {
    let home = TempDir::new()?;
    let id = Uuid::new_v4();
    let path = write_session_file(home.path(), "2025-01-03T12-00-00", id)?;
    Ok((home, ThreadId::from_string(&id.to_string())?, path))
}

fn age(path: &Path) -> TestResult<()> {
    fs::OpenOptions::new().write(true).open(path)?.set_times(
        fs::FileTimes::new().set_modified(SystemTime::now() - Duration::from_secs(8 * 86400)),
    )?;
    Ok(())
}

fn resume(thread_id: ThreadId, path: &Path, home: &Path) -> ResumeThreadParams {
    ResumeThreadParams {
        thread_id,
        rollout_path: Some(path.to_path_buf()),
        history: None,
        include_archived: true,
        metadata: ThreadPersistenceMetadata {
            cwd: Some(home.to_path_buf()),
            model_provider: "test-provider".into(),
            memory_mode: ThreadMemoryMode::Enabled,
        },
    }
}

fn message() -> RolloutItem {
    RolloutItem::EventMsg(EventMsg::UserMessage(UserMessageEvent {
        message: "acknowledged write".into(),
        ..Default::default()
    }))
}

async fn wait_until(ready: impl Fn() -> bool) {
    tokio::time::timeout(Duration::from_secs(10), async {
        while !ready() {
            tokio::time::sleep(Duration::from_millis(1)).await;
        }
    })
    .await
    .expect("background file work completes");
}

async fn compress(home: &Path) -> TestResult<()> {
    let marker = home.join(".tmp/rollout-compression.lock");
    if marker.exists() {
        fs::remove_file(&marker)?;
    }
    codex_rollout::spawn_rollout_compression_worker(
        home.to_path_buf(),
        codex_rollout::RolloutCompressionTrigger::Startup,
    );
    // The marker proves startup; the maintenance lock proves every blocking job finished.
    wait_until(|| {
        marker.exists()
            && codex_rollout::try_acquire_rollout_maintenance_lock(home)
                .unwrap()
                .is_some()
    })
    .await;
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn idle_recorder_retains_stable_thread_ownership_after_store_drop() -> TestResult<()> {
    let (home, thread_id, old_path) = fixture()?;
    let store = LocalThreadStore::new(test_config(home.path()), /*state_db*/ None);
    let path = old_path.with_file_name(format!(
        "rollout-2025-01-03T12-00-00-{thread_id}_{}.jsonl",
        Uuid::new_v4()
    ));
    fs::rename(old_path, &path)?;
    let cold = write_session_file(home.path(), "2025-01-03T12-00-01", Uuid::new_v4())?;
    age(&cold)?;
    store
        .resume_thread(resume(thread_id, &path, home.path()))
        .await?;
    let recorder = store
        .live_recorders
        .lock()
        .await
        .get(&thread_id)
        .unwrap()
        .recorder
        .clone();
    drop(store);
    age(&path)?;
    let (mut expected, _, _) = RolloutRecorder::load_rollout_items(&path).await?;
    compress(home.path()).await?;
    assert!(path.exists());
    assert!(!path.with_extension("jsonl.zst").exists());
    assert!(cold.with_extension("jsonl.zst").exists());

    recorder.record_canonical_items(&[message()]).await?;
    drop(recorder);
    let locks = Arc::new(WriterLockCoordinator::new(home.path()));
    // No yield since enqueueing: only the background task can own the pending write now.
    assert!(
        matches!(locks.acquire(thread_id), Err(err) if err.kind() == std::io::ErrorKind::WouldBlock)
    );
    wait_until(|| locks.acquire(thread_id).is_ok()).await;
    age(&path)?;
    compress(home.path()).await?;
    assert!(!path.exists());
    expected.push(message());
    let (actual, _, _) = RolloutRecorder::load_rollout_items(&path).await?;
    assert_eq!(json!(actual), json!(expected));
    Ok(())
}

#[tokio::test]
async fn metadata_updates_share_ownership_and_resume_compressed_rollouts() -> TestResult<()> {
    let (home, thread_id, path) = fixture()?;
    let config = test_config(home.path());
    let db = codex_state::StateRuntime::init(
        config.sqlite.clone(),
        config.default_model_provider_id.clone(),
    )
    .await?;
    let owner = LocalThreadStore::new(config.clone(), Some(db.clone()));
    owner
        .resume_thread(resume(thread_id, &path, home.path()))
        .await?;
    let competitor = LocalThreadStore::new(config, Some(db.clone()));
    let mut patch = UpdateThreadMetadataParams {
        thread_id,
        include_archived: true,
        patch: ThreadMetadataPatch {
            memory_mode: Some(ThreadMemoryMode::Enabled),
            ..Default::default()
        },
    };
    owner.update_thread_metadata(patch.clone()).await?;
    let original = fs::read(&path)?;
    let indexed_mode = db.get_thread_memory_mode(thread_id).await?;
    patch.patch.memory_mode = Some(ThreadMemoryMode::Disabled);
    let error = competitor
        .update_thread_metadata(patch.clone())
        .await
        .unwrap_err();
    assert!(matches!(error, crate::ThreadStoreError::Conflict { .. }));
    assert_eq!(fs::read(&path)?, original);
    assert_eq!(db.get_thread_memory_mode(thread_id).await?, indexed_mode);
    owner.update_thread_metadata(patch.clone()).await?;
    owner.shutdown_thread(thread_id).await?;
    let (items, _, _) = RolloutRecorder::load_rollout_items(&path).await?;
    let mut expected = codex_rollout::read_session_meta_line(&path).await?;
    expected.meta.memory_mode = Some("disabled".into());
    expected.git = None;
    assert_eq!(
        json!(items.last()),
        json!(Some(RolloutItem::SessionMeta(expected.clone())))
    );

    age(&path)?;
    compress(home.path()).await?;
    patch.patch.memory_mode = Some(ThreadMemoryMode::Enabled);
    competitor.update_thread_metadata(patch).await?;
    expected.meta.memory_mode = Some("enabled".into());
    let (items, _, _) = RolloutRecorder::load_rollout_items(&path).await?;
    assert_eq!(
        json!(items.last()),
        json!(Some(RolloutItem::SessionMeta(expected)))
    );
    Ok(())
}

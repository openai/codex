#![allow(warnings, clippy::all)]

use super::*;
use crate::RolloutItem;
use crate::RolloutLine;
use codex_protocol::protocol::SessionMeta;
use codex_protocol::protocol::SessionMetaLine;
use codex_protocol::protocol::SessionSource;
use pretty_assertions::assert_eq;
use std::collections::HashMap;
use std::collections::HashSet;
use tempfile::TempDir;
fn write_index(path: &Path, lines: &[SessionIndexEntry]) -> std::io::Result<()> {
    let mut out = String::new();
    for entry in lines {
        out.push_str(&serde_json::to_string(entry).unwrap());
        out.push('\n');
    }
    std::fs::write(path, out)
}

fn write_rollout_with_metadata(path: &Path, thread_id: ThreadId) -> std::io::Result<()> {
    write_rollout_with_source_and_provider(path, thread_id, SessionSource::Cli, "test-provider")
}

fn write_rollout_with_source_and_provider(
    path: &Path,
    thread_id: ThreadId,
    source: SessionSource,
    model_provider: &str,
) -> std::io::Result<()> {
    let timestamp = "2024-01-01T00-00-00Z".to_string();
    let line = RolloutLine {
        timestamp: timestamp.clone(),
        ordinal: None,
        item: RolloutItem::SessionMeta(SessionMetaLine {
            meta: SessionMeta {
                creator_user_id: None,
                creator_account_id: None,
                session_id: thread_id.into(),
                id: thread_id,
                forked_from_id: None,
                forked_from_ordinal_exclusive: None,
                parent_thread_id: None,
                timestamp,
                cwd: ".".into(),
                runtime_workspace_roots: None,
                originator: "test_originator".into(),
                cli_version: "test_version".into(),
                source,
                thread_source: None,
                agent_path: None,
                agent_nickname: None,
                agent_role: None,
                model_provider: Some(model_provider.to_string()),
                base_instructions: None,
                dynamic_tools: None,
                selected_capability_roots: Vec::new(),
                memory_mode: None,
                history_mode: Default::default(),
                history_base: None,
                subagent_history_start_ordinal: None,
                multi_agent_version: None,
                context_window: None,
            },
            git: None,
        }),
    };
    let body = serde_json::to_string(&line).map_err(std::io::Error::other)?;
    std::fs::write(path, format!("{body}\n"))
}

#[test]
fn find_thread_id_by_name_prefers_latest_entry() -> std::io::Result<()> {
    let temp = TempDir::new()?;
    let path = session_index_path(temp.path());
    let id1 = ThreadId::new();
    let id2 = ThreadId::new();
    let lines = vec![
        SessionIndexEntry {
            id: id1,
            thread_name: "same".to_string(),
            updated_at: "2024-01-01T00:00:00Z".to_string(),
        },
        SessionIndexEntry {
            id: id2,
            thread_name: "same".to_string(),
            updated_at: "2024-01-02T00:00:00Z".to_string(),
        },
    ];
    write_index(&path, &lines)?;

    let found = scan_index_from_end(&path, |entry| entry.thread_name == "same")?;
    assert_eq!(found.map(|entry| entry.id), Some(id2));
    Ok(())
}

#[tokio::test]
async fn find_thread_meta_by_name_str_skips_newest_entry_without_rollout() -> std::io::Result<()> {
    // A newer unsaved name entry should not shadow an older persisted rollout with the same name.
    let temp = TempDir::new()?;
    let path = session_index_path(temp.path());
    let saved_id = ThreadId::new();
    let unsaved_id = ThreadId::new();
    let saved_rollout_path = temp
        .path()
        .join("sessions/2024/01/01")
        .join(format!("rollout-2024-01-01T00-00-00-{saved_id}.jsonl"));
    std::fs::create_dir_all(saved_rollout_path.parent().expect("rollout parent"))?;
    write_rollout_with_metadata(&saved_rollout_path, saved_id)?;
    let lines = vec![
        SessionIndexEntry {
            id: saved_id,
            thread_name: "same".to_string(),
            updated_at: "2024-01-01T00:00:00Z".to_string(),
        },
        SessionIndexEntry {
            id: unsaved_id,
            thread_name: "same".to_string(),
            updated_at: "2024-01-02T00:00:00Z".to_string(),
        },
    ];
    write_index(&path, &lines)?;

    let found = find_thread_meta_by_name_str(temp.path(), "same", /*state_db_ctx*/ None).await?;

    assert_eq!(
        found.map(|(path, session_meta)| (path, session_meta.meta.id)),
        Some((saved_rollout_path, saved_id))
    );
    Ok(())
}

#[tokio::test]
async fn find_thread_meta_by_name_str_skips_partial_rollout() -> std::io::Result<()> {
    let temp = TempDir::new()?;
    let path = session_index_path(temp.path());
    let saved_id = ThreadId::new();
    let partial_id = ThreadId::new();
    let rollout_dir = temp.path().join("sessions/2024/01/01");
    let saved_rollout_path =
        rollout_dir.join(format!("rollout-2024-01-01T00-00-00-{saved_id}.jsonl"));
    let partial_rollout_path =
        rollout_dir.join(format!("rollout-2024-01-01T00-00-01-{partial_id}.jsonl"));
    std::fs::create_dir_all(&rollout_dir)?;
    write_rollout_with_metadata(&saved_rollout_path, saved_id)?;
    std::fs::write(&partial_rollout_path, "")?;
    let lines = vec![
        SessionIndexEntry {
            id: saved_id,
            thread_name: "same".to_string(),
            updated_at: "2024-01-01T00:00:00Z".to_string(),
        },
        SessionIndexEntry {
            id: partial_id,
            thread_name: "same".to_string(),
            updated_at: "2024-01-02T00:00:00Z".to_string(),
        },
    ];
    write_index(&path, &lines)?;

    let found = find_thread_meta_by_name_str(temp.path(), "same", /*state_db_ctx*/ None).await?;

    assert_eq!(found.map(|(path, _)| path), Some(saved_rollout_path));
    Ok(())
}

#[tokio::test]
async fn find_thread_meta_by_name_str_ignores_historical_name_after_rename() -> std::io::Result<()>
{
    let temp = TempDir::new()?;
    let path = session_index_path(temp.path());
    let renamed_id = ThreadId::new();
    let current_id = ThreadId::new();
    let current_rollout_path = temp
        .path()
        .join("sessions/2024/01/01")
        .join(format!("rollout-2024-01-01T00-00-00-{current_id}.jsonl"));
    std::fs::create_dir_all(current_rollout_path.parent().expect("rollout parent"))?;
    write_rollout_with_metadata(&current_rollout_path, current_id)?;
    let lines = vec![
        SessionIndexEntry {
            id: renamed_id,
            thread_name: "same".to_string(),
            updated_at: "2024-01-01T00:00:00Z".to_string(),
        },
        SessionIndexEntry {
            id: current_id,
            thread_name: "same".to_string(),
            updated_at: "2024-01-02T00:00:00Z".to_string(),
        },
        SessionIndexEntry {
            id: renamed_id,
            thread_name: "different".to_string(),
            updated_at: "2024-01-03T00:00:00Z".to_string(),
        },
    ];
    write_index(&path, &lines)?;

    let found = find_thread_meta_by_name_str(temp.path(), "same", /*state_db_ctx*/ None).await?;

    assert_eq!(found.map(|(path, _)| path), Some(current_rollout_path));
    Ok(())
}

#[tokio::test]
async fn find_thread_meta_candidates_filter_metadata_before_ranking() -> std::io::Result<()> {
    let temp = TempDir::new()?;
    let index_path = session_index_path(temp.path());
    let allowed_id = ThreadId::new();
    let other_id = ThreadId::new();
    let noninteractive_id = ThreadId::new();
    let rollout_dir = temp.path().join("sessions/2024/01/01");
    let allowed_path = rollout_dir.join(format!("rollout-2024-01-01T00-00-00-{allowed_id}.jsonl"));
    let other_path = rollout_dir.join(format!("rollout-2024-01-01T00-00-01-{other_id}.jsonl"));
    let noninteractive_path = rollout_dir.join(format!(
        "rollout-2024-01-01T00-00-02-{noninteractive_id}.jsonl"
    ));
    std::fs::create_dir_all(&rollout_dir)?;
    write_rollout_with_metadata(&allowed_path, allowed_id)?;
    write_rollout_with_source_and_provider(
        &other_path,
        other_id,
        SessionSource::Cli,
        "other-provider",
    )?;
    write_rollout_with_source_and_provider(
        &noninteractive_path,
        noninteractive_id,
        SessionSource::Exec,
        "test-provider",
    )?;
    write_index(
        &index_path,
        &[
            SessionIndexEntry {
                id: allowed_id,
                thread_name: "same".to_string(),
                updated_at: "2024-01-01T00:00:00Z".to_string(),
            },
            SessionIndexEntry {
                id: other_id,
                thread_name: "same".to_string(),
                updated_at: "2024-01-02T00:00:00Z".to_string(),
            },
            SessionIndexEntry {
                id: noninteractive_id,
                thread_name: "same".to_string(),
                updated_at: "2024-01-03T00:00:00Z".to_string(),
            },
        ],
    )?;
    std::fs::OpenOptions::new()
        .append(true)
        .open(&allowed_path)?
        .set_times(std::fs::FileTimes::new().set_modified(std::time::UNIX_EPOCH))?;
    let allowed_model_providers = vec!["test-provider".to_string()];

    let found = find_thread_meta_candidates_by_name_str(
        temp.path(),
        "same",
        /*state_db_ctx*/ None,
        &[SessionSource::Cli],
        &allowed_model_providers,
    )
    .await?;

    assert_eq!(
        found
            .into_iter()
            .map(|(path, session_meta)| (path, session_meta.meta.id))
            .collect::<Vec<_>>(),
        vec![(allowed_path, allowed_id)],
    );
    Ok(())
}

#[test]
fn find_thread_name_by_id_prefers_latest_entry() -> std::io::Result<()> {
    let temp = TempDir::new()?;
    let path = session_index_path(temp.path());
    let id = ThreadId::new();
    let lines = vec![
        SessionIndexEntry {
            id,
            thread_name: "first".to_string(),
            updated_at: "2024-01-01T00:00:00Z".to_string(),
        },
        SessionIndexEntry {
            id,
            thread_name: "second".to_string(),
            updated_at: "2024-01-02T00:00:00Z".to_string(),
        },
    ];
    write_index(&path, &lines)?;

    let found = scan_index_from_end_by_id(&path, &id)?;
    assert_eq!(
        found.map(|entry| entry.thread_name),
        Some("second".to_string())
    );
    Ok(())
}

#[test]
fn scan_index_returns_none_when_entry_missing() -> std::io::Result<()> {
    let temp = TempDir::new()?;
    let path = session_index_path(temp.path());
    let id = ThreadId::new();
    let lines = vec![SessionIndexEntry {
        id,
        thread_name: "present".to_string(),
        updated_at: "2024-01-01T00:00:00Z".to_string(),
    }];
    write_index(&path, &lines)?;

    let missing_name = scan_index_from_end(&path, |entry| entry.thread_name == "missing")?;
    assert_eq!(missing_name, None);

    let missing_id = scan_index_from_end_by_id(&path, &ThreadId::new())?;
    assert_eq!(missing_id, None);
    Ok(())
}

#[tokio::test]
async fn reverse_lookup_accepts_valid_eof_json_and_skips_invalid() -> std::io::Result<()> {
    let temp = TempDir::new()?;
    let path = session_index_path(temp.path());
    let expected = SessionIndexEntry {
        id: ThreadId::new(),
        thread_name: "expected".to_string(),
        updated_at: "2024-01-01T00:00:00Z".to_string(),
    };
    let unterminated = SessionIndexEntry {
        id: ThreadId::new(),
        thread_name: "unterminated".to_string(),
        updated_at: "2024-01-02T00:00:00Z".to_string(),
    };
    std::fs::write(
        &path,
        format!(
            "{}\nnot-json\n{}",
            serde_json::to_string(&expected)?,
            serde_json::to_string(&unterminated)?
        ),
    )?;

    assert_eq!(
        find_thread_name_by_id(temp.path(), &unterminated.id).await?,
        Some("unterminated".to_string())
    );
    assert_eq!(
        find_thread_name_by_id(temp.path(), &expected.id).await?,
        Some("expected".to_string())
    );
    Ok(())
}

#[tokio::test]
async fn append_and_remove_thread_names_preserves_other_entries() -> std::io::Result<()> {
    let temp = TempDir::new()?;
    let removed_id = ThreadId::new();
    let retained = SessionIndexEntry {
        id: ThreadId::new(),
        thread_name: "retained".to_string(),
        updated_at: "2024-01-01T00:00:00Z".to_string(),
    };
    for entry in [
        SessionIndexEntry {
            id: removed_id,
            thread_name: "original".to_string(),
            ..retained.clone()
        },
        retained.clone(),
        SessionIndexEntry {
            id: removed_id,
            thread_name: "renamed".to_string(),
            ..retained.clone()
        },
    ] {
        append_session_index_entry(temp.path(), entry).await?;
    }
    assert_eq!(
        find_thread_name_by_id(temp.path(), &removed_id).await?,
        Some("renamed".to_string()),
    );

    remove_thread_name_entries(temp.path(), removed_id).await?;
    assert_eq!(
        std::fs::read_to_string(session_index_path(temp.path()))?,
        format!("{}\n", serde_json::to_string(&retained)?),
    );
    Ok(())
}

#[tokio::test]
async fn find_thread_names_by_ids_prefers_latest_entry() -> std::io::Result<()> {
    let temp = TempDir::new()?;
    let path = session_index_path(temp.path());
    let id1 = ThreadId::new();
    let id2 = ThreadId::new();
    let lines = vec![
        SessionIndexEntry {
            id: id1,
            thread_name: "first".to_string(),
            updated_at: "2024-01-01T00:00:00Z".to_string(),
        },
        SessionIndexEntry {
            id: id2,
            thread_name: "other".to_string(),
            updated_at: "2024-01-01T00:00:00Z".to_string(),
        },
        SessionIndexEntry {
            id: id1,
            thread_name: "latest".to_string(),
            updated_at: "2024-01-02T00:00:00Z".to_string(),
        },
    ];
    write_index(&path, &lines)?;

    let mut ids = HashSet::new();
    ids.insert(id1);
    ids.insert(id2);

    let mut expected = HashMap::new();
    expected.insert(id1, "latest".to_string());
    expected.insert(id2, "other".to_string());

    let found = find_thread_names_by_ids(temp.path(), &ids).await?;
    assert_eq!(found, expected);
    Ok(())
}

#[tokio::test]
async fn find_thread_names_by_ids_skips_unusable_names_and_reads_across_chunks()
-> std::io::Result<()> {
    let temp = TempDir::new()?;
    let path = session_index_path(temp.path());
    let renamed = ThreadId::new();
    let older = ThreadId::new();
    let missing = ThreadId::new();
    let mut contents = String::new();
    for (id, thread_name) in [
        (renamed, "original".to_string()),
        (older, "  older name  ".to_string()),
        (ThreadId::new(), "unrelated".repeat(/*n*/ 10_000)),
        (renamed, "  最新 café  ".to_string()),
        (renamed, " \t ".to_string()),
    ] {
        let entry = SessionIndexEntry {
            id,
            thread_name,
            updated_at: "2024-01-01T00:00:00Z".to_string(),
        };
        contents.push_str(&serde_json::to_string(&entry)?);
        contents.push_str("\n\nnot json\n");
    }
    // A partial final write must not hide the latest complete name.
    contents.push_str("{\"id\":");
    std::fs::write(path, contents)?;

    for (ids, expected) in [
        (
            HashSet::from([renamed]),
            HashMap::from([(renamed, "最新 café".to_string())]),
        ),
        (
            HashSet::from([renamed, older]),
            HashMap::from([
                (renamed, "最新 café".to_string()),
                (older, "older name".to_string()),
            ]),
        ),
        (
            HashSet::from([renamed, older, missing]),
            HashMap::from([
                (renamed, "最新 café".to_string()),
                (older, "older name".to_string()),
            ]),
        ),
    ] {
        assert_eq!(find_thread_names_by_ids(temp.path(), &ids).await?, expected);
    }
    Ok(())
}

#[tokio::test]
async fn removal_preserves_other_names_and_malformed_lines() -> std::io::Result<()> {
    let temp = TempDir::new()?;
    let removed_id = ThreadId::new();
    let kept_id = ThreadId::new();
    append_thread_name(temp.path(), removed_id, "old").await?;
    append_thread_name(temp.path(), removed_id, "new").await?;
    append_thread_name(temp.path(), kept_id, "kept").await?;
    let path = session_index_path(temp.path());
    std::fs::OpenOptions::new()
        .append(true)
        .open(&path)?
        .write_all(b"malformed\n")?;

    remove_thread_name_entries(temp.path(), removed_id).await?;

    assert_eq!(
        find_thread_names_by_ids(temp.path(), &HashSet::from([removed_id, kept_id])).await?,
        HashMap::from([(kept_id, "kept".to_string())]),
    );
    assert!(
        std::fs::read_to_string(path)?
            .lines()
            .any(|line| line == "malformed")
    );
    Ok(())
}

#[tokio::test]
async fn cancelled_index_update_holds_lock_until_worker_finishes() -> std::io::Result<()> {
    let (reached_tx, reached_rx) = tokio::sync::oneshot::channel();
    let (resume_tx, resume_rx) = std::sync::mpsc::channel();
    let update = tokio::spawn(with_session_index_lock(&SESSION_INDEX_LOCK, move || {
        let _ = reached_tx.send(());
        // Dropping the sender on a test failure also releases the blocking worker.
        let _ = resume_rx.recv();
        Ok(())
    }));
    reached_rx.await.unwrap();
    update.abort();
    assert!(update.await.unwrap_err().is_cancelled());

    let lock_is_held = SESSION_INDEX_LOCK.try_lock().is_err();
    resume_tx.send(()).unwrap();
    with_session_index_lock(&SESSION_INDEX_LOCK, || Ok(())).await?;
    assert!(
        lock_is_held,
        "cancellation released the running worker's lock"
    );
    Ok(())
}

#[test]
fn cancelled_queued_index_update_preserves_the_next_write() -> std::io::Result<()> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .max_blocking_threads(1)
        .build()?;
    runtime.block_on(async {
        let home = TempDir::new()?;
        let path = home.path().join("thread-name");
        // A private lock ensures shared-process test runners cannot supply another owner's guard.
        let lock = Arc::new(tokio::sync::Mutex::new(()));

        let (reached_tx, reached_rx) = tokio::sync::oneshot::channel();
        let (resume_tx, resume_rx) = std::sync::mpsc::channel::<()>();
        let blocker = tokio::task::spawn_blocking(move || {
            let _ = reached_tx.send(());
            // Dropping the sender on a failure also lets runtime teardown finish.
            let _ = resume_rx.recv();
        });
        reached_rx.await.unwrap();

        let queued_path = path.clone();
        let mut update = Box::pin(with_session_index_lock(&lock, move || {
            std::fs::write(queued_path, "obsolete")
        }));
        let state = std::future::poll_fn(|cx| {
            std::task::Poll::Ready(std::future::Future::poll(update.as_mut(), cx))
        })
        .await;
        assert!(
            state.is_pending(),
            "the blocking pool should hold the update queued"
        );
        assert!(
            lock.try_lock().is_err(),
            "this update must own the lock before cancellation"
        );
        drop(update);
        let queued_update_holds_lock = lock.try_lock().is_err();

        drop(resume_tx);
        blocker.await.unwrap();
        let next_path = path.clone();
        with_session_index_lock(&lock, move || std::fs::write(next_path, "latest")).await?;
        assert_eq!(
            (queued_update_holds_lock, std::fs::read_to_string(path)?),
            (true, "latest".to_string())
        );
        Ok(())
    })
}

#[test]
fn scan_index_finds_latest_match_among_mixed_entries() -> std::io::Result<()> {
    let temp = TempDir::new()?;
    let path = session_index_path(temp.path());
    let id_target = ThreadId::new();
    let id_other = ThreadId::new();
    let expected = SessionIndexEntry {
        id: id_target,
        thread_name: "target".to_string(),
        updated_at: "2024-01-03T00:00:00Z".to_string(),
    };
    let expected_other = SessionIndexEntry {
        id: id_other,
        thread_name: "target".to_string(),
        updated_at: "2024-01-02T00:00:00Z".to_string(),
    };
    // Resolution is based on append order (scan from end), not updated_at.
    let lines = vec![
        SessionIndexEntry {
            id: id_target,
            thread_name: "target".to_string(),
            updated_at: "2024-01-01T00:00:00Z".to_string(),
        },
        expected_other.clone(),
        expected.clone(),
        SessionIndexEntry {
            id: ThreadId::new(),
            thread_name: "another".to_string(),
            updated_at: "2024-01-04T00:00:00Z".to_string(),
        },
    ];
    write_index(&path, &lines)?;

    let found_by_name = scan_index_from_end(&path, |entry| entry.thread_name == "target")?;
    assert_eq!(found_by_name, Some(expected.clone()));

    let found_by_id = scan_index_from_end_by_id(&path, &id_target)?;
    assert_eq!(found_by_id, Some(expected));

    let found_other_by_id = scan_index_from_end_by_id(&path, &id_other)?;
    assert_eq!(found_other_by_id, Some(expected_other));
    Ok(())
}

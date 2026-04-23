use chrono::Utc;
use codex_rollout::find_thread_path_by_id_str;

use super::LocalThreadStore;
use super::helpers::matching_rollout_file_name;
use super::helpers::scoped_rollout_path;
use crate::ArchiveThreadParams;
use crate::ThreadStoreError;
use crate::ThreadStoreResult;

pub(super) async fn archive_thread(
    store: &LocalThreadStore,
    params: ArchiveThreadParams,
) -> ThreadStoreResult<()> {
    let thread_id = params.thread_id;
    let rollout_path =
        find_thread_path_by_id_str(store.config.codex_home.as_path(), &thread_id.to_string())
            .await
            .map_err(|err| ThreadStoreError::InvalidRequest {
                message: format!("failed to locate thread id {thread_id}: {err}"),
            })?
            .ok_or_else(|| ThreadStoreError::InvalidRequest {
                message: format!("no rollout found for thread id {thread_id}"),
            })?;

    let canonical_rollout_path = scoped_rollout_path(
        store.config.codex_home.join(codex_rollout::SESSIONS_SUBDIR),
        rollout_path.as_path(),
        "sessions",
    )?;
    let file_name = matching_rollout_file_name(
        canonical_rollout_path.as_path(),
        thread_id,
        rollout_path.as_path(),
    )?;

    let archive_folder = store
        .config
        .codex_home
        .join(codex_rollout::ARCHIVED_SESSIONS_SUBDIR);
    std::fs::create_dir_all(&archive_folder).map_err(|err| ThreadStoreError::Internal {
        message: format!("failed to archive thread: {err}"),
    })?;
    let archived_path = archive_folder.join(&file_name);
    std::fs::rename(&canonical_rollout_path, &archived_path).map_err(|err| {
        ThreadStoreError::Internal {
            message: format!("failed to archive thread: {err}"),
        }
    })?;

    if let Some(ctx) = codex_rollout::state_db::get_state_db(&store.config).await {
        let _ = ctx
            .mark_archived(thread_id, archived_path.as_path(), Utc::now())
            .await;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use codex_protocol::ThreadId;
    use codex_protocol::protocol::SessionSource;
    use codex_rollout::ARCHIVED_SESSIONS_SUBDIR;
    use pretty_assertions::assert_eq;
    use tempfile::TempDir;
    use uuid::Uuid;

    use super::*;
    use crate::ListThreadsParams;
    use crate::ThreadSortKey;
    use crate::ThreadStore;
    use crate::local::LocalThreadStore;
    use crate::local::test_support::test_config;
    use crate::local::test_support::write_session_file;

    #[tokio::test]
    async fn archive_thread_moves_rollout_to_archived_collection() {
        let home = TempDir::new().expect("temp dir");
        let store = LocalThreadStore::new(test_config(home.path()));
        let uuid = Uuid::from_u128(201);
        let thread_id = ThreadId::from_string(&uuid.to_string()).expect("valid thread id");
        let active_path =
            write_session_file(home.path(), "2025-01-03T12-00-00", uuid).expect("session file");

        store
            .archive_thread(ArchiveThreadParams { thread_id })
            .await
            .expect("archive thread");

        assert!(!active_path.exists());
        let archived_path = home
            .path()
            .join(ARCHIVED_SESSIONS_SUBDIR)
            .join(active_path.file_name().expect("file name"));
        assert!(archived_path.exists());

        let archived = store
            .list_threads(ListThreadsParams {
                page_size: 10,
                cursor: None,
                sort_key: ThreadSortKey::CreatedAt,
                sort_direction: crate::SortDirection::Desc,
                allowed_sources: Vec::new(),
                model_providers: None,
                cwd_filters: None,
                archived: true,
                search_term: None,
                use_state_db_only: false,
            })
            .await
            .expect("archived listing");
        assert_eq!(archived.items.len(), 1);
        assert_eq!(archived.items[0].thread_id, thread_id);
        assert_eq!(archived.items[0].rollout_path, Some(archived_path));
        assert_eq!(
            archived.items[0].archived_at,
            Some(archived.items[0].updated_at)
        );
    }

    #[tokio::test]
    async fn archive_thread_updates_sqlite_metadata_when_present() {
        let home = TempDir::new().expect("temp dir");
        let config = test_config(home.path());
        let store = LocalThreadStore::new(config.clone());
        let uuid = Uuid::from_u128(202);
        let thread_id = ThreadId::from_string(&uuid.to_string()).expect("valid thread id");
        let active_path =
            write_session_file(home.path(), "2025-01-03T12-00-00", uuid).expect("session file");
        let runtime = codex_state::StateRuntime::init(
            home.path().to_path_buf(),
            config.model_provider_id.clone(),
        )
        .await
        .expect("state db should initialize");
        runtime
            .mark_backfill_complete(/*last_watermark*/ None)
            .await
            .expect("backfill should be complete");
        let mut builder = codex_state::ThreadMetadataBuilder::new(
            thread_id,
            active_path.clone(),
            Utc::now(),
            SessionSource::Cli,
        );
        builder.model_provider = Some(config.model_provider_id.clone());
        builder.cwd = home.path().to_path_buf();
        builder.cli_version = Some("test_version".to_string());
        let metadata = builder.build(config.model_provider_id.as_str());
        runtime
            .upsert_thread(&metadata)
            .await
            .expect("state db upsert should succeed");

        store
            .archive_thread(ArchiveThreadParams { thread_id })
            .await
            .expect("archive thread");

        let archived_path = home
            .path()
            .join(ARCHIVED_SESSIONS_SUBDIR)
            .join(active_path.file_name().expect("file name"));
        let updated = runtime
            .get_thread(thread_id)
            .await
            .expect("state db read should succeed")
            .expect("thread metadata should exist");
        assert_eq!(updated.rollout_path, archived_path);
        assert!(updated.archived_at.is_some());
    }
}

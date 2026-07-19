use std::collections::HashMap;

use codex_rollout::RolloutConfig;
use codex_rollout::RolloutRecorder;
use codex_rollout::parse_cursor;

use super::LocalThreadStore;
use super::helpers::resolve_thread_names;
use super::helpers::set_thread_name;
use super::helpers::stored_thread_from_rollout_item;
use crate::ListThreadsParams;
use crate::SortDirection;
use crate::ThreadPage;
use crate::ThreadRelationFilter;
use crate::ThreadSortKey;
use crate::ThreadStoreError;
use crate::ThreadStoreResult;

pub(super) async fn list_threads(
    store: &LocalThreadStore,
    params: ListThreadsParams,
) -> ThreadStoreResult<ThreadPage> {
    let cursor = params
        .cursor
        .as_deref()
        .map(|cursor| {
            parse_cursor(cursor).ok_or_else(|| ThreadStoreError::InvalidRequest {
                message: format!("invalid cursor: {cursor}"),
            })
        })
        .transpose()?;
    let sort_key = match params.sort_key {
        ThreadSortKey::CreatedAt => codex_rollout::ThreadSortKey::CreatedAt,
        ThreadSortKey::UpdatedAt => codex_rollout::ThreadSortKey::UpdatedAt,
        ThreadSortKey::RecencyAt => codex_rollout::ThreadSortKey::RecencyAt,
    };
    let sort_direction = match params.sort_direction {
        SortDirection::Asc => codex_rollout::SortDirection::Asc,
        SortDirection::Desc => codex_rollout::SortDirection::Desc,
    };
    let state_db = store.state_db().await;
    let rollout_config = RolloutConfig {
        codex_home: store.config.codex_home.clone(),
        sqlite_home: store.config.sqlite_home.clone(),
        cwd: store.config.codex_home.clone(),
        model_provider_id: store.config.default_model_provider_id.clone(),
        generate_memories: false,
    };
    let page = list_rollout_threads(
        state_db,
        &rollout_config,
        store.config.default_model_provider_id.as_str(),
        &params,
        cursor.as_ref(),
        sort_key,
        sort_direction,
    )
    .await?;

    let next_cursor = page
        .next_cursor
        .as_ref()
        .and_then(|cursor| serde_json::to_value(cursor).ok())
        .and_then(|value| value.as_str().map(str::to_owned));
    let mut items = page
        .items
        .into_iter()
        .filter_map(|item| {
            stored_thread_from_rollout_item(
                item,
                params.archived,
                store.config.default_model_provider_id.as_str(),
            )
        })
        .collect::<Vec<_>>();

    let thread_history_modes = items
        .iter()
        .map(|thread| (thread.thread_id, thread.history_mode))
        .collect::<HashMap<_, _>>();
    let names = resolve_thread_names(store, &thread_history_modes).await;
    for thread in &mut items {
        if let Some(name) = names.get(&thread.thread_id).cloned() {
            set_thread_name(thread, name);
        }
    }

    Ok(ThreadPage { items, next_cursor })
}

pub(super) async fn list_rollout_threads(
    state_db: Option<codex_rollout::StateDbHandle>,
    config: &RolloutConfig,
    default_model_provider_id: &str,
    params: &ListThreadsParams,
    cursor: Option<&codex_rollout::Cursor>,
    sort_key: codex_rollout::ThreadSortKey,
    sort_direction: codex_rollout::SortDirection,
) -> ThreadStoreResult<codex_rollout::ThreadsPage> {
    if let Some(relation_filter) = params.relation_filter {
        let relation_filter = match relation_filter {
            ThreadRelationFilter::DirectChildrenOf(parent_thread_id) => {
                codex_state::ThreadRelationFilter::DirectChildrenOf(parent_thread_id)
            }
            ThreadRelationFilter::DescendantsOf(ancestor_thread_id) => {
                codex_state::ThreadRelationFilter::DescendantsOf(ancestor_thread_id)
            }
        };
        let page = codex_rollout::state_db::list_threads_db(
            state_db.as_deref(),
            config.codex_home.as_path(),
            params.page_size,
            cursor,
            sort_key,
            sort_direction,
            params.allowed_sources.as_slice(),
            params.model_providers.as_deref(),
            params.cwd_filters.as_deref(),
            Some(relation_filter),
            params.archived,
            params.search_term.as_deref(),
        )
        .await
        .ok_or_else(|| ThreadStoreError::Internal {
            message: "state DB unavailable for relationship-filtered thread listing".to_string(),
        })?;
        return Ok(page.into());
    }

    let page = if params.use_state_db_only && params.archived {
        RolloutRecorder::list_archived_threads_from_state_db(
            state_db,
            config,
            params.page_size,
            cursor,
            sort_key,
            sort_direction,
            params.allowed_sources.as_slice(),
            params.model_providers.as_deref(),
            params.cwd_filters.as_deref(),
            default_model_provider_id,
            params.search_term.as_deref(),
        )
        .await
    } else if params.use_state_db_only {
        RolloutRecorder::list_threads_from_state_db(
            state_db,
            config,
            params.page_size,
            cursor,
            sort_key,
            sort_direction,
            params.allowed_sources.as_slice(),
            params.model_providers.as_deref(),
            params.cwd_filters.as_deref(),
            default_model_provider_id,
            params.search_term.as_deref(),
        )
        .await
    } else if params.archived {
        RolloutRecorder::list_archived_threads(
            state_db,
            config,
            params.page_size,
            cursor,
            sort_key,
            sort_direction,
            params.allowed_sources.as_slice(),
            params.model_providers.as_deref(),
            params.cwd_filters.as_deref(),
            default_model_provider_id,
            params.search_term.as_deref(),
        )
        .await
    } else {
        RolloutRecorder::list_threads(
            state_db,
            config,
            params.page_size,
            cursor,
            sort_key,
            sort_direction,
            params.allowed_sources.as_slice(),
            params.model_providers.as_deref(),
            params.cwd_filters.as_deref(),
            default_model_provider_id,
            params.search_term.as_deref(),
        )
        .await
    };
    page.map_err(|err| ThreadStoreError::Internal {
        message: format!("failed to list threads: {err}"),
    })
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use codex_protocol::ThreadId;
    use codex_protocol::protocol::SessionSource;
    use codex_protocol::protocol::ThreadHistoryMode;
    use pretty_assertions::assert_eq;
    use std::fs;
    use tempfile::TempDir;
    use uuid::Uuid;

    use super::*;
    use crate::ThreadStore;
    use crate::local::LocalThreadStore;
    use crate::local::test_support::test_config;
    use crate::local::test_support::write_archived_session_file;
    use crate::local::test_support::write_session_file;
    use crate::local::test_support::write_session_file_with;

    #[tokio::test]
    async fn list_threads_uses_default_provider_when_rollout_omits_provider() {
        let home = TempDir::new().expect("temp dir");
        let store = LocalThreadStore::new(test_config(home.path()), /*state_db*/ None);
        write_session_file_with(
            home.path(),
            home.path().join("sessions/2025/01/03"),
            "2025-01-03T12-00-00",
            Uuid::from_u128(102),
            "Hello from user",
            /*model_provider*/ None,
            ThreadHistoryMode::Legacy,
        )
        .expect("session file");

        let page = store
            .list_threads(ListThreadsParams {
                page_size: 10,
                cursor: None,
                sort_key: ThreadSortKey::CreatedAt,
                sort_direction: SortDirection::Desc,
                allowed_sources: Vec::new(),
                model_providers: None,
                cwd_filters: None,
                archived: false,
                search_term: None,
                relation_filter: None,
                use_state_db_only: false,
            })
            .await
            .expect("thread listing");

        assert_eq!(page.items.len(), 1);
        assert_eq!(page.items[0].model_provider, "test-provider");
    }

    #[tokio::test]
    async fn list_threads_preserves_sqlite_title_search_results() {
        let home = TempDir::new().expect("temp dir");
        let config = test_config(home.path());
        let uuid = Uuid::from_u128(103);
        let thread_id = ThreadId::from_string(&uuid.to_string()).expect("valid thread id");
        let rollout_path = home.path().join("rollout-title-search.jsonl");
        fs::write(&rollout_path, "").expect("placeholder rollout file");

        let runtime = codex_state::StateRuntime::init(
            home.path().to_path_buf(),
            config.default_model_provider_id.clone(),
        )
        .await
        .expect("state db should initialize");
        let store = LocalThreadStore::new(config.clone(), Some(runtime.clone()));
        runtime
            .mark_backfill_complete(/*last_watermark*/ None)
            .await
            .expect("backfill should be complete");
        let created_at = Utc::now();
        let mut builder = codex_state::ThreadMetadataBuilder::new(
            thread_id,
            rollout_path,
            created_at,
            SessionSource::Cli,
        );
        builder.model_provider = Some(config.default_model_provider_id.clone());
        builder.cwd = home.path().to_path_buf();
        builder.cli_version = Some("test_version".to_string());
        let mut metadata = builder.build(config.default_model_provider_id.as_str());
        metadata.title = "needle title".to_string();
        metadata.first_user_message = Some("plain preview".to_string());
        metadata.preview = metadata.first_user_message.clone();
        runtime
            .upsert_thread(&metadata)
            .await
            .expect("state db upsert should succeed");

        let page = store
            .list_threads(ListThreadsParams {
                page_size: 10,
                cursor: None,
                sort_key: ThreadSortKey::CreatedAt,
                sort_direction: SortDirection::Desc,
                allowed_sources: Vec::new(),
                model_providers: None,
                cwd_filters: None,
                archived: false,
                search_term: Some("needle".to_string()),
                relation_filter: None,
                use_state_db_only: true,
            })
            .await
            .expect("thread listing");

        let ids = page
            .items
            .iter()
            .map(|item| item.thread_id)
            .collect::<Vec<_>>();
        assert_eq!(ids, vec![thread_id]);
        assert_eq!(
            page.items[0].first_user_message.as_deref(),
            Some("plain preview")
        );
    }

    #[tokio::test]
    async fn list_paginated_threads_uses_sqlite_name_over_legacy_compatibility() {
        let home = TempDir::new().expect("temp dir");
        let config = test_config(home.path());
        let uuid = Uuid::from_u128(104);
        let thread_id = ThreadId::from_string(&uuid.to_string()).expect("valid thread id");
        let rollout_path = home.path().join("rollout-paginated-name-search.jsonl");
        fs::write(&rollout_path, "").expect("placeholder rollout file");

        let runtime = codex_state::StateRuntime::init(
            home.path().to_path_buf(),
            config.default_model_provider_id.clone(),
        )
        .await
        .expect("state db should initialize");
        let store = LocalThreadStore::new(config.clone(), Some(runtime.clone()));
        runtime
            .mark_backfill_complete(/*last_watermark*/ None)
            .await
            .expect("backfill should be complete");
        let mut builder = codex_state::ThreadMetadataBuilder::new(
            thread_id,
            rollout_path,
            Utc::now(),
            SessionSource::Cli,
        );
        builder.history_mode = ThreadHistoryMode::Paginated;
        builder.model_provider = Some(config.default_model_provider_id.clone());
        builder.cwd = home.path().to_path_buf();
        builder.cli_version = Some("test_version".to_string());
        let mut metadata = builder.build(config.default_model_provider_id.as_str());
        metadata.name = Some("canonical paginated name".to_string());
        metadata.title = "stale title name".to_string();
        metadata.first_user_message = Some("plain preview".to_string());
        metadata.preview = metadata.first_user_message.clone();
        runtime
            .upsert_thread(&metadata)
            .await
            .expect("state db upsert should succeed");
        codex_rollout::append_thread_name(home.path(), thread_id, "stale index name")
            .await
            .expect("append legacy thread name");

        let page = store
            .list_threads(ListThreadsParams {
                page_size: 10,
                cursor: None,
                sort_key: ThreadSortKey::CreatedAt,
                sort_direction: SortDirection::Desc,
                allowed_sources: Vec::new(),
                model_providers: None,
                cwd_filters: None,
                archived: false,
                search_term: Some("canonical".to_string()),
                relation_filter: None,
                use_state_db_only: true,
            })
            .await
            .expect("thread listing");

        assert_eq!(page.items.len(), 1);
        assert_eq!(page.items[0].thread_id, thread_id);
        assert_eq!(
            page.items[0].name.as_deref(),
            Some("canonical paginated name")
        );
    }

    #[tokio::test]
    async fn list_threads_selects_active_or_archived_collection() {
        let home = TempDir::new().expect("temp dir");
        let store = LocalThreadStore::new(test_config(home.path()), /*state_db*/ None);
        let active_uuid = Uuid::from_u128(105);
        let archived_uuid = Uuid::from_u128(106);
        write_session_file(home.path(), "2025-01-03T12-00-00", active_uuid)
            .expect("active session file");
        write_archived_session_file(home.path(), "2025-01-03T13-00-00", archived_uuid)
            .expect("archived session file");

        let active = store
            .list_threads(ListThreadsParams {
                page_size: 10,
                cursor: None,
                sort_key: ThreadSortKey::CreatedAt,
                sort_direction: SortDirection::Desc,
                allowed_sources: Vec::new(),
                model_providers: None,
                cwd_filters: None,
                archived: false,
                search_term: None,
                relation_filter: None,
                use_state_db_only: false,
            })
            .await
            .expect("active listing");
        let archived = store
            .list_threads(ListThreadsParams {
                page_size: 10,
                cursor: None,
                sort_key: ThreadSortKey::CreatedAt,
                sort_direction: SortDirection::Desc,
                allowed_sources: Vec::new(),
                model_providers: None,
                cwd_filters: None,
                archived: true,
                search_term: None,
                relation_filter: None,
                use_state_db_only: false,
            })
            .await
            .expect("archived listing");

        let active_id = ThreadId::from_string(&active_uuid.to_string()).expect("valid thread id");
        let archived_id =
            ThreadId::from_string(&archived_uuid.to_string()).expect("valid thread id");
        assert_eq!(
            active
                .items
                .iter()
                .map(|item| item.thread_id)
                .collect::<Vec<_>>(),
            vec![active_id]
        );
        assert_eq!(
            archived
                .items
                .iter()
                .map(|item| item.thread_id)
                .collect::<Vec<_>>(),
            vec![archived_id]
        );
        assert_eq!(active.items[0].archived_at, None);
        assert_eq!(
            archived.items[0].archived_at,
            Some(archived.items[0].updated_at)
        );
    }

    #[tokio::test]
    async fn list_threads_returns_local_rollout_summary() {
        let home = TempDir::new().expect("temp dir");
        let config = test_config(home.path());
        let store = LocalThreadStore::new(config, /*state_db*/ None);
        let uuid = Uuid::from_u128(101);
        let path =
            write_session_file(home.path(), "2025-01-03T12-00-00", uuid).expect("session file");

        let page = store
            .list_threads(ListThreadsParams {
                page_size: 10,
                cursor: None,
                sort_key: ThreadSortKey::CreatedAt,
                sort_direction: SortDirection::Desc,
                allowed_sources: vec![SessionSource::Cli],
                model_providers: Some(vec!["test-provider".to_string()]),
                cwd_filters: None,
                archived: false,
                search_term: None,
                relation_filter: None,
                use_state_db_only: false,
            })
            .await
            .expect("thread listing");

        let thread_id = ThreadId::from_string(&uuid.to_string()).expect("valid thread id");
        assert_eq!(page.next_cursor, None);
        assert_eq!(page.items.len(), 1);
        assert_eq!(page.items[0].thread_id, thread_id);
        assert_eq!(page.items[0].rollout_path, Some(path));
        assert_eq!(page.items[0].preview, "Hello from user");
        assert_eq!(
            page.items[0].first_user_message.as_deref(),
            Some("Hello from user")
        );
        assert_eq!(page.items[0].model_provider, "test-provider");
        assert_eq!(page.items[0].cli_version, "test_version");
        assert_eq!(page.items[0].source, SessionSource::Cli);
    }

    #[tokio::test]
    async fn list_threads_rejects_invalid_cursor() {
        let home = TempDir::new().expect("temp dir");
        let store = LocalThreadStore::new(test_config(home.path()), /*state_db*/ None);

        let err = store
            .list_threads(ListThreadsParams {
                page_size: 10,
                cursor: Some("not-a-cursor".to_string()),
                sort_key: ThreadSortKey::CreatedAt,
                sort_direction: SortDirection::Desc,
                allowed_sources: Vec::new(),
                model_providers: None,
                cwd_filters: None,
                archived: false,
                search_term: None,
                relation_filter: None,
                use_state_db_only: false,
            })
            .await
            .expect_err("invalid cursor should fail");

        assert!(matches!(err, ThreadStoreError::InvalidRequest { .. }));
    }
}

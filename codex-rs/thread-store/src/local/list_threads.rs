use std::collections::HashMap;

use chrono::DateTime;
use chrono::Utc;
use codex_protocol::ThreadId;
use codex_protocol::protocol::ThreadHistoryMode;
use codex_rollout::RolloutConfig;
use codex_rollout::RolloutRecorder;
use codex_rollout::parse_cursor;
use codex_state::ThreadFilterOptions;

use super::LocalThreadStore;
use super::helpers::indexed_thread_names;
use super::helpers::listed_thread_name;
use super::helpers::resolve_thread_names;
use super::helpers::resolve_thread_section_metadata;
use super::helpers::set_thread_name;
use super::helpers::stored_thread_from_rollout_item;
use super::read_thread::stored_thread_from_state_metadata;
use crate::ListThreadsParams;
use crate::SortDirection;
use crate::ThreadPage;
use crate::ThreadRelationFilter;
use crate::ThreadSortKey;
use crate::ThreadStoreError;
use crate::ThreadStoreResult;

/// A local thread listing that preserves full rows from the direct SQLite path.
pub(super) enum LocalThreadsPage {
    /// Rows from a direct SQLite listing, with titles and section metadata intact.
    State(codex_state::ThreadsPage),
    /// Summaries from the rollout recorder, built from rollout files or SQLite rows.
    /// The thread store resolves titles and section ordering metadata separately.
    Rollouts(codex_rollout::ThreadsPage),
}

impl From<LocalThreadsPage> for codex_rollout::ThreadsPage {
    fn from(page: LocalThreadsPage) -> Self {
        match page {
            LocalThreadsPage::State(page) => page.into(),
            LocalThreadsPage::Rollouts(page) => page,
        }
    }
}

pub(super) async fn list_threads(
    store: &LocalThreadStore,
    params: ListThreadsParams,
) -> ThreadStoreResult<ThreadPage> {
    if params.sort_key == ThreadSortKey::SectionPosition {
        return list_section_threads(store, params).await;
    }
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
        ThreadSortKey::SectionPosition => unreachable!("section order uses the state database"),
    };
    let sort_direction = match params.sort_direction {
        SortDirection::Asc => codex_rollout::SortDirection::Asc,
        SortDirection::Desc => codex_rollout::SortDirection::Desc,
    };
    let state_db = store.state_db().await;
    let rollout_config = RolloutConfig {
        codex_home: store.config.codex_home.clone(),
        sqlite: store.config.sqlite.clone(),
        cwd: store.config.codex_home.clone(),
        model_provider_id: store.config.default_model_provider_id.clone(),
        generate_memories: false,
    };
    let page = list_rollout_threads(
        state_db.clone(),
        &rollout_config,
        store.config.default_model_provider_id.as_str(),
        &params,
        cursor.as_ref(),
        sort_key,
        sort_direction,
    )
    .await?;

    let page = match page {
        LocalThreadsPage::State(page) => {
            let legacy_thread_ids = page
                .items
                .iter()
                .filter(|row| row.history_mode == ThreadHistoryMode::Legacy)
                .map(|row| row.id)
                .collect();
            let indexed_names =
                indexed_thread_names(store, &legacy_thread_ids, params.use_state_db_only).await?;
            return Ok(thread_page_from_state(
                page,
                &indexed_names,
                params.archived,
                &store.config.default_model_provider_id,
            ));
        }
        LocalThreadsPage::Rollouts(page) => page,
    };
    let next_cursor = encode_cursor(page.next_cursor.as_ref());
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
    if let Some(state_db) = state_db {
        let sectioned_thread_ids = items
            .iter()
            .filter(|thread| thread.section.is_some())
            .map(|thread| thread.thread_id)
            .collect::<Vec<_>>();
        let section_metadata =
            resolve_thread_section_metadata(state_db.as_ref(), &sectioned_thread_ids).await;
        for thread in items.iter_mut().filter(|thread| thread.section.is_some()) {
            if let Some((section_position, section_entered_at)) =
                section_metadata.get(&thread.thread_id)
            {
                thread.section_position = *section_position;
                thread.section_entered_at = *section_entered_at;
            }
        }
    }

    Ok(ThreadPage { items, next_cursor })
}

/// Projects the collected SQLite rows in order, including repeats across a refill.
fn thread_page_from_state(
    page: codex_state::ThreadsPage,
    indexed_names: &HashMap<ThreadId, String>,
    archived: bool,
    default_model_provider_id: &str,
) -> ThreadPage {
    let items = page
        .items
        .into_iter()
        .filter_map(|row| {
            let name =
                listed_thread_name(row.history_mode, &row, indexed_names.get(&row.id).cloned());
            let position = row.section_position;
            let entered_at = row.section_entered_at;
            let parent = page.parent_thread_ids.get(&row.id).copied();
            let item = codex_rollout::thread_item_from_state_metadata(row, parent);
            let mut thread =
                stored_thread_from_rollout_item(item, archived, default_model_provider_id)?;
            if let Some(name) = name {
                set_thread_name(&mut thread, name);
            }
            if thread.section.is_some() {
                thread.section_position = position;
                thread.section_entered_at = entered_at;
            }
            Some(thread)
        })
        .collect();
    let cursor = page.next_anchor.map(codex_rollout::Cursor::from);
    ThreadPage {
        items,
        next_cursor: encode_cursor(cursor.as_ref()),
    }
}

fn encode_cursor(cursor: Option<&codex_rollout::Cursor>) -> Option<String> {
    cursor
        .and_then(|cursor| serde_json::to_value(cursor).ok())
        .and_then(|value| value.as_str().map(str::to_owned))
}

async fn list_section_threads(
    store: &LocalThreadStore,
    params: ListThreadsParams,
) -> ThreadStoreResult<ThreadPage> {
    let section = params
        .section
        .as_ref()
        .and_then(Option::as_deref)
        .ok_or_else(|| ThreadStoreError::InvalidRequest {
            message: "section-position sorting requires a section filter".to_owned(),
        })?;
    let state_db = store
        .state_db()
        .await
        .ok_or_else(|| ThreadStoreError::Internal {
            message: "state DB unavailable for section-ordered thread listing".to_owned(),
        })?;

    let anchor = params
        .cursor
        .as_deref()
        .map(|cursor| -> ThreadStoreResult<codex_state::Anchor> {
            let (position, thread_id) =
                cursor
                    .split_once('|')
                    .ok_or_else(|| ThreadStoreError::InvalidRequest {
                        message: format!("invalid cursor: {cursor}"),
                    })?;
            let timestamp = position
                .parse::<i64>()
                .ok()
                .and_then(DateTime::<Utc>::from_timestamp_millis)
                .ok_or_else(|| ThreadStoreError::InvalidRequest {
                    message: format!("invalid cursor: {cursor}"),
                })?;
            let thread_id = codex_protocol::ThreadId::from_string(thread_id).map_err(|_| {
                ThreadStoreError::InvalidRequest {
                    message: format!("invalid cursor: {cursor}"),
                }
            })?;
            Ok(codex_state::Anchor {
                ts: timestamp,
                id: Some(thread_id),
            })
        })
        .transpose()?;
    let allowed_sources = params
        .allowed_sources
        .iter()
        .map(|source| match serde_json::to_value(source) {
            Ok(serde_json::Value::String(source)) => source,
            Ok(source) => source.to_string(),
            Err(_) => String::new(),
        })
        .collect::<Vec<_>>();
    let normalized_cwd_filters = params.cwd_filters.as_ref().map(|filters| {
        filters
            .iter()
            .map(|cwd| codex_rollout::state_db::normalize_cwd_for_state_db(cwd))
            .collect::<Vec<_>>()
    });
    let filters = ThreadFilterOptions {
        included_thread_ids: params.included_thread_ids.as_deref(),
        archived_only: params.archived,
        allowed_sources: allowed_sources.as_slice(),
        model_providers: params.model_providers.as_deref(),
        cwd_filters: normalized_cwd_filters.as_deref(),
        section: Some(Some(section)),
        project_id: params
            .project_id
            .as_ref()
            .map(|project_id| project_id.as_deref()),
        anchor: anchor.as_ref(),
        sort_key: codex_state::SortKey::SectionPosition,
        sort_direction: match params.sort_direction {
            SortDirection::Asc => codex_state::SortDirection::Asc,
            SortDirection::Desc => codex_state::SortDirection::Desc,
        },
        search_term: params.search_term.as_deref(),
    };
    let page = match params.relation_filter.map(state_relation_filter) {
        Some(relation_filter) => {
            state_db
                .list_threads_by_relation(params.page_size, relation_filter, filters)
                .await
        }
        None => state_db.list_threads(params.page_size, filters).await,
    }
    .map_err(|err| ThreadStoreError::Internal {
        message: format!("failed to list section-ordered threads: {err}"),
    })?;

    let codex_state::ThreadsPage {
        items: metadata_items,
        parent_thread_ids,
        next_anchor,
        ..
    } = page;
    let items = metadata_items
        .into_iter()
        .map(|metadata| {
            let parent_thread_id = parent_thread_ids.get(&metadata.id).copied();
            stored_thread_from_state_metadata(store, metadata, parent_thread_id)
        })
        .collect();
    let next_cursor = next_anchor.and_then(|anchor| {
        anchor.id.map(|thread_id| {
            let position = anchor.ts.timestamp_millis();
            format!("{position}|{thread_id}")
        })
    });
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
) -> ThreadStoreResult<LocalThreadsPage> {
    if params.use_state_db_only
        || params.included_thread_ids.is_some()
        || params
            .relation_filter
            .is_some_and(ThreadRelationFilter::is_parent_scoped)
        || params.section.is_some()
        || params.project_id.is_some()
    {
        let page = codex_rollout::state_db::list_threads_db(
            state_db.as_deref(),
            &config.sqlite,
            params.page_size,
            cursor,
            sort_key,
            sort_direction,
            params.allowed_sources.as_slice(),
            params.model_providers.as_deref(),
            params.cwd_filters.as_deref(),
            params.included_thread_ids.as_deref(),
            params.relation_filter.map(state_relation_filter),
            params.archived,
            params.section.as_ref().map(Option::as_deref),
            params.project_id.as_ref().map(Option::as_deref),
            params.search_term.as_deref(),
        )
        .await
        .ok_or_else(|| ThreadStoreError::Internal {
            message: "state DB unavailable for filtered thread listing".to_string(),
        })?;
        return Ok(LocalThreadsPage::State(page));
    }

    let include_attached_threads =
        params.relation_filter == Some(ThreadRelationFilter::IncludeAttached);
    let page = if params.archived {
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
            include_attached_threads,
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
            include_attached_threads,
        )
        .await
    };
    page.map(LocalThreadsPage::Rollouts)
        .map_err(|err| ThreadStoreError::Internal {
            message: format!("failed to list threads: {err}"),
        })
}

fn state_relation_filter(filter: ThreadRelationFilter) -> codex_state::ThreadRelationFilter {
    match filter {
        ThreadRelationFilter::IncludeAttached => codex_state::ThreadRelationFilter::IncludeAttached,
        ThreadRelationFilter::DirectChildrenOf(id) => {
            codex_state::ThreadRelationFilter::DirectChildrenOf(id)
        }
        ThreadRelationFilter::DescendantsOf(id) => {
            codex_state::ThreadRelationFilter::DescendantsOf(id)
        }
        ThreadRelationFilter::AttachedThreadsOf(id) => {
            codex_state::ThreadRelationFilter::AttachedThreadsOf(id)
        }
    }
}

#[cfg(test)]
#[path = "list_threads_tests.rs"]
mod state_page_tests;

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use codex_protocol::ThreadId;
    use codex_protocol::protocol::SessionSource;
    use codex_protocol::protocol::ThreadHistoryMode;
    use codex_state::PINNED_THREAD_SECTION_ID;
    use codex_utils_absolute_path::test_support::PathExt;
    use pretty_assertions::assert_eq;
    use std::fs;
    use tempfile::TempDir;
    use uuid::Uuid;

    use super::*;
    use crate::MoveThreadToSectionParams;
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
                included_thread_ids: None,
                page_size: 10,
                cursor: None,
                sort_key: ThreadSortKey::CreatedAt,
                sort_direction: SortDirection::Desc,
                allowed_sources: Vec::new(),
                model_providers: None,
                cwd_filters: None,
                section: None,
                project_id: None,
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
            codex_state::SqliteConfig::new_for_testing(home.path().abs()),
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
        let builder = codex_state::ThreadMetadataBuilder::new(
            thread_id,
            rollout_path,
            created_at,
            SessionSource::Cli,
        )
        .with_model_provider(Some(config.default_model_provider_id.clone()))
        .with_cwd(home.path().to_path_buf())
        .with_cli_version("test_version".to_string());
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
                included_thread_ids: None,
                page_size: 10,
                cursor: None,
                sort_key: ThreadSortKey::CreatedAt,
                sort_direction: SortDirection::Desc,
                allowed_sources: Vec::new(),
                model_providers: None,
                cwd_filters: None,
                section: None,
                project_id: None,
                archived: false,
                search_term: Some("needle".to_string()),
                relation_filter: None,
                use_state_db_only: true,
            })
            .await
            .expect("thread listing");

        let [thread] = page.items.as_slice() else {
            panic!("expected one thread matching the SQLite title");
        };
        assert_eq!(
            (
                thread.thread_id,
                thread.name.as_deref(),
                thread.first_user_message.as_deref(),
            ),
            (thread_id, Some("needle title"), Some("plain preview"))
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
            config.sqlite.clone(),
            config.default_model_provider_id.clone(),
        )
        .await
        .expect("state db should initialize");
        let store = LocalThreadStore::new(config.clone(), Some(runtime.clone()));
        runtime
            .mark_backfill_complete(/*last_watermark*/ None)
            .await
            .expect("backfill should be complete");
        let builder = codex_state::ThreadMetadataBuilder::new(
            thread_id,
            rollout_path,
            Utc::now(),
            SessionSource::Cli,
        )
        .with_history_mode(ThreadHistoryMode::Paginated)
        .with_model_provider(Some(config.default_model_provider_id.clone()))
        .with_cwd(home.path().to_path_buf())
        .with_cli_version("test_version".to_string());
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
                included_thread_ids: None,
                page_size: 10,
                cursor: None,
                sort_key: ThreadSortKey::CreatedAt,
                sort_direction: SortDirection::Desc,
                allowed_sources: Vec::new(),
                model_providers: None,
                cwd_filters: None,
                section: None,
                project_id: None,
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
                included_thread_ids: None,
                page_size: 10,
                cursor: None,
                sort_key: ThreadSortKey::CreatedAt,
                sort_direction: SortDirection::Desc,
                allowed_sources: Vec::new(),
                model_providers: None,
                cwd_filters: None,
                section: None,
                project_id: None,
                archived: false,
                search_term: None,
                relation_filter: None,
                use_state_db_only: false,
            })
            .await
            .expect("active listing");
        let archived = store
            .list_threads(ListThreadsParams {
                included_thread_ids: None,
                page_size: 10,
                cursor: None,
                sort_key: ThreadSortKey::CreatedAt,
                sort_direction: SortDirection::Desc,
                allowed_sources: Vec::new(),
                model_providers: None,
                cwd_filters: None,
                section: None,
                project_id: None,
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
        let runtime = codex_state::StateRuntime::init(
            config.sqlite.clone(),
            config.default_model_provider_id.clone(),
        )
        .await
        .expect("initialize state DB");
        let store = LocalThreadStore::new(config, Some(runtime.clone()));
        let uuid = Uuid::from_u128(101);
        let path =
            write_session_file(home.path(), "2025-01-03T12-00-00", uuid).expect("session file");

        let params = ListThreadsParams {
            included_thread_ids: None,
            page_size: 10,
            cursor: None,
            sort_key: ThreadSortKey::CreatedAt,
            sort_direction: SortDirection::Desc,
            allowed_sources: vec![SessionSource::Cli],
            model_providers: Some(vec!["test-provider".to_string()]),
            cwd_filters: None,
            section: None,
            project_id: None,
            archived: false,
            search_term: None,
            relation_filter: None,
            use_state_db_only: false,
        };
        let page = store
            .list_threads(params.clone())
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

        // Ordinary listing keeps its indexed-name fallback when the state DB is unavailable.
        codex_rollout::append_thread_name(home.path(), thread_id, "indexed name")
            .await
            .expect("write indexed name");
        runtime.close().await;
        let fallback = store
            .list_threads(params)
            .await
            .expect("ordinary listing must preserve filesystem fallback");
        assert_eq!(fallback.items[0].thread_id, thread_id);
        assert_eq!(fallback.items[0].name.as_deref(), Some("indexed name"));
    }

    #[tokio::test]
    async fn section_listing_uses_sqlite_metadata_without_reading_rollouts() {
        let home = TempDir::new().expect("temp dir");
        let config = test_config(home.path());
        let state = codex_state::StateRuntime::init(
            config.sqlite.clone(),
            config.default_model_provider_id.clone(),
        )
        .await
        .expect("initialize state");
        let store = LocalThreadStore::new(config, Some(state.clone()));
        let mut thread_ids = Vec::new();
        let mut first_rollout_path = None;

        for index in 0..3 {
            let uuid = Uuid::from_u128(975 + index);
            let thread_id = ThreadId::from_string(&uuid.to_string()).expect("valid thread id");
            let timestamp = format!("2025-01-03T16-{index:02}-00");
            let rollout_path =
                write_session_file(home.path(), &timestamp, uuid).expect("write rollout");
            codex_rollout::state_db::reconcile_rollout(
                Some(state.as_ref()),
                rollout_path.as_path(),
                "test-provider",
                /*builder*/ None,
                &[],
                /*archived_only*/ None,
                /*new_thread_memory_mode*/ None,
            )
            .await;
            store
                .move_thread_to_section(MoveThreadToSectionParams {
                    thread_id,
                    section: Some(PINNED_THREAD_SECTION_ID.to_owned()),
                    before_thread_id: None,
                })
                .await
                .expect("append section member");
            thread_ids.push(thread_id);
            if index == 0 {
                first_rollout_path = Some(rollout_path);
            }
        }
        fs::remove_file(first_rollout_path.expect("first rollout path"))
            .expect("remove rollout without invalidating SQLite metadata");

        let params = ListThreadsParams {
            included_thread_ids: None,
            page_size: 2,
            cursor: None,
            sort_key: ThreadSortKey::SectionPosition,
            sort_direction: SortDirection::Asc,
            allowed_sources: Vec::new(),
            model_providers: None,
            cwd_filters: None,
            section: Some(Some(PINNED_THREAD_SECTION_ID.to_owned())),
            project_id: None,
            archived: false,
            search_term: None,
            relation_filter: None,
            use_state_db_only: true,
        };
        let page = store
            .list_threads(params.clone())
            .await
            .expect("section listing should use SQLite metadata");

        assert_eq!(
            page.items
                .iter()
                .map(|thread| thread.thread_id)
                .collect::<Vec<_>>(),
            vec![thread_ids[0], thread_ids[1]]
        );
        assert_eq!(page.next_cursor, Some(format!("2000000|{}", thread_ids[1])));

        let recent_page = store
            .list_threads(ListThreadsParams {
                page_size: 3,
                sort_key: ThreadSortKey::UpdatedAt,
                sort_direction: SortDirection::Desc,
                section: None,
                ..params.clone()
            })
            .await
            .expect("list recent threads with section metadata");
        let section_member = recent_page
            .items
            .iter()
            .find(|thread| thread.thread_id == thread_ids[1])
            .expect("listed section member");
        let entered_at = page.items[1]
            .section_entered_at
            .expect("section entry time");
        assert_eq!(section_member.section_position, Some(2_000_000));
        assert_eq!(section_member.section_entered_at, Some(entered_at));

        let next_page = store
            .list_threads(ListThreadsParams {
                cursor: page.next_cursor,
                ..params
            })
            .await
            .expect("section cursor should continue listing from SQLite metadata");

        assert_eq!(
            next_page
                .items
                .iter()
                .map(|thread| thread.thread_id)
                .collect::<Vec<_>>(),
            vec![thread_ids[2]]
        );
        assert_eq!(next_page.next_cursor, None);
    }

    #[tokio::test]
    async fn list_threads_rejects_invalid_cursor() {
        let home = TempDir::new().expect("temp dir");
        let store = LocalThreadStore::new(test_config(home.path()), /*state_db*/ None);

        let err = store
            .list_threads(ListThreadsParams {
                included_thread_ids: None,
                page_size: 10,
                cursor: Some("not-a-cursor".to_string()),
                sort_key: ThreadSortKey::CreatedAt,
                sort_direction: SortDirection::Desc,
                allowed_sources: Vec::new(),
                model_providers: None,
                cwd_filters: None,
                section: None,
                project_id: None,
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

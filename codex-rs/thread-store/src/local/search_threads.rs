use std::collections::HashMap;
use std::collections::HashSet;

use codex_install_context::InstallContext;
use codex_protocol::ThreadId;
use codex_rollout::find_thread_names_by_ids;
use codex_rollout::first_rollout_content_match_snippet;
use codex_rollout::parse_cursor;
use codex_rollout::read_thread_item_from_rollout;
use codex_rollout::search_rollout_paths;

use super::LocalThreadStore;
use super::helpers::distinct_thread_metadata_title;
use super::helpers::set_thread_name_from_title;
use super::helpers::stored_thread_from_rollout_item;
use crate::SearchThreadsParams;
use crate::SortDirection;
use crate::StoredThreadSearchResult;
use crate::ThreadSearchPage;
use crate::ThreadSortKey;
use crate::ThreadStoreError;
use crate::ThreadStoreResult;

struct ThreadSearchItem {
    item: codex_rollout::ThreadItem,
    snippet: String,
}

pub(super) async fn search_threads(
    store: &LocalThreadStore,
    params: SearchThreadsParams,
) -> ThreadStoreResult<ThreadSearchPage> {
    let search_term = params.search_term.as_str();
    if search_term.is_empty() {
        return Err(ThreadStoreError::InvalidRequest {
            message: "thread/search requires search_term".to_string(),
        });
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
    let sort_direction = match params.sort_direction {
        SortDirection::Asc => codex_rollout::SortDirection::Asc,
        SortDirection::Desc => codex_rollout::SortDirection::Desc,
    };
    let rg_command = InstallContext::current().rg_command();
    let matching_paths = search_rollout_paths(
        rg_command.as_path(),
        store.config.codex_home.as_path(),
        params.archived,
        search_term,
    )
    .await
    .map_err(|err| ThreadStoreError::Internal {
        message: format!("failed to search rollout contents: {err}"),
    })?;
    if matching_paths.is_empty() {
        return Ok(ThreadSearchPage {
            items: Vec::new(),
            next_cursor: None,
        });
    }
    let mut candidates = Vec::with_capacity(matching_paths.len());
    for path in matching_paths {
        let Some(item) = read_thread_item_from_rollout(path).await else {
            continue;
        };
        if !params.allowed_sources.is_empty()
            && !item
                .source
                .as_ref()
                .is_some_and(|source| params.allowed_sources.contains(source))
        {
            continue;
        }
        let Some(item_cursor) = cursor_from_thread_item(&item, params.sort_key) else {
            continue;
        };
        if cursor.as_ref().is_some_and(|cursor| match sort_direction {
            codex_rollout::SortDirection::Asc => item_cursor.timestamp() <= cursor.timestamp(),
            codex_rollout::SortDirection::Desc => item_cursor.timestamp() >= cursor.timestamp(),
        }) {
            continue;
        }
        candidates.push((item_cursor.timestamp(), item));
    }
    candidates.sort_by(
        |(left_timestamp, left_item), (right_timestamp, right_item)| match sort_direction {
            codex_rollout::SortDirection::Asc => left_timestamp
                .cmp(right_timestamp)
                .then_with(|| left_item.path.cmp(&right_item.path)),
            codex_rollout::SortDirection::Desc => right_timestamp
                .cmp(left_timestamp)
                .then_with(|| right_item.path.cmp(&left_item.path)),
        },
    );

    let mut matching_items = Vec::new();
    for (_, item) in candidates {
        let Some(snippet) = first_rollout_content_match_snippet(item.path.as_path(), search_term)
            .await
            .map_err(|err| ThreadStoreError::Internal {
                message: format!("failed to read rollout search match: {err}"),
            })?
        else {
            continue;
        };
        matching_items.push(ThreadSearchItem { item, snippet });
        if matching_items.len() > params.page_size {
            break;
        }
    }

    let more_matches_available = matching_items.len() > params.page_size;
    matching_items.truncate(params.page_size);
    let next_cursor = if more_matches_available {
        matching_items
            .last()
            .and_then(|item| cursor_from_thread_item(&item.item, params.sort_key))
    } else {
        None
    }
    .as_ref()
    .and_then(|cursor| serde_json::to_value(cursor).ok())
    .and_then(|value| value.as_str().map(str::to_owned));

    let mut items = matching_items
        .into_iter()
        .filter_map(|item| {
            stored_thread_from_rollout_item(
                item.item,
                params.archived,
                store.config.default_model_provider_id.as_str(),
            )
            .map(|thread| StoredThreadSearchResult {
                thread,
                snippet: item.snippet,
            })
        })
        .collect::<Vec<_>>();
    set_thread_search_result_names(store, &mut items).await;

    Ok(ThreadSearchPage { items, next_cursor })
}

fn cursor_from_thread_item(
    item: &codex_rollout::ThreadItem,
    sort_key: ThreadSortKey,
) -> Option<codex_rollout::Cursor> {
    let timestamp = match sort_key {
        ThreadSortKey::CreatedAt => item.created_at.as_deref()?,
        ThreadSortKey::UpdatedAt => item.updated_at.as_deref().or(item.created_at.as_deref())?,
    };
    parse_cursor(timestamp)
}

async fn set_thread_search_result_names(
    store: &LocalThreadStore,
    items: &mut [StoredThreadSearchResult],
) {
    let thread_ids = items
        .iter()
        .map(|item| item.thread.thread_id)
        .collect::<HashSet<_>>();
    let mut names = HashMap::<ThreadId, String>::with_capacity(thread_ids.len());
    if let Some(state_db_ctx) = store.state_db().await {
        for &thread_id in &thread_ids {
            let Ok(Some(metadata)) = state_db_ctx.get_thread(thread_id).await else {
                continue;
            };
            if let Some(title) = distinct_thread_metadata_title(&metadata) {
                names.insert(thread_id, title);
            }
        }
    }
    if names.len() < thread_ids.len()
        && let Ok(legacy_names) =
            find_thread_names_by_ids(store.config.codex_home.as_path(), &thread_ids).await
    {
        for (thread_id, title) in legacy_names {
            names.entry(thread_id).or_insert(title);
        }
    }
    for item in items {
        if let Some(title) = names.get(&item.thread.thread_id).cloned() {
            set_thread_name_from_title(&mut item.thread, title);
        }
    }
}

#[cfg(test)]
mod tests {
    use codex_protocol::ThreadId;
    use pretty_assertions::assert_eq;
    use tempfile::TempDir;
    use uuid::Uuid;

    use super::*;
    use crate::ThreadStore;
    use crate::local::test_support::test_config;
    use crate::local::test_support::write_session_file_with;

    #[tokio::test]
    async fn search_threads_reads_matching_rollout_paths_not_state_db_listing() {
        let home = TempDir::new().expect("temp dir");
        let config = test_config(home.path());
        let runtime = codex_state::StateRuntime::init(
            config.sqlite_home.clone(),
            config.default_model_provider_id.clone(),
        )
        .await
        .expect("state db should initialize");
        runtime
            .mark_backfill_complete(/*last_watermark*/ None)
            .await
            .expect("backfill should be complete");
        let store = LocalThreadStore::new(config, Some(runtime));
        let uuid = Uuid::from_u128(1);
        let thread_id = ThreadId::from_string(&uuid.to_string()).expect("valid thread id");
        write_session_file_with(
            home.path(),
            home.path().join("sessions/2025/01/03"),
            "2025-01-03T12-00-00",
            uuid,
            "needle from rollout",
            Some("test-provider"),
        )
        .expect("session file");

        let page = store
            .search_threads(SearchThreadsParams {
                page_size: 10,
                cursor: None,
                sort_key: ThreadSortKey::CreatedAt,
                sort_direction: SortDirection::Desc,
                allowed_sources: Vec::new(),
                archived: false,
                search_term: "needle".to_string(),
            })
            .await
            .expect("thread search");

        assert_eq!(
            page.items
                .iter()
                .map(|item| item.thread.thread_id)
                .collect::<Vec<_>>(),
            vec![thread_id]
        );
    }
}

use std::collections::HashMap;
use std::collections::HashSet;

use codex_install_context::InstallContext;
use codex_protocol::ThreadId;
use codex_rollout::RolloutConfig;
use codex_rollout::find_thread_names_by_ids;
use codex_rollout::first_rollout_content_match_snippet;
use codex_rollout::parse_cursor;
use codex_rollout::search_rollout_matches;

use super::LocalThreadStore;
use super::helpers::distinct_thread_metadata_title;
use super::helpers::set_thread_name_from_title;
use super::helpers::stored_thread_from_rollout_item;
use super::list_threads::list_rollout_threads;
use crate::ListThreadsParams;
use crate::SearchThreadsParams;
use crate::SortDirection;
use crate::StoredThreadSearchResult;
use crate::ThreadSearchPage;
use crate::ThreadSortKey;
use crate::ThreadStoreError;
use crate::ThreadStoreResult;

#[cfg(test)]
#[path = "search_threads_tests.rs"]
mod tests;

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
    let rg_command = InstallContext::current().rg_command();
    let matching_rollouts = search_rollout_matches(
        rg_command.as_path(),
        store.config.codex_home.as_path(),
        params.archived,
        search_term,
    )
    .await
    .map_err(|err| ThreadStoreError::Internal {
        message: format!("failed to search rollout contents: {err}"),
    })?;
    if matching_rollouts.is_empty() {
        return Ok(ThreadSearchPage {
            items: Vec::new(),
            next_cursor: None,
        });
    }
    let mut matching_items = Vec::new();
    let mut page_cursor = cursor;
    let scan_page_size = params.page_size.saturating_mul(8).clamp(256, 2048);
    let scan_params = ListThreadsParams {
        page_size: scan_page_size,
        cursor: None,
        sort_key: params.sort_key,
        sort_direction: params.sort_direction,
        allowed_sources: params.allowed_sources.clone(),
        model_providers: None,
        cwd_filters: None,
        archived: params.archived,
        search_term: None,
        parent_thread_id: None,
        use_state_db_only: state_db.is_some(),
    };
    let mut remaining_rollouts = matching_rollouts;

    loop {
        let page = list_rollout_threads(
            state_db.clone(),
            &rollout_config,
            store.config.default_model_provider_id.as_str(),
            &scan_params,
            page_cursor.as_ref(),
            sort_key,
            sort_direction,
        )
        .await?;
        for item in page.items {
            let logical_path = codex_rollout::plain_rollout_path(item.path.as_path());
            let Some(snippet) = (match remaining_rollouts.remove(logical_path.as_path()) {
                Some(Some(snippet)) => Some(snippet),
                Some(None) => first_rollout_content_match_snippet(item.path.as_path(), search_term)
                    .await
                    .map_err(|err| ThreadStoreError::Internal {
                        message: format!("failed to read rollout search match: {err}"),
                    })?,
                None => None,
            }) else {
                continue;
            };
            matching_items.push(ThreadSearchItem { item, snippet });
            if matching_items.len() > params.page_size {
                break;
            }
        }
        page_cursor = page.next_cursor;
        if matching_items.len() > params.page_size
            || remaining_rollouts.is_empty()
            || page_cursor.is_none()
        {
            break;
        }
    }

    let more_matches_available = matching_items.len() > params.page_size;
    matching_items.truncate(params.page_size);
    let next_cursor = if more_matches_available {
        matching_items
            .last()
            .and_then(|item| cursor_from_thread_search_item(item, params.sort_key))
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

fn cursor_from_thread_search_item(
    item: &ThreadSearchItem,
    sort_key: ThreadSortKey,
) -> Option<codex_rollout::Cursor> {
    let timestamp = match sort_key {
        ThreadSortKey::CreatedAt => item.item.created_at.as_deref()?,
        ThreadSortKey::UpdatedAt => item
            .item
            .updated_at
            .as_deref()
            .or(item.item.created_at.as_deref())?,
        ThreadSortKey::RecencyAt => item
            .item
            .recency_at
            .as_deref()
            .or(item.item.updated_at.as_deref())
            .or(item.item.created_at.as_deref())?,
    };
    match sort_key {
        ThreadSortKey::RecencyAt => parse_cursor(&format!("{timestamp}|{}", item.item.thread_id?)),
        ThreadSortKey::CreatedAt | ThreadSortKey::UpdatedAt => parse_cursor(timestamp),
    }
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

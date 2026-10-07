//! Resolve a displayed session label through the app server before acting on its thread ID.

use std::path::Path;

use crate::app_server_session::AppServerSession;
use codex_app_server_client::TypedRequestError;
use codex_app_server_protocol::Thread;
use codex_app_server_protocol::ThreadHistoryMode;
use codex_app_server_protocol::ThreadListParams;
use codex_app_server_protocol::ThreadSortKey;
use codex_app_server_protocol::ThreadSourceKind;
use codex_protocol::ThreadId;
use color_eyre::eyre::Result;
use color_eyre::eyre::WrapErr;

#[derive(Clone, Copy)]
pub(super) enum SessionCollection {
    Active,
    Archived,
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum AmbiguousSessionName {
    #[error(
        "Multiple sessions match '{name}' (including {first_id} and {second_id}); use a session UUID to disambiguate."
    )]
    Multiple {
        name: String,
        first_id: String,
        second_id: String,
    },
    #[error(
        "Cannot verify a unique session label across server pages; matching session UUID: {0}. Use it only if this is the session you want."
    )]
    Paginated(String),
}

pub(super) fn display_label(thread: &Thread) -> &str {
    thread
        .name
        .as_deref()
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| thread.preview.trim())
}

/// Resolve server-listed labels and reject distinct matches before selecting an ID.
pub(super) async fn lookup(
    app_server: &mut AppServerSession,
    codex_home: &Path,
    name: &str,
    collections: &[SessionCollection],
    source_kind_filters: &[Vec<ThreadSourceKind>],
    model_provider: Option<&str>,
) -> Result<Option<Thread>> {
    if name.trim().is_empty() {
        return Ok(None);
    }
    let mut matched: Option<Thread> = None;
    let mut unverified_pagination = false;
    for collection in collections {
        for source_kinds in source_kind_filters {
            let mut cursor = None;
            loop {
                let response = app_server
                    .thread_list(ThreadListParams {
                        excluded_thread_ids: None,
                        originators: None,
                        cursor,
                        limit: Some(100),
                        // Activity must not move an unread thread ahead of the cursor.
                        sort_key: Some(ThreadSortKey::CreatedAt),
                        sort_direction: None,
                        model_providers: model_provider.map(|provider| vec![provider.to_string()]),
                        source_kinds: Some(source_kinds.clone()),
                        archived: Some(matches!(collection, SessionCollection::Archived)),
                        section_id: None,
                        project_id: None,
                        parent_thread_id: None,
                        ancestor_thread_id: None,
                        cwd: None,
                        use_state_db_only: true,
                        search_term: None,
                    })
                    .await
                    .wrap_err("failed to list sessions while resolving session label")?;
                unverified_pagination |= response.next_cursor.as_deref().is_some_and(|cursor| {
                    !cursor.contains('|') || codex_rollout::parse_cursor(cursor).is_none()
                });
                for thread in response.data {
                    if display_label(&thread) != name {
                        continue;
                    }
                    if !app_server.uses_remote_workspace()
                        && let Some(path) = thread.path.as_ref()
                    {
                        let expected_root = codex_home.join(match collection {
                            SessionCollection::Active => codex_rollout::SESSIONS_SUBDIR,
                            SessionCollection::Archived => codex_rollout::ARCHIVED_SESSIONS_SUBDIR,
                        });
                        if !path.starts_with(expected_root)
                            || (thread.history_mode == ThreadHistoryMode::Legacy
                                && codex_rollout::existing_rollout_path(path).await.is_none())
                        {
                            continue;
                        }
                    }
                    let thread_id = ThreadId::from_string(&thread.id).wrap_err_with(|| {
                        format!("app server returned invalid session id `{}`", thread.id)
                    })?;
                    let current = match app_server
                        .thread_read(thread_id, /*include_turns*/ false)
                        .await
                    {
                        Ok(current) => current,
                        Err(err) => {
                            let Some(TypedRequestError::Server { source, .. }) =
                                err.downcast_ref::<TypedRequestError>()
                            else {
                                return Err(err);
                            };
                            if source.message == format!("thread not loaded: {thread_id}") {
                                if app_server.uses_embedded_app_server() {
                                    continue;
                                }
                                thread.clone()
                            } else if (source.message.starts_with("failed to read thread: thread-store internal error: session metadata ")
                                && source.message.contains(" belongs to thread ")
                                && source.message.ends_with(&format!(", expected {thread_id}")))
                                || thread.path.as_ref().is_some_and(|path| {
                                    source.message.starts_with(&format!(
                                        "failed to read thread: thread-store internal error: failed to read session metadata {}: ",
                                        path.display()
                                    ))
                                })
                            {
                                continue;
                            } else {
                                return Err(err);
                            }
                        }
                    };
                    if current.id != thread.id || display_label(&current) != name {
                        continue;
                    }
                    if let Some(previous) = matched.as_ref()
                        && previous.id != current.id
                    {
                        return Err(AmbiguousSessionName::Multiple {
                            name: name.to_string(),
                            first_id: previous.id.clone(),
                            second_id: current.id,
                        }
                        .into());
                    }
                    matched = Some(current);
                }
                let Some(next_cursor) = response.next_cursor else {
                    break;
                };
                cursor = Some(next_cursor);
            }
        }
    }
    // Older server cursors can skip equal timestamps at a page boundary.
    if let Some(thread) = matched.as_ref()
        && unverified_pagination
    {
        return Err(AmbiguousSessionName::Paginated(thread.id.clone()).into());
    }
    Ok(matched)
}

#[cfg(test)]
#[path = "named_session_lookup_tests.rs"]
mod tests;

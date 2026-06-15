//! Projection, filtering, and pagination helpers for stored thread data.

use codex_app_server_protocol::GitInfo;
use codex_app_server_protocol::JSONRPCErrorError;
use codex_app_server_protocol::SortDirection;
use codex_app_server_protocol::Thread;
use codex_app_server_protocol::ThreadHistoryBuilder;
use codex_app_server_protocol::ThreadItem;
use codex_app_server_protocol::ThreadLoadedListResponse;
use codex_app_server_protocol::ThreadSourceKind;
use codex_app_server_protocol::ThreadStatus;
use codex_app_server_protocol::ThreadTurnsListResponse;
use codex_app_server_protocol::Turn;
use codex_app_server_protocol::TurnItemsView;
use codex_core::INTERACTIVE_SESSION_SOURCES;
use codex_core::path_utils;
use codex_protocol::ThreadId;
use codex_protocol::protocol::RolloutItem;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::SubAgentSource;
use codex_rollout::is_persisted_rollout_item;
use codex_thread_store::StoredThread;
use codex_thread_store::StoredThreadHistory;
use codex_utils_absolute_path::AbsolutePathBuf;
use tracing::warn;

use crate::error_code::internal_error;
use crate::error_code::invalid_request;

const THREAD_TURNS_DEFAULT_LIMIT: usize = 25;
const THREAD_TURNS_MAX_LIMIT: usize = 100;

/// Projects a stored thread into the app-server representation.
///
/// When stored history is present, the returned thread contains reconstructed
/// turns. Runtime-only status and active-turn state are intentionally omitted.
pub fn from_stored_thread(
    stored_thread: StoredThread,
    fallback_provider: &str,
    fallback_cwd: &AbsolutePathBuf,
) -> Thread {
    let (mut thread, history) =
        from_stored_thread_with_history(stored_thread, fallback_provider, fallback_cwd);
    if let Some(history) = history {
        thread.turns = build_api_turns_from_rollout_items(&history.items);
    }
    thread
}

pub(crate) fn from_stored_thread_with_history(
    stored_thread: StoredThread,
    fallback_provider: &str,
    fallback_cwd: &AbsolutePathBuf,
) -> (Thread, Option<StoredThreadHistory>) {
    let path = stored_thread.rollout_path;
    let git_info = stored_thread.git_info.map(|info| GitInfo {
        sha: info.commit_hash.map(|sha| sha.0),
        branch: info.branch,
        origin_url: info.repository_url,
    });
    let cwd = AbsolutePathBuf::relative_to_current_dir(path_utils::normalize_for_native_workdir(
        stored_thread.cwd,
    ))
    .unwrap_or_else(|err| {
        warn!("failed to normalize thread cwd while reading stored thread: {err}");
        fallback_cwd.clone()
    });
    let source = with_thread_spawn_agent_metadata(
        stored_thread.source,
        stored_thread.agent_nickname.clone(),
        stored_thread.agent_role.clone(),
    );
    let history = stored_thread.history;
    let thread_id = stored_thread.thread_id.to_string();
    let thread = Thread {
        id: thread_id.clone(),
        extra: None,
        session_id: thread_id,
        forked_from_id: stored_thread.forked_from_id.map(|id| id.to_string()),
        parent_thread_id: stored_thread.parent_thread_id.map(|id| id.to_string()),
        preview: stored_thread.preview,
        ephemeral: false,
        model_provider: if stored_thread.model_provider.is_empty() {
            fallback_provider.to_string()
        } else {
            stored_thread.model_provider
        },
        created_at: stored_thread.created_at.timestamp(),
        updated_at: stored_thread.updated_at.timestamp(),
        recency_at: Some(stored_thread.recency_at.timestamp()),
        status: ThreadStatus::NotLoaded,
        path,
        cwd,
        cli_version: stored_thread.cli_version,
        agent_nickname: source.get_nickname(),
        agent_role: source.get_agent_role(),
        source: source.into(),
        thread_source: stored_thread.thread_source.map(Into::into),
        git_info,
        name: stored_thread.name,
        turns: Vec::new(),
    };
    (thread, history)
}

pub(crate) fn build_api_turns_from_rollout_items(items: &[RolloutItem]) -> Vec<Turn> {
    let mut builder = ThreadHistoryBuilder::new();
    for item in items {
        if is_persisted_rollout_item(item) {
            builder.handle_rollout_item(item);
        }
    }
    builder.finish()
}

/// Source filtering policy shared by thread list and search projections.
#[derive(Clone, Debug)]
pub struct ThreadSourceFilter {
    store_sources: Vec<SessionSource>,
    source_kinds: Option<Vec<ThreadSourceKind>>,
}

impl ThreadSourceFilter {
    /// Creates a filter from requested app-server source kinds.
    ///
    /// An absent or empty request selects interactive sources. An empty value
    /// from [`store_sources`](Self::store_sources) means the backing store must
    /// query all sources and let [`matches`](Self::matches) apply the precise
    /// classification afterwards.
    pub fn new(source_kinds: Option<Vec<ThreadSourceKind>>) -> Self {
        let Some(source_kinds) = source_kinds.filter(|source_kinds| !source_kinds.is_empty())
        else {
            return Self {
                store_sources: INTERACTIVE_SESSION_SOURCES.to_vec(),
                source_kinds: None,
            };
        };

        let requires_post_filter = source_kinds.iter().any(|kind| {
            matches!(
                kind,
                ThreadSourceKind::Exec
                    | ThreadSourceKind::AppServer
                    | ThreadSourceKind::SubAgent
                    | ThreadSourceKind::SubAgentReview
                    | ThreadSourceKind::SubAgentCompact
                    | ThreadSourceKind::SubAgentThreadSpawn
                    | ThreadSourceKind::SubAgentOther
                    | ThreadSourceKind::Unknown
            )
        });
        let store_sources = if requires_post_filter {
            Vec::new()
        } else {
            source_kinds
                .iter()
                .filter_map(|kind| match kind {
                    ThreadSourceKind::Cli => Some(SessionSource::Cli),
                    ThreadSourceKind::VsCode => Some(SessionSource::VSCode),
                    ThreadSourceKind::Exec
                    | ThreadSourceKind::AppServer
                    | ThreadSourceKind::SubAgent
                    | ThreadSourceKind::SubAgentReview
                    | ThreadSourceKind::SubAgentCompact
                    | ThreadSourceKind::SubAgentThreadSpawn
                    | ThreadSourceKind::SubAgentOther
                    | ThreadSourceKind::Unknown => None,
                })
                .collect()
        };
        Self {
            store_sources,
            source_kinds: Some(source_kinds),
        }
    }

    /// Returns the coarse source set suitable for a backing-store query.
    pub fn store_sources(&self) -> &[SessionSource] {
        &self.store_sources
    }

    /// Returns whether a projected source satisfies the precise filter.
    pub fn matches(&self, source: &SessionSource) -> bool {
        self.source_kinds.as_ref().is_none_or(|source_kinds| {
            source_kinds.iter().any(|kind| match kind {
                ThreadSourceKind::Cli => matches!(source, SessionSource::Cli),
                ThreadSourceKind::VsCode => matches!(source, SessionSource::VSCode),
                ThreadSourceKind::Exec => matches!(source, SessionSource::Exec),
                ThreadSourceKind::AppServer => matches!(source, SessionSource::Mcp),
                ThreadSourceKind::SubAgent => matches!(source, SessionSource::SubAgent(_)),
                ThreadSourceKind::SubAgentReview => {
                    matches!(source, SessionSource::SubAgent(SubAgentSource::Review))
                }
                ThreadSourceKind::SubAgentCompact => {
                    matches!(source, SessionSource::SubAgent(SubAgentSource::Compact))
                }
                ThreadSourceKind::SubAgentThreadSpawn => matches!(
                    source,
                    SessionSource::SubAgent(SubAgentSource::ThreadSpawn { .. })
                ),
                ThreadSourceKind::SubAgentOther => {
                    matches!(source, SessionSource::SubAgent(SubAgentSource::Other(_)))
                }
                ThreadSourceKind::Unknown => matches!(source, SessionSource::Unknown),
            })
        })
    }
}

/// Sorts and paginates a snapshot of loaded thread IDs.
pub fn paginate_loaded_thread_ids(
    mut data: Vec<String>,
    cursor: Option<&str>,
    limit: Option<u32>,
) -> Result<ThreadLoadedListResponse, JSONRPCErrorError> {
    if data.is_empty() {
        return Ok(ThreadLoadedListResponse {
            data,
            next_cursor: None,
        });
    }

    data.sort();
    let total = data.len();
    let start = match cursor {
        Some(cursor) => {
            let cursor = ThreadId::from_string(cursor)
                .map_err(|_| invalid_request(format!("invalid cursor: {cursor}")))?
                .to_string();
            match data.binary_search(&cursor) {
                Ok(index) => index + 1,
                Err(index) => index,
            }
        }
        None => 0,
    };
    let limit = limit.unwrap_or(total as u32).max(1) as usize;
    let end = start.saturating_add(limit).min(total);
    let data = data.into_iter().skip(start).take(limit).collect::<Vec<_>>();
    let next_cursor = data.last().filter(|_| end < total).cloned();
    Ok(ThreadLoadedListResponse { data, next_cursor })
}

/// Applies an item view and paginates reconstructed turns.
pub fn paginate_turns(
    mut turns: Vec<Turn>,
    cursor: Option<&str>,
    limit: Option<u32>,
    sort_direction: SortDirection,
    items_view: TurnItemsView,
) -> Result<ThreadTurnsListResponse, JSONRPCErrorError> {
    apply_turn_items_view(&mut turns, items_view);
    let page = paginate_turns_inner(turns, cursor, limit, sort_direction)?;
    Ok(ThreadTurnsListResponse {
        data: page.turns,
        next_cursor: page.next_cursor,
        backwards_cursor: page.backwards_cursor,
    })
}

struct TurnsPage {
    turns: Vec<Turn>,
    next_cursor: Option<String>,
    backwards_cursor: Option<String>,
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct TurnsCursor {
    turn_id: String,
    include_anchor: bool,
}

fn paginate_turns_inner(
    turns: Vec<Turn>,
    cursor: Option<&str>,
    limit: Option<u32>,
    sort_direction: SortDirection,
) -> Result<TurnsPage, JSONRPCErrorError> {
    if turns.is_empty() {
        return Ok(TurnsPage {
            turns: Vec::new(),
            next_cursor: None,
            backwards_cursor: None,
        });
    }

    let anchor = cursor.map(parse_turns_cursor).transpose()?;
    let page_size = limit
        .map(|value| value as usize)
        .unwrap_or(THREAD_TURNS_DEFAULT_LIMIT)
        .clamp(1, THREAD_TURNS_MAX_LIMIT);
    let anchor_index = anchor
        .as_ref()
        .and_then(|anchor| turns.iter().position(|turn| turn.id == anchor.turn_id));
    if anchor.is_some() && anchor_index.is_none() {
        return Err(invalid_request(
            "invalid cursor: anchor turn is no longer present",
        ));
    }

    let mut keyed_turns: Vec<_> = turns.into_iter().enumerate().collect();
    match sort_direction {
        SortDirection::Asc => {
            if let (Some(anchor), Some(anchor_index)) = (anchor.as_ref(), anchor_index) {
                keyed_turns.retain(|(index, _)| {
                    if anchor.include_anchor {
                        *index >= anchor_index
                    } else {
                        *index > anchor_index
                    }
                });
            }
        }
        SortDirection::Desc => {
            keyed_turns.reverse();
            if let (Some(anchor), Some(anchor_index)) = (anchor.as_ref(), anchor_index) {
                keyed_turns.retain(|(index, _)| {
                    if anchor.include_anchor {
                        *index <= anchor_index
                    } else {
                        *index < anchor_index
                    }
                });
            }
        }
    }

    let more_turns_available = keyed_turns.len() > page_size;
    keyed_turns.truncate(page_size);
    let backwards_cursor = keyed_turns
        .first()
        .map(|(_, turn)| serialize_turns_cursor(&turn.id, /*include_anchor*/ true))
        .transpose()?;
    let next_cursor = if more_turns_available {
        keyed_turns
            .last()
            .map(|(_, turn)| serialize_turns_cursor(&turn.id, /*include_anchor*/ false))
            .transpose()?
    } else {
        None
    };
    let turns = keyed_turns.into_iter().map(|(_, turn)| turn).collect();

    Ok(TurnsPage {
        turns,
        next_cursor,
        backwards_cursor,
    })
}

fn serialize_turns_cursor(
    turn_id: &str,
    include_anchor: bool,
) -> Result<String, JSONRPCErrorError> {
    serde_json::to_string(&TurnsCursor {
        turn_id: turn_id.to_string(),
        include_anchor,
    })
    .map_err(|err| internal_error(format!("failed to serialize cursor: {err}")))
}

fn parse_turns_cursor(cursor: &str) -> Result<TurnsCursor, JSONRPCErrorError> {
    serde_json::from_str(cursor).map_err(|_| invalid_request(format!("invalid cursor: {cursor}")))
}

fn apply_turn_items_view(turns: &mut [Turn], items_view: TurnItemsView) {
    for turn in turns {
        match items_view {
            TurnItemsView::NotLoaded => {
                turn.items.clear();
                turn.items_view = TurnItemsView::NotLoaded;
            }
            TurnItemsView::Summary => {
                let first_user_message = turn
                    .items
                    .iter()
                    .find(|item| matches!(item, ThreadItem::UserMessage { .. }))
                    .cloned();
                let final_agent_message = turn
                    .items
                    .iter()
                    .rev()
                    .find(|item| matches!(item, ThreadItem::AgentMessage { .. }))
                    .cloned();
                turn.items = match (first_user_message, final_agent_message) {
                    (Some(user_message), Some(agent_message))
                        if user_message.id() != agent_message.id() =>
                    {
                        vec![user_message, agent_message]
                    }
                    (Some(user_message), _) => vec![user_message],
                    (None, Some(agent_message)) => vec![agent_message],
                    (None, None) => Vec::new(),
                };
                turn.items_view = TurnItemsView::Summary;
            }
            TurnItemsView::Full => {
                turn.items_view = TurnItemsView::Full;
            }
        }
    }
}

pub(crate) fn with_thread_spawn_agent_metadata(
    source: SessionSource,
    agent_nickname: Option<String>,
    agent_role: Option<String>,
) -> SessionSource {
    if agent_nickname.is_none() && agent_role.is_none() {
        return source;
    }

    match source {
        SessionSource::SubAgent(SubAgentSource::ThreadSpawn {
            parent_thread_id,
            depth,
            agent_path,
            agent_nickname: existing_agent_nickname,
            agent_role: existing_agent_role,
        }) => SessionSource::SubAgent(SubAgentSource::ThreadSpawn {
            parent_thread_id,
            depth,
            agent_path,
            agent_nickname: agent_nickname.or(existing_agent_nickname),
            agent_role: agent_role.or(existing_agent_role),
        }),
        _ => source,
    }
}

#[cfg(test)]
#[path = "thread_views_tests.rs"]
mod tests;

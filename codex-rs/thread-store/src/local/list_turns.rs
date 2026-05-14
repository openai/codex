use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::RolloutItem;
use codex_protocol::protocol::TurnContextItem;
use serde::Deserialize;
use serde::Serialize;

use super::LocalThreadStore;
use super::read_thread;
use crate::ListTurnsParams;
use crate::ReadThreadParams;
use crate::SortDirection;
use crate::StoredTurn;
use crate::StoredTurnError;
use crate::StoredTurnItemsView;
use crate::StoredTurnStatus;
use crate::ThreadStoreError;
use crate::ThreadStoreResult;
use crate::TurnPage;

const DEFAULT_TURN_PAGE_SIZE: usize = 50;
const MAX_TURN_PAGE_SIZE: usize = 200;

pub(super) async fn list_turns(
    store: &LocalThreadStore,
    params: ListTurnsParams,
) -> ThreadStoreResult<TurnPage> {
    let thread = read_thread::read_thread(
        store,
        ReadThreadParams {
            thread_id: params.thread_id,
            include_archived: params.include_archived,
            include_history: true,
        },
    )
    .await?;
    let history = thread.history.ok_or_else(|| ThreadStoreError::Internal {
        message: format!("failed to load history for thread {}", params.thread_id),
    })?;
    let turns = build_stored_turns_from_rollout_items(&history.items, params.items_view);
    paginate_turns(
        turns,
        params.cursor.as_deref(),
        params.page_size,
        params.sort_direction,
    )
}

fn build_stored_turns_from_rollout_items(
    items: &[RolloutItem],
    items_view: StoredTurnItemsView,
) -> Vec<StoredTurn> {
    let mut builder = StoredTurnBuilder::new(items_view);
    for (rollout_index, item) in items.iter().enumerate() {
        builder.handle_rollout_item(rollout_index, item);
    }
    builder.finish()
}

struct StoredTurnBuilder {
    turns: Vec<PendingStoredTurn>,
    current_turn: Option<PendingStoredTurn>,
    items_view: StoredTurnItemsView,
}

impl StoredTurnBuilder {
    fn new(items_view: StoredTurnItemsView) -> Self {
        Self {
            turns: Vec::new(),
            current_turn: None,
            items_view,
        }
    }

    fn finish(mut self) -> Vec<StoredTurn> {
        self.finish_current_turn();
        self.turns
            .into_iter()
            .map(|turn| turn.into_stored_turn(self.items_view))
            .collect()
    }

    fn handle_rollout_item(&mut self, rollout_index: usize, item: &RolloutItem) {
        match item {
            RolloutItem::SessionMeta(_) => {}
            RolloutItem::TurnContext(payload) => {
                self.handle_turn_context(rollout_index, payload, item);
            }
            RolloutItem::Compacted(_) => {
                let turn = self.ensure_turn(rollout_index);
                turn.saw_compaction = true;
                turn.items.push(item.clone());
            }
            RolloutItem::ResponseItem(payload) => {
                if response_item_starts_turn(payload) {
                    self.maybe_finish_implicit_turn();
                }
                self.ensure_turn(rollout_index).items.push(item.clone());
            }
            RolloutItem::EventMsg(payload) => self.handle_event(rollout_index, payload, item),
        }
    }

    fn handle_turn_context(
        &mut self,
        rollout_index: usize,
        payload: &TurnContextItem,
        item: &RolloutItem,
    ) {
        if let Some(turn_id) = payload.turn_id.as_deref()
            && let Some(turn) = self.find_turn_mut(turn_id)
        {
            turn.items.push(item.clone());
            return;
        }
        let turn = self.ensure_turn(rollout_index);
        if turn.items.is_empty()
            && let Some(turn_id) = payload.turn_id.as_deref()
        {
            turn.turn_id = turn_id.to_string();
        }
        turn.items.push(item.clone());
    }

    fn handle_event(&mut self, rollout_index: usize, event: &EventMsg, item: &RolloutItem) {
        match event {
            EventMsg::TurnStarted(payload) => {
                self.finish_current_turn();
                let mut turn = PendingStoredTurn::new(payload.turn_id.clone());
                turn.status = StoredTurnStatus::InProgress;
                turn.started_at = payload.started_at;
                turn.opened_explicitly = true;
                turn.items.push(item.clone());
                self.current_turn = Some(turn);
            }
            EventMsg::TurnComplete(payload) => {
                let should_finish_current = self
                    .current_turn
                    .as_ref()
                    .is_some_and(|turn| turn.turn_id == payload.turn_id);
                let turn = self.turn_for_completion(rollout_index, payload.turn_id.as_str());
                turn.items.push(item.clone());
                if matches!(
                    turn.status,
                    StoredTurnStatus::Completed | StoredTurnStatus::InProgress
                ) {
                    turn.status = StoredTurnStatus::Completed;
                }
                turn.completed_at = payload.completed_at;
                turn.duration_ms = payload.duration_ms;
                if should_finish_current {
                    self.finish_current_turn();
                }
            }
            EventMsg::TurnAborted(payload) => {
                let should_finish_current = payload.turn_id.as_ref().is_none_or(|turn_id| {
                    self.current_turn
                        .as_ref()
                        .is_some_and(|turn| turn.turn_id == *turn_id)
                });
                let turn = match payload.turn_id.as_deref() {
                    Some(turn_id) => self.turn_for_completion(rollout_index, turn_id),
                    None => self.ensure_turn(rollout_index),
                };
                turn.items.push(item.clone());
                turn.status = StoredTurnStatus::Interrupted;
                turn.completed_at = payload.completed_at;
                turn.duration_ms = payload.duration_ms;
                if should_finish_current {
                    self.finish_current_turn();
                }
            }
            EventMsg::ThreadRolledBack(payload) => {
                self.finish_current_turn();
                let n = usize::try_from(payload.num_turns).unwrap_or(usize::MAX);
                if n >= self.turns.len() {
                    self.turns.clear();
                } else {
                    self.turns.truncate(self.turns.len().saturating_sub(n));
                }
            }
            EventMsg::UserMessage(_) => {
                self.maybe_finish_implicit_turn();
                self.ensure_turn(rollout_index).items.push(item.clone());
            }
            EventMsg::Error(payload) => {
                let turn = self.ensure_turn(rollout_index);
                turn.items.push(item.clone());
                if payload.affects_turn_status() {
                    turn.status = StoredTurnStatus::Failed;
                    turn.error = Some(StoredTurnError {
                        message: payload.message.clone(),
                        additional_details: None,
                    });
                }
            }
            _ => {
                self.ensure_turn(rollout_index).items.push(item.clone());
            }
        }
    }

    fn ensure_turn(&mut self, rollout_index: usize) -> &mut PendingStoredTurn {
        self.current_turn
            .get_or_insert_with(|| PendingStoredTurn::new(format!("rollout-{rollout_index}")))
    }

    fn finish_current_turn(&mut self) {
        if let Some(turn) = self.current_turn.take()
            && (!turn.items.is_empty() || turn.opened_explicitly || turn.saw_compaction)
        {
            self.turns.push(turn);
        }
    }

    fn maybe_finish_implicit_turn(&mut self) {
        if self
            .current_turn
            .as_ref()
            .is_some_and(|turn| !turn.opened_explicitly && !turn.items.is_empty())
        {
            self.finish_current_turn();
        }
    }

    fn turn_for_completion(
        &mut self,
        rollout_index: usize,
        turn_id: &str,
    ) -> &mut PendingStoredTurn {
        if self
            .current_turn
            .as_ref()
            .is_some_and(|turn| turn.turn_id == turn_id)
        {
            let Some(turn) = self.current_turn.as_mut() else {
                unreachable!("current turn exists after matching above");
            };
            return turn;
        }
        if let Some(index) = self.turns.iter().position(|turn| turn.turn_id == turn_id) {
            return &mut self.turns[index];
        }
        self.ensure_turn(rollout_index)
    }

    fn find_turn_mut(&mut self, turn_id: &str) -> Option<&mut PendingStoredTurn> {
        if self
            .current_turn
            .as_ref()
            .is_some_and(|turn| turn.turn_id == turn_id)
        {
            return self.current_turn.as_mut();
        }
        self.turns.iter_mut().find(|turn| turn.turn_id == turn_id)
    }
}

struct PendingStoredTurn {
    turn_id: String,
    items: Vec<RolloutItem>,
    status: StoredTurnStatus,
    error: Option<StoredTurnError>,
    started_at: Option<i64>,
    completed_at: Option<i64>,
    duration_ms: Option<i64>,
    opened_explicitly: bool,
    saw_compaction: bool,
}

impl PendingStoredTurn {
    fn new(turn_id: String) -> Self {
        Self {
            turn_id,
            items: Vec::new(),
            status: StoredTurnStatus::Completed,
            error: None,
            started_at: None,
            completed_at: None,
            duration_ms: None,
            opened_explicitly: false,
            saw_compaction: false,
        }
    }

    fn into_stored_turn(self, items_view: StoredTurnItemsView) -> StoredTurn {
        StoredTurn {
            turn_id: self.turn_id,
            items: filter_items_for_view(self.items, items_view),
            items_view,
            status: self.status,
            error: self.error,
            started_at: self.started_at,
            completed_at: self.completed_at,
            duration_ms: self.duration_ms,
        }
    }
}

fn filter_items_for_view(
    items: Vec<RolloutItem>,
    items_view: StoredTurnItemsView,
) -> Vec<RolloutItem> {
    match items_view {
        StoredTurnItemsView::NotLoaded => Vec::new(),
        StoredTurnItemsView::Summary => summary_items(items),
        StoredTurnItemsView::Full
        | StoredTurnItemsView::ResponseItems
        | StoredTurnItemsView::EventItems
        | StoredTurnItemsView::ResponseAndEventItems => items
            .into_iter()
            .filter(|item| items_view.includes_persisted_item(item))
            .collect(),
    }
}

fn summary_items(items: Vec<RolloutItem>) -> Vec<RolloutItem> {
    let first_user_index = items.iter().position(is_user_message_item);
    let final_agent_index = items.iter().rposition(is_agent_message_item);
    let mut summary = Vec::new();
    if let Some(index) = first_user_index {
        summary.push(items[index].clone());
    }
    if let Some(index) = final_agent_index
        && Some(index) != first_user_index
    {
        summary.push(items[index].clone());
    }
    summary
}

fn response_item_starts_turn(item: &ResponseItem) -> bool {
    matches!(
        item,
        ResponseItem::Message { role, .. } if role == "user"
    )
}

fn is_user_message_item(item: &RolloutItem) -> bool {
    match item {
        RolloutItem::EventMsg(EventMsg::UserMessage(_)) => true,
        RolloutItem::ResponseItem(ResponseItem::Message { role, .. }) => role == "user",
        _ => false,
    }
}

fn is_agent_message_item(item: &RolloutItem) -> bool {
    match item {
        RolloutItem::EventMsg(EventMsg::AgentMessage(_)) => true,
        RolloutItem::ResponseItem(ResponseItem::Message { role, .. }) => role == "assistant",
        _ => false,
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TurnCursor {
    turn_id: String,
    include_anchor: bool,
}

fn paginate_turns(
    turns: Vec<StoredTurn>,
    cursor: Option<&str>,
    page_size: usize,
    sort_direction: SortDirection,
) -> ThreadStoreResult<TurnPage> {
    if turns.is_empty() {
        return Ok(TurnPage {
            turns: Vec::new(),
            next_cursor: None,
            backwards_cursor: None,
        });
    }

    let anchor = cursor.map(parse_turn_cursor).transpose()?;
    let page_size = if page_size == 0 {
        DEFAULT_TURN_PAGE_SIZE
    } else {
        page_size
    }
    .clamp(1, MAX_TURN_PAGE_SIZE);

    let anchor_index = anchor
        .as_ref()
        .and_then(|anchor| turns.iter().position(|turn| turn.turn_id == anchor.turn_id));
    if anchor.is_some() && anchor_index.is_none() {
        return Err(ThreadStoreError::InvalidRequest {
            message: "invalid cursor: anchor turn is no longer present".to_string(),
        });
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
        .map(|(_, turn)| serialize_turn_cursor(&turn.turn_id, /*include_anchor*/ true))
        .transpose()?;
    let next_cursor = if more_turns_available {
        keyed_turns
            .last()
            .map(|(_, turn)| serialize_turn_cursor(&turn.turn_id, /*include_anchor*/ false))
            .transpose()?
    } else {
        None
    };
    let turns = keyed_turns.into_iter().map(|(_, turn)| turn).collect();

    Ok(TurnPage {
        turns,
        next_cursor,
        backwards_cursor,
    })
}

fn serialize_turn_cursor(turn_id: &str, include_anchor: bool) -> ThreadStoreResult<String> {
    serde_json::to_string(&TurnCursor {
        turn_id: turn_id.to_string(),
        include_anchor,
    })
    .map_err(|err| ThreadStoreError::Internal {
        message: format!("failed to serialize cursor: {err}"),
    })
}

fn parse_turn_cursor(cursor: &str) -> ThreadStoreResult<TurnCursor> {
    serde_json::from_str(cursor).map_err(|_| ThreadStoreError::InvalidRequest {
        message: format!("invalid cursor: {cursor}"),
    })
}

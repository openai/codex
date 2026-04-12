//! SQLite-backed runtime bridge for thread timers and queued thread messages.
//!
//! This module connects [`Session`] to the persistent state database, keeps the
//! in-memory timer scheduler reconciled with cross-instance changes, and
//! converts claimed timers/messages into generated model input plus
//! transcript-safe delivery events.
//!
//! Timer and queued-message delivery must be single-consumer across all harness
//! instances for a thread, even though those instances share the same SQLite
//! state database. In other words, if two app or CLI processes are attached to
//! the same thread, a due timer or queued message should be injected by at most
//! one of them.
//!
//! The database is the authority for that guarantee. Before this module
//! delivers a queued message, it calls into the state layer to atomically claim
//! and remove the next eligible row. Timers are first selected from local
//! memory, but delivery proceeds only if the matching SQLite claim also wins:
//! one-shot timers are deleted as part of the claim, and recurring timers are
//! updated with the expected previous run timestamp so competing instances
//! cannot both observe and persist the same run. If another instance wins the
//! database race, this runtime refreshes its local timer view from SQLite and
//! skips delivery.
//!
//! The local `timer_start_in_progress` flag is still useful, but only as an
//! in-process guard. It prevents this [`Session`] from starting multiple pending
//! timer/message deliveries concurrently; cross-process exclusivity comes from
//! the SQLite claim operations above.

use super::BackgroundEventEvent;
use super::Event;
use super::EventMsg;
use super::INITIAL_SUBMIT_ID;
use super::Session;
use crate::injected_message::MessageInvocationContext;
use crate::injected_message::MessageInvocationKind;
use crate::injected_message::MessagePayload;
use crate::injected_message::db_message_to_thread_message;
use crate::injected_message::injected_message_event;
use crate::injected_message::message_prompt_input_item;
use crate::injected_message::validate_meta;
use crate::pending_input::GeneratedMessageInput;
use crate::pending_input::PendingInputItem;
use crate::timers::ClaimedTimer;
use crate::timers::CreateTimer;
use crate::timers::MAX_ACTIVE_TIMERS_PER_THREAD;
use crate::timers::PersistedTimer;
use crate::timers::RecurringTimerPolicy;
use crate::timers::RestoredTimerTask;
use crate::timers::TIMER_FIRED_BACKGROUND_EVENT_PREFIX;
use crate::timers::TIMER_UPDATED_BACKGROUND_EVENT_PREFIX;
use crate::timers::ThreadTimer;
use crate::timers::ThreadTimerTrigger;
use crate::timers::TimerDelivery;
use crate::timers::TimerTaskSpec;
use crate::timers::TimersState;
use crate::timers::timer_message_invocation_context;
use chrono::Utc;
use codex_features::Feature;
use codex_rollout::state_db;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;
use tokio_util::sync::CancellationToken;
use tracing::warn;

const TIMER_SOURCE_AGENT: &str = "agent";
const TIMER_CLIENT_ID_FALLBACK: &str = "codex-cli";
const TIMER_DB_SYNC_INTERVAL: Duration = Duration::from_secs(15);
const TIMER_DB_MAX_REFRESH_INTERVAL: Duration = Duration::from_secs(60);

enum PendingMessageStart {
    Started,
    NotReady,
    None,
}

enum PendingMessageClaim {
    Claimed(Box<PendingInputItem>, TimerDelivery),
    NotReady,
}

fn db_timer_to_persisted_timer(row: codex_state::ThreadTimer) -> Option<PersistedTimer> {
    let trigger = match serde_json::from_str(&row.trigger_json) {
        Ok(trigger) => trigger,
        Err(err) => {
            warn!("skipping invalid persisted timer {} trigger: {err}", row.id);
            return None;
        }
    };
    let delivery =
        match serde_json::from_value::<TimerDelivery>(serde_json::Value::String(row.delivery)) {
            Ok(delivery) => delivery,
            Err(err) => {
                warn!(
                    "skipping invalid persisted timer {} delivery: {err}",
                    row.id
                );
                return None;
            }
        };
    let meta = match serde_json::from_str(&row.meta_json) {
        Ok(meta) => meta,
        Err(err) => {
            warn!(
                "skipping invalid persisted timer {} metadata: {err}",
                row.id
            );
            return None;
        }
    };
    if let Err(err) = validate_meta(&meta) {
        warn!(
            "skipping invalid persisted timer {} metadata: {err}",
            row.id
        );
        return None;
    }
    Some(PersistedTimer {
        timer: ThreadTimer {
            id: row.id,
            trigger,
            content: row.content,
            instructions: row.instructions,
            meta,
            delivery,
            created_at: row.created_at,
            next_run_at: row.next_run_at,
            last_run_at: row.last_run_at,
        },
        pending_run: row.pending_run,
    })
}

impl Session {
    pub(crate) async fn list_timers(self: &Arc<Self>) -> Vec<ThreadTimer> {
        if !self.timers_feature_enabled() {
            return Vec::new();
        }
        self.sync_timers_from_db(/*emit_update*/ false).await;
        self.list_timers_from_memory().await
    }

    async fn list_timers_from_memory(&self) -> Vec<ThreadTimer> {
        self.timers.lock().await.list_timers()
    }

    pub(crate) async fn create_timer(
        self: &Arc<Self>,
        trigger: ThreadTimerTrigger,
        payload: MessagePayload,
        delivery: TimerDelivery,
    ) -> Result<ThreadTimer, String> {
        if !self.timers_feature_enabled() {
            return Err("timers feature is disabled".to_string());
        }
        validate_meta(&payload.meta)?;
        self.ensure_rollout_materialized().await;
        let state_db = self.timer_state_db().await?;
        self.start_timer_db_sync_task(state_db.clone());

        let timer_cancel = CancellationToken::new();
        let id = uuid::Uuid::new_v4().to_string();
        let (timer, persisted_timer, timer_spec) = {
            let mut timers = self.timers.lock().await;
            let (timer, timer_spec) = timers.create_timer(
                CreateTimer {
                    id: id.clone(),
                    trigger,
                    payload,
                    delivery,
                    now: Utc::now(),
                },
                Some(timer_cancel.clone()),
            )?;
            let persisted_timer = timers
                .persisted_timer(&id)
                .ok_or_else(|| format!("created timer {id} was not stored in memory"))?;
            (timer, persisted_timer, timer_spec)
        };
        let params = self
            .thread_timer_create_params(&persisted_timer, TIMER_SOURCE_AGENT)
            .await?;
        match state_db
            .create_thread_timer_if_below_limit(&params, MAX_ACTIVE_TIMERS_PER_THREAD)
            .await
        {
            Ok(true) => {}
            Ok(false) => {
                if let Some(runtime) = self.timers.lock().await.remove_timer(&id) {
                    TimersState::cancel_runtime(&runtime);
                }
                return Err(format!(
                    "too many active timers; each thread supports at most {MAX_ACTIVE_TIMERS_PER_THREAD} timers"
                ));
            }
            Err(err) => {
                if let Some(runtime) = self.timers.lock().await.remove_timer(&id) {
                    TimersState::cancel_runtime(&runtime);
                }
                return Err(format!("failed to persist timer to sqlite: {err}"));
            }
        }

        if let Some(timer_spec) = timer_spec {
            self.spawn_timer_task(id, timer_spec, timer_cancel);
        }
        self.emit_timer_updated_notification().await;
        self.maybe_start_pending_timer().await;
        Ok(timer)
    }

    pub(crate) async fn delete_timer(self: &Arc<Self>, id: &str) -> Result<bool, String> {
        if !self.timers_feature_enabled() {
            return Err("timers feature is disabled".to_string());
        }
        self.sync_timers_from_db(/*emit_update*/ false).await;
        let state_db = self.timer_state_db().await?;
        self.start_timer_db_sync_task(state_db.clone());

        let deleted = match state_db
            .delete_thread_timer(&self.thread_id_string(), id)
            .await
        {
            Ok(deleted) => deleted,
            Err(err) => return Err(format!("failed to delete timer from sqlite: {err}")),
        };
        let runtime = self.timers.lock().await.remove_timer(id);
        let Some(runtime) = runtime else {
            return Ok(deleted);
        };
        TimersState::cancel_runtime(&runtime);
        self.emit_timer_updated_notification().await;
        Ok(deleted)
    }

    pub(crate) async fn maybe_start_pending_timer(self: &Arc<Self>) {
        if self
            .try_start_pending_timer(RecurringTimerPolicy::IncludeOnlyNeverRun)
            .await
        {
            return;
        }
        match self.maybe_start_pending_message().await {
            PendingMessageStart::Started | PendingMessageStart::NotReady => return,
            PendingMessageStart::None => {}
        }
        self.try_start_pending_timer(RecurringTimerPolicy::IncludeAll)
            .await;
    }

    async fn try_start_pending_timer(
        self: &Arc<Self>,
        recurring_timer_policy: RecurringTimerPolicy,
    ) -> bool {
        let Some(ClaimedTimer {
            timer,
            context,
            deleted_one_shot_timer,
            ..
        }) = self
            .claim_next_timer_for_delivery(recurring_timer_policy)
            .await
        else {
            return false;
        };

        self.emit_timer_fired_notification(&timer).await;
        if deleted_one_shot_timer {
            self.emit_timer_updated_notification().await;
        }
        let message_context = timer_message_invocation_context(&context);
        let input_item = PendingInputItem::GeneratedMessage(GeneratedMessageInput {
            item: message_prompt_input_item(&message_context),
            injected_event: injected_message_event(&message_context),
        });
        match context.delivery {
            TimerDelivery::SteerCurrentTurn => {
                if !self.inject_timer_into_active_turn(input_item.clone()).await {
                    self.queue_pending_input_for_next_turn(vec![input_item])
                        .await;
                    self.maybe_start_turn_for_pending_work().await;
                }
            }
            TimerDelivery::AfterTurn => {
                self.queue_pending_input_for_next_turn(vec![input_item])
                    .await;
                self.maybe_start_turn_for_pending_work().await;
            }
        }
        *self.timer_start_in_progress.lock().await = false;
        true
    }

    async fn maybe_start_pending_message(self: &Arc<Self>) -> PendingMessageStart {
        let Some(claim) = self.claim_next_message_for_delivery().await else {
            return PendingMessageStart::None;
        };
        let PendingMessageClaim::Claimed(input_item, delivery) = claim else {
            return PendingMessageStart::NotReady;
        };
        let input_item = *input_item;

        match delivery {
            TimerDelivery::SteerCurrentTurn => {
                if !self
                    .inject_message_into_active_turn(input_item.clone())
                    .await
                {
                    self.queue_pending_input_for_next_turn(vec![input_item])
                        .await;
                    self.maybe_start_turn_for_pending_work().await;
                }
            }
            TimerDelivery::AfterTurn => {
                self.queue_pending_input_for_next_turn(vec![input_item])
                    .await;
                self.maybe_start_turn_for_pending_work().await;
            }
        }
        *self.timer_start_in_progress.lock().await = false;
        PendingMessageStart::Started
    }

    async fn claim_next_message_for_delivery(self: &Arc<Self>) -> Option<PendingMessageClaim> {
        if !self.queued_messages_feature_enabled() {
            return None;
        }
        let mut timer_start_in_progress = self.timer_start_in_progress.lock().await;
        if *timer_start_in_progress {
            return None;
        }
        *timer_start_in_progress = true;
        drop(timer_start_in_progress);

        let has_pending_turn_inputs = self.has_queued_response_items_for_next_turn().await
            || self.has_trigger_turn_mailbox_items().await;
        let (has_active_turn, active_turn_is_regular) = {
            let active_turn = self.active_turn.lock().await;
            let has_active_turn = active_turn.is_some();
            let active_turn_is_regular = active_turn
                .as_ref()
                .and_then(|turn| turn.tasks.first())
                .is_some_and(|(_, task)| matches!(task.kind, crate::state::TaskKind::Regular));
            (has_active_turn, active_turn_is_regular)
        };
        let can_after_turn = !has_active_turn && !has_pending_turn_inputs;
        let can_steer_current_turn = active_turn_is_regular;
        let state_db = match self.timer_state_db().await {
            Ok(state_db) => state_db,
            Err(err) => {
                warn!("failed to claim queued message from sqlite: {err}");
                *self.timer_start_in_progress.lock().await = false;
                return None;
            }
        };
        self.start_timer_db_sync_task(state_db.clone());

        loop {
            let claim = match state_db
                .claim_next_thread_message(
                    &self.thread_id_string(),
                    can_after_turn,
                    can_steer_current_turn,
                )
                .await
            {
                Ok(claim) => claim,
                Err(err) => {
                    warn!("failed to claim queued message from sqlite: {err}");
                    *self.timer_start_in_progress.lock().await = false;
                    return None;
                }
            };
            match claim {
                Some(codex_state::ThreadMessageClaim::Claimed(row)) => {
                    let message = match db_message_to_thread_message(row) {
                        Ok(message) => message,
                        Err(err) => {
                            warn!("{err}");
                            continue;
                        }
                    };
                    let delivery = message.delivery;
                    let message_context = MessageInvocationContext {
                        kind: MessageInvocationKind::External,
                        source: message.source.clone(),
                        content: message.content,
                        instructions: None,
                    };
                    let input_item = PendingInputItem::GeneratedMessage(GeneratedMessageInput {
                        item: message_prompt_input_item(&message_context),
                        injected_event: injected_message_event(&message_context),
                    });
                    return Some(PendingMessageClaim::Claimed(Box::new(input_item), delivery));
                }
                Some(codex_state::ThreadMessageClaim::Invalid { id, reason }) => {
                    warn!("dropped invalid queued message {id}: {reason}");
                    continue;
                }
                Some(codex_state::ThreadMessageClaim::NotReady) | None => {
                    *self.timer_start_in_progress.lock().await = false;
                    return claim.map(|_| PendingMessageClaim::NotReady);
                }
            }
        }
    }

    async fn claim_next_timer_for_delivery(
        self: &Arc<Self>,
        recurring_timer_policy: RecurringTimerPolicy,
    ) -> Option<ClaimedTimer> {
        if !self.timers_feature_enabled() {
            return None;
        }
        let mut timer_start_in_progress = self.timer_start_in_progress.lock().await;
        if *timer_start_in_progress {
            return None;
        }
        *timer_start_in_progress = true;
        drop(timer_start_in_progress);

        let has_pending_turn_inputs = self.has_queued_response_items_for_next_turn().await
            || self.has_trigger_turn_mailbox_items().await;

        let (has_active_turn, active_turn_is_regular) = {
            let active_turn = self.active_turn.lock().await;
            let has_active_turn = active_turn.is_some();
            let active_turn_is_regular = active_turn
                .as_ref()
                .and_then(|turn| turn.tasks.first())
                .is_some_and(|(_, task)| matches!(task.kind, crate::state::TaskKind::Regular));
            (has_active_turn, active_turn_is_regular)
        };
        let can_after_turn = !has_active_turn && !has_pending_turn_inputs;
        let claimed = self.timers.lock().await.claim_next_timer(
            Utc::now(),
            can_after_turn,
            active_turn_is_regular,
            recurring_timer_policy,
        );
        let Some(claimed) = claimed else {
            *self.timer_start_in_progress.lock().await = false;
            return None;
        };

        if !self.try_claim_timer_in_db(&claimed).await {
            self.sync_timers_from_db(/*emit_update*/ true).await;
            *self.timer_start_in_progress.lock().await = false;
            return None;
        }
        Some(claimed)
    }

    async fn inject_timer_into_active_turn(&self, item: PendingInputItem) -> bool {
        self.inject_message_into_active_turn(item).await
    }

    async fn inject_message_into_active_turn(&self, item: PendingInputItem) -> bool {
        let turn_state = {
            let active = self.active_turn.lock().await;
            let Some(active_turn) = active.as_ref() else {
                return false;
            };

            match active_turn.tasks.first().map(|(_, task)| task.kind) {
                Some(crate::state::TaskKind::Regular) => Arc::clone(&active_turn.turn_state),
                Some(crate::state::TaskKind::Review | crate::state::TaskKind::Compact) | None => {
                    return false;
                }
            }
        };

        let mut turn_state = turn_state.lock().await;
        turn_state.push_pending_input(item);
        true
    }

    fn spawn_timer_task(
        self: &Arc<Self>,
        id: String,
        timer_spec: TimerTaskSpec,
        cancellation_token: CancellationToken,
    ) {
        let weak = Arc::downgrade(self);
        let session_cancel = self.timer_tasks_cancellation_token.clone();
        tokio::spawn(async move {
            let mut delay = timer_spec.delay;
            loop {
                tokio::select! {
                    _ = session_cancel.cancelled() => break,
                    _ = cancellation_token.cancelled() => break,
                    _ = tokio::time::sleep(delay) => {}
                }
                let Some(session) = weak.upgrade() else {
                    break;
                };
                let changed = session.timers.lock().await.mark_timer_due(&id, Utc::now());
                if changed && !session.persist_timer_due_best_effort(&id).await {
                    session.sync_timers_from_db(/*emit_update*/ true).await;
                    continue;
                }
                session.maybe_start_pending_timer().await;
                let next_timer_spec = session
                    .timers
                    .lock()
                    .await
                    .timer_spec_for_timer(&id, Utc::now());
                let Some(next_timer_spec) = next_timer_spec else {
                    break;
                };
                delay = next_timer_spec.delay;
            }
        });
    }

    async fn thread_timer_create_params(
        &self,
        persisted_timer: &PersistedTimer,
        source: &str,
    ) -> Result<codex_state::ThreadTimerCreateParams, String> {
        let timer = &persisted_timer.timer;
        Ok(codex_state::ThreadTimerCreateParams {
            id: timer.id.clone(),
            thread_id: self.thread_id_string(),
            source: source.to_string(),
            client_id: self.timer_client_id().await,
            trigger_json: serde_json::to_string(&timer.trigger)
                .map_err(|err| format!("failed to serialize timer trigger: {err}"))?,
            content: timer.content.clone(),
            instructions: timer.instructions.clone(),
            meta_json: serde_json::to_string(&timer.meta)
                .map_err(|err| format!("failed to serialize timer metadata: {err}"))?,
            delivery: timer.delivery.as_str().to_string(),
            created_at: timer.created_at,
            next_run_at: timer.next_run_at,
            last_run_at: timer.last_run_at,
            pending_run: persisted_timer.pending_run,
        })
    }

    fn thread_timer_update_params(
        &self,
        persisted_timer: &PersistedTimer,
    ) -> Result<codex_state::ThreadTimerUpdateParams, String> {
        let timer = &persisted_timer.timer;
        Ok(codex_state::ThreadTimerUpdateParams {
            trigger_json: serde_json::to_string(&timer.trigger)
                .map_err(|err| format!("failed to serialize timer trigger: {err}"))?,
            content: timer.content.clone(),
            instructions: timer.instructions.clone(),
            meta_json: serde_json::to_string(&timer.meta)
                .map_err(|err| format!("failed to serialize timer metadata: {err}"))?,
            delivery: timer.delivery.as_str().to_string(),
            next_run_at: timer.next_run_at,
            last_run_at: timer.last_run_at,
            pending_run: persisted_timer.pending_run,
        })
    }

    async fn persist_timer_due_best_effort(&self, id: &str) -> bool {
        let state_db = match self.timer_state_db().await {
            Ok(state_db) => state_db,
            Err(err) => {
                warn!("failed to persist due timer {id}: {err}");
                return false;
            }
        };
        let Some(persisted_timer) = self.timers.lock().await.persisted_timer(id) else {
            return false;
        };
        match state_db
            .update_thread_timer_due(
                &self.thread_id_string(),
                id,
                persisted_timer.timer.next_run_at,
            )
            .await
        {
            Ok(updated) => updated,
            Err(err) => {
                warn!("failed to persist due timer {id}: {err}");
                false
            }
        }
    }

    async fn try_claim_timer_in_db(&self, claimed: &ClaimedTimer) -> bool {
        let state_db = match self.timer_state_db().await {
            Ok(state_db) => state_db,
            Err(err) => {
                warn!(
                    "failed to claim timer {} in sqlite: {err}",
                    claimed.timer.id
                );
                return false;
            }
        };
        let thread_id = self.thread_id_string();
        let result = if claimed.deleted_one_shot_timer {
            state_db
                .claim_one_shot_thread_timer(
                    &thread_id,
                    &claimed.timer.id,
                    claimed.context.queued_at,
                )
                .await
        } else {
            let persisted_timer = PersistedTimer {
                timer: claimed.timer.clone(),
                pending_run: claimed.timer.trigger.is_idle_recurring(),
            };
            let Ok(params) = self.thread_timer_update_params(&persisted_timer) else {
                return false;
            };
            state_db
                .claim_recurring_thread_timer(
                    &thread_id,
                    &claimed.timer.id,
                    claimed.context.queued_at,
                    claimed.previous_last_run_at,
                    &params,
                )
                .await
        };
        match result {
            Ok(claimed) => claimed,
            Err(err) => {
                warn!(
                    "failed to claim timer {} in sqlite: {err}",
                    claimed.timer.id
                );
                false
            }
        }
    }

    pub(crate) async fn restore_timers_from_db(self: &Arc<Self>) {
        if !self.timer_db_sync_feature_enabled() {
            return;
        }
        let Ok(state_db) = self.timer_state_db().await else {
            return;
        };
        self.start_timer_db_sync_task(state_db);
        if self.timers_feature_enabled() {
            self.sync_timers_from_db(/*emit_update*/ true).await;
        }
        self.maybe_start_pending_timer().await;
    }

    fn start_timer_db_sync_task(self: &Arc<Self>, state_db: state_db::StateDbHandle) {
        if !self.timer_db_sync_feature_enabled() {
            return;
        }
        if self
            .timer_db_sync_started
            .swap(/*val*/ true, Ordering::SeqCst)
        {
            return;
        }
        let weak = Arc::downgrade(self);
        let session_cancel = self.timer_tasks_cancellation_token.clone();
        tokio::spawn(async move {
            let checker = match state_db.timer_data_version_checker().await {
                Ok(checker) => checker,
                Err(err) => {
                    warn!("failed to start timer db sync: {err}");
                    if let Some(session) = weak.upgrade() {
                        session.timer_db_sync_started.store(false, Ordering::SeqCst);
                    }
                    return;
                }
            };
            let mut last_data_version = checker.data_version().await.ok();
            let mut last_full_refresh = tokio::time::Instant::now();
            let mut interval = tokio::time::interval(TIMER_DB_SYNC_INTERVAL);
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            if let Some(session) = weak.upgrade() {
                session.sync_timers_from_db(/*emit_update*/ true).await;
                session.maybe_start_pending_timer().await;
                last_full_refresh = tokio::time::Instant::now();
            }

            loop {
                tokio::select! {
                    _ = session_cancel.cancelled() => break,
                    _ = interval.tick() => {}
                }

                let current_data_version = match checker.data_version().await {
                    Ok(version) => Some(version),
                    Err(err) => {
                        warn!("failed to poll timer db data_version: {err}");
                        None
                    }
                };
                let version_changed =
                    current_data_version.is_some() && current_data_version != last_data_version;
                let max_refresh_elapsed =
                    last_full_refresh.elapsed() >= TIMER_DB_MAX_REFRESH_INTERVAL;
                if !version_changed && !max_refresh_elapsed {
                    continue;
                }
                last_data_version = current_data_version.or(last_data_version);
                let Some(session) = weak.upgrade() else {
                    break;
                };
                session.sync_timers_from_db(/*emit_update*/ true).await;
                session.maybe_start_pending_timer().await;
                last_full_refresh = tokio::time::Instant::now();
            }
        });
    }

    async fn sync_timers_from_db(self: &Arc<Self>, emit_update: bool) -> bool {
        if !self.timers_feature_enabled() {
            return false;
        }
        let Ok(state_db) = self.timer_state_db().await else {
            return false;
        };
        self.start_timer_db_sync_task(state_db.clone());
        let thread_id = self.thread_id_string();
        let db_timers = match state_db.list_thread_timers(&thread_id).await {
            Ok(timers) => timers,
            Err(err) => {
                warn!("failed to load timers from sqlite for thread {thread_id}: {err}");
                return false;
            }
        };
        let persisted = db_timers
            .into_iter()
            .filter_map(db_timer_to_persisted_timer)
            .collect::<Vec<_>>();
        let (changed, restored_tasks) = self
            .timers
            .lock()
            .await
            .replace_timers_if_changed(persisted, Utc::now());
        self.spawn_restored_timer_tasks(restored_tasks);
        if changed && emit_update {
            self.emit_timer_updated_notification().await;
        }
        changed
    }

    fn timers_feature_enabled(&self) -> bool {
        self.features.enabled(Feature::Timers)
    }

    fn queued_messages_feature_enabled(&self) -> bool {
        self.features.enabled(Feature::QueuedMessages)
    }

    fn timer_db_sync_feature_enabled(&self) -> bool {
        self.timers_feature_enabled() || self.queued_messages_feature_enabled()
    }

    fn spawn_restored_timer_tasks(self: &Arc<Self>, restored_tasks: Vec<RestoredTimerTask>) {
        for RestoredTimerTask {
            id,
            timer_spec,
            timer_cancel,
        } in restored_tasks
        {
            self.spawn_timer_task(id, timer_spec, timer_cancel);
        }
    }

    fn thread_id_string(&self) -> String {
        self.conversation_id.to_string()
    }

    async fn timer_client_id(&self) -> String {
        let state = self.state.lock().await;
        state
            .session_configuration
            .app_server_client_name
            .clone()
            .unwrap_or_else(|| TIMER_CLIENT_ID_FALLBACK.to_string())
    }

    async fn emit_timer_updated_notification(&self) {
        let timers = self.list_timers_from_memory().await;
        let Ok(payload) = serde_json::to_string(&timers) else {
            warn!("failed to serialize timer update payload");
            return;
        };
        self.send_event_raw(Event {
            id: INITIAL_SUBMIT_ID.to_owned(),
            msg: EventMsg::BackgroundEvent(BackgroundEventEvent {
                message: format!("{TIMER_UPDATED_BACKGROUND_EVENT_PREFIX}{payload}"),
            }),
        })
        .await;
    }

    async fn emit_timer_fired_notification(&self, timer: &ThreadTimer) {
        let Ok(payload) = serde_json::to_string(timer) else {
            warn!("failed to serialize timer fired payload");
            return;
        };
        self.send_event_raw(Event {
            id: INITIAL_SUBMIT_ID.to_owned(),
            msg: EventMsg::BackgroundEvent(BackgroundEventEvent {
                message: format!("{TIMER_FIRED_BACKGROUND_EVENT_PREFIX}{payload}"),
            }),
        })
        .await;
    }
}

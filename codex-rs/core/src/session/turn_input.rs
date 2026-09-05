//! Handles reply-bearing turn-input operations.
//!
//! This is the one place Core decides whether submitted input starts a turn,
//! steers an active turn, or is rejected. It replies after that decision; it
//! does not wait for user-prompt hooks, updating the in-memory model context,
//! rollout persistence, or sampling.
//!
//! Persistent thread settings apply on Started and Steered. Turn start
//! options only apply on Started.

use super::TurnInput;
use super::session::Session;
use super::session::SessionConfiguration;
use super::session::SessionSettingsUpdate;
use super::thread_settings;
use super::turn_context::NewTurnContextOptions;
use super::turn_context::TurnContext;
use crate::state::ActiveTurn;
use crate::state::TurnState;
use crate::tasks::RegularTask;
use codex_protocol::config_types::ModeKind;
use codex_protocol::error::CodexErr;
use codex_protocol::error::Result as CodexResult;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::AdditionalContextEntry;
use codex_protocol::protocol::CodexErrorInfo;
use codex_protocol::protocol::ErrorEvent;
use codex_protocol::protocol::Event;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::NonSteerableTurnKind;
use codex_protocol::protocol::ThreadSettingsOverrides;
use codex_protocol::turn_input::NotSubmittedReason;
use codex_protocol::turn_input::TurnInput as SubmittedTurnInput;
use codex_protocol::turn_input::TurnInputMode;
use codex_protocol::turn_input::TurnInputRequest;
use codex_protocol::turn_input::TurnInputSubmission;
use codex_protocol::turn_input::TurnStartOptions;
use codex_protocol::user_input::UserInput;
use serde_json::Value;
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

#[cfg(test)]
#[path = "turn_input_tests.rs"]
mod tests;

/// Why input is starting a turn; shared by admission and input delivery.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TurnStartKind {
    User,
    Automatic,
    Recovery,
}

impl TurnStartKind {
    fn permits_mode(self, mode: ModeKind) -> bool {
        match self {
            Self::User | Self::Recovery => true,
            Self::Automatic => mode != ModeKind::Plan,
        }
    }

    /// Automatic work may neither leave an existing Plan mode nor enter it.
    fn permits_settings(
        self,
        current: &SessionConfiguration,
        proposed: &SessionConfiguration,
    ) -> bool {
        self.permits_mode(current.step_settings.collaboration_mode.mode)
            && self.permits_mode(proposed.step_settings.collaboration_mode.mode)
    }
}

/// Thread settings and start-only options prepared before Core knows whether
/// turn input starts or steers.
///
/// Thread settings are validated up front but only applied after Core accepts
/// the input. Start-only options are only consumed by `apply_started`.
struct PreparedTurnInputSettings {
    thread_settings_update: Option<SessionSettingsUpdate>,
    start_options: TurnStartOptions,
}

impl PreparedTurnInputSettings {
    /// Validates turn-input settings without applying them so rejected input
    /// leaves the thread unchanged.
    async fn prepare(
        session: &Session,
        thread_settings: ThreadSettingsOverrides,
        start_options: TurnStartOptions,
    ) -> CodexResult<Self> {
        let thread_settings_update = if thread_settings == ThreadSettingsOverrides::default() {
            None
        } else {
            let updates = thread_settings::prepare_update(thread_settings);
            session
                .preview_settings(&updates)
                .await
                .map_err(|error| CodexErr::InvalidRequest(error.to_string()))?;
            Some(updates)
        };
        Ok(Self {
            thread_settings_update,
            start_options,
        })
    }

    fn required_active_final_output_json_schema(&self) -> Option<&Value> {
        self.start_options.final_output_json_schema.as_ref()
    }

    /// Applies persistent settings and start-only options before creating a
    /// new turn context. Returns `None` if admission rejects the candidate,
    /// without committing its settings.
    async fn apply_started(
        self,
        session: &Arc<Session>,
        submission_id: String,
        kind: TurnStartKind,
    ) -> CodexResult<Option<Arc<TurnContext>>> {
        let TurnStartOptions {
            guardian_ticket,
            turn_trigger,
            final_output_json_schema,
            service_tier,
            parent_turn_id,
            root_turn_id,
            cyber_access_program,
        } = self.start_options;
        let emit_thread_settings_applied = self.thread_settings_update.is_some();
        let _settings_guard = if emit_thread_settings_applied {
            Some(thread_settings::acquire_persistence_lock(session).await)
        } else {
            None
        };
        let mut updates = self.thread_settings_update.unwrap_or_default();
        updates.service_tier_for_turn = service_tier;

        let options = NewTurnContextOptions {
            guardian_ticket,
            final_output_json_schema,
            cyber_access_program,
        };
        let turn_context = match kind {
            TurnStartKind::User | TurnStartKind::Recovery => Some(
                session
                    .new_turn_with_sub_id(submission_id.clone(), updates, options)
                    .await?,
            ),
            TurnStartKind::Automatic => {
                session
                    .new_turn_with_sub_id_if(
                        submission_id.clone(),
                        updates,
                        options,
                        |current, proposed| kind.permits_settings(current, proposed),
                    )
                    .await?
            }
        };
        let Some((turn_context, settings_snapshot)) = turn_context else {
            return Ok(None);
        };
        if let Some(turn_trigger) = turn_trigger {
            turn_context
                .turn_metadata_state
                .set_turn_trigger(turn_trigger);
        }
        if emit_thread_settings_applied {
            thread_settings::emit_applied(session, submission_id, settings_snapshot).await;
        }
        if let Some(parent_turn_id) = parent_turn_id {
            turn_context
                .turn_metadata_state
                .set_parent_turn_id(parent_turn_id);
        }
        if let Some(root_turn_id) = root_turn_id {
            turn_context
                .turn_metadata_state
                .set_root_turn_id(root_turn_id);
        }
        Ok(Some(turn_context))
    }

    /// Applies only persistent settings after steering succeeds. The active
    /// turn keeps its existing context; subsequent turns see the update.
    async fn apply_steered(self, session: &Session, submission_id: String) -> CodexResult<()> {
        let Some(thread_settings_update) = self.thread_settings_update else {
            return Ok(());
        };
        thread_settings::apply_update(session, submission_id, thread_settings_update)
            .await
            .map_err(|error| CodexErr::InvalidRequest(error.to_string()))
    }
}

pub(super) async fn handle(
    session: &Arc<Session>,
    request: TurnInputRequest,
    mode: TurnInputMode,
    submission_id: String,
) -> CodexResult<TurnInputSubmission> {
    match mode {
        TurnInputMode::StartOrSteer => start_or_steer(session, request, submission_id).await,
        TurnInputMode::StartIfIdle => {
            let kind = match &request.input {
                SubmittedTurnInput::UserInput { content, .. } if !content.is_empty() => {
                    TurnStartKind::User
                }
                SubmittedTurnInput::UserInput { .. }
                | SubmittedTurnInput::ResponseItem(_)
                | SubmittedTurnInput::InterAgentCommunication(_) => TurnStartKind::Automatic,
            };
            start_if_idle(session, request, submission_id, kind).await
        }
        TurnInputMode::Steer { expected_turn_id } => {
            steer(session, request, expected_turn_id, submission_id).await
        }
    }
}

pub(super) async fn handle_recovery(
    session: &Arc<Session>,
    thread_settings: ThreadSettingsOverrides,
    start_options: TurnStartOptions,
    submission_id: String,
) -> CodexResult<TurnInputSubmission> {
    let request = TurnInputRequest::user_input(Vec::new())
        .with_thread_settings(thread_settings)
        .on_start(TurnStartOptions {
            turn_trigger: Some("retry".to_string()),
            ..start_options
        });
    start_if_idle(session, request, submission_id, TurnStartKind::Recovery).await
}

async fn start_or_steer(
    session: &Arc<Session>,
    request: TurnInputRequest,
    submission_id: String,
) -> CodexResult<TurnInputSubmission> {
    let TurnInputRequest {
        mut input,
        thread_settings,
        start,
        additional_context,
        responsesapi_client_metadata,
        ..
    } = request;
    let has_explicit_input = match &input {
        SubmittedTurnInput::UserInput { content, .. } => !content.is_empty(),
        SubmittedTurnInput::ResponseItem(ResponseItem::FunctionCallOutput {
            call_id: None,
            ..
        }) => true,
        _ => {
            return Err(CodexErr::InvalidRequest(
                "only user input or standalone function-call outputs can start or steer a turn"
                    .to_string(),
            ));
        }
    };
    let settings = PreparedTurnInputSettings::prepare(session, thread_settings, start).await?;
    match session
        .steer_input(
            &mut input,
            additional_context.clone(),
            /*expected_turn_id*/ None,
            settings.required_active_final_output_json_schema(),
            responsesapi_client_metadata.clone(),
        )
        .await
    {
        Ok(turn_id) => {
            settings.apply_steered(session, submission_id).await?;
            Ok(TurnInputSubmission::Steered { turn_id })
        }
        Err(NotSubmittedReason::NoActiveTurn) => {
            let Some(turn_context) = settings
                .apply_started(session, submission_id.clone(), TurnStartKind::User)
                .await?
            else {
                unreachable!("explicit user input can enter Plan mode");
            };
            if let Some(responsesapi_client_metadata) = responsesapi_client_metadata {
                turn_context
                    .turn_metadata_state
                    .set_responsesapi_client_metadata(responsesapi_client_metadata);
            }
            session
                .maybe_emit_model_warnings_for_turn(turn_context.as_ref())
                .await;
            if let SubmittedTurnInput::UserInput { content, .. } = &input {
                turn_context.session_telemetry.user_prompt(content);
            }
            let mut task_input = merge_additional_context_input(session, additional_context).await;
            if has_explicit_input {
                task_input.push(pending_turn_input(session, input).await);
            }
            session
                .spawn_task(turn_context, task_input, RegularTask::new())
                .await;
            Ok(TurnInputSubmission::Started {
                turn_id: submission_id,
            })
        }
        Err(reason) => Ok(TurnInputSubmission::NotSubmitted { reason }),
    }
}

async fn start_if_idle(
    session: &Arc<Session>,
    request: TurnInputRequest,
    submission_id: String,
    kind: TurnStartKind,
) -> CodexResult<TurnInputSubmission> {
    let TurnInputRequest {
        input,
        thread_settings,
        start,
        additional_context,
        responsesapi_client_metadata,
        ..
    } = request;
    if session.input_queue.has_trigger_turn_mailbox_items().await {
        return Ok(TurnInputSubmission::NotSubmitted {
            reason: NotSubmittedReason::PendingTriggerTurn,
        });
    }
    // Preserve current-Plan rejection before reservation and settings errors.
    // The commit-time decision also checks the proposed mode.
    if kind == TurnStartKind::Automatic
        && !kind.permits_mode(session.collaboration_mode().await.mode)
    {
        return Ok(TurnInputSubmission::NotSubmitted {
            reason: NotSubmittedReason::PlanMode,
        });
    }

    let turn_state = {
        let mut active_turn = session.active_turn.lock().await;
        if active_turn.is_some() {
            return Ok(TurnInputSubmission::NotSubmitted {
                reason: NotSubmittedReason::NotIdle,
            });
        }
        let active_turn = active_turn.get_or_insert_with(ActiveTurn::default);
        Arc::clone(&active_turn.turn_state)
    };

    if session.input_queue.has_trigger_turn_mailbox_items().await {
        session.clear_reserved_idle_turn(&turn_state).await;
        session.maybe_start_turn_for_pending_work().await;
        return Ok(TurnInputSubmission::NotSubmitted {
            reason: NotSubmittedReason::PendingTriggerTurn,
        });
    }

    let settings = match PreparedTurnInputSettings::prepare(session, thread_settings, start).await {
        Ok(settings) => settings,
        Err(error) => {
            session.clear_reserved_idle_turn(&turn_state).await;
            return Err(error);
        }
    };
    let turn_context = match settings
        .apply_started(session, submission_id.clone(), kind)
        .await
    {
        Ok(Some(turn_context)) => turn_context,
        Ok(None) => {
            session.clear_reserved_idle_turn(&turn_state).await;
            return Ok(TurnInputSubmission::NotSubmitted {
                reason: NotSubmittedReason::PlanMode,
            });
        }
        Err(error) => {
            session.clear_reserved_idle_turn(&turn_state).await;
            return Err(error);
        }
    };
    if let Some(responsesapi_client_metadata) = responsesapi_client_metadata {
        turn_context
            .turn_metadata_state
            .set_responsesapi_client_metadata(responsesapi_client_metadata);
    }
    session
        .maybe_emit_model_warnings_for_turn(turn_context.as_ref())
        .await;

    let mut task_input = merge_additional_context_input(session, additional_context).await;
    match kind {
        TurnStartKind::User => {
            session.clear_connector_selection().await;
            if let SubmittedTurnInput::UserInput { content, .. } = &input {
                turn_context.session_telemetry.user_prompt(content);
            }
            task_input.push(pending_turn_input(session, input).await);
        }
        TurnStartKind::Automatic => {
            // Empty automatic user input resumes sampling without a new message.
            if !matches!(&input, SubmittedTurnInput::UserInput { .. }) {
                session
                    .input_queue
                    .extend_pending_input_for_turn_state(
                        turn_state.as_ref(),
                        vec![pending_turn_input(session, input).await],
                    )
                    .await;
            }
        }
        TurnStartKind::Recovery => {
            // Recovery resumes an existing turn without a new empty user message.
        }
    }
    session
        .start_task(turn_context, task_input, RegularTask::new())
        .await;
    Ok(TurnInputSubmission::Started {
        turn_id: submission_id,
    })
}

async fn steer(
    session: &Arc<Session>,
    request: TurnInputRequest,
    expected_turn_id: String,
    submission_id: String,
) -> CodexResult<TurnInputSubmission> {
    let TurnInputRequest {
        mut input,
        thread_settings,
        start,
        additional_context,
        responsesapi_client_metadata,
        ..
    } = request;
    if !matches!(&input, SubmittedTurnInput::UserInput { .. }) {
        return Err(CodexErr::InvalidRequest(
            "only user input can steer a turn".to_string(),
        ));
    }
    let settings = PreparedTurnInputSettings::prepare(session, thread_settings, start).await?;
    match session
        .steer_input(
            &mut input,
            additional_context,
            Some(expected_turn_id.as_str()),
            settings.required_active_final_output_json_schema(),
            responsesapi_client_metadata,
        )
        .await
    {
        Ok(turn_id) => {
            settings.apply_steered(session, submission_id).await?;
            Ok(TurnInputSubmission::Steered { turn_id })
        }
        Err(reason) => Ok(TurnInputSubmission::NotSubmitted { reason }),
    }
}

impl Session {
    pub(crate) async fn route_realtime_text_input(self: &Arc<Self>, text: String) {
        let submission_id = Uuid::now_v7().to_string();
        let submission = handle(
            self,
            TurnInputRequest::user_input(vec![UserInput::Text {
                text,
                text_elements: Vec::new(),
            }])
            .on_start(TurnStartOptions {
                turn_trigger: Some("realtime".to_string()),
                ..Default::default()
            }),
            TurnInputMode::StartOrSteer,
            submission_id.clone(),
        )
        .await;
        match submission {
            Ok(TurnInputSubmission::Started { .. } | TurnInputSubmission::Steered { .. }) => {}
            Ok(TurnInputSubmission::NotSubmitted { reason }) => {
                self.send_event_raw(Event {
                    id: submission_id,
                    msg: EventMsg::Error(ErrorEvent {
                        misalignment: None,
                        message: format!("failed to submit turn input: {reason:?}"),
                        codex_error_info: Some(CodexErrorInfo::BadRequest),
                    }),
                })
                .await;
            }
            Err(error) => {
                self.send_event_raw(Event {
                    id: submission_id,
                    msg: EventMsg::Error(error.to_error_event(/*message_prefix*/ None)),
                })
                .await;
            }
        }
    }

    async fn clear_reserved_idle_turn(&self, turn_state: &Arc<tokio::sync::Mutex<TurnState>>) {
        let mut active_turn_guard = self.active_turn.lock().await;
        if let Some(active_turn) = active_turn_guard.as_ref()
            && active_turn.task.is_none()
            && Arc::ptr_eq(&active_turn.turn_state, turn_state)
        {
            *active_turn_guard = None;
        }
    }

    /// Inject additional user input or a standalone tool output into the active turn.
    ///
    /// Returns the active turn id when accepted.
    #[expect(
        clippy::await_holding_invalid_type,
        reason = "active turn checks and turn state updates must remain atomic"
    )]
    async fn steer_input(
        &self,
        input: &mut SubmittedTurnInput,
        additional_context: BTreeMap<String, AdditionalContextEntry>,
        expected_turn_id: Option<&str>,
        required_final_output_json_schema: Option<&Value>,
        responsesapi_client_metadata: Option<HashMap<String, String>>,
    ) -> Result<String, NotSubmittedReason> {
        let mut active = self.active_turn.lock().await;
        let Some(active_turn) = active.as_mut() else {
            return Err(NotSubmittedReason::NoActiveTurn);
        };

        let Some(active_task) = active_turn.task.as_ref() else {
            return Err(NotSubmittedReason::NoActiveTurn);
        };
        let active_turn_id = &active_task.turn_context.sub_id;

        if let Some(expected_turn_id) = expected_turn_id
            && expected_turn_id != active_turn_id
        {
            return Err(NotSubmittedReason::ExpectedTurnMismatch {
                expected: expected_turn_id.to_string(),
                actual: active_turn_id.clone(),
            });
        }

        match active_task.kind {
            crate::state::TaskKind::Regular => {}
            crate::state::TaskKind::Review => {
                return Err(NotSubmittedReason::ActiveTurnNotSteerable {
                    turn_kind: NonSteerableTurnKind::Review,
                });
            }
            crate::state::TaskKind::Compact => {
                return Err(NotSubmittedReason::ActiveTurnNotSteerable {
                    turn_kind: NonSteerableTurnKind::Compact,
                });
            }
        }

        if matches!(input, SubmittedTurnInput::UserInput { content, .. } if content.is_empty()) {
            return Err(NotSubmittedReason::EmptyInput);
        }
        // Compare JSON values directly instead of serialized schema text.
        // Value equality ignores object key order while preserving array and
        // scalar distinctions; broader JSON Schema equivalence is out of scope.
        if let Some(required_schema) = required_final_output_json_schema
            && active_task.turn_context.final_output_json_schema.as_ref() != Some(required_schema)
        {
            return Err(NotSubmittedReason::ActiveTurnOutputSchemaMismatch);
        }
        let mut pending_input = merge_additional_context_input(self, additional_context).await;

        if let Some(responsesapi_client_metadata) = responsesapi_client_metadata {
            active_task
                .turn_context
                .turn_metadata_state
                .set_responsesapi_client_metadata(responsesapi_client_metadata);
        }

        let input = match input {
            SubmittedTurnInput::UserInput { content, client_id } => {
                active_task
                    .turn_context
                    .session_telemetry
                    .user_prompt(content);
                TurnInput::UserInput {
                    content: std::mem::take(content),
                    client_id: client_id.clone(),
                    acceptance_order: self.reserve_user_input_order().await,
                }
            }
            input => pending_turn_input(self, input.clone()).await,
        };
        pending_input.push(input);
        self.input_queue
            .extend_pending_input_and_accept_mailbox_delivery_for_turn_state(
                active_turn.turn_state.as_ref(),
                pending_input,
            )
            .await;
        Ok(active_turn_id.clone())
    }
}

async fn merge_additional_context_input(
    session: &Session,
    additional_context: BTreeMap<String, AdditionalContextEntry>,
) -> Vec<TurnInput> {
    let additional_context_input = {
        let mut state = session.state.lock().await;
        state.additional_context.merge(additional_context)
    };
    additional_context_input
        .into_iter()
        .map(|item| session.annotate_client_response_item(item))
        .map(TurnInput::ResponseItem)
        .collect()
}

async fn pending_turn_input(session: &Session, input: SubmittedTurnInput) -> TurnInput {
    match input {
        SubmittedTurnInput::UserInput { content, client_id } => TurnInput::UserInput {
            content,
            client_id,
            acceptance_order: session.reserve_user_input_order().await,
        },
        SubmittedTurnInput::ResponseItem(mut item)
            if matches!(
                &item,
                ResponseItem::FunctionCallOutput { call_id: None, .. }
            ) =>
        {
            Session::assign_missing_response_item_id(&mut item);
            TurnInput::FunctionCallOutput(item)
        }
        SubmittedTurnInput::ResponseItem(item) => TurnInput::ResponseItem(item.into()),
        SubmittedTurnInput::InterAgentCommunication(communication) => {
            TurnInput::InterAgentCommunication(communication)
        }
    }
}

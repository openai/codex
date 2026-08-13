//! Turn-input request and result types shared by Core's submission APIs.

use crate::models::ResponseItem;
use crate::protocol::AdditionalContextEntry;
use crate::protocol::InterAgentCommunication;
use crate::protocol::NonSteerableTurnKind;
use crate::protocol::ThreadSettingsOverrides;
use crate::protocol::W3cTraceContext;
use crate::user_input::UserInput;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;
use std::collections::BTreeMap;
use std::collections::HashMap;

/// Input consumed by a regular turn.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum TurnInput {
    UserInput {
        content: Vec<UserInput>,
        client_id: Option<String>,
    },
    ResponseItem(ResponseItem),
    InterAgentCommunication(InterAgentCommunication),
}

/// One turn input and the context that follows it through submission.
///
/// Callers choose start-or-steer, idle-start, or steer-only behavior through
/// the corresponding `CodexThread` method.
#[derive(Clone, Debug)]
pub struct TurnInputRequest {
    pub input: TurnInput,
    pub thread_settings: ThreadSettingsOverrides,
    pub start: TurnStartOptions,
    pub additional_context: BTreeMap<String, AdditionalContextEntry>,
    pub responsesapi_client_metadata: Option<HashMap<String, String>>,
    pub trace: Option<W3cTraceContext>,
}

/// Request to resume sampling for an interrupted regular turn.
///
/// Sampling restarts under `turn_id`, which must be the ID already recorded
/// for that turn.
#[derive(Clone, Debug)]
pub struct RecoverTurnRequest {
    pub turn_id: String,
    pub thread_settings: ThreadSettingsOverrides,
    pub trace: Option<W3cTraceContext>,
}

impl TurnInputRequest {
    /// Creates turn input that can be passed to one of the submission methods.
    pub fn new(input: TurnInput) -> Self {
        Self {
            input,
            thread_settings: ThreadSettingsOverrides::default(),
            start: TurnStartOptions::default(),
            additional_context: BTreeMap::new(),
            responsesapi_client_metadata: None,
            trace: None,
        }
    }

    /// Creates ordinary user input without a client-provided message id.
    pub fn user_input(content: Vec<UserInput>) -> Self {
        Self::new(TurnInput::UserInput {
            content,
            client_id: None,
        })
    }

    /// Persistent thread settings applied when Core accepts this input.
    ///
    /// Settings are applied for both `Started` and `Steered`. A steered input
    /// cannot change its already-active turn context, so those settings apply
    /// to subsequent turns. `NotSubmitted` leaves them unapplied.
    pub fn with_thread_settings(mut self, thread_settings: ThreadSettingsOverrides) -> Self {
        self.thread_settings = thread_settings;
        self
    }

    /// Options consulted only by start-capable submission methods when this
    /// request starts a turn.
    pub fn on_start(mut self, start: TurnStartOptions) -> Self {
        self.start = start;
        self
    }

    /// Context merged whether this request starts or steers.
    pub fn with_additional_context(
        mut self,
        additional_context: BTreeMap<String, AdditionalContextEntry>,
    ) -> Self {
        self.additional_context = additional_context;
        self
    }

    /// Responses metadata attached whether this request starts or steers.
    pub fn with_responses_metadata(
        mut self,
        responsesapi_client_metadata: Option<HashMap<String, String>>,
    ) -> Self {
        self.responsesapi_client_metadata = responsesapi_client_metadata;
        self
    }

    /// Trace context used when this request crosses the session loop.
    pub fn with_trace(mut self, trace: Option<W3cTraceContext>) -> Self {
        self.trace = trace;
        self
    }
}

/// How Core should route submitted turn input.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TurnInputMode {
    /// Start a regular turn when idle, otherwise steer the active regular turn.
    StartOrSteer,
    /// Start only when the thread is idle.
    StartIfIdle,
    /// Steer only if this exact turn is active.
    Steer { expected_turn_id: String },
}

/// Options for the new-turn branch of a submission.
///
/// Core only records these options when input starts a turn. When input steers,
/// `final_output_json_schema` is a compatibility requirement: Core only
/// accepts the steer if the active turn already uses the same schema. For
/// child input, Core also compares root lineage to detect ambiguity.
#[derive(Clone, Debug, Default)]
pub struct TurnStartOptions {
    /// Structured-output schema for a new turn. When steering, Core rejects
    /// the input if the active turn uses a different schema.
    pub final_output_json_schema: Option<Value>,
    /// Parent turn lineage recorded if this request starts a new turn.
    pub parent_turn_id: Option<String>,
    /// Causal root turn lineage recorded if this request starts a new turn.
    pub root_turn_id: Option<String>,
}

/// What Core did with input submitted through `start_or_steer_turn`.
///
/// Started and Steered only mean Core accepted the input for turn processing. They
/// do not wait for user-prompt hooks, updating the in-memory model context,
/// rollout persistence, or sampling.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TurnInputSubmission {
    /// Core started a turn. Persistent thread settings and start options were applied.
    Started { turn_id: String },
    /// Core steered an active turn. Persistent thread settings were applied for
    /// subsequent turns. No new turn was created, so lineage metadata was not
    /// recorded. If the request included `final_output_json_schema`, the active
    /// turn already used the same schema.
    Steered { turn_id: String },
    /// Core rejected the input without applying settings or start options.
    NotSubmitted { reason: NotSubmittedReason },
}

/// What Core did with input submitted only for an idle turn start.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StartIfIdleSubmission {
    /// Core started a turn. Persistent thread settings and start options were applied.
    Started { turn_id: String },
    /// Core rejected the input without applying settings or start options.
    NotSubmitted { reason: NotSubmittedReason },
}

/// What Core did with input submitted only for steering.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SteerSubmission {
    /// Core steered an active turn. Persistent thread settings were applied for
    /// subsequent turns.
    Steered { turn_id: String },
    /// Core rejected the input without applying settings.
    NotSubmitted { reason: NotSubmittedReason },
}

/// Why Core did not accept submitted turn input for turn processing.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NotSubmittedReason {
    /// `start_turn_if_idle` found an active turn.
    NotIdle,

    /// `start_turn_if_idle` yielded to higher-priority trigger-turn mailbox input.
    PendingTriggerTurn,

    /// `start_turn_if_idle` received automatic non-user input for a turn that
    /// would run in Plan mode.
    PlanMode,

    /// `steer_turn` found no active turn.
    NoActiveTurn,

    /// `steer_turn` found a different active turn.
    ExpectedTurnMismatch { expected: String, actual: String },

    /// `start_or_steer_turn` or `steer_turn` found an active turn that does not accept steering.
    ActiveTurnNotSteerable { turn_kind: NonSteerableTurnKind },

    /// `start_or_steer_turn` or `steer_turn` required a final output schema
    /// that differs from the active turn's schema.
    ActiveTurnOutputSchemaMismatch,

    /// `start_or_steer_turn` or `steer_turn` reached a steering path with empty user input.
    EmptyInput,
}

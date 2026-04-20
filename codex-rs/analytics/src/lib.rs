mod client;
mod events;
mod facts;
mod reducer;

use serde::Serialize;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

pub use client::AnalyticsEventsClient;
pub use events::AppServerRpcTransport;
pub use events::GuardianApprovalRequestSource;
pub use events::GuardianReviewDecision;
pub use events::GuardianReviewEventParams;
pub use events::GuardianReviewFailureReason;
pub use events::GuardianReviewSessionKind;
pub use events::GuardianReviewTerminalStatus;
pub use events::GuardianReviewedAction;
pub use facts::AnalyticsJsonRpcError;
pub use facts::AppInvocation;
pub use facts::CodexCompactionEvent;
pub use facts::CodexResponseItemType;
pub use facts::CodexResponsesApiCallFact;
pub use facts::CodexResponsesApiCallStatus;
pub use facts::CodexResponsesApiItemMetadata;
pub use facts::CodexResponsesApiItemPhase;
pub use facts::CodexTurnSteerEvent;
pub use facts::CompactionImplementation;
pub use facts::CompactionPhase;
pub use facts::CompactionReason;
pub use facts::CompactionStatus;
pub use facts::CompactionStrategy;
pub use facts::CompactionTrigger;
pub use facts::HookRunFact;
pub use facts::InputError;
pub use facts::InvocationType;
pub use facts::SkillInvocation;
pub use facts::SubAgentThreadStartedInput;
pub use facts::ThreadInitializationMode;
pub use facts::TrackEventsContext;
pub use facts::TurnResolvedConfigFact;
pub use facts::TurnStatus;
pub use facts::TurnSteerRejectionReason;
pub use facts::TurnSteerRequestError;
pub use facts::TurnSteerResult;
pub use facts::TurnTokenUsageFact;
pub use facts::build_track_events_context;
pub use facts::response_items_metadata;

#[cfg(test)]
mod analytics_client_tests;

pub fn now_unix_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

pub(crate) fn serialized_string<T: Serialize>(value: &T) -> Option<String> {
    match serde_json::to_value(value).ok()? {
        serde_json::Value::String(value) => Some(value),
        value => Some(value.to_string()),
    }
}

pub(crate) fn serialized_bytes<T: Serialize>(value: &T) -> Option<i64> {
    serde_json::to_string(value)
        .ok()
        .map(|value| byte_len(&value))
}

pub(crate) fn nonzero_i64(value: i64) -> Option<i64> {
    (value > 0).then_some(value)
}

pub(crate) fn byte_len(value: &str) -> i64 {
    i64::try_from(value.len()).unwrap_or(i64::MAX)
}

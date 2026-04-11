mod client;
mod events;
mod facts;
mod reducer;

use std::time::SystemTime;
use std::time::UNIX_EPOCH;

pub use client::AnalyticsEventsClient;
pub use events::AppServerRpcTransport;
pub use facts::AnalyticsJsonRpcError;
pub use facts::AppInvocation;
pub use facts::CodexCompactionEvent;
pub use facts::CodexTurnSteerEvent;
pub use facts::CompactionImplementation;
pub use facts::CompactionPhase;
pub use facts::CompactionReason;
pub use facts::CompactionStatus;
pub use facts::CompactionStrategy;
pub use facts::CompactionTrigger;
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

#[cfg(test)]
mod analytics_client_tests;

pub fn now_unix_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

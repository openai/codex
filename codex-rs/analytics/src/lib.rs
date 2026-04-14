mod client;
mod events;
mod facts;
mod reducer;

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
pub use facts::AppInvocation;
pub use facts::CodexCompactionEvent;
pub use facts::CompactionImplementation;
pub use facts::CompactionPhase;
pub use facts::CompactionReason;
pub use facts::CompactionStatus;
pub use facts::CompactionStrategy;
pub use facts::CompactionTrigger;
pub use facts::InvocationType;
pub use facts::SkillInvocation;
pub use facts::SubAgentThreadStartedInput;
pub use facts::TrackEventsContext;
pub use facts::build_track_events_context;

#[cfg(test)]
mod analytics_client_tests;

pub fn now_unix_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

mod api;

pub use api::{
    ApplyOutcome, ApplyStatus, AttemptStatus, CloudBackend, CloudTaskError, CreatedTask,
    DiffSummary, Result, TaskId, TaskStatus, TaskSummary, TaskText, TurnAttempt,
};

#[cfg(feature = "mock")]
mod mock;

#[cfg(feature = "online")]
mod http;

#[cfg(feature = "online")]
pub use http::HttpClient;
#[cfg(feature = "mock")]
pub use mock::MockClient;

// Reusable apply engine now lives in the shared crate `codex-git`.

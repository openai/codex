//! Lets an embedding observe completed agent work for host-managed result delivery.

use futures::future::BoxFuture;

use crate::agent::api::AgentTurnOutcome;

/// Observes the turn outcomes eligible for native parent result delivery.
///
/// Install an `Arc<dyn AgentTurnOutcomeReporter>` in
/// `StartThreadOptions::thread_extension_init` for each thread to observe,
/// including on cold resume. Native parent notifications are independent.
/// Core awaits the callback after emitting the terminal turn event. Graceful
/// shutdown waits for outstanding handoffs before running thread-stop hooks.
/// Hand off promptly to host-owned delivery. Waiting for this thread to shut
/// down inside the callback would deadlock. The host owns transport and retries.
/// The embedding can supply the session ID from its thread metadata.
pub trait AgentTurnOutcomeReporter: Send + Sync {
    fn report(&self, outcome: AgentTurnOutcome) -> BoxFuture<'_, ()>;
}

//! Lets Core turn-input submissions participate in a host's shutdown drain.

/// A host-provided gate checked before Core starts a turn-input submission.
///
/// Implementations return a permit for work admitted before shutdown and
/// Core retains it through submission. `None` skips the start without consuming
/// pending input. Steering an existing turn does not acquire a new permit.
/// Explicitly triggered mailbox work and parent-delegated subagent input bypass
/// this gate so delegated work can finish before exit. Automatic mailbox wakes
/// remain gated.
pub trait TurnStartAdmission: std::fmt::Debug + Send + Sync {
    fn admit_turn_start(&self) -> Option<Box<dyn Send>>;

    /// Signals that a temporary pause ended and pending work can retry admission.
    /// Subscribe before attempting starts; a notification does not itself grant a permit.
    /// Hosts that never reopen admission can leave this unimplemented.
    fn admission_updates(&self) -> Option<tokio::sync::watch::Receiver<()>> {
        None
    }
}

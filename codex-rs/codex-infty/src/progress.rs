use std::path::Path;

use codex_core::protocol::AgentMessageEvent;
use codex_core::protocol::EventMsg;

use crate::signals::AggregatedVerifierVerdict;
use crate::signals::DirectiveResponse;
use crate::signals::VerifierVerdict;

pub trait ProgressReporter: Send + Sync {
    fn objective_posted(&self, _objective: &str) {}
    fn solver_event(&self, _event: &EventMsg) {}
    fn role_event(&self, _role: &str, _event: &EventMsg) {}
    fn solver_agent_message(&self, _message: &AgentMessageEvent) {}
    /// Called when the solver emits a message that failed to parse as a valid
    /// JSON signal according to the expected `solver_signal_schema`.
    fn invalid_solver_signal(&self, _raw_message: &str) {}
    fn direction_request(&self, _prompt: &str) {}
    fn director_response(&self, _directive: &DirectiveResponse) {}
    fn verification_request(&self, _claim_path: &str, _notes: Option<&str>) {}
    fn verifier_verdict(&self, _role: &str, _verdict: &VerifierVerdict) {}
    fn verification_summary(&self, _summary: &AggregatedVerifierVerdict) {}
    fn final_delivery(&self, _deliverable_path: &Path, _summary: Option<&str>) {}
    fn run_interrupted(&self) {}
}

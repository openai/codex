#[cfg(unix)]
mod unix {
    mod escalate_client;
    mod escalate_protocol;
    mod escalate_server;
    mod escalation_policy;
    mod socket;
    mod stopwatch;

    pub use self::escalate_client::run;
    pub use self::escalate_protocol::EscalateAction;
    pub use self::escalate_server::EscalationPolicyFactory;
    pub use self::escalate_server::ExecParams;
    pub use self::escalate_server::ExecResult;
    pub use self::escalate_server::run_escalate_server;
    pub use self::escalation_policy::EscalationPolicy;
    pub use self::stopwatch::Stopwatch;
}

#[cfg(unix)]
pub use unix::*;

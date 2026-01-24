pub(crate) mod control;
pub(crate) mod role;
pub(crate) mod status;

// core/src/agent/mod.rs
pub(crate) use codex_protocol::protocol::AgentStatus;
pub(crate) use control::AgentControl;
pub(crate) use role::AgentRole;
pub(crate) use status::agent_status_from_event;

pub(crate) mod control;
mod guards;
pub(crate) mod role;
pub(crate) mod status;
mod watchdog;

pub(crate) use codex_protocol::protocol::AgentStatus;
pub(crate) use control::AgentControl;
pub(crate) use control::WatchdogParentCompactionResult;
pub(crate) use guards::MAX_THREAD_SPAWN_DEPTH;
pub(crate) use guards::exceeds_thread_spawn_depth_limit;
pub(crate) use guards::next_thread_spawn_depth;
pub(crate) use role::AgentRole;
pub(crate) use status::agent_status_from_event;
pub(crate) use watchdog::DEFAULT_WATCHDOG_INTERVAL_S;
pub(crate) use watchdog::WatchdogRegistration;

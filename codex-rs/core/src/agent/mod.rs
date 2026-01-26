mod collab_completion_warning;
pub(crate) mod control;
mod guards;
pub(crate) mod role;
pub(crate) mod status;

pub(crate) use codex_protocol::protocol::AgentStatus;
pub(crate) use collab_completion_warning::spawn_collab_completion_warning_watcher;
pub(crate) use control::AgentControl;
pub(crate) use guards::MAX_THREAD_SPAWN_DEPTH;
pub(crate) use guards::exceeds_thread_spawn_depth_limit;
pub(crate) use guards::next_thread_spawn_depth;
pub(crate) use role::AgentRole;
pub(crate) use status::agent_status_from_event;

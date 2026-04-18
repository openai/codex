//! Shared runtime primitives for Codex multi-agent orchestration.

pub mod control;
pub mod mailbox;
pub mod registry;
pub mod status;

pub use codex_protocol::protocol::AgentStatus;
pub use control::LiveAgent;
pub use control::SpawnAgentForkMode;
pub use control::SpawnAgentOptions;
pub use control::agent_matches_prefix;
pub use control::keep_forked_rollout_item;
pub use control::render_input_preview;
pub use control::thread_spawn_depth;
pub use control::thread_spawn_parent_thread_id;
pub use mailbox::Mailbox;
pub use mailbox::MailboxReceiver;
pub use registry::AgentMetadata;
pub use registry::AgentRegistry;
pub use registry::SpawnReservation;
pub use registry::exceeds_thread_spawn_depth_limit;
pub use registry::next_thread_spawn_depth;
pub use status::agent_status_from_event;

const AGENT_NAMES: &str = include_str!("agent_names.txt");

pub fn default_agent_nickname_list() -> Vec<&'static str> {
    AGENT_NAMES
        .lines()
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .collect()
}

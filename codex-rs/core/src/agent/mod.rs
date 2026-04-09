pub(crate) mod agent_resolver;
pub(crate) mod control;
pub(crate) mod mailbox;
mod registry;
pub(crate) mod role;
pub(crate) mod status;
pub(crate) mod subagent_config;

pub(crate) use codex_protocol::protocol::AgentStatus;
pub(crate) use control::AgentControl;
pub(crate) use mailbox::Mailbox;
pub(crate) use mailbox::MailboxReceiver;
pub(crate) use registry::exceeds_thread_spawn_depth_limit;
pub(crate) use registry::next_thread_spawn_depth;
pub(crate) use status::agent_status_from_event;
pub(crate) use subagent_config::SubAgentConfigBuilder;
pub(crate) use subagent_config::SubAgentExtensionInheritance;
pub(crate) use subagent_config::SubAgentPromptInheritance;
pub(crate) use subagent_config::SubAgentPromptSelection;

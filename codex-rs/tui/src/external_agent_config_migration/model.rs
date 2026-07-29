use codex_app_server_protocol::ExternalAgentConfigMigrationItem;
use codex_app_server_protocol::ExternalAgentConfigMigrationItemType;
use std::collections::BTreeSet;

#[derive(Clone, Debug)]
pub(crate) struct ExternalAgentConfigMigrationGroupModel {
    pub(crate) label: String,
    pub(crate) description: &'static str,
    pub(crate) item_indices: Vec<usize>,
}

pub(crate) fn external_agent_config_migration_groups(
    items: &[ExternalAgentConfigMigrationItem],
) -> Vec<ExternalAgentConfigMigrationGroupModel> {
    let tools_and_setup = items
        .iter()
        .enumerate()
        .filter_map(|(idx, item)| {
            (item.cwd.is_none() && item.item_type != ExternalAgentConfigMigrationItemType::Sessions)
                .then_some(idx)
        })
        .collect::<Vec<_>>();
    let projects = items
        .iter()
        .enumerate()
        .filter_map(|(idx, item)| {
            (item.cwd.is_some() && item.item_type != ExternalAgentConfigMigrationItemType::Sessions)
                .then_some(idx)
        })
        .collect::<Vec<_>>();
    let chat_sessions = items
        .iter()
        .enumerate()
        .filter_map(|(idx, item)| {
            (item.item_type == ExternalAgentConfigMigrationItemType::Sessions).then_some(idx)
        })
        .collect::<Vec<_>>();

    let mut groups = Vec::new();
    if !tools_and_setup.is_empty() {
        groups.push(ExternalAgentConfigMigrationGroupModel {
            label: "Tools & setup".to_string(),
            description: "Settings, instructions, integrations, agents, commands, and skills",
            item_indices: tools_and_setup,
        });
    }
    if !projects.is_empty() {
        let project_count = projects
            .iter()
            .filter_map(|idx| items[*idx].cwd.as_deref())
            .collect::<BTreeSet<_>>()
            .len();
        groups.push(ExternalAgentConfigMigrationGroupModel {
            label: if project_count == 1 {
                "Current project".to_string()
            } else {
                format!("Projects ({project_count})")
            },
            description: "Add Codex files alongside your existing project files",
            item_indices: projects,
        });
    }
    if !chat_sessions.is_empty() {
        let session_count = chat_sessions
            .iter()
            .filter_map(|idx| items[*idx].details.as_ref())
            .map(|details| details.sessions.len())
            .sum::<usize>();
        groups.push(ExternalAgentConfigMigrationGroupModel {
            label: format!("Chat sessions ({session_count})"),
            description: "Last 30 days of chats",
            item_indices: chat_sessions,
        });
    }
    groups
}

pub(crate) fn external_agent_config_migration_item_label(
    item: &ExternalAgentConfigMigrationItem,
) -> &'static str {
    match item.item_type {
        ExternalAgentConfigMigrationItemType::AgentsMd => "Instructions",
        ExternalAgentConfigMigrationItemType::Config => "Settings",
        ExternalAgentConfigMigrationItemType::Skills => "Skills",
        ExternalAgentConfigMigrationItemType::Plugins => "Plugins",
        ExternalAgentConfigMigrationItemType::McpServerConfig => "MCP servers",
        ExternalAgentConfigMigrationItemType::Subagents => "Agents",
        ExternalAgentConfigMigrationItemType::Hooks => "Hooks",
        ExternalAgentConfigMigrationItemType::Commands => "Slash commands",
        ExternalAgentConfigMigrationItemType::Memory => "Memory",
        ExternalAgentConfigMigrationItemType::Sessions => "Recent chat sessions",
    }
}

pub(crate) fn external_agent_config_migration_type_label(
    item_type: ExternalAgentConfigMigrationItemType,
) -> &'static str {
    match item_type {
        ExternalAgentConfigMigrationItemType::AgentsMd => "Instructions",
        ExternalAgentConfigMigrationItemType::Config => "Settings",
        ExternalAgentConfigMigrationItemType::Skills => "Skills",
        ExternalAgentConfigMigrationItemType::Plugins => "Plugins",
        ExternalAgentConfigMigrationItemType::McpServerConfig => "MCP servers",
        ExternalAgentConfigMigrationItemType::Subagents => "Agents",
        ExternalAgentConfigMigrationItemType::Hooks => "Hooks",
        ExternalAgentConfigMigrationItemType::Commands => "Slash commands",
        ExternalAgentConfigMigrationItemType::Memory => "Memory",
        ExternalAgentConfigMigrationItemType::Sessions => "Chat sessions",
    }
}

/// Summarizes the concrete objects represented by selected migration items.
///
/// Most detected item types carry the objects they will import in `details`; types without
/// details represent one importable file or source directory per migration item.
pub(crate) fn external_agent_config_migration_count_summary<'a>(
    items: impl IntoIterator<Item = &'a ExternalAgentConfigMigrationItem>,
) -> String {
    let mut counts = Vec::<(ExternalAgentConfigMigrationItemType, usize)>::new();
    for item in items {
        let count = external_agent_config_migration_item_count(item);
        if let Some((_, type_count)) = counts
            .iter_mut()
            .find(|(item_type, _)| *item_type == item.item_type)
        {
            *type_count += count;
        } else {
            counts.push((item.item_type, count));
        }
    }

    counts
        .into_iter()
        .map(|(item_type, count)| {
            format!(
                "{} {count}",
                external_agent_config_migration_type_label(item_type)
            )
        })
        .collect::<Vec<_>>()
        .join(", ")
}

pub(crate) fn external_agent_config_migration_item_count(
    item: &ExternalAgentConfigMigrationItem,
) -> usize {
    match item.item_type {
        ExternalAgentConfigMigrationItemType::Plugins => {
            item.details.as_ref().map_or(1, |details| {
                details
                    .plugins
                    .iter()
                    .map(|plugin_group| plugin_group.plugin_names.len())
                    .sum()
            })
        }
        ExternalAgentConfigMigrationItemType::McpServerConfig => item
            .details
            .as_ref()
            .map_or(1, |details| details.mcp_servers.len()),
        ExternalAgentConfigMigrationItemType::Subagents => item
            .details
            .as_ref()
            .map_or(1, |details| details.subagents.len()),
        ExternalAgentConfigMigrationItemType::Hooks => item
            .details
            .as_ref()
            .map_or(1, |details| details.hooks.len()),
        ExternalAgentConfigMigrationItemType::Commands => item
            .details
            .as_ref()
            .map_or(1, |details| details.commands.len()),
        ExternalAgentConfigMigrationItemType::Memory => item
            .details
            .as_ref()
            .map_or(0, |details| details.memory.len()),
        ExternalAgentConfigMigrationItemType::Sessions => item
            .details
            .as_ref()
            .map_or(1, |details| details.sessions.len()),
        ExternalAgentConfigMigrationItemType::Skills => item
            .details
            .as_ref()
            .map_or(1, |details| details.skills.len()),
        ExternalAgentConfigMigrationItemType::AgentsMd
        | ExternalAgentConfigMigrationItemType::Config => 1,
    }
}

pub(crate) fn external_agent_config_migration_item_detail(
    item: &ExternalAgentConfigMigrationItem,
) -> Option<String> {
    let details = item.details.as_ref()?;
    match item.item_type {
        ExternalAgentConfigMigrationItemType::Plugins => None,
        ExternalAgentConfigMigrationItemType::Skills => Some(format_counted_details(
            "skill",
            details.skills.len(),
            details.skills.iter().map(|skill| skill.name.as_str()),
        )),
        ExternalAgentConfigMigrationItemType::McpServerConfig => Some(format_counted_details(
            "MCP server",
            details.mcp_servers.len(),
            details
                .mcp_servers
                .iter()
                .map(|server| server.name.as_str()),
        )),
        ExternalAgentConfigMigrationItemType::Subagents => Some(format_counted_details(
            "agent",
            details.subagents.len(),
            details.subagents.iter().map(|agent| agent.name.as_str()),
        )),
        ExternalAgentConfigMigrationItemType::Hooks => Some(format_counted_details(
            "hook",
            details.hooks.len(),
            details.hooks.iter().map(|hook| hook.name.as_str()),
        )),
        ExternalAgentConfigMigrationItemType::Commands => Some(format_counted_details(
            "slash command",
            details.commands.len(),
            details.commands.iter().map(|command| command.name.as_str()),
        )),
        ExternalAgentConfigMigrationItemType::Memory => {
            let memory = &details.memory;
            let count = memory.len();
            let noun = if count == 1 { "memory" } else { "memories" };
            let names = memory
                .iter()
                .map(String::as_str)
                .take(4)
                .collect::<Vec<_>>();
            Some(if names.is_empty() {
                format!("{count} {noun}")
            } else {
                format!("{count} {noun}: {}", names.join(", "))
            })
        }
        ExternalAgentConfigMigrationItemType::Sessions => Some(format_counted_details(
            "chat session",
            details.sessions.len(),
            details
                .sessions
                .iter()
                .filter_map(|session| session.title.as_deref()),
        )),
        ExternalAgentConfigMigrationItemType::AgentsMd
        | ExternalAgentConfigMigrationItemType::Config => None,
    }
}

fn format_counted_details<'a>(
    noun: &str,
    count: usize,
    names: impl Iterator<Item = &'a str>,
) -> String {
    let suffix = if count == 1 { "" } else { "s" };
    match names.take(4).collect::<Vec<_>>() {
        names if names.is_empty() => format!("{count} {noun}{suffix}"),
        names => format!("{count} {noun}{suffix}: {}", names.join(", ")),
    }
}

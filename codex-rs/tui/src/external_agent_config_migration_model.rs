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
        ExternalAgentConfigMigrationItemType::AgentsMd => "Instructions (CLAUDE.md -> AGENTS.md)",
        ExternalAgentConfigMigrationItemType::Config => "Settings (settings.json -> config.toml)",
        ExternalAgentConfigMigrationItemType::Skills => "Skills",
        ExternalAgentConfigMigrationItemType::Plugins => "Plugins",
        ExternalAgentConfigMigrationItemType::McpServerConfig => "MCP servers",
        ExternalAgentConfigMigrationItemType::Subagents => "Agents",
        ExternalAgentConfigMigrationItemType::Hooks => "Hooks",
        ExternalAgentConfigMigrationItemType::Commands => "Slash commands",
        ExternalAgentConfigMigrationItemType::Sessions => "Recent chat sessions",
    }
}

pub(crate) fn external_agent_config_migration_item_detail(
    item: &ExternalAgentConfigMigrationItem,
) -> Option<String> {
    let details = item.details.as_ref()?;
    match item.item_type {
        ExternalAgentConfigMigrationItemType::Plugins => None,
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
        ExternalAgentConfigMigrationItemType::Sessions => Some(format_counted_details(
            "chat session",
            details.sessions.len(),
            details
                .sessions
                .iter()
                .filter_map(|session| session.title.as_deref()),
        )),
        ExternalAgentConfigMigrationItemType::AgentsMd
        | ExternalAgentConfigMigrationItemType::Config
        | ExternalAgentConfigMigrationItemType::Skills => None,
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

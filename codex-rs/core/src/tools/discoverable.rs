use crate::plugins::PluginCapabilitySummary;
use codex_app_server_protocol::AppInfo;
use serde::Deserialize;
use serde::Serialize;

const TUI_APP_SERVER_CLIENT_NAME: &str = "codex-tui";

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum DiscoverableToolType {
    Connector,
    Plugin,
}

impl DiscoverableToolType {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Connector => "connector",
            Self::Plugin => "plugin",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum DiscoverableToolAction {
    Install,
    Enable,
}

impl DiscoverableToolAction {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Install => "install",
            Self::Enable => "enable",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum DiscoverableTool {
    Connector(Box<AppInfo>),
    Plugin(Box<DiscoverablePluginInfo>),
}

impl DiscoverableTool {
    pub(crate) fn tool_type(&self) -> DiscoverableToolType {
        match self {
            Self::Connector(_) => DiscoverableToolType::Connector,
            Self::Plugin(_) => DiscoverableToolType::Plugin,
        }
    }

    pub(crate) fn id(&self) -> &str {
        match self {
            Self::Connector(connector) => connector.id.as_str(),
            Self::Plugin(plugin) => plugin.id.as_str(),
        }
    }

    pub(crate) fn name(&self) -> &str {
        match self {
            Self::Connector(connector) => connector.name.as_str(),
            Self::Plugin(plugin) => plugin.name.as_str(),
        }
    }

    pub(crate) fn description(&self) -> Option<&str> {
        match self {
            Self::Connector(connector) => connector.description.as_deref(),
            Self::Plugin(plugin) => plugin.description.as_deref(),
        }
    }

    pub(crate) fn install_url(&self) -> Option<&str> {
        match self {
            Self::Connector(connector) => connector.install_url.as_deref(),
            Self::Plugin(_) => None,
        }
    }
}

impl From<AppInfo> for DiscoverableTool {
    fn from(value: AppInfo) -> Self {
        Self::Connector(Box::new(value))
    }
}

impl From<DiscoverablePluginInfo> for DiscoverableTool {
    fn from(value: DiscoverablePluginInfo) -> Self {
        Self::Plugin(Box::new(value))
    }
}

pub(crate) fn filter_tool_suggest_discoverable_tools_for_client(
    discoverable_tools: Vec<DiscoverableTool>,
    app_server_client_name: Option<&str>,
) -> Vec<DiscoverableTool> {
    if app_server_client_name != Some(TUI_APP_SERVER_CLIENT_NAME) {
        return discoverable_tools;
    }

    discoverable_tools
        .into_iter()
        .filter(|tool| !matches!(tool, DiscoverableTool::Plugin(_)))
        .collect()
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct DiscoverablePluginInfo {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) description: Option<String>,
    pub(crate) has_skills: bool,
    pub(crate) mcp_server_names: Vec<String>,
    pub(crate) app_connector_ids: Vec<String>,
}

impl From<PluginCapabilitySummary> for DiscoverablePluginInfo {
    fn from(value: PluginCapabilitySummary) -> Self {
        Self {
            id: value.config_name,
            name: value.display_name,
            description: value.description,
            has_skills: value.has_skills,
            mcp_server_names: value.mcp_server_names,
            app_connector_ids: value
                .app_connector_ids
                .into_iter()
                .map(|connector_id| connector_id.0)
                .collect(),
        }
    }
}

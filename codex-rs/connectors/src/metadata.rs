use crate::AppInfo;

/// Stable Apps MCP resource namespace and connector-server name prefix.
pub const CODEX_APPS_MCP_SERVER_NAME: &str = "codex_apps";

pub fn connector_display_label(connector: &AppInfo) -> String {
    connector.name.clone()
}

pub fn connector_mention_slug(connector: &AppInfo) -> String {
    connector_mention_slug_from_name(&connector_display_label(connector))
}

pub fn connector_mention_slug_from_name(name: &str) -> String {
    crate::connector_name_slug(name)
}

pub fn connector_install_url(name: &str, connector_id: &str) -> String {
    crate::connector_install_url(name, connector_id)
}

pub fn sanitize_name(name: &str) -> String {
    crate::connector_name_slug(name).replace("-", "_")
}

/// Returns the connector-scoped MCP server name used by Codex Apps tools.
pub fn connector_mcp_server_name(connector_name: &str) -> String {
    format!(
        "{CODEX_APPS_MCP_SERVER_NAME}__{}",
        sanitize_name(connector_name)
    )
}

/// Removes the connector prefix from an upstream Apps tool name after sanitizing both values.
pub fn connector_tool_name(
    tool_name: &str,
    connector_id: Option<&str>,
    connector_name: Option<&str>,
) -> String {
    let tool_name = sanitize_name(tool_name);

    for connector_prefix in [connector_name, connector_id]
        .into_iter()
        .flatten()
        .map(str::trim)
        .filter(|prefix| !prefix.is_empty())
        .map(sanitize_name)
    {
        if let Some(stripped) = tool_name.strip_prefix(&connector_prefix)
            && !stripped.is_empty()
        {
            return stripped.to_string();
        }
    }

    tool_name
}

/// Removes the exact connector-name prefix from an upstream Apps tool title.
pub fn connector_tool_title(connector_name: Option<&str>, title: &str) -> String {
    let Some(connector_name) = connector_name
        .map(str::trim)
        .filter(|name| !name.is_empty())
    else {
        return title.to_string();
    };

    let prefix = format!("{connector_name}_");
    title
        .strip_prefix(&prefix)
        .filter(|stripped| !stripped.is_empty())
        .unwrap_or(title)
        .to_string()
}

pub(crate) fn sort_connectors_by_accessibility_and_name(connectors: &mut [AppInfo]) {
    connectors.sort_by(|left, right| {
        right
            .is_accessible
            .cmp(&left.is_accessible)
            .then_with(|| left.name.cmp(&right.name))
            .then_with(|| left.id.cmp(&right.id))
    });
}

use async_trait::async_trait;
use bm25::Document;
use bm25::Language;
use bm25::SearchEngineBuilder;
use codex_app_server_protocol::AppInfo;
use serde::Deserialize;
use serde::Serialize;
use serde_json::json;
use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::LazyLock;

use crate::connectors;
use crate::function_tool::FunctionCallError;
use crate::mcp::CODEX_APPS_MCP_SERVER_NAME;
use crate::mcp_connection_manager::ToolInfo;
use crate::tools::context::FunctionToolOutput;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolPayload;
use crate::tools::handlers::parse_arguments;
use crate::tools::handlers::tool_suggest::ToolSuggestionToolType;
use crate::tools::handlers::tool_suggest::ToolSuggestionType;
use crate::tools::registry::ToolHandler;
use crate::tools::registry::ToolKind;

pub struct SearchToolBm25Handler;

pub(crate) const SEARCH_TOOL_BM25_TOOL_NAME: &str = "search_tool_bm25";
pub(crate) const DEFAULT_LIMIT: usize = 8;
const ALLOWLISTED_DISCOVERABLE_CONNECTOR_ID: &str = "connector_2128aebfecb84f64a069897515042a44";
pub(crate) static INSTALLABLE_DISCOVERABLE_CONNECTOR_IDS: LazyLock<HashSet<&str>> =
    LazyLock::new(|| HashSet::from([ALLOWLISTED_DISCOVERABLE_CONNECTOR_ID]));

fn default_limit() -> usize {
    DEFAULT_LIMIT
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "lowercase")]
pub(crate) enum SearchToolMode {
    #[default]
    Enabled,
    Discoverable,
}

#[derive(Deserialize)]
struct SearchToolBm25Args {
    query: String,
    #[serde(default = "default_limit")]
    limit: usize,
    #[serde(default)]
    mode: SearchToolMode,
}

#[derive(Clone)]
struct ToolEntry {
    name: String,
    server_name: String,
    title: Option<String>,
    description: Option<String>,
    connector_name: Option<String>,
    input_keys: Vec<String>,
    search_text: String,
}

impl ToolEntry {
    fn new(name: String, info: ToolInfo) -> Self {
        let input_keys = info
            .tool
            .input_schema
            .get("properties")
            .and_then(serde_json::Value::as_object)
            .map(|map| map.keys().cloned().collect::<Vec<_>>())
            .unwrap_or_default();
        let search_text = build_search_text(&name, &info, &input_keys);
        Self {
            name,
            server_name: info.server_name,
            title: info.tool.title,
            description: info
                .tool
                .description
                .map(|description| description.to_string()),
            connector_name: info.connector_name,
            input_keys,
            search_text,
        }
    }
}

#[derive(Clone)]
struct ConnectorEntry {
    connector_id: String,
    connector_name: String,
    connector_description: Option<String>,
    suggestion_type: ToolSuggestionType,
    search_text: String,
}

impl ConnectorEntry {
    fn new(connector: AppInfo, suggestion_type: ToolSuggestionType) -> Self {
        let search_text = build_connector_search_text(&connector);
        Self {
            connector_id: connector.id,
            connector_name: connector.name,
            connector_description: connector.description,
            suggestion_type,
            search_text,
        }
    }
}

#[async_trait]
impl ToolHandler for SearchToolBm25Handler {
    type Output = FunctionToolOutput;

    fn kind(&self) -> ToolKind {
        ToolKind::Function
    }

    async fn handle(&self, invocation: ToolInvocation) -> Result<Self::Output, FunctionCallError> {
        let ToolInvocation {
            payload,
            session,
            turn,
            ..
        } = invocation;

        let arguments = match payload {
            ToolPayload::Function { arguments } => arguments,
            _ => {
                return Err(FunctionCallError::Fatal(format!(
                    "{SEARCH_TOOL_BM25_TOOL_NAME} handler received unsupported payload"
                )));
            }
        };

        let args: SearchToolBm25Args = parse_arguments(&arguments)?;
        let query = args.query.trim();
        if query.is_empty() {
            return Err(FunctionCallError::RespondToModel(
                "query must not be empty".to_string(),
            ));
        }

        if args.limit == 0 {
            return Err(FunctionCallError::RespondToModel(
                "limit must be greater than zero".to_string(),
            ));
        }

        let content = match args.mode {
            SearchToolMode::Enabled => {
                let mcp_tools = session
                    .services
                    .mcp_connection_manager
                    .read()
                    .await
                    .list_all_tools()
                    .await;
                let enabled_connector_overrides = session.get_connector_selection().await;

                let connectors = connectors::with_app_enabled_state(
                    connectors::accessible_connectors_from_mcp_tools(&mcp_tools),
                    &turn.config,
                );
                let mcp_tools = filter_codex_apps_mcp_tools(
                    mcp_tools,
                    &connectors,
                    &enabled_connector_overrides,
                );
                let mcp_tools =
                    connectors::filter_codex_apps_tools_by_policy_with_enabled_connector_overrides(
                        mcp_tools,
                        &turn.config,
                        &enabled_connector_overrides,
                    );

                let mut entries: Vec<ToolEntry> = mcp_tools
                    .into_iter()
                    .map(|(name, info)| ToolEntry::new(name, info))
                    .collect();
                entries.sort_by(|a, b| a.name.cmp(&b.name));

                if entries.is_empty() {
                    let active_selected_tools =
                        session.get_mcp_tool_selection().await.unwrap_or_default();
                    json!({
                        "query": query,
                        "mode": SearchToolMode::Enabled,
                        "total_tools": 0,
                        "active_selected_tools": active_selected_tools,
                        "tools": [],
                    })
                    .to_string()
                } else {
                    let documents: Vec<Document<usize>> = entries
                        .iter()
                        .enumerate()
                        .map(|(idx, entry)| Document::new(idx, entry.search_text.clone()))
                        .collect();
                    let search_engine =
                        SearchEngineBuilder::<usize>::with_documents(Language::English, documents)
                            .build();
                    let results = search_engine.search(query, args.limit);

                    let mut selected_tools = Vec::new();
                    let mut result_payloads = Vec::new();
                    for result in results {
                        let Some(entry) = entries.get(result.document.id) else {
                            continue;
                        };
                        selected_tools.push(entry.name.clone());
                        result_payloads.push(json!({
                            "name": entry.name.clone(),
                            "server": entry.server_name.clone(),
                            "title": entry.title.clone(),
                            "description": entry.description.clone(),
                            "connector_name": entry.connector_name.clone(),
                            "input_keys": entry.input_keys.clone(),
                            "score": result.score,
                        }));
                    }

                    let active_selected_tools =
                        session.merge_mcp_tool_selection(selected_tools).await;

                    json!({
                        "query": query,
                        "mode": SearchToolMode::Enabled,
                        "total_tools": entries.len(),
                        "active_selected_tools": active_selected_tools,
                        "tools": result_payloads,
                    })
                    .to_string()
                }
            }
            SearchToolMode::Discoverable => {
                let all_connectors = match connectors::list_cached_connectors(&turn.config).await {
                    Some(connectors) => connectors,
                    None => connectors::list_connectors(&turn.config)
                        .await
                        .map_err(|err| {
                            FunctionCallError::RespondToModel(format!(
                                "failed to load discoverable apps: {err}"
                            ))
                        })?,
                };
                let active_selected_tools =
                    session.get_mcp_tool_selection().await.unwrap_or_default();
                let (installable_entries, disabled_entries) =
                    discoverable_connector_entries(all_connectors);
                let total_connectors = installable_entries.len() + disabled_entries.len();

                if total_connectors == 0 {
                    json!({
                        "query": query,
                        "mode": SearchToolMode::Discoverable,
                        "total_connectors": 0,
                        "active_selected_tools": active_selected_tools,
                        "connectors": [],
                    })
                    .to_string()
                } else {
                    let mut result_payloads = Vec::new();
                    let installable_results =
                        search_connector_entries(&installable_entries, query, args.limit);
                    for (entry, score) in installable_results {
                        result_payloads.push(json!({
                            "connector_id": entry.connector_id.clone(),
                            "connector_name": entry.connector_name.clone(),
                            "connector_description": entry.connector_description.clone(),
                            "tool_type": ToolSuggestionToolType::Connector,
                            "suggestion_type": entry.suggestion_type,
                            "score": score,
                        }));
                    }

                    let remaining = args.limit.saturating_sub(result_payloads.len());
                    if remaining > 0 {
                        for (entry, score) in
                            search_connector_entries(&disabled_entries, query, remaining)
                        {
                            result_payloads.push(json!({
                                "connector_id": entry.connector_id.clone(),
                                "connector_name": entry.connector_name.clone(),
                                "connector_description": entry.connector_description.clone(),
                                "tool_type": ToolSuggestionToolType::Connector,
                                "suggestion_type": entry.suggestion_type,
                                "score": score,
                            }));
                        }
                    }

                    json!({
                        "query": query,
                        "mode": SearchToolMode::Discoverable,
                        "total_connectors": total_connectors,
                        "active_selected_tools": active_selected_tools,
                        "connectors": result_payloads,
                    })
                    .to_string()
                }
            }
        };

        Ok(FunctionToolOutput::from_text(content, Some(true)))
    }
}

fn filter_codex_apps_mcp_tools(
    mut mcp_tools: HashMap<String, ToolInfo>,
    connectors: &[AppInfo],
    enabled_connector_overrides: &HashSet<String>,
) -> HashMap<String, ToolInfo> {
    let enabled_connectors: HashSet<&str> = connectors
        .iter()
        .filter(|connector| {
            connector.is_enabled || enabled_connector_overrides.contains(connector.id.as_str())
        })
        .map(|connector| connector.id.as_str())
        .collect();

    mcp_tools.retain(|_, tool| {
        if tool.server_name != CODEX_APPS_MCP_SERVER_NAME {
            return false;
        }

        tool.connector_id
            .as_deref()
            .is_some_and(|connector_id| enabled_connectors.contains(connector_id))
    });
    mcp_tools
}

fn discoverable_connector_entries(
    connectors: Vec<AppInfo>,
) -> (Vec<ConnectorEntry>, Vec<ConnectorEntry>) {
    let mut installable_entries = Vec::new();
    let mut disabled_entries = Vec::new();

    for connector in connectors {
        if !connector.is_accessible {
            if INSTALLABLE_DISCOVERABLE_CONNECTOR_IDS.contains(connector.id.as_str()) {
                installable_entries
                    .push(ConnectorEntry::new(connector, ToolSuggestionType::Install));
            }
        } else if !connector.is_enabled {
            disabled_entries.push(ConnectorEntry::new(connector, ToolSuggestionType::Enable));
        }
    }

    installable_entries.sort_by(|a, b| {
        a.connector_name
            .cmp(&b.connector_name)
            .then_with(|| a.connector_id.cmp(&b.connector_id))
    });
    disabled_entries.sort_by(|a, b| {
        a.connector_name
            .cmp(&b.connector_name)
            .then_with(|| a.connector_id.cmp(&b.connector_id))
    });

    (installable_entries, disabled_entries)
}

fn search_connector_entries<'a>(
    entries: &'a [ConnectorEntry],
    query: &str,
    limit: usize,
) -> Vec<(&'a ConnectorEntry, f32)> {
    if entries.is_empty() || limit == 0 {
        return Vec::new();
    }

    let documents: Vec<Document<usize>> = entries
        .iter()
        .enumerate()
        .map(|(idx, entry)| Document::new(idx, entry.search_text.clone()))
        .collect();
    let search_engine =
        SearchEngineBuilder::<usize>::with_documents(Language::English, documents).build();
    let results = search_engine.search(query, limit);

    results
        .into_iter()
        .filter_map(|result| {
            entries
                .get(result.document.id)
                .map(|entry| (entry, result.score))
        })
        .collect()
}

fn build_search_text(name: &str, info: &ToolInfo, input_keys: &[String]) -> String {
    let mut parts = vec![
        name.to_string(),
        info.tool_name.clone(),
        info.server_name.clone(),
    ];

    if let Some(title) = info.tool.title.as_deref()
        && !title.trim().is_empty()
    {
        parts.push(title.to_string());
    }

    if let Some(description) = info.tool.description.as_deref()
        && !description.trim().is_empty()
    {
        parts.push(description.to_string());
    }

    if let Some(connector_name) = info.connector_name.as_deref()
        && !connector_name.trim().is_empty()
    {
        parts.push(connector_name.to_string());
    }

    if !input_keys.is_empty() {
        parts.extend(input_keys.iter().cloned());
    }

    parts.join(" ")
}

fn build_connector_search_text(connector: &AppInfo) -> String {
    let mut parts = vec![connector.id.clone(), connector.name.clone()];

    if let Some(description) = connector.description.as_deref()
        && !description.trim().is_empty()
    {
        parts.push(description.to_string());
    }

    if let Some(labels) = connector.labels.as_ref() {
        parts.extend(
            labels
                .iter()
                .flat_map(|(key, value)| [key.clone(), value.clone()]),
        );
    }

    if let Some(branding) = connector.branding.as_ref() {
        if let Some(category) = branding.category.as_deref()
            && !category.trim().is_empty()
        {
            parts.push(category.to_string());
        }
        if let Some(developer) = branding.developer.as_deref()
            && !developer.trim().is_empty()
        {
            parts.push(developer.to_string());
        }
    }

    if let Some(app_metadata) = connector.app_metadata.as_ref() {
        if let Some(categories) = app_metadata.categories.as_ref() {
            parts.extend(categories.iter().cloned());
        }
        if let Some(sub_categories) = app_metadata.sub_categories.as_ref() {
            parts.extend(sub_categories.iter().cloned());
        }
        if let Some(seo_description) = app_metadata.seo_description.as_deref()
            && !seo_description.trim().is_empty()
        {
            parts.push(seo_description.to_string());
        }
        if let Some(developer) = app_metadata.developer.as_deref()
            && !developer.trim().is_empty()
        {
            parts.push(developer.to_string());
        }
    }

    if !connector.plugin_display_names.is_empty() {
        parts.extend(connector.plugin_display_names.iter().cloned());
    }

    parts.join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_app_server_protocol::AppInfo;
    use pretty_assertions::assert_eq;
    use rmcp::model::JsonObject;
    use rmcp::model::Tool;
    use std::sync::Arc;

    fn make_connector(id: &str, enabled: bool) -> AppInfo {
        AppInfo {
            id: id.to_string(),
            name: id.to_string(),
            description: None,
            logo_url: None,
            logo_url_dark: None,
            distribution_channel: None,
            branding: None,
            app_metadata: None,
            labels: None,
            install_url: None,
            is_accessible: true,
            is_enabled: enabled,
            plugin_display_names: Vec::new(),
        }
    }

    fn make_tool(
        qualified_name: &str,
        server_name: &str,
        tool_name: &str,
        connector_id: Option<&str>,
    ) -> (String, ToolInfo) {
        (
            qualified_name.to_string(),
            ToolInfo {
                server_name: server_name.to_string(),
                tool_name: tool_name.to_string(),
                tool: Tool {
                    name: tool_name.to_string().into(),
                    title: None,
                    description: Some(format!("Test tool: {tool_name}").into()),
                    input_schema: Arc::new(JsonObject::default()),
                    output_schema: None,
                    annotations: None,
                    execution: None,
                    icons: None,
                    meta: None,
                },
                connector_id: connector_id.map(str::to_string),
                connector_name: connector_id.map(str::to_string),
                plugin_display_names: Vec::new(),
            },
        )
    }

    #[test]
    fn filter_codex_apps_mcp_tools_keeps_enabled_apps_only() {
        let mcp_tools = HashMap::from([
            make_tool(
                "mcp__codex_apps__calendar_create_event",
                CODEX_APPS_MCP_SERVER_NAME,
                "calendar_create_event",
                Some("calendar"),
            ),
            make_tool(
                "mcp__codex_apps__drive_search",
                CODEX_APPS_MCP_SERVER_NAME,
                "drive_search",
                Some("drive"),
            ),
            make_tool("mcp__rmcp__echo", "rmcp", "echo", None),
        ]);
        let connectors = vec![
            make_connector("calendar", false),
            make_connector("drive", true),
        ];

        let mut filtered: Vec<String> =
            filter_codex_apps_mcp_tools(mcp_tools, &connectors, &HashSet::new())
                .into_keys()
                .collect();
        filtered.sort();

        assert_eq!(filtered, vec!["mcp__codex_apps__drive_search".to_string()]);
    }

    #[test]
    fn filter_codex_apps_mcp_tools_keeps_temporarily_enabled_apps() {
        let mcp_tools = HashMap::from([make_tool(
            "mcp__codex_apps__calendar_create_event",
            CODEX_APPS_MCP_SERVER_NAME,
            "calendar_create_event",
            Some("calendar"),
        )]);
        let connectors = vec![make_connector("calendar", false)];
        let enabled_connector_overrides = HashSet::from(["calendar".to_string()]);

        let filtered: Vec<String> =
            filter_codex_apps_mcp_tools(mcp_tools, &connectors, &enabled_connector_overrides)
                .into_keys()
                .collect();

        assert_eq!(
            filtered,
            vec!["mcp__codex_apps__calendar_create_event".to_string()]
        );
    }

    #[test]
    fn filter_codex_apps_mcp_tools_drops_apps_without_connector_id() {
        let mcp_tools = HashMap::from([
            make_tool(
                "mcp__codex_apps__unknown",
                CODEX_APPS_MCP_SERVER_NAME,
                "unknown",
                None,
            ),
            make_tool("mcp__rmcp__echo", "rmcp", "echo", None),
        ]);

        let mut filtered: Vec<String> = filter_codex_apps_mcp_tools(
            mcp_tools,
            &[make_connector("calendar", true)],
            &HashSet::new(),
        )
        .into_keys()
        .collect();
        filtered.sort();

        assert_eq!(filtered, Vec::<String>::new());
    }

    #[test]
    fn discoverable_connector_entries_prioritize_install_before_enable() {
        let installable = AppInfo {
            id: ALLOWLISTED_DISCOVERABLE_CONNECTOR_ID.to_string(),
            name: "Calendar".to_string(),
            description: None,
            logo_url: None,
            logo_url_dark: None,
            distribution_channel: None,
            branding: None,
            app_metadata: None,
            labels: None,
            install_url: None,
            is_accessible: false,
            is_enabled: false,
            plugin_display_names: Vec::new(),
        };
        let non_allowlisted_installable = AppInfo {
            id: "calendar".to_string(),
            name: "Calendar".to_string(),
            description: None,
            logo_url: None,
            logo_url_dark: None,
            distribution_channel: None,
            branding: None,
            app_metadata: None,
            labels: None,
            install_url: None,
            is_accessible: false,
            is_enabled: false,
            plugin_display_names: Vec::new(),
        };
        let disabled = AppInfo {
            id: "drive".to_string(),
            name: "Drive".to_string(),
            description: None,
            logo_url: None,
            logo_url_dark: None,
            distribution_channel: None,
            branding: None,
            app_metadata: None,
            labels: None,
            install_url: None,
            is_accessible: true,
            is_enabled: false,
            plugin_display_names: Vec::new(),
        };

        let (installable_entries, disabled_entries) = discoverable_connector_entries(vec![
            disabled,
            installable,
            non_allowlisted_installable,
        ]);

        assert_eq!(
            installable_entries
                .iter()
                .map(|entry| (entry.connector_id.clone(), entry.suggestion_type))
                .collect::<Vec<_>>(),
            vec![(
                ALLOWLISTED_DISCOVERABLE_CONNECTOR_ID.to_string(),
                ToolSuggestionType::Install,
            )]
        );
        assert_eq!(
            disabled_entries
                .iter()
                .map(|entry| (entry.connector_id.clone(), entry.suggestion_type))
                .collect::<Vec<_>>(),
            vec![("drive".to_string(), ToolSuggestionType::Enable)]
        );
    }
}

use std::collections::HashSet;

use codex_tools::DiscoverableTool;
use codex_tools::DiscoverableToolType;
use codex_tools::FunctionCallError;
use codex_tools::LIST_AVAILABLE_PLUGINS_TO_INSTALL_TOOL_NAME;

use crate::RequestPluginInstallEntryResult;
use crate::RequestPluginInstallPickerCategory;
use crate::RequestPluginInstallPickerEntry;
use crate::RequestPluginInstallResolvedPickerEntry;
use crate::RequestPluginInstallsArgs;
use crate::ToolSuggestPresentation;

pub const MAX_REQUEST_PLUGIN_INSTALLS_ENTRIES: usize = 16;

pub fn validate_request_plugin_install_picker_args<'a>(
    args: &'a RequestPluginInstallsArgs,
    discoverable_tools: &'a [DiscoverableTool],
    app_server_client_name: Option<&str>,
    presentation: ToolSuggestPresentation,
) -> Result<Vec<RequestPluginInstallResolvedPickerEntry<'a>>, FunctionCallError> {
    if app_server_client_name == Some("codex-tui")
        && (args.categories.is_some()
            || args
                .entries
                .as_ref()
                .is_some_and(|entries| entries.len() != 1))
    {
        return Err(FunctionCallError::RespondToModel(
            "multi-tool install requests are not available in codex-tui yet".to_string(),
        ));
    }

    let mut resolved_entries = Vec::new();
    let mut seen_tools = HashSet::new();

    match (&args.entries, &args.categories) {
        (Some(entries), None) => {
            if entries.len() > MAX_REQUEST_PLUGIN_INSTALLS_ENTRIES {
                return Err(too_many_request_plugin_installs_entries_error());
            }
            for entry in entries {
                resolved_entries.push(validate_request_plugin_install_picker_entry(
                    /*category_index*/ None,
                    entry,
                    discoverable_tools,
                    app_server_client_name,
                    presentation,
                    &mut seen_tools,
                )?);
            }
            if resolved_entries.is_empty() {
                return Err(FunctionCallError::RespondToModel(
                    "picker install requests must include at least one entry".to_string(),
                ));
            }
        }
        (None, Some(categories)) => {
            if categories
                .iter()
                .try_fold(0usize, |count, category| {
                    count.checked_add(category.entries.len())
                })
                .is_none_or(|count| count > MAX_REQUEST_PLUGIN_INSTALLS_ENTRIES)
            {
                return Err(too_many_request_plugin_installs_entries_error());
            }
            validate_request_plugin_install_picker_categories(
                categories,
                discoverable_tools,
                app_server_client_name,
                presentation,
                &mut seen_tools,
                &mut resolved_entries,
            )?;
        }
        _ => {
            return Err(FunctionCallError::RespondToModel(
                "picker install requests must include exactly one of entries or categories"
                    .to_string(),
            ));
        }
    }

    Ok(resolved_entries)
}

fn too_many_request_plugin_installs_entries_error() -> FunctionCallError {
    FunctionCallError::RespondToModel(format!(
        "picker install requests support at most {MAX_REQUEST_PLUGIN_INSTALLS_ENTRIES} entries"
    ))
}

fn validate_request_plugin_install_picker_categories<'a>(
    categories: &'a [RequestPluginInstallPickerCategory],
    discoverable_tools: &'a [DiscoverableTool],
    app_server_client_name: Option<&str>,
    presentation: ToolSuggestPresentation,
    seen_tools: &mut HashSet<(DiscoverableToolType, String)>,
    resolved_entries: &mut Vec<RequestPluginInstallResolvedPickerEntry<'a>>,
) -> Result<(), FunctionCallError> {
    if categories.is_empty() {
        return Err(FunctionCallError::RespondToModel(
            "picker install requests must include at least one category".to_string(),
        ));
    }

    for (category_index, category) in categories.iter().enumerate() {
        if category.title.trim().is_empty() {
            return Err(FunctionCallError::RespondToModel(
                "categories[].title must not be empty".to_string(),
            ));
        }
        if category.entries.is_empty() {
            return Err(FunctionCallError::RespondToModel(
                "categories[].entries must include at least one install candidate".to_string(),
            ));
        }
        for entry in &category.entries {
            resolved_entries.push(validate_request_plugin_install_picker_entry(
                Some(category_index),
                entry,
                discoverable_tools,
                app_server_client_name,
                presentation,
                seen_tools,
            )?);
        }
    }

    Ok(())
}

fn validate_request_plugin_install_picker_entry<'a>(
    category_index: Option<usize>,
    entry: &'a RequestPluginInstallPickerEntry,
    discoverable_tools: &'a [DiscoverableTool],
    app_server_client_name: Option<&str>,
    presentation: ToolSuggestPresentation,
    seen_tools: &mut HashSet<(DiscoverableToolType, String)>,
) -> Result<RequestPluginInstallResolvedPickerEntry<'a>, FunctionCallError> {
    if entry.tool_id.trim().is_empty() {
        return Err(FunctionCallError::RespondToModel(
            "entries[].tool_id must not be empty".to_string(),
        ));
    }
    if entry.tool_type == DiscoverableToolType::Plugin
        && app_server_client_name == Some("codex-tui")
    {
        return Err(FunctionCallError::RespondToModel(
            "plugin install requests are not available in codex-tui yet".to_string(),
        ));
    }

    if !seen_tools.insert((entry.tool_type, entry.tool_id.clone())) {
        return Err(FunctionCallError::RespondToModel(
            "picker install requests must not repeat a tool_type/tool_id pair".to_string(),
        ));
    }

    let tool = discoverable_tools
        .iter()
        .find(|tool| tool.tool_type() == entry.tool_type && tool.id() == entry.tool_id)
        .ok_or_else(|| {
            let source = match presentation {
                ToolSuggestPresentation::ListTool => format!(
                    "the discoverable tools returned by {LIST_AVAILABLE_PLUGINS_TO_INSTALL_TOOL_NAME}"
                ),
                ToolSuggestPresentation::RecommendationContext => {
                    "the <recommended_plugins> list".to_string()
                }
            };
            FunctionCallError::RespondToModel(format!(
                "entries[].tool_id must match one of {source}"
            ))
        })?;

    Ok(RequestPluginInstallResolvedPickerEntry {
        category_index,
        tool,
    })
}

pub fn request_plugin_install_picker_completed(
    entries: &[RequestPluginInstallEntryResult],
) -> bool {
    entries.iter().any(|entry| entry.completed)
}

//! Plugin attribution stays bounded without changing tool definitions or membership.

use super::*;
use pretty_assertions::assert_eq;

fn app_tool(description: &str) -> ToolInfo {
    tool_info_from_listed_tool(
        CODEX_APPS_MCP_SERVER_NAME,
        /*is_codex_apps_mcp_server*/ true,
        /*server_instructions*/ None,
        ToolWithConnectorId {
            tool: RmcpTool::new("search", description.to_string(), Arc::default()),
            connector_id: Some("shared-connector".to_string()),
            connector_name: None,
            connector_description: None,
        },
    )
}

#[test]
fn provenance_preserves_short_descriptions_and_membership() {
    for (description, names, expected_description) in [
        ("Search", vec![], "Search"),
        ("", vec!["Alpha"], "This tool is part of plugin `Alpha`."),
        (
            "Search",
            vec!["Alpha"],
            "Search. This tool is part of plugin `Alpha`.",
        ),
        (
            "Search!",
            vec!["Alpha", "Beta"],
            "Search! This tool is part of plugins `Alpha`, `Beta`.",
        ),
    ] {
        let names = names.into_iter().map(str::to_string).collect::<Vec<_>>();
        let mut tool = app_tool(description);
        let mut expected = tool.clone();
        expected.tool.description = Some(expected_description.to_string().into());
        expected.plugin_display_names = names.clone();
        add_plugin_provenance_to_tool(&mut tool, &names);
        assert_eq!(tool, expected);
    }
}

#[test]
fn provenance_bounds_aggregate_names_and_retains_all_associations() {
    let exact_name =
        "a".repeat(MAX_PLUGIN_PROVENANCE_BYTES - "This tool is part of plugin ``.".len());
    for names in [
        vec![exact_name.clone()],
        vec![format!("{exact_name}a")],
        vec!["界".repeat(MAX_PLUGIN_PROVENANCE_BYTES)],
        (0..512)
            .map(|index| format!("{index:03}{}", "界".repeat(61)))
            .collect(),
    ] {
        let mut tool = app_tool("Search.");
        let original = tool.clone();
        add_plugin_provenance_to_tool(&mut tool, &names);
        let note = tool
            .tool
            .description
            .as_deref()
            .unwrap()
            .strip_prefix("Search. ")
            .unwrap();
        assert!(note.len() <= MAX_PLUGIN_PROVENANCE_BYTES);
        if names[0] == exact_name {
            assert_eq!(note.len(), MAX_PLUGIN_PROVENANCE_BYTES);
        } else if names.len() == 1 {
            assert_eq!(note, "This tool is part of 1 plugin.");
        } else {
            let shown = note.matches('`').count() / 2;
            assert!(shown > 0 && shown < names.len());
            assert!(note.starts_with(&format!("This tool is part of plugins `{}`", names[0])));
            assert!(note.ends_with(&format!(", and {} more.", names.len() - shown)));
        }
        let mut expected = original;
        expected.tool.description = tool.tool.description.clone();
        expected.plugin_display_names = names;
        assert_eq!(tool, expected);
    }
}

//! Checks prefix normalization and rendering without exercising tool execution.

use super::*;
use codex_code_mode::CodeModeToolKind;
use codex_protocol::openai_models::IndirectDescriptionPrefixes;
use codex_protocol::openai_models::ToolMessages;
use pretty_assertions::assert_eq;
use serde_json::json;

#[test]
fn plain_tools_join_an_existing_functions_group_only_with_a_prefix() {
    let original_tools = [
        ToolName::plain("plain"),
        ToolName::namespaced("functions", "namespaced"),
    ]
    .map(|tool_name| ToolDefinition {
        name: tool_name.name.clone(),
        tool_name,
        description: "Tool guidance.".to_string(),
        kind: CodeModeToolKind::Function,
        input_schema: None,
        input_schema_max_bytes: None,
        output_schema: None,
    });
    let original_namespaces = BTreeMap::from([(
        "functions".to_string(),
        ToolNamespaceDescription {
            name: "functions".to_string(),
            description: "Original namespace guidance.".to_string(),
        },
    )]);
    let render = |tools: &[ToolDefinition],
                  namespaces: &BTreeMap<String, ToolNamespaceDescription>| {
        codex_code_mode::build_exec_tool_description(
            tools,
            &[],
            namespaces,
            codex_code_mode::DEFAULT_EXEC_YIELD_TIME_MS,
            /*code_mode_only*/ true,
            codex_code_mode::ImageDetailVisibility::Visible,
            /*messages*/ None,
        )
    };
    let original = render(&original_tools, &original_namespaces);
    for messages in [
        json!({}),
        json!({"indirect_description_prefixes": null}),
        json!({"indirect_description_prefixes": {}}),
        json!({"indirect_description_prefixes": {"namespaces": null, "mcp_servers": null}}),
        json!({"indirect_description_prefixes": {"namespaces": {}, "mcp_servers": {}}}),
        json!({"indirect_description_prefixes": {"namespaces": {"functions": " \n"}}}),
        json!({"indirect_description_prefixes": {
            "namespaces": {"unavailable": "Unused guidance."},
        }}),
    ] {
        let configured: ToolMessages = serde_json::from_value(messages).unwrap();
        let mut tools = original_tools.clone();
        let mut namespaces = original_namespaces.clone();
        IndirectNamespacePrefixes::new(configured.indirect_description_prefixes.as_ref(), [])
            .unwrap()
            .apply_exec_prompt(&mut tools, &mut namespaces);
        assert_eq!(render(&tools, &namespaces), original);
    }
    let configured = IndirectDescriptionPrefixes {
        namespaces: Some(BTreeMap::from([(
            "functions".to_string(),
            "Catalog guidance.".to_string(),
        )])),
        ..Default::default()
    };
    let mut tools = original_tools;
    let mut namespaces = original_namespaces;
    IndirectNamespacePrefixes::new(Some(&configured), [])
        .unwrap()
        .apply_exec_prompt(&mut tools, &mut namespaces);
    let rendered = render(&tools, &namespaces);
    assert_eq!(rendered.matches("## functions").count(), 1);
    assert!(rendered.contains("## functions\nCatalog guidance.\n\nOriginal namespace guidance."));
    assert!(rendered.find("## functions").unwrap() < rendered.find("### `plain`").unwrap());
}

#[test]
fn flat_tool_descriptions_use_trimmed_namespace_prefixes() {
    let configured = IndirectDescriptionPrefixes {
        namespaces: Some(BTreeMap::from([(
            "functions".to_string(),
            " \nFunction guidance.\t ".to_string(),
        )])),
        ..Default::default()
    };
    let original = ToolDefinition {
        name: "inspect".to_string(),
        tool_name: ToolName::plain("inspect"),
        description: "Inspect.".to_string(),
        kind: CodeModeToolKind::Function,
        input_schema: None,
        input_schema_max_bytes: None,
        output_schema: None,
    };
    let mut tools = [original.clone()];
    IndirectNamespacePrefixes::new(Some(&configured), [])
        .unwrap()
        .apply_code_mode(&mut tools);
    assert_eq!(
        tools,
        [ToolDefinition {
            description: "Function guidance.\n\nInspect.".to_string(),
            ..original
        }]
    );
}

#[test]
fn matching_prefixes_cover_each_namespace_once() {
    let servers = BTreeMap::from([
        ("codex_apps".to_string(), " App guidance. ".to_string()),
        ("apps_alias".to_string(), "App guidance.\n".to_string()),
    ]);
    let exact = BTreeMap::from([(
        "mcp__codex_apps__gmail".to_string(),
        "App guidance.".to_string(),
    )]);
    let configured = IndirectDescriptionPrefixes {
        namespaces: Some(exact),
        mcp_servers: Some(servers),
    };
    let original = ["gmail", "calendar", "drive", "slack"].map(|app| ToolDefinition {
        name: format!("mcp__codex_apps__{app}__search"),
        tool_name: ToolName::namespaced(format!("mcp__codex_apps__{app}"), "search"),
        description: "Search.".to_string(),
        kind: CodeModeToolKind::Function,
        input_schema: None,
        input_schema_max_bytes: None,
        output_schema: None,
    });
    let mut tools = original.clone();
    let namespaces = original
        .iter()
        .map(|tool| ("codex_apps", tool.tool_name.namespace.as_deref().unwrap()))
        .chain([
            ("codex_apps", "mcp__codex_apps__gmail"),
            ("apps_alias", "mcp__codex_apps__gmail"),
        ]);
    IndirectNamespacePrefixes::new(Some(&configured), namespaces)
        .unwrap()
        .apply_code_mode(&mut tools);
    let expected = original
        .into_iter()
        .map(|tool| ToolDefinition {
            description: "App guidance.\n\nSearch.".to_string(),
            ..tool
        })
        .collect::<Vec<_>>();
    assert_eq!(tools.as_slice(), expected);
}

#[test]
fn conflicting_namespace_and_server_prefixes_report_keys_without_values() {
    for (namespace_prefix, server_prefix) in [
        ("Namespace guidance.", "Server guidance."),
        ("", "Server guidance."),
        ("Namespace guidance.", " \n"),
    ] {
        let configured = IndirectDescriptionPrefixes {
            namespaces: Some(BTreeMap::from([(
                "mcp__reports_one".to_string(),
                namespace_prefix.to_string(),
            )])),
            mcp_servers: Some(BTreeMap::from([(
                "reports.one".to_string(),
                server_prefix.to_string(),
            )])),
        };
        let error = IndirectNamespacePrefixes::new(
            Some(&configured),
            [("reports.one", "mcp__reports_one")],
        )
        .err()
        .expect("conflicting prefixes")
        .to_string();
        assert_eq!(
            error,
            "Conflicting indirect description prefixes for namespace `mcp__reports_one`: `namespaces.mcp__reports_one` and `mcp_servers.reports.one`",
        );
    }
}

#[test]
fn conflicting_mcp_servers_report_the_shared_namespace() {
    let configured = IndirectDescriptionPrefixes {
        mcp_servers: Some(BTreeMap::from([
            ("reports.one".to_string(), "First guidance.".to_string()),
            ("reports.two".to_string(), "Second guidance.".to_string()),
        ])),
        ..Default::default()
    };
    let error = IndirectNamespacePrefixes::new(
        Some(&configured),
        [("reports.one", "shared"), ("reports.two", "shared")],
    )
    .err()
    .expect("conflicting prefixes")
    .to_string();
    assert_eq!(
        error,
        "Conflicting indirect description prefixes for namespace `shared`: `mcp_servers.reports.one` and `mcp_servers.reports.two`",
    );
}

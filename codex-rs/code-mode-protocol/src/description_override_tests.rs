//! Tests catalog exec template rendering and runtime description composition.

use super::CodeModeToolKind;
use super::DEFERRED_NESTED_TOOLS_GUIDANCE;
use super::DeferredToolDiscovery;
use super::ImageDetailVisibility;
use super::LEGACY_IMAGE_HELPER_DESCRIPTION;
use super::MCP_TYPESCRIPT_PREAMBLE;
use super::TOOL_SEARCH_GUIDANCE;
use super::ToolDefinition;
use super::UNIFIED_IMAGE_HELPER_DESCRIPTION;
use super::build_exec_tool_description;
use codex_protocol::ToolName;
use codex_protocol::openai_models::CodeModeToolMessages;
use codex_protocol::openai_models::ToolMessage;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::collections::BTreeMap;

#[test]
fn exec_override_renders_only_known_literal_placeholders() {
    for (image_detail_visibility, image_helper) in [
        (
            ImageDetailVisibility::Visible,
            LEGACY_IMAGE_HELPER_DESCRIPTION,
        ),
        (
            ImageDetailVisibility::Hidden,
            UNIFIED_IMAGE_HELPER_DESCRIPTION,
        ),
    ] {
        let description = build_exec_tool_description(
            &[],
            &[],
            &BTreeMap::new(),
            /*default_exec_yield_time_ms*/ 4567,
            /*code_mode_only*/ false,
            image_detail_visibility,
            DeferredToolDiscovery::Catalog,
            Some(&CodeModeToolMessages {
                exec: Some(ToolMessage {
                    description: Some(" \nDefaults to 10000 ms. {{ default_exec_yield_time_ms }} ms.\n{{ image_helper }}\n{{ unknown }} {{default_exec_yield_time_ms}}\t ".to_string()),
                    ..Default::default()
                }),
                ..Default::default()
            }),
        );

        assert_eq!(
            description,
            format!(
                " \nDefaults to 10000 ms. 4567 ms.\n{image_helper}\n{{{{ unknown }}}} {{{{default_exec_yield_time_ms}}}}\t \n\n{DEFERRED_NESTED_TOOLS_GUIDANCE}"
            ),
        );
    }
}

#[test]
fn exec_override_preserves_empty_and_whitespace_only_text() {
    for description_override in ["", " \n\t "] {
        assert_eq!(
            build_exec_tool_description(
                &[],
                &[],
                &BTreeMap::new(),
                crate::DEFAULT_EXEC_YIELD_TIME_MS,
                /*code_mode_only*/ true,
                ImageDetailVisibility::Visible,
                DeferredToolDiscovery::Catalog,
                Some(&CodeModeToolMessages {
                    exec: Some(ToolMessage {
                        description: Some(description_override.to_string()),
                        ..Default::default()
                    }),
                    deferred_nested_tools_guidance: Some(
                        "No deferred tools; omit this.".to_string()
                    ),
                    mcp_typescript_preamble: Some("No MCP tools; omit this.".to_string()),
                    ..Default::default()
                }),
            ),
            [
                description_override,
                "No deferred tools; omit this.",
                "Shared MCP Types:\n```ts\nNo MCP tools; omit this.\n```"
            ]
            .into_iter()
            .filter(|section| !section.is_empty())
            .collect::<Vec<_>>()
            .join("\n\n"),
        );
    }
}

#[test]
fn exec_override_preserves_runtime_sections() {
    let enabled_tools = [ToolDefinition {
        name: "alpha".to_string(),
        tool_name: ToolName::plain("alpha"),
        description: "First tool".to_string(),
        kind: CodeModeToolKind::Function,
        input_schema: None,
        input_schema_max_bytes: None,
        output_schema: None,
    }];
    let deferred_tools = [ToolDefinition {
        name: "mcp__sample__beta".to_string(),
        tool_name: ToolName::namespaced("mcp__sample__", "beta"),
        description: "Deferred tool".to_string(),
        kind: CodeModeToolKind::Function,
        input_schema: None,
        input_schema_max_bytes: None,
        output_schema: Some(json!({
            "type": "object",
            "properties": {
                "content": { "type": "array", "items": { "type": "object" } },
                "isError": { "type": "boolean" },
                "_meta": { "type": "object" }
            }
        })),
    }];
    let declaration = "### `alpha`\nFirst tool\n\nexec tool declaration:\n```ts\ndeclare const tools: { alpha(args: unknown): Promise<unknown>; };\n```";
    for (code_mode_only, guidance, preamble, expected) in [
        (true, Some(""), Some(""), declaration.to_string()),
        (
            true,
            None,
            Some(""),
            format!("{DEFERRED_NESTED_TOOLS_GUIDANCE}\n\n{declaration}"),
        ),
        (
            true,
            Some(""),
            None,
            format!("Shared MCP Types:\n```ts\n{MCP_TYPESCRIPT_PREAMBLE}\n```\n\n{declaration}"),
        ),
        (
            true,
            Some("  {{ image_helper }}\n"),
            Some("  {{ default_exec_yield_time_ms }}\n"),
            format!(
                "  {{{{ image_helper }}}}\n\n\nShared MCP Types:\n```ts\n  {{{{ default_exec_yield_time_ms }}}}\n\n```\n\n{declaration}"
            ),
        ),
        (
            false,
            Some("Catalog discovery."),
            Some("No types outside Code Mode Only."),
            "Catalog discovery.".to_string(),
        ),
    ] {
        assert_eq!(
            build_exec_tool_description(
                &enabled_tools,
                &deferred_tools,
                &BTreeMap::new(),
                crate::DEFAULT_EXEC_YIELD_TIME_MS,
                code_mode_only,
                ImageDetailVisibility::Visible,
                DeferredToolDiscovery::Catalog,
                Some(&CodeModeToolMessages {
                    exec: Some(ToolMessage {
                        description: Some(String::new()),
                        ..Default::default()
                    }),
                    deferred_nested_tools_guidance: guidance.map(str::to_string),
                    mcp_typescript_preamble: preamble.map(str::to_string),
                    ..Default::default()
                }),
            ),
            expected,
        );
    }
}

#[test]
fn ranked_search_guidance_honors_overrides() {
    let deferred_tools = [ToolDefinition {
        name: "deferred_tool".to_string(),
        tool_name: ToolName::plain("deferred_tool"),
        description: "Deferred tool".to_string(),
        kind: CodeModeToolKind::Function,
        input_schema: None,
        input_schema_max_bytes: None,
        output_schema: None,
    }];
    for code_mode_only in [false, true] {
        for (guidance, expected) in [
            (
                None,
                if code_mode_only {
                    DEFERRED_NESTED_TOOLS_GUIDANCE.to_string()
                } else {
                    format!("{DEFERRED_NESTED_TOOLS_GUIDANCE}\n\n{TOOL_SEARCH_GUIDANCE}")
                },
            ),
            (Some(""), String::new()),
            (Some("Catalog discovery."), "Catalog discovery.".to_string()),
        ] {
            let description = build_exec_tool_description(
                &[],
                &deferred_tools,
                &BTreeMap::new(),
                crate::DEFAULT_EXEC_YIELD_TIME_MS,
                code_mode_only,
                ImageDetailVisibility::Visible,
                DeferredToolDiscovery::RankedSearch,
                Some(&CodeModeToolMessages {
                    exec: Some(ToolMessage {
                        description: Some(String::new()),
                        ..Default::default()
                    }),
                    deferred_nested_tools_guidance: guidance.map(str::to_string),
                    mcp_typescript_preamble: Some(String::new()),
                    ..Default::default()
                }),
            );
            assert_eq!(description, expected);
        }
    }
}

#[test]
fn mcp_types_stay_stable_when_a_deferred_tool_changes_its_output_schema() {
    let mut tool = ToolDefinition {
        name: "sample".to_string(),
        tool_name: ToolName::plain("sample"),
        description: "Deferred tool".to_string(),
        kind: CodeModeToolKind::Function,
        input_schema: None,
        input_schema_max_bytes: None,
        output_schema: None,
    };
    let render = |tool: &ToolDefinition, preamble: Option<&str>| {
        build_exec_tool_description(
            &[],
            std::slice::from_ref(tool),
            &BTreeMap::new(),
            crate::DEFAULT_EXEC_YIELD_TIME_MS,
            /*code_mode_only*/ true,
            ImageDetailVisibility::Visible,
            DeferredToolDiscovery::Catalog,
            Some(&CodeModeToolMessages {
                mcp_typescript_preamble: preamble.map(str::to_string),
                ..Default::default()
            }),
        )
    };
    // Keep one deferred tool throughout to isolate this from the independent
    // discovery guidance transition when the entire deferred catalog empties.
    for preamble in [None, Some("type CustomMcpResult = string;"), Some("")] {
        tool.output_schema = None;
        let before = render(&tool, preamble);
        tool.output_schema = Some(json!({
            "type": "object",
            "properties": {
                "content": { "type": "array", "items": { "type": "object" } },
                "isError": { "type": "boolean" },
                "_meta": { "type": "object" }
            }
        }));
        assert_eq!(before, render(&tool, preamble));
    }
}

#[test]
fn nested_guidance_survives_catalog_changes_and_respects_overrides() {
    let deferred_tools = [ToolDefinition {
        name: "deferred_echo".to_string(),
        tool_name: ToolName::plain("deferred_echo"),
        description: "Echo a value.".to_string(),
        kind: CodeModeToolKind::Function,
        input_schema: None,
        input_schema_max_bytes: None,
        output_schema: None,
    }];
    let render = |tools: &[ToolDefinition], code_mode_only, guidance: Option<&str>| {
        build_exec_tool_description(
            &[],
            tools,
            &BTreeMap::new(),
            crate::DEFAULT_EXEC_YIELD_TIME_MS,
            code_mode_only,
            ImageDetailVisibility::Visible,
            DeferredToolDiscovery::Catalog,
            Some(&CodeModeToolMessages {
                exec: Some(ToolMessage {
                    description: Some(String::new()),
                    ..Default::default()
                }),
                deferred_nested_tools_guidance: guidance.map(str::to_string),
                mcp_typescript_preamble: Some(String::new()),
                ..Default::default()
            }),
        )
    };
    for code_mode_only in [false, true] {
        for (guidance, expected) in [
            (None, DEFERRED_NESTED_TOOLS_GUIDANCE),
            (Some("Catalog discovery."), "Catalog discovery."),
            (Some(""), ""),
        ] {
            let prompt = render(&[], code_mode_only, guidance);
            assert_eq!(prompt, expected);
            assert_eq!(prompt, render(&deferred_tools, code_mode_only, guidance));
        }
    }
    assert!(DEFERRED_NESTED_TOOLS_GUIDANCE.contains("Tool availability can change between calls"));
}

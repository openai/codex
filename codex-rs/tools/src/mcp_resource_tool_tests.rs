use super::*;
use pretty_assertions::assert_eq;
use std::collections::BTreeMap;

#[test]
fn list_mcp_resources_tool_matches_expected_spec() {
    assert_eq!(
        create_list_mcp_resources_tool(),
        ToolSpec::Function(ResponsesApiTool {
            name: "list_mcp_resources".to_string(),
            description: "Lists resources provided by MCP servers. Resources allow servers to share data that provides context to language models, such as files, database schemas, or application-specific information. Prefer resources over web search when possible.".to_string(),
            strict: false,
            defer_loading: None,
            parameters: JsonSchema::Object {
                properties: BTreeMap::from([
                    (
                        "server".to_string(),
                        JsonSchema::String {
                            description: Some(
                                "Optional MCP server name. When omitted, lists resources from every configured server."
                                    .to_string(),
                            ),
                        },
                    ),
                    (
                        "cursor".to_string(),
                        JsonSchema::String {
                            description: Some(
                                "Opaque cursor returned by a previous list_mcp_resources call for the same server."
                                    .to_string(),
                            ),
                        },
                    ),
                ]),
                required: None,
                additional_properties: Some(false.into()),
            },
            output_schema: None,
        })
    );
}

#[test]
fn list_mcp_resource_templates_tool_matches_expected_spec() {
    assert_eq!(
        create_list_mcp_resource_templates_tool(),
        ToolSpec::Function(ResponsesApiTool {
            name: "list_mcp_resource_templates".to_string(),
            description: "Lists resource templates provided by MCP servers. Parameterized resource templates allow servers to share data that takes parameters and provides context to language models, such as files, database schemas, or application-specific information. Prefer resource templates over web search when possible.".to_string(),
            strict: false,
            defer_loading: None,
            parameters: JsonSchema::Object {
                properties: BTreeMap::from([
                    (
                        "server".to_string(),
                        JsonSchema::String {
                            description: Some(
                                "Optional MCP server name. When omitted, lists resource templates from all configured servers."
                                    .to_string(),
                            ),
                        },
                    ),
                    (
                        "cursor".to_string(),
                        JsonSchema::String {
                            description: Some(
                                "Opaque cursor returned by a previous list_mcp_resource_templates call for the same server."
                                    .to_string(),
                            ),
                        },
                    ),
                ]),
                required: None,
                additional_properties: Some(false.into()),
            },
            output_schema: None,
        })
    );
}

#[test]
fn read_mcp_resource_tool_matches_expected_spec() {
    assert_eq!(
        create_read_mcp_resource_tool(),
        ToolSpec::Function(ResponsesApiTool {
            name: "read_mcp_resource".to_string(),
            description:
                "Read a specific resource from an MCP server given the server name and resource URI."
                    .to_string(),
            strict: false,
            defer_loading: None,
            parameters: JsonSchema::Object {
                properties: BTreeMap::from([
                    (
                        "server".to_string(),
                        JsonSchema::String {
                            description: Some(
                                "MCP server name exactly as configured. Must match the 'server' field returned by list_mcp_resources."
                                    .to_string(),
                            ),
                        },
                    ),
                    (
                        "uri".to_string(),
                        JsonSchema::String {
                            description: Some(
                                "Resource URI to read. Must be one of the URIs returned by list_mcp_resources."
                                    .to_string(),
                            ),
                        },
                    ),
                ]),
                required: Some(vec!["server".to_string(), "uri".to_string()]),
                additional_properties: Some(false.into()),
            },
            output_schema: None,
        })
    );
}

use super::*;
use codex_tools::JsonSchema;
use pretty_assertions::assert_eq;
use std::collections::BTreeMap;

#[test]
fn list_mcp_resources_tool_matches_expected_spec() {
    assert_eq!(
        create_list_mcp_resources_tool(/*messages*/ None),
        ToolSpec::Function(ResponsesApiTool {
            name: "list_mcp_resources".to_string(),
            description: "Lists resources provided by MCP servers. Resources allow servers to share data that provides context to language models, such as files, database schemas, or application-specific information. Prefer resources over web search when possible.".to_string(),
            strict: false,
            defer_loading: None,
            parameters: JsonSchema::object(BTreeMap::from([
                    (
                        "server".to_string(),
                        JsonSchema::string(Some(
                                "MCP server name. Omit to list resources from every configured server."
                                    .to_string(),
                            ),),
                    ),
                    (
                        "cursor".to_string(),
                        JsonSchema::string(Some(
                                "Opaque cursor from a previous list_mcp_resources call; omit for the first page."
                                    .to_string(),
                            ),),
                    ),
                ]), /*required*/ None, Some(false.into())),
            output_schema: None,
        })
    );
}

#[test]
fn list_mcp_resource_templates_tool_matches_expected_spec() {
    assert_eq!(
        create_list_mcp_resource_templates_tool(/*messages*/ None),
        ToolSpec::Function(ResponsesApiTool {
            name: "list_mcp_resource_templates".to_string(),
            description: "Lists resource templates provided by MCP servers. Parameterized resource templates allow servers to share data that takes parameters and provides context to language models, such as files, database schemas, or application-specific information. Prefer resource templates over web search when possible.".to_string(),
            strict: false,
            defer_loading: None,
            parameters: JsonSchema::object(BTreeMap::from([
                    (
                        "server".to_string(),
                        JsonSchema::string(Some(
                                "MCP server name. Omit to list resource templates from every configured server."
                                    .to_string(),
                            ),),
                    ),
                    (
                        "cursor".to_string(),
                        JsonSchema::string(Some(
                                "Opaque cursor from a previous list_mcp_resource_templates call; omit for the first page."
                                    .to_string(),
                            ),),
                    ),
                ]), /*required*/ None, Some(false.into())),
            output_schema: None,
        })
    );
}

#[test]
fn read_mcp_resource_tool_matches_expected_spec() {
    assert_eq!(
        create_read_mcp_resource_tool(/*messages*/ None),
        ToolSpec::Function(ResponsesApiTool {
            name: "read_mcp_resource".to_string(),
            description:
                "Read a specific resource from an MCP server given the server name and resource URI."
                    .to_string(),
            strict: false,
            defer_loading: None,
            parameters: JsonSchema::object(BTreeMap::from([
                    (
                        "server".to_string(),
                        JsonSchema::string(Some(
                                "MCP server name exactly as configured. Must match the 'server' field returned by list_mcp_resources."
                                    .to_string(),
                            ),),
                    ),
                    (
                        "uri".to_string(),
                        JsonSchema::string(Some(
                                "Resource URI to read. Must be one of the URIs returned by list_mcp_resources."
                                    .to_string(),
                            ),),
                    ),
                ]), Some(vec!["server".to_string(), "uri".to_string()]), Some(false.into())),
            output_schema: None,
        })
    );
}

#[test]
fn catalog_fields_override_independently_and_invalid_parameters_fall_back() {
    let parameters = r#"{"type":"object","properties":{"server":{"type":"string","description":"Catalog server guidance."}},"additionalProperties":false}"#;
    for create_tool in [
        create_list_mcp_resources_tool,
        create_list_mcp_resource_templates_tool,
        create_read_mcp_resource_tool,
    ] {
        for (description, schema, valid_schema) in [
            (None, None, false),
            (Some(""), None, false),
            (None, Some(parameters), true),
            (
                Some("  Catalog {{literal}} guidance.\n"),
                Some(parameters),
                true,
            ),
            (Some("Catalog guidance."), Some("invalid JSON"), false),
            (None, Some(r#"{"type":"string"}"#), false),
        ] {
            let messages = ToolMessage {
                description: description.map(str::to_owned),
                parameters: schema.map(str::to_owned),
            };
            let mut expected = create_tool(/*messages*/ None);
            let ToolSpec::Function(tool) = &mut expected else {
                panic!("expected a function tool");
            };
            if let Some(description) = description {
                tool.description = description.to_owned();
            }
            if valid_schema {
                tool.parameters = serde_json::from_str(parameters).unwrap();
            }
            assert_eq!(create_tool(Some(&messages)), expected);
        }
    }
}

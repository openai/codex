use codex_tools::JsonSchema;
use codex_tools::ResponsesApiTool;
use codex_tools::ToolSpec;
use std::collections::BTreeMap;

pub(crate) const SEARCH_PLUGINS_TOOL_NAME: &str = "search_plugins";

pub(crate) fn create_search_plugins_tool() -> ToolSpec {
    let properties = BTreeMap::from([(
        "q".to_string(),
        JsonSchema::string(Some(
            "Plugin discovery query using the language documented in this tool description."
                .to_string(),
        )),
    )]);
    let description = "# Search global plugins

Search the global plugin catalog. Installed plugins are excluded. This returns the first page only.

The `q` argument uses a small KQL/Lucene-style filter language. Unfielded terms search plugin names, plugin descriptions, and callable app action names. To search a specific field, use `plugin_name:`, `plugin_description:`, or `tool_name:`. `tool_name` refers to callable app action names, not plugin skill names.

Matching is case-insensitive substring matching. Double-quoted phrases match contiguous substrings including spaces. Parentheses group expressions. Use `AND`, `OR`, and unary `NOT`; adjacent expressions are an implicit `AND`. Field scopes can apply to groups, such as `tool_name:(search OR messages)`.

Examples: `slack`, `plugin_name:slack`, `tool_name:\"search messages\"`, `plugin_name:slack AND tool_name:(search OR messages)`, `NOT tool_name:search`.

Wildcards, fuzziness, regex, ranges, scoring, and raw SQL are unsupported. Invalid queries return an error."
        .to_string();

    ToolSpec::Function(ResponsesApiTool {
        name: SEARCH_PLUGINS_TOOL_NAME.to_string(),
        description,
        strict: false,
        defer_loading: None,
        parameters: JsonSchema::object(properties, Some(vec!["q".to_string()]), Some(false.into())),
        output_schema: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn create_search_plugins_tool_uses_q_only_schema_and_documents_query_language() {
        let ToolSpec::Function(ResponsesApiTool {
            name,
            description,
            strict,
            defer_loading,
            parameters,
            output_schema,
        }) = create_search_plugins_tool()
        else {
            panic!("expected function tool");
        };
        assert_eq!(name, SEARCH_PLUGINS_TOOL_NAME);
        assert!(!strict);
        assert_eq!(defer_loading, None);
        assert_eq!(
            parameters,
            JsonSchema::object(
                BTreeMap::from([(
                    "q".to_string(),
                    JsonSchema::string(Some(
                        "Plugin discovery query using the language documented in this tool description."
                            .to_string(),
                    )),
                )]),
                Some(vec!["q".to_string()]),
                Some(false.into()),
            )
        );
        assert_eq!(output_schema, None);
        for expected in [
            "plugin_name:",
            "plugin_description:",
            "tool_name:",
            "case-insensitive substring",
            "Double-quoted phrases",
            "Parentheses",
            "`AND`, `OR`, and unary `NOT`",
            "implicit `AND`",
            "tool_name:(search OR messages)",
            "Wildcards, fuzziness, regex, ranges, scoring, and raw SQL are unsupported",
        ] {
            assert!(
                description.contains(expected),
                "tool description should mention {expected:?}"
            );
        }
    }
}

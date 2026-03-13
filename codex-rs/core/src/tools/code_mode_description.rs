use crate::client_common::tools::ToolSpec;
use crate::mcp::split_qualified_tool_name;
use crate::tools::code_mode::PUBLIC_TOOL_NAME;
use serde_json::Value as JsonValue;

pub(crate) struct CodeModeToolReference {
    pub(crate) module_path: String,
    pub(crate) namespace: Vec<String>,
    pub(crate) tool_key: String,
}

pub(crate) fn code_mode_tool_reference(tool_name: &str) -> CodeModeToolReference {
    if let Some((server_name, tool_key)) = split_qualified_tool_name(tool_name) {
        let namespace = vec!["mcp".to_string(), server_name];
        return CodeModeToolReference {
            module_path: format!("tools/{}.js", namespace.join("/")),
            namespace,
            tool_key,
        };
    }

    CodeModeToolReference {
        module_path: "tools.js".to_string(),
        namespace: Vec::new(),
        tool_key: tool_name.to_string(),
    }
}

pub(crate) fn augment_tool_spec_for_code_mode(spec: ToolSpec, code_mode_enabled: bool) -> ToolSpec {
    if !code_mode_enabled {
        return spec;
    }

    match spec {
        ToolSpec::Function(mut tool) => {
            if tool.name != PUBLIC_TOOL_NAME {
                tool.description = append_code_mode_sample(
                    &tool.description,
                    &tool.name,
                    "args",
                    serde_json::to_value(&tool.parameters)
                        .ok()
                        .as_ref()
                        .map(render_json_schema_to_typescript)
                        .unwrap_or_else(|| "unknown".to_string()),
                    tool.output_schema
                        .as_ref()
                        .map(render_json_schema_to_typescript)
                        .unwrap_or_else(|| "unknown".to_string()),
                );
            }
            ToolSpec::Function(tool)
        }
        ToolSpec::Freeform(mut tool) => {
            if tool.name != PUBLIC_TOOL_NAME {
                tool.description = append_code_mode_sample(
                    &tool.description,
                    &tool.name,
                    "input",
                    "string".to_string(),
                    "unknown".to_string(),
                );
            }
            ToolSpec::Freeform(tool)
        }
        other => other,
    }
}

fn append_code_mode_sample(
    description: &str,
    tool_name: &str,
    input_name: &str,
    input_type: String,
    output_type: String,
) -> String {
    let declaration = format!(
        "declare const tools: {{\n  {}\n}};",
        render_code_mode_tool_declaration(tool_name, input_name, input_type, output_type)
    );
    format!("{description}\n\nCode mode declaration:\n```ts\n{declaration}\n```")
}

fn render_code_mode_tool_declaration(
    tool_name: &str,
    input_name: &str,
    input_type: String,
    output_type: String,
) -> String {
    let input_type = indent_multiline_type(&input_type, 2);
    let output_type = indent_multiline_type(&output_type, 2);
    let tool_name = normalize_code_mode_identifier(tool_name);
    format!("{tool_name}({input_name}: {input_type}): Promise<{output_type}>;")
}

fn indent_multiline_type(type_name: &str, spaces: usize) -> String {
    let indent = " ".repeat(spaces);
    type_name
        .lines()
        .enumerate()
        .map(|(index, line)| {
            if index == 0 {
                line.to_string()
            } else {
                format!("{indent}{line}")
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub(crate) fn normalize_code_mode_identifier(tool_key: &str) -> String {
    let mut identifier = String::new();

    for (index, ch) in tool_key.chars().enumerate() {
        let is_valid = if index == 0 {
            ch == '_' || ch == '$' || ch.is_ascii_alphabetic()
        } else {
            ch == '_' || ch == '$' || ch.is_ascii_alphanumeric()
        };

        if is_valid {
            identifier.push(ch);
        } else {
            identifier.push('_');
        }
    }

    if identifier.is_empty() {
        "_".to_string()
    } else {
        identifier
    }
}

fn render_json_schema_to_typescript(schema: &JsonValue) -> String {
    render_json_schema_to_typescript_inner(schema, 0)
}

fn render_json_schema_to_typescript_inner(schema: &JsonValue, indent: usize) -> String {
    match schema {
        JsonValue::Bool(true) => "unknown".to_string(),
        JsonValue::Bool(false) => "never".to_string(),
        JsonValue::Object(map) => {
            if let Some(value) = map.get("const") {
                return render_json_schema_literal(value);
            }

            if let Some(values) = map.get("enum").and_then(serde_json::Value::as_array) {
                let rendered = values
                    .iter()
                    .map(render_json_schema_literal)
                    .collect::<Vec<_>>();
                if !rendered.is_empty() {
                    return rendered.join(" | ");
                }
            }

            for key in ["anyOf", "oneOf"] {
                if let Some(variants) = map.get(key).and_then(serde_json::Value::as_array) {
                    let rendered = variants
                        .iter()
                        .map(|variant| render_json_schema_to_typescript_inner(variant, indent))
                        .collect::<Vec<_>>();
                    if !rendered.is_empty() {
                        return rendered.join(" | ");
                    }
                }
            }

            if let Some(variants) = map.get("allOf").and_then(serde_json::Value::as_array) {
                let rendered = variants
                    .iter()
                    .map(|variant| render_json_schema_to_typescript_inner(variant, indent))
                    .collect::<Vec<_>>();
                if !rendered.is_empty() {
                    return rendered.join(" & ");
                }
            }

            if let Some(schema_type) = map.get("type") {
                if let Some(types) = schema_type.as_array() {
                    let rendered = types
                        .iter()
                        .filter_map(serde_json::Value::as_str)
                        .map(|schema_type| {
                            render_json_schema_type_keyword(map, schema_type, indent)
                        })
                        .collect::<Vec<_>>();
                    if !rendered.is_empty() {
                        return rendered.join(" | ");
                    }
                }

                if let Some(schema_type) = schema_type.as_str() {
                    return render_json_schema_type_keyword(map, schema_type, indent);
                }
            }

            if map.contains_key("properties")
                || map.contains_key("additionalProperties")
                || map.contains_key("required")
            {
                return render_json_schema_object(map, indent);
            }

            if map.contains_key("items") || map.contains_key("prefixItems") {
                return render_json_schema_array(map, indent);
            }

            "unknown".to_string()
        }
        _ => "unknown".to_string(),
    }
}

fn render_json_schema_type_keyword(
    map: &serde_json::Map<String, JsonValue>,
    schema_type: &str,
    indent: usize,
) -> String {
    match schema_type {
        "string" => "string".to_string(),
        "number" | "integer" => "number".to_string(),
        "boolean" => "boolean".to_string(),
        "null" => "null".to_string(),
        "array" => render_json_schema_array(map, indent),
        "object" => render_json_schema_object(map, indent),
        _ => "unknown".to_string(),
    }
}

fn render_json_schema_array(map: &serde_json::Map<String, JsonValue>, indent: usize) -> String {
    if let Some(items) = map.get("items") {
        let item_type = render_json_schema_to_typescript_inner(items, indent + 2);
        return format!("Array<{item_type}>");
    }

    if let Some(items) = map.get("prefixItems").and_then(serde_json::Value::as_array) {
        let item_types = items
            .iter()
            .map(|item| render_json_schema_to_typescript_inner(item, indent + 2))
            .collect::<Vec<_>>();
        if !item_types.is_empty() {
            return format!("[{}]", item_types.join(", "));
        }
    }

    "unknown[]".to_string()
}

fn render_json_schema_object(map: &serde_json::Map<String, JsonValue>, indent: usize) -> String {
    let required = map
        .get("required")
        .and_then(serde_json::Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(serde_json::Value::as_str)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let properties = map
        .get("properties")
        .and_then(serde_json::Value::as_object)
        .cloned()
        .unwrap_or_default();

    let mut sorted_properties = properties.iter().collect::<Vec<_>>();
    sorted_properties.sort_unstable_by(|(name_a, _), (name_b, _)| name_a.cmp(name_b));

    let mut lines = sorted_properties
        .into_iter()
        .map(|(name, value)| {
            let optional = if required.iter().any(|required_name| required_name == name) {
                ""
            } else {
                "?"
            };
            let property_name = render_json_schema_property_name(name);
            let property_type = render_json_schema_to_typescript_inner(value, indent + 2);
            format!(
                "{}{property_name}{optional}: {property_type};",
                " ".repeat(indent + 2)
            )
        })
        .collect::<Vec<_>>();

    if let Some(additional_properties) = map.get("additionalProperties") {
        let additional_type = match additional_properties {
            JsonValue::Bool(true) => Some("unknown".to_string()),
            JsonValue::Bool(false) => None,
            value => Some(render_json_schema_to_typescript_inner(value, indent + 2)),
        };

        if let Some(additional_type) = additional_type {
            lines.push(format!(
                "{}[key: string]: {additional_type};",
                " ".repeat(indent + 2)
            ));
        }
    } else if properties.is_empty() {
        lines.push(format!("{}[key: string]: unknown;", " ".repeat(indent + 2)));
    }

    if lines.is_empty() {
        return "{}".to_string();
    }

    format!("{{\n{}\n{}}}", lines.join("\n"), " ".repeat(indent))
}

fn render_json_schema_property_name(name: &str) -> String {
    if normalize_code_mode_identifier(name) == name {
        name.to_string()
    } else {
        serde_json::to_string(name).unwrap_or_else(|_| format!("\"{}\"", name.replace('"', "\\\"")))
    }
}

fn render_json_schema_literal(value: &JsonValue) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "unknown".to_string())
}

#[cfg(test)]
#[path = "code_mode_description_tests.rs"]
mod tests;

use codex_tools::ToolName;
use codex_tools::mcp_tool_to_responses_api_tool;
use pretty_assertions::assert_eq;
use serde::Deserialize;
use serde_json::Value;
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

const FIXTURE_PATHS: [&str; 5] = [
    "tests/fixtures/json_schema_policy/slack.json",
    "tests/fixtures/json_schema_policy/google_calendar.json",
    "tests/fixtures/json_schema_policy/google_drive.json",
    "tests/fixtures/json_schema_policy/notion.json",
    "tests/fixtures/json_schema_policy/microsoft_outlook_email.json",
];

#[derive(Debug, Deserialize)]
struct FixtureFile {
    source: String,
    tools: Vec<FixtureTool>,
}

#[derive(Debug, Deserialize)]
struct FixtureTool {
    name: String,
    description: String,
    input_schema: Value,
    #[serde(default)]
    expected_preserved: Vec<ExpectedValue>,
    #[serde(default)]
    expected_pruned: Vec<String>,
    expected_markers: ExpectedMarkers,
}

#[derive(Debug, Deserialize)]
struct ExpectedValue {
    pointer: String,
    value: Value,
}

#[derive(Debug, Deserialize)]
struct ExpectedMarkers {
    input: MarkerCounts,
    output: MarkerCounts,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
struct MarkerCounts {
    schema_refs: usize,
    defs: usize,
    definitions: usize,
    any_of: usize,
    one_of: usize,
    all_of: usize,
    descriptions: usize,
    enums: usize,
}

#[test]
fn json_schema_policy_fixtures_convert_to_responses_tools() {
    for fixture in load_fixtures() {
        for fixture_tool in &fixture.tools {
            let responses_tool = convert_fixture_tool(&fixture.source, fixture_tool);
            let parameters = serde_json::to_value(&responses_tool.parameters)
                .expect("responses parameters should serialize");

            assert_eq!(
                responses_tool.name, fixture_tool.name,
                "{} should preserve the tool name",
                fixture_tool.name
            );
            assert_eq!(
                responses_tool.description, fixture_tool.description,
                "{} should preserve the tool description",
                fixture_tool.name
            );
            assert!(
                !responses_tool.strict,
                "{} should remain a strict:false tool",
                fixture_tool.name
            );
            assert_eq!(
                parameters.get("type"),
                Some(&Value::String("object".to_string())),
                "{} should produce object-shaped parameters",
                fixture_tool.name
            );
            assert!(
                parameters.get("properties").is_some_and(Value::is_object),
                "{} should produce a parameters.properties object",
                fixture_tool.name
            );
        }
    }
}

#[test]
fn json_schema_policy_fixtures_preserve_model_visible_guidance() {
    for fixture in load_fixtures() {
        for fixture_tool in &fixture.tools {
            let responses_tool = convert_fixture_tool(&fixture.source, fixture_tool);
            let parameters = serde_json::to_value(&responses_tool.parameters)
                .expect("responses parameters should serialize");

            for expected in &fixture_tool.expected_preserved {
                assert_eq!(
                    parameters.pointer(&expected.pointer),
                    Some(&expected.value),
                    "{} should preserve {}",
                    fixture_tool.name,
                    expected.pointer
                );
            }
        }
    }
}

#[test]
fn json_schema_policy_fixtures_prune_unreachable_definitions() {
    for fixture in load_fixtures() {
        for fixture_tool in &fixture.tools {
            let responses_tool = convert_fixture_tool(&fixture.source, fixture_tool);
            let parameters = serde_json::to_value(&responses_tool.parameters)
                .expect("responses parameters should serialize");

            for pointer in &fixture_tool.expected_pruned {
                assert!(
                    parameters.pointer(pointer).is_none(),
                    "{} should prune unreachable definition {pointer}",
                    fixture_tool.name
                );
            }

            let output_refs = collect_local_definition_refs(&parameters);
            for target in output_refs {
                if input_schema_defines_target(&fixture_tool.input_schema, &target) {
                    assert!(
                        output_schema_defines_target(&parameters, &target),
                        "{} should not leave reachable local ref {} dangling",
                        fixture_tool.name,
                        target.schema_ref
                    );
                }
            }
        }
    }
}

#[test]
fn json_schema_policy_fixtures_match_marker_baselines() {
    for fixture in load_fixtures() {
        for fixture_tool in &fixture.tools {
            let responses_tool = convert_fixture_tool(&fixture.source, fixture_tool);
            let parameters = serde_json::to_value(&responses_tool.parameters)
                .expect("responses parameters should serialize");

            assert_eq!(
                marker_counts(&fixture_tool.input_schema),
                fixture_tool.expected_markers.input,
                "{} input marker baseline changed",
                fixture_tool.name
            );
            assert_eq!(
                marker_counts(&parameters),
                fixture_tool.expected_markers.output,
                "{} output marker baseline changed",
                fixture_tool.name
            );
        }
    }
}

fn load_fixtures() -> Vec<FixtureFile> {
    FIXTURE_PATHS
        .into_iter()
        .map(|path| {
            let path = fixture_path(path);
            let fixture = fs::read_to_string(&path)
                .unwrap_or_else(|err| panic!("read fixture {}: {err}", path.display()));
            serde_json::from_str(&fixture)
                .unwrap_or_else(|err| panic!("parse fixture {}: {err}", path.display()))
        })
        .collect()
}

fn fixture_path(path: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(path)
}

fn convert_fixture_tool(source: &str, fixture_tool: &FixtureTool) -> codex_tools::ResponsesApiTool {
    let input_schema = fixture_tool
        .input_schema
        .as_object()
        .unwrap_or_else(|| panic!("{} input_schema should be an object", fixture_tool.name))
        .clone();
    let tool = rmcp::model::Tool {
        name: fixture_tool.name.clone().into(),
        title: None,
        description: Some(fixture_tool.description.clone().into()),
        input_schema: Arc::new(input_schema),
        output_schema: None,
        annotations: None,
        execution: None,
        icons: None,
        meta: None,
    };

    mcp_tool_to_responses_api_tool(&ToolName::namespaced(source, &fixture_tool.name), &tool)
        .unwrap_or_else(|err| panic!("convert {} from {source}: {err}", fixture_tool.name))
}

fn marker_counts(value: &Value) -> MarkerCounts {
    MarkerCounts {
        schema_refs: count_key(value, "$ref"),
        defs: count_key(value, "$defs"),
        definitions: count_key(value, "definitions"),
        any_of: count_key(value, "anyOf"),
        one_of: count_key(value, "oneOf"),
        all_of: count_key(value, "allOf"),
        descriptions: count_key(value, "description"),
        enums: count_key(value, "enum"),
    }
}

fn count_key(value: &Value, target: &str) -> usize {
    match value {
        Value::Array(values) => values.iter().map(|value| count_key(value, target)).sum(),
        Value::Object(map) => {
            let current = usize::from(map.contains_key(target));
            current
                + map
                    .values()
                    .map(|value| count_key(value, target))
                    .sum::<usize>()
        }
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => 0,
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct LocalDefinitionTarget {
    schema_ref: String,
    table: &'static str,
    name: String,
}

fn collect_local_definition_refs(value: &Value) -> BTreeSet<LocalDefinitionTarget> {
    let mut refs = BTreeSet::new();
    collect_local_definition_refs_from_value(value, &mut refs);
    refs
}

fn collect_local_definition_refs_from_value(
    value: &Value,
    refs: &mut BTreeSet<LocalDefinitionTarget>,
) {
    match value {
        Value::Array(values) => {
            for value in values {
                collect_local_definition_refs_from_value(value, refs);
            }
        }
        Value::Object(map) => {
            if let Some(Value::String(schema_ref)) = map.get("$ref")
                && let Some(target) = local_definition_target(schema_ref)
            {
                refs.insert(target);
            }
            for value in map.values() {
                collect_local_definition_refs_from_value(value, refs);
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
    }
}

fn local_definition_target(schema_ref: &str) -> Option<LocalDefinitionTarget> {
    let (table, name) = schema_ref
        .strip_prefix("#/$defs/")
        .map(|rest| ("$defs", rest))
        .or_else(|| {
            schema_ref
                .strip_prefix("#/definitions/")
                .map(|rest| ("definitions", rest))
        })?;
    let name = name.split('/').next().unwrap_or_default();
    Some(LocalDefinitionTarget {
        schema_ref: schema_ref.to_string(),
        table,
        name: decode_fixture_ref_name(name),
    })
}

fn decode_fixture_ref_name(name: &str) -> String {
    name.replace("%20", " ")
        .replace("%24", "$")
        .replace("%7E0", "~")
        .replace("%7e0", "~")
}

fn input_schema_defines_target(value: &Value, target: &LocalDefinitionTarget) -> bool {
    root_definition_table(value, target.table)
        .and_then(|definitions| definitions.get(&target.name))
        .is_some()
}

fn output_schema_defines_target(value: &Value, target: &LocalDefinitionTarget) -> bool {
    input_schema_defines_target(value, target)
}

fn root_definition_table<'a>(
    value: &'a Value,
    table: &str,
) -> Option<&'a serde_json::Map<String, Value>> {
    value
        .as_object()
        .and_then(|map| map.get(table))
        .and_then(Value::as_object)
}

//! Measure connector schema preparation and Responses Lite catalog encoding.
//! Run `cargo bench -p codex-tools --bench tool_catalog` from codex-rs.
//! For a baseline, copy this file and its Cargo bench target and dev-dependency
//! to a checkout of the base revision, then run the same command there.

#![allow(clippy::expect_used)]

use codex_tools::ResponsesApiNamespace;
use codex_tools::ResponsesApiNamespaceTool;
use codex_tools::ResponsesApiTool;
use codex_tools::ToolSpec;
use codex_tools::create_tools_json_for_responses_lite;
use codex_tools::parse_tool_input_schema;
use divan::Bencher;
use serde_json::Value;

const SCHEMAS: [&str; 6] = [
    "google_calendar",
    "google_drive",
    "microsoft_outlook_email",
    "notion",
    "slack",
    "oversized_notion_create_page_input_schema",
];

fn main() {
    divan::main();
}

fn load_schema(name: &str) -> Value {
    let path = format!("tests/fixtures/json_schema_policy/{name}.json");
    let path = codex_utils_cargo_bin::find_resource!(path).expect("resolve connector fixture");
    let fixture = std::fs::read(path).expect("read connector fixture");
    let mut fixture: Value = serde_json::from_slice(&fixture).expect("decode connector fixture");
    fixture["tools"][0]["input_schema"].take()
}

#[divan::bench(args = SCHEMAS)]
fn prepare_connector_schema(bencher: Bencher, name: &str) {
    let schema = load_schema(name);
    bencher.bench_local(|| {
        parse_tool_input_schema(divan::black_box(&schema)).expect("prepare fixture schema")
    });
}

#[divan::bench(args = [1, 64])]
fn encode_responses_lite_catalog(bencher: Bencher, count: usize) {
    let schema =
        parse_tool_input_schema(&load_schema("google_drive")).expect("prepare tool schema");
    let mut specs = Vec::new();
    for i in 0..count {
        let tool = ResponsesApiTool {
            name: format!("lookup_{i}"),
            description: "Look up a file in the selected drive.".to_string(),
            strict: false,
            defer_loading: None,
            parameters: schema.clone(),
            output_schema: None,
        };
        let spec = match i % 3 {
            0 => ToolSpec::Function(tool),
            kind => ToolSpec::Namespace(ResponsesApiNamespace {
                name: if kind == 1 {
                    "functions".to_string()
                } else {
                    format!("provider_{i}")
                },
                description: format!("File tools for collection {i}"),
                tools: vec![ResponsesApiNamespaceTool::Function(tool)],
            }),
        };
        specs.push(spec);
    }
    bencher.bench_local(|| {
        let tools = create_tools_json_for_responses_lite(divan::black_box(specs.as_slice()))
            .expect("serialize Responses Lite catalog");
        serde_json::to_vec(&tools).expect("encode catalog")
    });
}

use std::path::PathBuf;

use core_test_support::agent_harness;
use core_test_support::agent_harness::HarnessOutputs;
use pretty_assertions::assert_eq;
use serde_json::Value;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn single_turn_fixture_matches_expectations() {
    let fixture_dir =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/harness/single_turn");
    let harness = agent_harness::run_fixture(fixture_dir)
        .await
        .expect("run agent harness");

    assert_harness_outputs_match(&harness.actual, &harness.expected);
}

fn assert_harness_outputs_match(actual: &HarnessOutputs, expected: &HarnessOutputs) {
    assert_value_matches(&actual.request, &expected.request, "request");
    assert_eq!(
        actual.events.len(),
        expected.events.len(),
        "event count mismatch: expected {} events but got {}",
        expected.events.len(),
        actual.events.len()
    );
    for (index, expected_event) in expected.events.iter().enumerate() {
        let path = format!("events[{index}]");
        let actual_event = actual
            .events
            .get(index)
            .unwrap_or_else(|| panic!("missing actual event at {path}"));
        assert_value_matches(actual_event, expected_event, &path);
    }
}

fn assert_value_matches(actual: &Value, expected: &Value, path: &str) {
    match expected {
        Value::Object(expected_map) => {
            let actual_map = actual
                .as_object()
                .unwrap_or_else(|| panic!("expected object at {path}, got {actual:?}"));
            for (key, expected_value) in expected_map {
                let next_path = if path.is_empty() {
                    key.to_string()
                } else {
                    format!("{path}.{key}")
                };
                let actual_value = actual_map
                    .get(key)
                    .unwrap_or_else(|| panic!("missing field {next_path} in actual value"));
                assert_value_matches(actual_value, expected_value, &next_path);
            }
        }
        Value::Array(expected_items) => {
            let actual_items = actual
                .as_array()
                .unwrap_or_else(|| panic!("expected array at {path}, got {actual:?}"));
            assert_eq!(
                actual_items.len(),
                expected_items.len(),
                "array length mismatch at {}",
                path
            );
            for (index, expected_value) in expected_items.iter().enumerate() {
                let next_path = if path.is_empty() {
                    format!("[{index}]")
                } else {
                    format!("{path}[{index}]")
                };
                let actual_value = actual_items
                    .get(index)
                    .unwrap_or_else(|| panic!("missing array element at {next_path}"));
                assert_value_matches(actual_value, expected_value, &next_path);
            }
        }
        _ => {
            assert_eq!(actual, expected, "mismatch at {path}");
        }
    }
}

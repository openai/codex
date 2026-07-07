use super::*;
use codex_protocol::ThreadId;
use pretty_assertions::assert_eq;
use serde_json::Value;

#[test]
fn feedback_snapshot_is_bounded_and_omits_sensitive_payloads() {
    let recorder = ToolSearchPipelineDiagnostics::default();
    let identities = (0..(MAX_STAGE_IDENTITIES + 20))
        .map(identity)
        .collect::<Vec<_>>();
    {
        let mut state = recorder.state();
        state.inventory_generation = "generation".to_string();
        record_stage(
            &mut state,
            "mcp_inventory_snapshot",
            "generation",
            identities,
            StageMetadata::default(),
            Vec::new(),
        );
    }
    for search_index in 0..(MAX_SEARCHES + 4) {
        let results = (0..(MAX_SEARCH_RESULTS + 7))
            .map(|result_index| SearchResultDiagnostic {
                rank: result_index + 1,
                score: result_index as f32,
                document_index: result_index,
                identity: identity(result_index),
            })
            .collect();
        recorder.record_search(
            &format!("{} sensitive-search-text", "q".repeat(MAX_QUERY_CHARS + 40)),
            search_index + 1,
            MAX_SEARCH_RESULTS + 7,
            results,
        );
    }

    let bytes = recorder
        .feedback_json(ThreadId::new(), ThreadId::new().into())
        .expect("snapshot should be present");
    let text = String::from_utf8(bytes.clone()).expect("json should be utf8");
    let json: Value = serde_json::from_slice(&bytes).expect("json should parse");

    assert_eq!(
        json["stages"][0]["identities"]
            .as_array()
            .expect("identities should be an array")
            .len(),
        MAX_STAGE_IDENTITIES
    );
    assert_eq!(
        json["searches"]
            .as_array()
            .expect("searches should be an array")
            .len(),
        MAX_SEARCHES
    );
    assert_eq!(
        json["searches"][0]["results"]
            .as_array()
            .expect("results should be an array")
            .len(),
        MAX_SEARCH_RESULTS
    );
    assert!(!text.contains("input_schema"));
    assert!(!text.contains("output_schema"));
    assert!(!text.contains("search_text"));
}

#[test]
fn changed_fingerprint_records_added_and_removed_canonical_identities() {
    let recorder = ToolSearchPipelineDiagnostics::default();
    {
        let mut state = recorder.state();
        state.inventory_generation = "generation".to_string();
        record_stage(
            &mut state,
            "mcp_inventory_snapshot",
            "generation",
            vec![identity(/*index*/ 1)],
            StageMetadata::default(),
            Vec::new(),
        );
        record_stage(
            &mut state,
            "mcp_inventory_snapshot",
            "generation",
            vec![identity(/*index*/ 2)],
            StageMetadata::default(),
            Vec::new(),
        );
    }

    let bytes = recorder
        .feedback_json(ThreadId::new(), ThreadId::new().into())
        .expect("snapshot should be present");
    let json: Value = serde_json::from_slice(&bytes).expect("json should parse");
    let stage = &json["stages"][0];

    assert_eq!(
        stage["added"],
        serde_json::json!(["server_2|raw_tool_2|namespace.tool_2|connector_2"])
    );
    assert_eq!(
        stage["removed"],
        serde_json::json!(["server_1|raw_tool_1|namespace.tool_1|connector_1"])
    );
}

#[test]
fn trace_subscriber_does_not_enable_verbose_tool_search_payloads() {
    let recorder = ToolSearchPipelineDiagnostics::default();
    let subscriber = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::TRACE)
        .finish();

    tracing::subscriber::with_default(subscriber, || {
        let mut state = recorder.state();
        state.inventory_generation = "generation".to_string();
        record_stage(
            &mut state,
            "mcp_inventory_snapshot",
            "generation",
            vec![identity(/*index*/ 1)],
            StageMetadata::default(),
            Vec::new(),
        );
    });

    let bytes = recorder
        .feedback_json(ThreadId::new(), ThreadId::new().into())
        .expect("always-on snapshot should not depend on RUST_LOG");
    let text = String::from_utf8(bytes).expect("json should be utf8");
    assert!(text.contains("mcp_inventory_snapshot"));
    assert!(!text.contains("input_schema"));
    assert!(!text.contains("output_schema"));
    assert!(!text.contains("search_text"));
}

fn identity(index: usize) -> DiagnosticToolIdentity {
    DiagnosticToolIdentity {
        server_name: format!("server_{index}"),
        raw_tool_name: format!("raw_tool_{index}"),
        tool_name: Some(format!("namespace.tool_{index}")),
        connector_id: Some(format!("connector_{index}")),
        connector_name: Some(format!("connector name {index}")),
        plugin_display_names: vec![format!("plugin_{index}")],
    }
}

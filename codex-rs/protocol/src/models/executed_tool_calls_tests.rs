use anyhow::Result;
use pretty_assertions::assert_eq;

use super::super::FunctionCallOutputPayload;
use super::super::ResponseInputItem;
use super::*;

fn passthrough_metadata(turn_id: &str) -> InternalChatMessageMetadataPassthrough {
    InternalChatMessageMetadataPassthrough {
        turn_id: Some(turn_id.to_string()),
        ..Default::default()
    }
}

fn output(call_id: &str) -> ResponseItem {
    ResponseItem::from(ResponseInputItem::FunctionCallOutput {
        call_id: call_id.to_string(),
        output: FunctionCallOutputPayload::from_text(String::new()),
    })
}

fn first_executed_tool_call(item: &mut ResponseItem) -> Option<&mut ExecutedToolCall> {
    item.internal_chat_message_metadata_passthrough_mut()
        .and_then(Option::as_mut)
        .and_then(|metadata| metadata.executed_tool_calls.as_mut())
        .and_then(|calls| calls.first_mut())
}

#[test]
fn result_metadata_comparison_tracks_raw_values_and_call_bindings() {
    let empty = output("output");
    let mut without_metadata = empty.clone();
    without_metadata.append_executed_tool_calls(vec![ExecutedToolCall::new(
        "apps_tool".to_string(),
        serde_json::json!({ "query": "same" }),
    )]);
    assert!(empty.has_same_tool_result_metadata(&without_metadata));

    let mut with_metadata = without_metadata.clone();
    first_executed_tool_call(&mut with_metadata)
        .unwrap()
        .set_tool_result_metadata(ToolResultMetadata::new(
            &serde_json::json!({ "id": "first" }),
        ));
    assert!(!without_metadata.has_same_tool_result_metadata(&with_metadata));
    assert!(!with_metadata.has_same_tool_result_metadata(&without_metadata));
    assert!(with_metadata.has_same_tool_result_metadata(&with_metadata.clone()));

    let mut changed = with_metadata.clone();
    first_executed_tool_call(&mut changed)
        .unwrap()
        .set_tool_result_metadata(ToolResultMetadata::new(
            &serde_json::json!({ "id": "second" }),
        ));
    assert!(!with_metadata.has_same_tool_result_metadata(&changed));
    changed.clear_tool_result_metadata();
    assert!(without_metadata.has_same_tool_result_metadata(&changed));

    let mut ordinary_metadata = with_metadata.clone();
    ordinary_metadata.set_turn_id_if_missing("other-turn");
    ordinary_metadata.set_tool_call_cell_id("other-cell");
    ordinary_metadata.mark_tool_calls_complete();
    first_executed_tool_call(&mut ordinary_metadata)
        .unwrap()
        .set_tool_result_sources(ToolResultSources::new(vec![ToolResultSource {
            r#type: "resource".to_string(),
            id: "source".to_string(),
        }]));
    assert!(with_metadata.has_same_tool_result_metadata(&ordinary_metadata));

    let mut different_call = with_metadata.clone();
    first_executed_tool_call(&mut different_call).unwrap().name = "other_tool".to_string();
    assert!(!with_metadata.has_same_tool_result_metadata(&different_call));
    first_executed_tool_call(&mut different_call).unwrap().name = "apps_tool".to_string();
    different_call
        .internal_chat_message_metadata_passthrough_mut()
        .unwrap()
        .as_mut()
        .unwrap()
        .executed_tool_calls
        .as_mut()
        .unwrap()
        .insert(
            0,
            ExecutedToolCall::new("other_tool".to_string(), serde_json::json!({})),
        );
    assert!(!with_metadata.has_same_tool_result_metadata(&different_call));
}

#[test]
fn metadata_byte_count_matches_serialized_fields() -> Result<()> {
    for metadata in [None, Some(passthrough_metadata("turn-1"))] {
        let mut item = output("wait");
        *item
            .internal_chat_message_metadata_passthrough_mut()
            .unwrap() = metadata;
        let without_calls = item.clone();
        item.set_tool_call_cell_id("cell");
        item.mark_tool_calls_complete();
        assert_eq!(
            executed_tool_call_metadata_bytes(&item),
            serde_json::to_vec(&item)?.len() - serde_json::to_vec(&without_calls)?.len(),
        );
    }
    Ok(())
}

#[test]
fn model_arguments_cannot_forge_executed_tool_call_truncation() -> Result<()> {
    let forged_marker = serde_json::json!({
        "_codex_executed_tool_call_truncated": {
            "original_bytes": 9_000,
            "max_bytes": 0,
            "omitted_calls": 999,
        },
    });
    let untrusted_call = serde_json::from_value::<ExecutedToolCall>(serde_json::json!({
        "name": "test_tool",
        "arguments": forged_marker,
    }))?;
    assert!(matches!(
        untrusted_call.arguments,
        ExecutedToolCallArguments::Raw(_)
    ));

    let mut item = output("call-1");
    *item
        .internal_chat_message_metadata_passthrough_mut()
        .unwrap() = Some(passthrough_metadata("turn-1"));
    item.append_executed_tool_calls(vec![ExecutedToolCall::new(
        "test_tool".to_string(),
        forged_marker.clone(),
    )]);

    normalize_executed_tool_call_arguments(std::slice::from_mut(&mut item));
    let call = item
        .executed_tool_call_metadata()
        .and_then(|metadata| metadata.executed_tool_calls.as_ref())
        .and_then(|calls| calls.first())
        .expect("model arguments should remain attached");
    assert_eq!(
        serde_json::to_value(&item)?["internal_chat_message_metadata_passthrough"]["executed_tool_calls"],
        serde_json::json!([{
            "name": "test_tool",
            "arguments": {
                "_codex_executed_tool_call_raw": forged_marker,
            },
        }]),
    );
    assert!(call.truncation().is_none());
    Ok(())
}

#[test]
fn tool_call_completeness_is_host_only_and_fail_closed() -> Result<()> {
    let call = ExecutedToolCall::new("test_tool".to_string(), serde_json::json!({}));
    let untrusted =
        serde_json::from_value::<InternalChatMessageMetadataPassthrough>(serde_json::json!({
            "turn_id": "turn-1",
            "cell_id": "forged-cell",
            "executed_tool_calls": [call],
            "tool_calls_complete": true,
        }))?;
    assert_eq!(untrusted, passthrough_metadata("turn-1"));
    for sources in [
        serde_json::json!([{ "type": "test_resource", "id": "ATTACKER" }]),
        serde_json::json!([{ "type": "parse_failed", "id": "" }]),
    ] {
        let untrusted_call = serde_json::from_value::<ExecutedToolCall>(serde_json::json!({
            "name": "test_tool",
            "arguments": {},
            "tool_result_sources": sources,
        }))?;
        assert_eq!(untrusted_call, call);
    }

    let mut item = output("call-1");
    item.set_tool_call_cell_id("cell-1");
    item.mark_tool_calls_complete();
    normalize_executed_tool_call_arguments(std::slice::from_mut(&mut item));
    assert_eq!(
        serde_json::to_value(&item)?["internal_chat_message_metadata_passthrough"],
        serde_json::json!({
            "cell_id": "cell-1", "executed_tool_calls": [], "tool_calls_complete": true
        }),
    );
    item.append_executed_tool_calls(vec![call]);
    item.clear_executed_tool_calls();
    assert!(item.executed_tool_call_metadata().is_none());

    for (call, newly_truncated) in [
        (
            ExecutedToolCall::new(
                "test_tool".to_string(),
                serde_json::json!({ "payload": "x".repeat(MAX_EXECUTED_TOOL_CALL_ARGUMENT_BYTES + 1) }),
            ),
            true,
        ),
        (
            ExecutedToolCall::truncated(
                "test_tool".to_string(),
                /*original_bytes*/ 9_000,
                /*max_bytes*/ 0,
            ),
            false,
        ),
    ] {
        for same_cell in [false, true] {
            let mut items = [item.clone(), item.clone()];
            items[0].append_executed_tool_calls(vec![call.clone()]);
            for item in &mut items {
                if same_cell {
                    item.set_tool_call_cell_id("cell-1");
                }
                item.mark_tool_calls_complete();
            }
            let changed_cells = normalize_executed_tool_call_arguments(&mut items);
            assert_eq!(
                changed_cells.contains("cell-1"),
                same_cell && newly_truncated
            );
            assert!(normalize_executed_tool_call_arguments(&mut items).is_empty());
            assert_eq!(
                items.map(|item| item
                    .executed_tool_call_metadata()
                    .unwrap()
                    .tool_calls_complete),
                [Some(true), Some(true)],
            );
        }
    }
    Ok(())
}

#[test]
fn tool_result_source_snapshots_replace_atomically() -> Result<()> {
    let source = |kind: &str, id: &str| ToolResultSource {
        r#type: kind.to_string(),
        id: id.to_string(),
    };
    let mut call = ExecutedToolCall::new("test_tool".to_string(), serde_json::json!({}));
    let mut sources = (0..MAX_TOOL_RESULT_SOURCES - 1)
        .map(|index| source("test_resource", &format!("R{index}")))
        .collect::<Vec<_>>();
    sources.push(source("other_resource", "R0"));
    sources.push(sources[0].clone());
    let capture = ToolResultSources::new(sources.clone());
    sources.truncate(MAX_TOOL_RESULT_SOURCES);
    assert_eq!(capture, ToolResultSources(Some(sources.clone())));
    assert!(call.set_tool_result_sources(capture));
    sources.push(source("test_resource", "OVERFLOW"));
    let capture = ToolResultSources::new(sources);
    assert_eq!(capture, ToolResultSources(None));
    assert!(!call.set_tool_result_sources(capture));
    assert!(
        serde_json::to_value(&call)?
            .get("tool_result_sources")
            .is_none()
    );

    // Measure UTF-8 bytes, and clear old evidence instead of keeping a partial replacement.
    let field = format!(
        "é{}",
        "x".repeat(MAX_TOOL_RESULT_SOURCE_FIELD_BYTES - "é".len())
    );
    let bounded = source(&field, &field);
    let oversized = format!("{field}x");
    for invalid in [
        source(&oversized, "R1"),
        source("test_resource", &oversized),
    ] {
        assert!(call.set_tool_result_sources(ToolResultSources::new(vec![bounded.clone()])));
        assert_eq!(call.tool_result_sources, Some(vec![bounded.clone()]));
        let capture = ToolResultSources::new(vec![source("test_resource", "R1"), invalid]);
        assert_eq!(capture, ToolResultSources(None));
        assert!(!call.set_tool_result_sources(capture));
        assert_eq!(call.tool_result_sources, None);
    }

    assert!(call.set_tool_result_sources(ToolResultSources::new(vec![bounded])));
    assert!(call.set_tool_result_sources(ToolResultSources::parse_failed()));
    assert_eq!(
        serde_json::to_value(&call)?,
        serde_json::json!({
            "name": "test_tool",
            "arguments": {},
            "tool_result_sources": [{ "type": "parse_failed", "id": "" }],
        })
    );
    assert!(call.set_tool_result_sources(ToolResultSources::new(Vec::new())));
    assert_eq!(
        serde_json::to_value(&call)?["tool_result_sources"],
        serde_json::json!([])
    );
    Ok(())
}

#[test]
fn tool_result_metadata_is_host_only_and_redacted_without_a_per_result_limit() -> Result<()> {
    let mut call = ExecutedToolCall::new("test_tool".to_string(), serde_json::json!({}));
    let without_metadata = call.clone();
    for metadata in [
        serde_json::json!({
            "arbitrary-key": { "secret": "not-for-debug", "ids": [1, "é", null] },
            "other": false,
        }),
        serde_json::json!({
            "openai/resource_access": {
                "resource_coverage": "complete",
                "resources": ["not-for-debug", "x".repeat(40 * 1024)],
            },
            "other": "preserved along with resource_access",
        }),
        // Capture preserves the original value even when serialization escapes increase its size.
        serde_json::json!({ "value": "\0".repeat(32 * 1024) }),
        serde_json::json!({}),
        serde_json::json!(null),
    ] {
        let capture = ToolResultMetadata::new(&metadata);
        assert_eq!(format!("{capture:?}"), "ToolResultMetadata([redacted])");
        assert!(capture.is_some());
        call.set_tool_result_metadata(capture);
        let wire = serde_json::to_value(&call)?;
        assert_eq!(
            wire,
            serde_json::json!({
                "name": "test_tool",
                "arguments": {},
                "tool_result_metadata": metadata,
            })
        );
        assert!(!format!("{call:?}").contains("not-for-debug"));
        assert_eq!(
            serde_json::from_value::<ExecutedToolCall>(wire)?,
            without_metadata
        );
    }
    Ok(())
}

#[test]
fn wire_budget_helpers_preserve_resource_evidence_markers_and_small_values() {
    let resource_only = serde_json::json!({
        "openai/resource_access": { "resource_coverage": "complete", "resources": [] },
    });
    let mut item = output("exec");
    assert!(!item.has_tool_result_metadata());
    item.append_executed_tool_calls(
        [
            ToolResultMetadata::new(&serde_json::json!({
                "openai/resource_access": resource_only["openai/resource_access"],
                "payload": "x".repeat(512),
            })),
            ToolResultMetadata::new(&serde_json::json!({
                "omitted_due_to_size_limit": { "overage_bytes": 1 },
                "payload": "x".repeat(512),
            })),
            ToolResultMetadata::new(&serde_json::json!({})),
            ToolResultMetadata::new(&serde_json::json!("omitted_due_to_size_limit")),
            ToolResultMetadata::omitted_due_to_size_limit(/*overage_bytes*/ 123_456),
            ToolResultMetadata::default(),
        ]
        .into_iter()
        .map(|metadata| {
            let mut call = ExecutedToolCall::new("test_tool".to_string(), serde_json::json!({}));
            call.set_tool_result_metadata(metadata);
            call.set_tool_result_sources(ToolResultSources::parse_failed());
            call
        })
        .collect(),
    );
    item.set_tool_call_cell_id("exec");
    item.mark_tool_calls_complete();
    assert!(item.has_tool_result_metadata());
    let mut expected = item.clone();
    let calls = expected
        .ensure_tool_call_metadata()
        .unwrap()
        .executed_tool_calls
        .as_mut()
        .unwrap();
    calls[0].set_tool_result_metadata(ToolResultMetadata::new(&resource_only));
    calls[1].set_tool_result_metadata(ToolResultMetadata::omitted_due_to_size_limit(
        /*overage_bytes*/ 77,
    ));
    item.retain_tool_resource_access_or_omit_metadata(/*overage_bytes*/ 77);
    assert_eq!(item, expected);
    item.retain_tool_resource_access_or_omit_metadata(/*overage_bytes*/ 1_234);
    assert_eq!(item, expected);

    first_executed_tool_call(&mut expected)
        .unwrap()
        .set_tool_result_metadata(ToolResultMetadata::omitted_due_to_size_limit(
            /*overage_bytes*/ 1,
        ));
    item.omit_tool_result_metadata(/*overage_bytes*/ 1);
    assert_eq!(item, expected);
    item.omit_tool_result_metadata(/*overage_bytes*/ 99_999);
    assert_eq!(item, expected);
    item.clear_tool_result_metadata();
    assert!(!item.has_tool_result_metadata());
}

#[test]
fn message_budget_keeps_smaller_resource_evidence_when_larger_evidence_must_be_shed() {
    for (sizes, larger_index) in [([1024, 512], 0), ([512, 1024], 1)] {
        let mut items = sizes.map(|size| {
            let mut item = ResponseItem::from(ResponseInputItem::FunctionCallOutput {
                call_id: format!("call-{size}"),
                output: FunctionCallOutputPayload::from_text("unchanged output".to_string()),
            });
            let mut call =
                ExecutedToolCall::new(format!("tool_{size}"), serde_json::json!({"query": "kept"}));
            call.set_tool_result_sources(ToolResultSources::new(vec![ToolResultSource {
                r#type: "test_resource".to_string(),
                id: format!("resource-{size}"),
            }]));
            call.set_tool_result_metadata(ToolResultMetadata::new(&serde_json::json!({
                "openai/resource_access": {"opaque": "é".repeat(size)},
            })));
            item.append_executed_tool_calls(vec![call]);
            item.set_tool_call_cell_id(&format!("cell-{size}"));
            item.mark_tool_calls_complete();
            item
        });
        let mut expected = items.clone();
        for item in &mut expected {
            first_executed_tool_call(item).unwrap().tool_result_sources = None;
        }
        let budget = expected
            .iter()
            .map(executed_tool_call_metadata_bytes)
            .sum::<usize>()
            - 128;
        first_executed_tool_call(&mut expected[larger_index])
            .unwrap()
            .set_tool_result_metadata(ToolResultMetadata::omitted_due_to_size_limit(
                /*overage_bytes*/ 128,
            ));

        bound_executed_tool_calls_for_message(&mut items, budget);
        assert_eq!(items, expected);
        assert!(
            items
                .iter()
                .map(executed_tool_call_metadata_bytes)
                .sum::<usize>()
                <= budget
        );
        bound_executed_tool_calls_for_message(&mut items, budget);
        assert_eq!(items, expected);
    }
}

#[test]
fn message_budget_drops_only_the_sources_needed_to_fit() {
    let mut items = ["first", "second"].map(|id| {
        let mut item = ResponseItem::from(ResponseInputItem::FunctionCallOutput {
            call_id: id.to_string(),
            output: FunctionCallOutputPayload::from_text("unchanged output".to_string()),
        });
        let mut call =
            ExecutedToolCall::new(format!("tool_{id}"), serde_json::json!({"query": "kept"}));
        call.set_tool_result_sources(ToolResultSources::new(vec![ToolResultSource {
            r#type: "test_resource".to_string(),
            id: id.to_string(),
        }]));
        call.set_tool_result_metadata(ToolResultMetadata::new(&serde_json::json!({
            "openai/resource_access": {"resource_coverage": "complete", "resources": []},
        })));
        item.append_executed_tool_calls(vec![call]);
        item.set_tool_call_cell_id(id);
        item.mark_tool_calls_complete();
        item
    });
    let mut expected = items.clone();
    first_executed_tool_call(&mut expected[0])
        .unwrap()
        .tool_result_sources = None;
    let budget = expected
        .iter()
        .map(executed_tool_call_metadata_bytes)
        .sum::<usize>();
    assert!(
        items
            .iter()
            .map(executed_tool_call_metadata_bytes)
            .sum::<usize>()
            > budget
    );

    bound_executed_tool_calls_for_message(&mut items, budget);
    assert_eq!(items, expected);
    bound_executed_tool_calls_for_message(&mut items, budget);
    assert_eq!(items, expected);
}

#[test]
fn message_budget_removes_generic_fields_before_resources_in_the_same_output() {
    for generic in [
        serde_json::json!({}),
        serde_json::json!(null),
        serde_json::json!("omitted_due_to_size_limit"),
    ] {
        let mut item = output("exec");
        for metadata in [
            serde_json::json!({"openai/resource_access": {"opaque": "é".repeat(256)}}),
            generic.clone(),
            generic,
        ] {
            let mut call = ExecutedToolCall::new("read".to_string(), serde_json::json!({}));
            call.set_tool_result_metadata(ToolResultMetadata::new(&metadata));
            item.append_executed_tool_calls(vec![call]);
        }
        item.set_tool_call_cell_id("cell");
        item.mark_tool_calls_complete();
        let mut expected = item.clone();
        expected
            .ensure_tool_call_metadata()
            .unwrap()
            .executed_tool_calls
            .as_mut()
            .unwrap()[1]
            .set_tool_result_metadata(ToolResultMetadata::default());
        let budget = executed_tool_call_metadata_bytes(&expected);
        bound_executed_tool_calls_for_message(std::slice::from_mut(&mut item), budget);
        assert_eq!(item, expected);
    }
}

#[test]
fn message_budget_preserves_call_inventory_when_arguments_are_truncated() {
    let arguments = serde_json::json!({"payload": "é".repeat(256)});
    let argument_bytes = serde_json::to_vec(&arguments).unwrap().len();
    let mut exec = output("exec");
    exec.append_executed_tool_calls(vec![
        ExecutedToolCall::new("first_é\"".to_string(), arguments.clone()),
        ExecutedToolCall::new("second".to_string(), arguments),
    ]);
    first_executed_tool_call(&mut exec)
        .unwrap()
        .set_tool_result_metadata(ToolResultMetadata::new(&serde_json::json!({
            "openai/resource_access": {"resources": ["R1"]}
        })));
    let mut wait = output("wait");
    for item in [&mut exec, &mut wait] {
        item.set_tool_call_cell_id("cell_é\"");
        item.mark_tool_calls_complete();
        let metadata = item.ensure_tool_call_metadata().unwrap();
        metadata.turn_id = Some("turn_é\"".to_string());
        metadata.create_time = Some(serde_json::Number::from(123));
    }
    let original = vec![exec, wait];
    let original_metadata_bytes = original
        .iter()
        .map(executed_tool_call_metadata_bytes)
        .sum::<usize>();
    // The first two cases fit after truncating only the first call's arguments;
    // the second call's arguments and both completion claims must survive.
    for budget in [
        original_metadata_bytes - 64,
        original_metadata_bytes - (argument_bytes - 128),
        0,
    ] {
        let mut items = original.clone();
        let mut expected = original.clone();
        if budget == 0 {
            for item in &mut expected {
                item.clear_executed_tool_calls();
            }
        } else {
            first_executed_tool_call(&mut expected[0])
                .unwrap()
                .set_truncation(
                    argument_bytes,
                    argument_bytes - (original_metadata_bytes - budget),
                    /*omitted_calls*/ None,
                );
        }
        bound_executed_tool_calls_for_message(&mut items, budget);
        assert_eq!(items, expected);
        let bounded_metadata_bytes = items
            .iter()
            .map(executed_tool_call_metadata_bytes)
            .sum::<usize>();
        assert!(bounded_metadata_bytes <= budget);
        assert_eq!(
            serde_json::to_vec(&original).unwrap().len()
                - serde_json::to_vec(&items).unwrap().len(),
            original_metadata_bytes - bounded_metadata_bytes,
        );
    }
}

#[test]
fn inventory_loss_still_clears_completeness() {
    for (omitted_calls, original_name_bytes) in [(Some(1), None), (None, Some(20))] {
        let mut call = ExecutedToolCall::new("test_tool".to_string(), serde_json::json!({}));
        call.set_truncation_with_name(
            /*original_bytes*/ 9_000,
            /*max_bytes*/ 0,
            omitted_calls,
            original_name_bytes,
        );
        let mut item = output("call");
        item.append_executed_tool_calls(vec![call]);
        item.mark_tool_calls_complete();
        normalize_executed_tool_call_arguments(std::slice::from_mut(&mut item));
        assert_eq!(
            item.executed_tool_call_metadata()
                .unwrap()
                .tool_calls_complete,
            None
        );
    }
}

#[test]
fn result_metadata_shedding_keeps_smaller_results_within_one_output() {
    let budget = 2 * 1024 * 1024;
    let resource_only = serde_json::json!({
        "openai/resource_access": { "resource_coverage": "complete", "resources": [] },
    });
    for (metadata, reduced_index, replacement) in [
        (
            [
                serde_json::json!({}),
                serde_json::json!({ "value": "é".repeat(10_000) }),
                serde_json::json!({ "value": "é".repeat(10_000) }),
            ],
            1,
            None,
        ),
        (
            [
                serde_json::json!({}),
                serde_json::json!({ "value": "x".repeat(8_000) }),
                serde_json::json!({ "value": "\0".repeat(4_500) }),
            ],
            2,
            None,
        ),
        (
            [
                serde_json::json!({}),
                serde_json::json!({ "value": "x".repeat(8_000) }),
                serde_json::json!({
                    "value": "\0".repeat(4_500),
                    "openai/resource_access": resource_only["openai/resource_access"],
                }),
            ],
            2,
            Some(ToolResultMetadata::new(&resource_only)),
        ),
    ] {
        let mut metadata = metadata.to_vec();
        metadata.extend(vec![
            serde_json::json!({ "padding": "x".repeat(17_000) });
            6
        ]);
        let name_padding = (budget - 128 * 1024) / metadata.len();
        let mut item = output("exec");
        item.append_executed_tool_calls(
            metadata
                .iter()
                .enumerate()
                .map(|(index, metadata)| {
                    let mut call = ExecutedToolCall::new(
                        format!("tool_{index}{}", "x".repeat(name_padding)),
                        serde_json::json!({ "argument": index }),
                    );
                    call.set_tool_result_sources(ToolResultSources::new(vec![ToolResultSource {
                        r#type: "test_resource".to_string(),
                        id: format!("R{index}"),
                    }]));
                    call.set_tool_result_metadata(ToolResultMetadata::new(metadata));
                    call
                })
                .collect(),
        );
        item.set_tool_call_cell_id("exec");
        item.mark_tool_calls_complete();
        assert!(executed_tool_call_metadata_bytes(&item) > budget);
        let replacement = replacement.unwrap_or_else(|| {
            ToolResultMetadata::omitted_due_to_size_limit(
                executed_tool_call_metadata_bytes(&item) - budget,
            )
        });
        let mut expected = item.clone();
        expected
            .ensure_tool_call_metadata()
            .unwrap()
            .executed_tool_calls
            .as_mut()
            .unwrap()[reduced_index]
            .set_tool_result_metadata(replacement);
        let mut bounded = item.clone();
        bound_executed_tool_calls_for_message(std::slice::from_mut(&mut bounded), budget);
        assert_eq!(bounded, expected);
        assert!(executed_tool_call_metadata_bytes(&bounded) <= budget);
        bound_executed_tool_calls_for_message(std::slice::from_mut(&mut bounded), budget);
        assert_eq!(bounded, expected);
    }
}

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
fn executed_tool_call_prompt_budget_includes_metadata_fields() -> Result<()> {
    let metadata_bytes = |items: &[ResponseItem]| -> Result<usize> {
        items.iter().try_fold(0_usize, |bytes, item| {
            let with_metadata = serde_json::to_vec(item)?.len();
            let mut without_metadata = item.clone();
            without_metadata.clear_executed_tool_calls();
            let without_metadata = serde_json::to_vec(&without_metadata)?.len();
            Ok(bytes + with_metadata.saturating_sub(without_metadata))
        })
    };

    for with_turn_id in [true, false] {
        let mut items = (0..4_000)
            .map(|index| {
                let mut item = output(&index.to_string());
                *item
                    .internal_chat_message_metadata_passthrough_mut()
                    .unwrap() = with_turn_id.then(|| passthrough_metadata("turn-1"));
                item.append_executed_tool_calls(vec![ExecutedToolCall::new(
                    String::new(),
                    serde_json::Value::Null,
                )]);
                item.set_tool_call_cell_id(&format!("cell-\"\\{}", "x".repeat(512)));
                item.mark_tool_calls_complete();
                item
            })
            .collect::<Vec<_>>();

        assert!(metadata_bytes(&items)? > MAX_EXECUTED_TOOL_CALL_METADATA_BYTES);
        bound_executed_tool_calls_for_prompt(&mut items);
        assert!(metadata_bytes(&items)? <= MAX_EXECUTED_TOOL_CALL_METADATA_BYTES);
        for metadata in items
            .iter()
            .filter_map(ResponseItem::executed_tool_call_metadata)
        {
            assert_eq!(metadata.tool_calls_complete, None);
        }

        let calls = items
            .iter()
            .filter_map(ResponseItem::executed_tool_call_metadata)
            .filter_map(|metadata| metadata.executed_tool_calls.as_ref())
            .flatten()
            .collect::<Vec<_>>();
        // Shares smaller than a local marker omit that output; surviving calls stay unchanged.
        assert!(!calls.is_empty() && calls.len() < 4_000);
        assert!(calls.iter().all(|call| {
            **call == ExecutedToolCall::new(String::new(), serde_json::Value::Null)
        }));

        let bounded_items = items.clone();
        bound_executed_tool_calls_for_prompt(&mut items);
        assert_eq!(items, bounded_items);

        let oversized_names =
            ["\0", "é"].map(|name| name.repeat(MAX_EXECUTED_TOOL_CALL_METADATA_BYTES));
        let mut oversized_items = oversized_names
            .iter()
            .enumerate()
            .map(|(index, name)| {
                let mut item = output(&index.to_string());
                *item
                    .internal_chat_message_metadata_passthrough_mut()
                    .unwrap() = with_turn_id.then(|| passthrough_metadata("turn-1"));
                item.append_executed_tool_calls(vec![ExecutedToolCall::new(
                    name.clone(),
                    serde_json::Value::Null,
                )]);
                item.set_tool_call_cell_id("cell-\"\\");
                item
            })
            .collect::<Vec<_>>();

        bound_executed_tool_calls_for_prompt(&mut oversized_items);
        assert!(metadata_bytes(&oversized_items)? <= MAX_EXECUTED_TOOL_CALL_METADATA_BYTES);
        let calls = oversized_items
            .iter()
            .filter_map(ResponseItem::executed_tool_call_metadata)
            .filter_map(|metadata| metadata.executed_tool_calls.as_ref())
            .flatten()
            .collect::<Vec<_>>();
        assert_eq!(calls.len(), 2);
        for ((item, call), oversized_name) in
            oversized_items.iter().zip(calls).zip(&oversized_names)
        {
            assert_eq!(
                item.executed_tool_call_metadata()
                    .unwrap()
                    .cell_id
                    .as_deref(),
                Some("cell-\"\\"),
            );
            assert!(call.name.len() < oversized_name.len());
            assert!(oversized_name.starts_with(&call.name));
            let truncation = call.truncation().expect("trusted omission marker");
            assert_eq!(truncation.omitted_calls, None);
            assert_eq!(truncation.original_name_bytes, Some(oversized_name.len()));
            assert_eq!(
                serde_json::to_value(&call.arguments)?["_codex_executed_tool_call_truncated"]["original_name_bytes"],
                serde_json::json!(oversized_name.len()),
            );
        }

        let bounded_items = oversized_items.clone();
        bound_executed_tool_calls_for_prompt(&mut oversized_items);
        assert_eq!(oversized_items, bounded_items);
    }

    let arguments = serde_json::json!({ "payload": "x".repeat(7 * 1024) });
    let argument_bytes = serde_json::to_vec(&arguments)?.len();
    assert!(argument_bytes < MAX_EXECUTED_TOOL_CALL_ARGUMENT_BYTES);
    let call_count = MAX_EXECUTED_TOOL_CALL_METADATA_BYTES / argument_bytes + 1;
    assert!(call_count * argument_bytes > MAX_EXECUTED_TOOL_CALL_METADATA_BYTES);
    let mut items = ["A", "wait-A", "B"].map(|call_id| {
        let mut item = output(call_id);
        item.mark_tool_calls_complete();
        item
    });
    for item in &mut items[..2] {
        item.set_tool_call_cell_id("cell-A");
    }
    let calls = vec![ExecutedToolCall::new("test_tool".to_string(), arguments); call_count];
    items[0].append_executed_tool_calls(calls.clone());
    items[2].append_executed_tool_calls(vec![ExecutedToolCall::new(
        "test_tool".to_string(),
        serde_json::json!({}),
    )]);
    assert!(metadata_bytes(&items)? > MAX_EXECUTED_TOOL_CALL_METADATA_BYTES);
    for bound in [
        bound_executed_tool_calls_for_prompt,
        bound_executed_tool_calls_for_prompt_prioritizing_recent,
    ] {
        let mut bounded = items.clone();
        bound(&mut bounded);
        assert!(metadata_bytes(&bounded)? <= MAX_EXECUTED_TOOL_CALL_METADATA_BYTES);
        assert_eq!(bounded[2], items[2]);
        let metadata = bounded[0].executed_tool_call_metadata().unwrap();
        assert_eq!(metadata.tool_calls_complete, None);
        let recorded = metadata.executed_tool_calls.as_ref().unwrap();
        assert_eq!(recorded.len(), 1);
        assert_eq!(recorded[0].name, "test_tool");
        let truncation = recorded[0].truncation().unwrap();
        assert_eq!(truncation.original_bytes, argument_bytes);
        assert_eq!(truncation.omitted_calls, Some(call_count - 1));
        assert_eq!(
            bounded[1]
                .executed_tool_call_metadata()
                .unwrap()
                .tool_calls_complete,
            None,
        );
        bounded[0].append_executed_tool_calls(calls.clone());
        assert!(metadata_bytes(&bounded)? > MAX_EXECUTED_TOOL_CALL_METADATA_BYTES);
        bound(&mut bounded);
        assert!(metadata_bytes(&bounded)? <= MAX_EXECUTED_TOOL_CALL_METADATA_BYTES);
        assert_eq!(bounded[2], items[2]);
        assert_eq!(
            first_executed_tool_call(&mut bounded[0])
                .unwrap()
                .truncation()
                .unwrap()
                .omitted_calls,
            Some(2 * call_count - 1),
        );
    }

    let mut items = ["exec", "wait"].map(|call_id| {
        let mut item = output(call_id);
        item.set_tool_call_cell_id("cell-\"\\");
        item
    });
    items[0].append_executed_tool_calls(vec![
        ExecutedToolCall::new(
            "test_tool".to_string(),
            serde_json::json!({ "payload": "x".repeat(MAX_EXECUTED_TOOL_CALL_ARGUMENT_BYTES - 256) }),
        );
        16
    ]);
    items[1].mark_tool_calls_complete();
    // Leave room for calls, but not their sources, without exceeding the argument limit.
    let padding_bytes = MAX_EXECUTED_TOOL_CALL_METADATA_BYTES - metadata_bytes(&items)? - 1024;
    let calls = items[0]
        .ensure_tool_call_metadata()
        .unwrap()
        .executed_tool_calls
        .as_mut()
        .unwrap();
    let call_count = calls.len();
    for (index, call) in calls.iter_mut().enumerate() {
        call.name.push_str(
            &"x".repeat(
                padding_bytes / call_count + usize::from(index < padding_bytes % call_count),
            ),
        );
    }
    let expected = items.clone();
    assert!(
        first_executed_tool_call(&mut items[0])
            .expect("recorded call should exist")
            .set_tool_result_sources(ToolResultSources::new(
                (0..MAX_TOOL_RESULT_SOURCES)
                    .map(|index| ToolResultSource {
                        r#type: "document".to_string(),
                        id: format!(
                            "R{index:0width$}",
                            width = MAX_TOOL_RESULT_SOURCE_FIELD_BYTES - 1
                        ),
                    })
                    .collect(),
            ))
    );
    assert!(metadata_bytes(&items)? > MAX_EXECUTED_TOOL_CALL_METADATA_BYTES);
    bound_executed_tool_calls_for_prompt(&mut items);
    assert_eq!(items, expected);

    // Empty waits can carry only the marker; its bytes still count toward the budget.
    for metadata in [None, Some(passthrough_metadata("turn-1"))] {
        let mut item = output("wait");
        *item
            .internal_chat_message_metadata_passthrough_mut()
            .unwrap() = metadata;
        let without_marker = item.clone();
        item.set_tool_call_cell_id(&format!("cell-\"\\{}", "x".repeat(256)));
        item.mark_tool_calls_complete();
        assert_eq!(
            metadata_bytes(std::slice::from_ref(&item))?,
            executed_tool_call_metadata_bytes(&item),
        );
        let mut items = vec![item; 8_000];
        assert!(metadata_bytes(&items)? > MAX_EXECUTED_TOOL_CALL_METADATA_BYTES);
        bound_executed_tool_calls_for_prompt_prioritizing_recent(&mut items);
        assert!(metadata_bytes(&items)? <= MAX_EXECUTED_TOOL_CALL_METADATA_BYTES);
        let retained = items
            .iter()
            .filter(|item| executed_tool_call_metadata_bytes(item) > 0)
            .count();
        assert!(retained > 0 && retained < 8_000);
        for item in &mut items {
            assert!(
                item.executed_tool_call_metadata()
                    .is_none_or(|metadata| metadata.tool_calls_complete.is_none())
            );
            item.clear_executed_tool_calls();
        }
        assert_eq!(items, vec![without_marker; 8_000]);
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

    bound_executed_tool_calls_for_prompt(std::slice::from_mut(&mut item));
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
    bound_executed_tool_calls_for_prompt(std::slice::from_mut(&mut item));
    assert_eq!(
        serde_json::to_value(&item)?["internal_chat_message_metadata_passthrough"],
        serde_json::json!({
            "cell_id": "cell-1", "executed_tool_calls": [], "tool_calls_complete": true
        }),
    );
    item.append_executed_tool_calls(vec![call]);
    item.clear_executed_tool_calls();
    assert!(item.executed_tool_call_metadata().is_none());

    for call in [
        ExecutedToolCall::new(
            "test_tool".to_string(),
            serde_json::json!({ "payload": "x".repeat(MAX_EXECUTED_TOOL_CALL_ARGUMENT_BYTES + 1) }),
        ),
        ExecutedToolCall::truncated(
            "test_tool".to_string(),
            /*original_bytes*/ 9_000,
            /*max_bytes*/ 0,
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
            bound_executed_tool_calls_for_prompt(&mut items);
            assert_eq!(
                items.map(|item| item
                    .executed_tool_call_metadata()
                    .unwrap()
                    .tool_calls_complete),
                [None, (!same_cell).then_some(true)],
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
fn message_budget_preserves_names_and_turn_metadata_and_invalidates_shared_cell_completion() {
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
    // The middle case needs the sibling wait's completeness bytes to fit after
    // truncating only the first call; the second call's arguments must survive.
    for budget in [
        original_metadata_bytes - 64,
        original_metadata_bytes - (argument_bytes - 32),
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
            for item in &mut expected {
                item.clear_tool_calls_complete();
            }
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
fn raw_result_metadata_is_shed_before_sources_calls_or_completion() -> Result<()> {
    // Keep each result below its capture limit while filling the larger request budget.
    let name = format!(
        "test_tool{}",
        "x".repeat(MAX_EXECUTED_TOOL_CALL_METADATA_BYTES / 8 - 16 * 1024)
    );
    let mut call = ExecutedToolCall::new(name, serde_json::json!({}));
    call.set_tool_result_sources(ToolResultSources::new(vec![ToolResultSource {
        r#type: "test_resource".to_string(),
        id: "R1".to_string(),
    }]));
    let mut items = [
        "first",
        "second",
        "third",
        "fourth",
        "fifth",
        "sixth",
        "seventh",
        "without-metadata",
    ]
    .map(|call_id| {
        let mut item = ResponseItem::from(ResponseInputItem::FunctionCallOutput {
            call_id: call_id.to_string(),
            output: FunctionCallOutputPayload::from_text("unchanged output".to_string()),
        });
        item.append_executed_tool_calls(vec![call.clone()]);
        item.set_tool_call_cell_id(call_id);
        item.mark_tool_calls_complete();
        item
    });
    let metadata = ToolResultMetadata::new(&serde_json::json!({ "large": "x".repeat(20_000) }));
    for item in &mut items[..7] {
        first_executed_tool_call(item)
            .unwrap()
            .set_tool_result_metadata(metadata.clone());
    }
    let original_bytes = items
        .iter()
        .map(executed_tool_call_metadata_bytes)
        .sum::<usize>();
    let mut expected = items.clone();
    first_executed_tool_call(&mut expected[0])
        .unwrap()
        .set_tool_result_metadata(ToolResultMetadata::omitted_due_to_size_limit(
            original_bytes - MAX_EXECUTED_TOOL_CALL_METADATA_BYTES,
        ));
    assert!(original_bytes > MAX_EXECUTED_TOOL_CALL_METADATA_BYTES);
    for bound in [
        bound_executed_tool_calls_for_prompt,
        bound_executed_tool_calls_for_prompt_prioritizing_recent,
    ] {
        let mut bounded = items.clone();
        bound(&mut bounded);
        assert_eq!(bounded, expected);
        assert!(
            bounded
                .iter()
                .map(executed_tool_call_metadata_bytes)
                .sum::<usize>()
                <= MAX_EXECUTED_TOOL_CALL_METADATA_BYTES
        );
        bound(&mut bounded);
        assert_eq!(bounded, expected);
    }
    Ok(())
}

#[test]
fn result_metadata_shedding_keeps_smaller_results_within_one_output() {
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
        let name_padding = (MAX_EXECUTED_TOOL_CALL_METADATA_BYTES - 128 * 1024) / metadata.len();
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
        assert!(executed_tool_call_metadata_bytes(&item) > MAX_EXECUTED_TOOL_CALL_METADATA_BYTES);
        let replacement = replacement.unwrap_or_else(|| {
            ToolResultMetadata::omitted_due_to_size_limit(
                executed_tool_call_metadata_bytes(&item) - MAX_EXECUTED_TOOL_CALL_METADATA_BYTES,
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
        for bound in [
            bound_executed_tool_calls_for_prompt,
            bound_executed_tool_calls_for_prompt_prioritizing_recent,
            |items: &mut [ResponseItem]| {
                bound_executed_tool_calls_for_message(items, MAX_EXECUTED_TOOL_CALL_METADATA_BYTES)
            },
        ] {
            let mut bounded = item.clone();
            bound(std::slice::from_mut(&mut bounded));
            assert_eq!(bounded, expected);
            assert!(
                executed_tool_call_metadata_bytes(&bounded)
                    <= MAX_EXECUTED_TOOL_CALL_METADATA_BYTES
            );
            bound(std::slice::from_mut(&mut bounded));
            assert_eq!(bounded, expected);
        }
    }
}

#[test]
fn result_metadata_shedding_removes_only_markers_needed_to_fit() {
    let mut item = output("exec");
    let mut call = ExecutedToolCall::new("test_tool".to_string(), serde_json::json!({}));
    call.set_tool_result_sources(ToolResultSources::new(vec![ToolResultSource {
        r#type: "test_resource".to_string(),
        id: "R1".to_string(),
    }]));
    let mut calls = vec![call; 20];
    let omitted = ToolResultMetadata::omitted_due_to_size_limit(/*overage_bytes*/ 1);
    let marker_bytes = serde_json::to_vec(&omitted).unwrap().len();
    // A real object tied with a marker must survive even though it is older.
    let equal_sized = ToolResultMetadata::new(&serde_json::json!({
        "value": "x".repeat(marker_bytes - r#"{"value":""}"#.len()),
    }));
    assert_eq!(
        serde_json::to_vec(&equal_sized).unwrap().len(),
        marker_bytes
    );
    calls[0].set_tool_result_metadata(equal_sized);
    for index in [1, 3] {
        calls[index].set_tool_result_metadata(omitted.clone());
    }
    calls[2].set_tool_result_metadata(ToolResultMetadata::new(&serde_json::json!({})));
    item.append_executed_tool_calls(calls);
    item.set_tool_call_cell_id("exec");
    item.mark_tool_calls_complete();
    let remaining_bytes =
        MAX_EXECUTED_TOOL_CALL_METADATA_BYTES + 1 - executed_tool_call_metadata_bytes(&item);
    let calls = item
        .ensure_tool_call_metadata()
        .unwrap()
        .executed_tool_calls
        .as_mut()
        .unwrap();
    let call_count = calls.len();
    for (index, call) in calls.iter_mut().enumerate() {
        // Pad names so the per-argument limit cannot preempt result-metadata shedding.
        call.name.push_str(&"x".repeat(
            remaining_bytes / call_count + usize::from(index < remaining_bytes % call_count),
        ));
    }
    assert_eq!(
        executed_tool_call_metadata_bytes(&item),
        MAX_EXECUTED_TOOL_CALL_METADATA_BYTES + 1
    );
    let mut expected = item.clone();
    expected
        .ensure_tool_call_metadata()
        .unwrap()
        .executed_tool_calls
        .as_mut()
        .unwrap()[1]
        .set_tool_result_metadata(ToolResultMetadata::default());
    let tied_markers = item.clone();
    // The new marker has a larger overage value and therefore more bytes. Remove
    // that marker first, while tied smaller markers still precede provider data.
    item.ensure_tool_call_metadata()
        .unwrap()
        .executed_tool_calls
        .as_mut()
        .unwrap()[3]
        .set_tool_result_metadata(ToolResultMetadata::new(&serde_json::json!({
            "large": "x".repeat(4 * 1024),
        })));
    let mut larger_marker_expected = item.clone();
    larger_marker_expected
        .ensure_tool_call_metadata()
        .unwrap()
        .executed_tool_calls
        .as_mut()
        .unwrap()[3]
        .set_tool_result_metadata(ToolResultMetadata::default());
    for (item, expected) in [(tied_markers, expected), (item, larger_marker_expected)] {
        for bound in [
            bound_executed_tool_calls_for_prompt,
            bound_executed_tool_calls_for_prompt_prioritizing_recent,
        ] {
            let mut bounded = item.clone();
            bound(std::slice::from_mut(&mut bounded));
            assert_eq!(bounded, expected);
            assert!(
                executed_tool_call_metadata_bytes(&bounded)
                    <= MAX_EXECUTED_TOOL_CALL_METADATA_BYTES
            );
            bound(std::slice::from_mut(&mut bounded));
            assert_eq!(bounded, expected);
        }
    }
}

#[test]
fn result_metadata_markers_do_not_displace_existing_evidence() {
    let mut item = ResponseItem::from(ResponseInputItem::FunctionCallOutput {
        call_id: "exec".to_string(),
        output: FunctionCallOutputPayload::from_text("unchanged output".to_string()),
    });
    let mut call = ExecutedToolCall::new("test_tool".to_string(), serde_json::json!({}));
    call.set_tool_result_sources(ToolResultSources::new(vec![ToolResultSource {
        r#type: "test_resource".to_string(),
        id: "R1".to_string(),
    }]));
    item.append_executed_tool_calls(vec![call; 20]);
    item.set_tool_call_cell_id("exec");
    item.mark_tool_calls_complete();
    let remaining_bytes =
        MAX_EXECUTED_TOOL_CALL_METADATA_BYTES - executed_tool_call_metadata_bytes(&item);
    let calls = item
        .ensure_tool_call_metadata()
        .unwrap()
        .executed_tool_calls
        .as_mut()
        .unwrap();
    let call_count = calls.len();
    for (index, call) in calls.iter_mut().enumerate() {
        // Fill the budget exactly without increasing arguments past their individual limit.
        call.name.push_str(&"x".repeat(
            remaining_bytes / call_count + usize::from(index < remaining_bytes % call_count),
        ));
    }
    assert_eq!(
        executed_tool_call_metadata_bytes(&item),
        MAX_EXECUTED_TOOL_CALL_METADATA_BYTES
    );
    let expected = item.clone();
    for metadata in [
        serde_json::json!({ "too-large": "x".repeat(32 * 1024) }),
        serde_json::json!({
            "openai/resource_access": { "resource_coverage": "complete", "resources": [] },
        }),
    ] {
        let mut item = expected.clone();
        first_executed_tool_call(&mut item)
            .unwrap()
            .set_tool_result_metadata(ToolResultMetadata::new(&metadata));
        bound_executed_tool_calls_for_prompt(std::slice::from_mut(&mut item));
        assert_eq!(item, expected);
    }
}

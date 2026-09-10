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
        let mut items = (0..1_000)
            .map(|index| {
                let mut item = output(&index.to_string());
                *item
                    .internal_chat_message_metadata_passthrough_mut()
                    .unwrap() = with_turn_id.then(|| passthrough_metadata("turn-1"));
                item.append_executed_tool_calls(vec![ExecutedToolCall::new(
                    String::new(),
                    serde_json::Value::Null,
                )]);
                item.set_tool_call_cell_id("cell-\"\\");
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
        assert!(!calls.is_empty() && calls.len() < 1_000);
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
    assert!(6 * argument_bytes > MAX_EXECUTED_TOOL_CALL_METADATA_BYTES);
    let mut items = ["A", "wait-A", "B"].map(|call_id| {
        let mut item = output(call_id);
        item.mark_tool_calls_complete();
        item
    });
    for item in &mut items[..2] {
        item.set_tool_call_cell_id("cell-A");
    }
    let calls = vec![ExecutedToolCall::new("test_tool".to_string(), arguments); 6];
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
        assert_eq!(truncation.omitted_calls, Some(5));
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
            Some(11),
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
        4
    ]);
    items[1].mark_tool_calls_complete();
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
        item.set_tool_call_cell_id("cell-\"\\");
        item.mark_tool_calls_complete();
        assert_eq!(
            metadata_bytes(std::slice::from_ref(&item))?,
            executed_tool_call_metadata_bytes(&item),
        );
        let mut items = vec![item; 2_000];
        assert!(metadata_bytes(&items)? > MAX_EXECUTED_TOOL_CALL_METADATA_BYTES);
        bound_executed_tool_calls_for_prompt_prioritizing_recent(&mut items);
        assert!(metadata_bytes(&items)? <= MAX_EXECUTED_TOOL_CALL_METADATA_BYTES);
        let retained = items
            .iter()
            .filter(|item| executed_tool_call_metadata_bytes(item) > 0)
            .count();
        assert!(retained > 0 && retained < 2_000);
        for item in &mut items {
            assert!(
                item.executed_tool_call_metadata()
                    .is_none_or(|metadata| metadata.tool_calls_complete.is_none())
            );
            item.clear_executed_tool_calls();
        }
        assert_eq!(items, vec![without_marker; 2_000]);
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
        serde_json::json!({ "cell_id": "cell-1", "tool_calls_complete": true }),
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
fn tool_result_metadata_is_host_only_bounded_and_redacted() -> Result<()> {
    let mut call = ExecutedToolCall::new("test_tool".to_string(), serde_json::json!({}));
    let without_metadata = call.clone();
    for metadata in [
        serde_json::json!({
            "arbitrary-key": { "secret": "not-for-debug", "ids": [1, "é", null] },
            "other": false,
        }),
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

    let exact_limit = serde_json::json!({
        "value": "x".repeat(MAX_EXECUTED_TOOL_CALL_METADATA_BYTES - r#"{"value":""}"#.len()),
    });
    let capture = ToolResultMetadata::new(&exact_limit);
    assert!(capture.is_some());
    call.set_tool_result_metadata(capture);
    assert_eq!(
        serde_json::to_value(&call)?["tool_result_metadata"],
        exact_limit
    );
    // JSON escaping counts toward the bound, not just the in-memory string length.
    let oversized =
        serde_json::json!({ "value": "\0".repeat(MAX_EXECUTED_TOOL_CALL_METADATA_BYTES / 2) });
    let capture = ToolResultMetadata::new(&oversized);
    assert!(capture.is_some());
    call.set_tool_result_metadata(capture);
    let wire = serde_json::to_value(&call)?;
    assert_eq!(
        wire,
        serde_json::json!({
            "name": "test_tool",
            "arguments": {},
            "tool_result_metadata": "omitted_due_to_size_limit",
        })
    );
    assert_eq!(
        serde_json::from_value::<ExecutedToolCall>(wire)?,
        without_metadata
    );
    Ok(())
}

#[test]
fn raw_result_metadata_is_shed_before_sources_calls_or_completion() -> Result<()> {
    let mut call = ExecutedToolCall::new("test_tool".to_string(), serde_json::json!({}));
    call.set_tool_result_sources(ToolResultSources::new(vec![ToolResultSource {
        r#type: "test_resource".to_string(),
        id: "R1".to_string(),
    }]));
    let mut items = ["first", "second", "without-metadata"].map(|call_id| {
        let mut item = ResponseItem::from(ResponseInputItem::FunctionCallOutput {
            call_id: call_id.to_string(),
            output: FunctionCallOutputPayload::from_text("unchanged output".to_string()),
        });
        item.append_executed_tool_calls(vec![call.clone()]);
        item.set_tool_call_cell_id(call_id);
        item.mark_tool_calls_complete();
        item
    });
    let mut expected = items.clone();
    let metadata = ToolResultMetadata::new(&serde_json::json!({ "large": "x".repeat(20_000) }));
    for item in &mut items[..2] {
        first_executed_tool_call(item)
            .unwrap()
            .set_tool_result_metadata(metadata.clone());
    }
    for item in &mut expected[..2] {
        first_executed_tool_call(item)
            .unwrap()
            .set_tool_result_metadata(ToolResultMetadata::new(&serde_json::json!(
                "omitted_due_to_size_limit"
            )));
    }
    assert!(
        items
            .iter()
            .map(executed_tool_call_metadata_bytes)
            .sum::<usize>()
            > MAX_EXECUTED_TOOL_CALL_METADATA_BYTES
    );
    bound_executed_tool_calls_for_prompt(&mut items);
    assert_eq!(items, expected);
    assert!(
        items
            .iter()
            .map(executed_tool_call_metadata_bytes)
            .sum::<usize>()
            <= MAX_EXECUTED_TOOL_CALL_METADATA_BYTES
    );
    bound_executed_tool_calls_for_prompt(&mut items);
    assert_eq!(items, expected);
    Ok(())
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
    item.append_executed_tool_calls(vec![call; 4]);
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
    for (index, call) in calls.iter_mut().enumerate() {
        // Fill the budget exactly, with every argument still below its individual limit.
        call.arguments = ExecutedToolCallArguments::Raw(serde_json::json!(
            "x".repeat(remaining_bytes / 4 + usize::from(index < remaining_bytes % 4))
        ));
    }
    assert_eq!(
        executed_tool_call_metadata_bytes(&item),
        MAX_EXECUTED_TOOL_CALL_METADATA_BYTES
    );
    let expected = item.clone();
    first_executed_tool_call(&mut item)
        .unwrap()
        .set_tool_result_metadata(ToolResultMetadata::new(
            &serde_json::json!({ "too-large": "x".repeat(32 * 1024) }),
        ));
    bound_executed_tool_calls_for_prompt(std::slice::from_mut(&mut item));
    assert_eq!(item, expected);
}

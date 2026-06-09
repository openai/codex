use crate::synthetic_conversation;
use crate::synthetic_conversation::SYNTHETIC_CONVERSATION_SOURCE;
use anyhow::Context;
use anyhow::Result;
use anyhow::bail;
use duckdb::Connection;
use duckdb::params_from_iter;
use duckdb::types::Value as DuckValue;
use serde_json::Value as JsonValue;
use std::collections::HashMap;

pub(super) fn materialize_context_windows(connection: &Connection) -> Result<()> {
    let calls = load_context_window_calls(connection)?;
    let mut windows = reduce_context_windows(calls)?;
    assign_window_metadata(&mut windows);
    insert_context_windows(connection, &windows)?;
    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ContextWindowKey {
    session_id: String,
    thread_id: String,
    context_window_id: String,
}

#[derive(Clone, Debug)]
struct ResponsesCallRow {
    session_id: String,
    thread_id: String,
    turn_id: Option<String>,
    responses_call_id: String,
    context_window_id: String,
    status: String,
    request_started_at_epoch_millis: i64,
    response_id: Option<String>,
    request_json: String,
    response_json: Option<String>,
}

impl ResponsesCallRow {
    fn context_window_key(&self) -> ContextWindowKey {
        ContextWindowKey {
            session_id: self.session_id.clone(),
            thread_id: self.thread_id.clone(),
            context_window_id: self.context_window_id.clone(),
        }
    }
}

#[derive(Clone)]
struct CompletedCall {
    full_input: Vec<JsonValue>,
    output_items: Vec<JsonValue>,
}

struct ReconstructedPrompt {
    request_json: JsonValue,
    full_input: Vec<JsonValue>,
}

struct ContextWindowDraft {
    session_id: String,
    thread_id: String,
    context_window_id: String,
    context_window_ordinal: i64,
    first_turn_id: Option<String>,
    last_turn_id: Option<String>,
    first_responses_call_id: String,
    last_responses_call_id: String,
    opened_at_epoch_millis: i64,
    closed_at_epoch_millis: Option<i64>,
    close_reason: Option<String>,
    message_count: i64,
    conversation_json: String,
}

fn load_context_window_calls(connection: &Connection) -> Result<Vec<ResponsesCallRow>> {
    let mut statement = connection.prepare(
        r#"
        SELECT
            session_id,
            thread_id,
            turn_id,
            responses_call_id,
            context_window_id,
            status,
            request_started_at_epoch_millis,
            response_id,
            request_json,
            response_json
        FROM viewer_responses_calls_v1
        WHERE session_id IS NOT NULL
          AND thread_id IS NOT NULL
          AND context_window_id IS NOT NULL
        ORDER BY
            session_id,
            thread_id,
            context_window_id,
            request_started_at_epoch_millis,
            responses_call_id
        "#,
    )?;
    let calls = statement.query_map([], |row| {
        Ok(ResponsesCallRow {
            session_id: row.get(0)?,
            thread_id: row.get(1)?,
            turn_id: row.get(2)?,
            responses_call_id: row.get(3)?,
            context_window_id: row.get(4)?,
            status: row.get(5)?,
            request_started_at_epoch_millis: row.get(6)?,
            response_id: row.get(7)?,
            request_json: row.get(8)?,
            response_json: row.get(9)?,
        })
    })?;
    Ok(calls.collect::<duckdb::Result<Vec<_>>>()?)
}

fn reduce_context_windows(calls: Vec<ResponsesCallRow>) -> Result<Vec<ContextWindowDraft>> {
    let mut windows = Vec::new();
    let mut current_key: Option<ContextWindowKey> = None;
    let mut current_calls = Vec::new();

    for call in calls {
        let call_key = call.context_window_key();
        if current_key.as_ref().is_some_and(|key| key != &call_key) {
            windows.push(reduce_context_window_calls(std::mem::take(
                &mut current_calls,
            ))?);
        }
        current_key = Some(call_key);
        current_calls.push(call);
    }
    if !current_calls.is_empty() {
        windows.push(reduce_context_window_calls(current_calls)?);
    }
    Ok(windows)
}

fn reduce_context_window_calls(calls: Vec<ResponsesCallRow>) -> Result<ContextWindowDraft> {
    let first = calls
        .first()
        .context("context window reducer received no Responses calls")?;
    let last = calls
        .last()
        .context("context window reducer received no Responses calls")?;
    let key = first.context_window_key();
    let mut completed_calls = HashMap::<String, CompletedCall>::new();
    let mut last_prompt = None;

    for call in &calls {
        let request_json: JsonValue =
            serde_json::from_str(&call.request_json).with_context(|| {
                format!(
                    "parse request_json for Responses call {}",
                    call.responses_call_id
                )
            })?;
        let delta_input = request_input_items(&request_json, call)?;
        let full_input = if let Some(previous_response_id) = request_json
            .get("previous_response_id")
            .and_then(JsonValue::as_str)
        {
            let previous = completed_calls
                .get(previous_response_id)
                .with_context(|| {
                    format!(
                        "unknown incremental Responses predecessor {previous_response_id} for call {} in context window {}",
                        call.responses_call_id, call.context_window_id
                    )
                })?;
            let mut full_input = previous.full_input.clone();
            full_input.extend(previous.output_items.clone());
            full_input.extend(delta_input);
            full_input
        } else {
            delta_input
        };
        let output_items = response_output_items(call)?;
        if call.status == "completed"
            && let Some(response_id) = call.response_id.as_ref()
            && completed_calls
                .insert(
                    response_id.clone(),
                    CompletedCall {
                        full_input: full_input.clone(),
                        output_items,
                    },
                )
                .is_some()
        {
            bail!(
                "duplicate completed Responses response_id {response_id} in context window {}",
                call.context_window_id
            );
        }
        last_prompt = Some(ReconstructedPrompt {
            request_json,
            full_input,
        });
    }

    let last_prompt =
        last_prompt.context("context window reducer did not reconstruct a Responses prompt")?;
    let conversation = synthetic_conversation::synthetic_conversation(
        &key.session_id,
        &key.thread_id,
        &key.context_window_id,
        &last_prompt.request_json,
        &last_prompt.full_input,
    )?;
    let message_count = conversation
        .get("messages")
        .and_then(JsonValue::as_array)
        .map_or(0, std::vec::Vec::len);

    Ok(ContextWindowDraft {
        session_id: key.session_id,
        thread_id: key.thread_id,
        context_window_id: key.context_window_id,
        context_window_ordinal: 0,
        first_turn_id: first.turn_id.clone(),
        last_turn_id: last.turn_id.clone(),
        first_responses_call_id: first.responses_call_id.clone(),
        last_responses_call_id: last.responses_call_id.clone(),
        opened_at_epoch_millis: first.request_started_at_epoch_millis,
        closed_at_epoch_millis: None,
        close_reason: None,
        message_count: i64::try_from(message_count)?,
        conversation_json: serde_json::to_string(&conversation)?,
    })
}

fn request_input_items(
    request_json: &JsonValue,
    call: &ResponsesCallRow,
) -> Result<Vec<JsonValue>> {
    request_json
        .get("input")
        .and_then(JsonValue::as_array)
        .cloned()
        .with_context(|| {
            format!(
                "Responses call {} request_json has no input array",
                call.responses_call_id
            )
        })
}

fn response_output_items(call: &ResponsesCallRow) -> Result<Vec<JsonValue>> {
    let Some(response_json) = call.response_json.as_ref() else {
        return Ok(Vec::new());
    };
    let response_json: JsonValue = serde_json::from_str(response_json).with_context(|| {
        format!(
            "parse response_json for Responses call {}",
            call.responses_call_id
        )
    })?;
    response_json
        .get("output_items")
        .and_then(JsonValue::as_array)
        .cloned()
        .with_context(|| {
            format!(
                "Responses call {} response_json has no output_items array",
                call.responses_call_id
            )
        })
}

fn assign_window_metadata(windows: &mut [ContextWindowDraft]) {
    windows.sort_by(|left, right| {
        (
            left.session_id.as_str(),
            left.thread_id.as_str(),
            left.opened_at_epoch_millis,
            left.context_window_id.as_str(),
        )
            .cmp(&(
                right.session_id.as_str(),
                right.thread_id.as_str(),
                right.opened_at_epoch_millis,
                right.context_window_id.as_str(),
            ))
    });

    let mut previous_index: Option<usize> = None;
    let mut context_window_ordinal = 0;
    for index in 0..windows.len() {
        if let Some(previous_index) = previous_index
            && windows[previous_index].session_id == windows[index].session_id
            && windows[previous_index].thread_id == windows[index].thread_id
        {
            context_window_ordinal += 1;
            windows[previous_index].closed_at_epoch_millis =
                Some(windows[index].opened_at_epoch_millis);
            windows[previous_index].close_reason = Some("compaction".to_string());
        } else {
            context_window_ordinal = 1;
        }
        windows[index].context_window_ordinal = context_window_ordinal;
        previous_index = Some(index);
    }
}

fn insert_context_windows(connection: &Connection, windows: &[ContextWindowDraft]) -> Result<()> {
    let mut statement = connection.prepare(
        r#"
        INSERT INTO viewer_context_windows_v1 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )?;
    for window in windows {
        statement.execute(params_from_iter([
            text(window.session_id.clone()),
            text(window.thread_id.clone()),
            text(window.context_window_id.clone()),
            integer(window.context_window_ordinal),
            optional_text(window.first_turn_id.clone()),
            optional_text(window.last_turn_id.clone()),
            text(window.first_responses_call_id.clone()),
            text(window.last_responses_call_id.clone()),
            integer(window.opened_at_epoch_millis),
            optional_integer(window.closed_at_epoch_millis),
            optional_text(window.close_reason.clone()),
            integer(window.message_count),
            text(window.conversation_json.clone()),
            text(SYNTHETIC_CONVERSATION_SOURCE.to_string()),
            DuckValue::Boolean(true),
        ]))?;
    }
    Ok(())
}

fn text(value: String) -> DuckValue {
    DuckValue::Text(value)
}

fn optional_text(value: Option<String>) -> DuckValue {
    value.map_or(DuckValue::Null, DuckValue::Text)
}

fn integer(value: i64) -> DuckValue {
    DuckValue::BigInt(value)
}

fn optional_integer(value: Option<i64>) -> DuckValue {
    value.map_or(DuckValue::Null, DuckValue::BigInt)
}

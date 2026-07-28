use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;
use ts_rs::TS;

use super::InternalChatMessageMetadataPassthrough;
use super::ResponseItem;

const MAX_EXECUTED_TOOL_CALL_ARGUMENT_BYTES: usize = 8 * 1024;
/// Maximum serialized warehouse-only attempted-tool metadata in one request.
const MAX_EXECUTED_TOOL_CALL_METADATA_BYTES: usize = 32 * 1024;
const EXECUTED_TOOL_CALL_METADATA_FIELD_BYTES: usize = b"\"executed_tool_calls\":".len();
const INTERNAL_CHAT_MESSAGE_METADATA_PASSTHROUGH_FIELD_BYTES: usize =
    b"\"internal_chat_message_metadata_passthrough\":".len();

fn executed_tool_call_metadata_field_bytes(
    metadata: &InternalChatMessageMetadataPassthrough,
) -> usize {
    EXECUTED_TOOL_CALL_METADATA_FIELD_BYTES
        + if metadata.turn_id.is_some() {
            1
        } else {
            INTERNAL_CHAT_MESSAGE_METADATA_PASSTHROUGH_FIELD_BYTES + 3
        }
}

fn executed_tool_call_metadata_bytes(item: &ResponseItem) -> usize {
    let Some(metadata) = item.executed_tool_call_metadata() else {
        return 0;
    };
    let Some(calls) = metadata
        .executed_tool_calls
        .as_ref()
        .filter(|calls| !calls.is_empty())
    else {
        return 0;
    };
    serde_json::to_vec(calls)
        .map(|bytes| {
            bytes
                .len()
                .saturating_add(executed_tool_call_metadata_field_bytes(metadata))
        })
        .unwrap_or(usize::MAX)
}

/// Bounds attempted-tool metadata fairly across the complete serialized request.
pub fn bound_executed_tool_calls_for_prompt(items: &mut [ResponseItem]) {
    let mut remaining_items = 0_usize;
    let mut original_calls = 0_usize;
    let mut original_metadata_bytes = 0_usize;

    for item in items.iter_mut() {
        let Some(calls) = item
            .internal_chat_message_metadata_passthrough_mut()
            .and_then(Option::as_mut)
            .and_then(|metadata| metadata.executed_tool_calls.as_mut())
            .filter(|calls| !calls.is_empty())
        else {
            continue;
        };
        for call in calls {
            let argument_bytes = serde_json::to_vec(&call.arguments)
                .map(|bytes| bytes.len())
                .unwrap_or(usize::MAX);
            if call.truncation().is_none() && argument_bytes > MAX_EXECUTED_TOOL_CALL_ARGUMENT_BYTES
            {
                call.set_truncation(
                    argument_bytes,
                    MAX_EXECUTED_TOOL_CALL_ARGUMENT_BYTES,
                    /*omitted_calls*/ None,
                );
            }
            original_calls = original_calls.saturating_add(1).saturating_add(
                call.truncation()
                    .and_then(|truncation| truncation.omitted_calls)
                    .unwrap_or_default(),
            );
        }
        remaining_items += 1;
        original_metadata_bytes =
            original_metadata_bytes.saturating_add(executed_tool_call_metadata_bytes(item));
    }

    if original_metadata_bytes <= MAX_EXECUTED_TOOL_CALL_METADATA_BYTES {
        return;
    }

    let overflow_fallback = items.iter().enumerate().find_map(|(index, item)| {
        item.executed_tool_call_metadata()
            .and_then(|metadata| metadata.executed_tool_calls.as_ref())
            .and_then(|calls| calls.first())
            .map(|call| (index, call.clone()))
    });

    let omitted_call_reservation = serde_json::to_vec(&serde_json::json!({
        "_codex_executed_tool_call_truncated": ExecutedToolCallTruncation {
            original_bytes: usize::MAX,
            max_bytes: usize::MAX,
            omitted_calls: Some(usize::MAX),
            original_name_bytes: Some(usize::MAX),
        },
    }))
    .map(|bytes| bytes.len())
    .unwrap_or(usize::MAX);
    let mut remaining_bytes =
        MAX_EXECUTED_TOOL_CALL_METADATA_BYTES.saturating_sub(omitted_call_reservation);
    for item in items.iter_mut() {
        let Some(metadata) = item.executed_tool_call_metadata() else {
            continue;
        };
        if metadata
            .executed_tool_calls
            .as_ref()
            .is_none_or(Vec::is_empty)
        {
            continue;
        }

        let metadata_field_bytes = executed_tool_call_metadata_field_bytes(metadata);
        item.bound_executed_tool_calls_with_budget(
            (remaining_bytes / remaining_items).saturating_sub(metadata_field_bytes),
        );
        remaining_bytes = remaining_bytes.saturating_sub(executed_tool_call_metadata_bytes(item));
        remaining_items -= 1;
    }

    let represented_calls = items
        .iter()
        .filter_map(ResponseItem::executed_tool_call_metadata)
        .filter_map(|metadata| metadata.executed_tool_calls.as_ref())
        .flatten()
        .map(|call| {
            1 + call
                .truncation()
                .and_then(|truncation| truncation.omitted_calls)
                .unwrap_or_default()
        })
        .sum::<usize>();
    if represented_calls == original_calls {
        return;
    }

    if represented_calls == 0 {
        if let Some((index, mut call)) = overflow_fallback {
            let original_bytes = call
                .truncation()
                .map(|truncation| truncation.original_bytes)
                .unwrap_or_else(|| {
                    serde_json::to_vec(&call.arguments)
                        .map(|bytes| bytes.len())
                        .unwrap_or(usize::MAX)
                });
            let original_name_bytes = call.name.len();
            let name_boundary = call.name.floor_char_boundary(
                original_name_bytes.min(MAX_EXECUTED_TOOL_CALL_ARGUMENT_BYTES / 2),
            );
            call.name.truncate(name_boundary);
            call.set_truncation_with_name(
                original_bytes,
                /*max_bytes*/ 0,
                Some(original_calls.saturating_sub(1)),
                (name_boundary < original_name_bytes).then_some(original_name_bytes),
            );
            items[index].append_executed_tool_calls(vec![call]);
        }
        return;
    }

    if let Some(call) = items
        .iter_mut()
        .filter_map(ResponseItem::internal_chat_message_metadata_passthrough_mut)
        .filter_map(Option::as_mut)
        .filter_map(|metadata| metadata.executed_tool_calls.as_mut())
        .find_map(|calls| calls.first_mut())
    {
        let original_bytes = call
            .truncation()
            .map(|truncation| truncation.original_bytes)
            .unwrap_or_else(|| {
                serde_json::to_vec(&call.arguments)
                    .map(|bytes| bytes.len())
                    .unwrap_or(usize::MAX)
            });
        let max_bytes = call
            .truncation()
            .map(|truncation| truncation.max_bytes)
            .unwrap_or_default();
        let previous_omissions = call
            .truncation()
            .and_then(|truncation| truncation.omitted_calls)
            .unwrap_or_default();
        call.set_truncation(
            original_bytes,
            max_bytes,
            Some(
                previous_omissions.saturating_add(original_calls.saturating_sub(represented_calls)),
            ),
        );
    }
}

/// Raw model arguments or trusted truncation metadata for an attempted tool call.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, JsonSchema, TS)]
#[serde(untagged)]
pub enum ExecutedToolCallArguments {
    Raw(serde_json::Value),
    #[serde(skip_deserializing)]
    Truncated {
        #[serde(rename = "_codex_executed_tool_call_truncated")]
        truncation: ExecutedToolCallTruncation,
    },
}

/// A model-attempted Codex tool invocation captured at the shared runtime boundary.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, JsonSchema, TS)]
pub struct ExecutedToolCall {
    pub name: String,
    #[ts(type = "unknown")]
    arguments: ExecutedToolCallArguments,
}

/// Trusted truncation details generated locally for an oversized attempted tool call.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, JsonSchema, TS)]
pub struct ExecutedToolCallTruncation {
    original_bytes: usize,
    max_bytes: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    omitted_calls: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    original_name_bytes: Option<usize>,
}

impl ExecutedToolCall {
    /// Creates a recorded call without treating model-provided JSON as trusted metadata.
    pub fn new(name: String, arguments: serde_json::Value) -> Self {
        let arguments = if arguments
            .as_object()
            .is_some_and(|object| object.contains_key("_codex_executed_tool_call_truncated"))
        {
            serde_json::json!({ "_codex_executed_tool_call_raw": arguments })
        } else {
            arguments
        };
        Self {
            name,
            arguments: ExecutedToolCallArguments::Raw(arguments),
        }
    }

    /// Replaces oversized arguments with internally generated truncation metadata.
    pub fn truncated(name: String, original_bytes: usize, max_bytes: usize) -> Self {
        let mut call = Self::new(name, serde_json::Value::Null);
        call.set_truncation(original_bytes, max_bytes, /*omitted_calls*/ None);
        call
    }

    /// Returns the raw arguments or locally generated truncation payload.
    pub fn arguments(&self) -> &ExecutedToolCallArguments {
        &self.arguments
    }

    fn truncation(&self) -> Option<&ExecutedToolCallTruncation> {
        match &self.arguments {
            ExecutedToolCallArguments::Raw(_) => None,
            ExecutedToolCallArguments::Truncated { truncation } => Some(truncation),
        }
    }

    fn set_truncation(
        &mut self,
        original_bytes: usize,
        max_bytes: usize,
        omitted_calls: Option<usize>,
    ) {
        self.set_truncation_with_name(
            original_bytes,
            max_bytes,
            omitted_calls,
            /*original_name_bytes*/ None,
        );
    }

    fn set_truncation_with_name(
        &mut self,
        original_bytes: usize,
        max_bytes: usize,
        omitted_calls: Option<usize>,
        original_name_bytes: Option<usize>,
    ) {
        self.arguments = ExecutedToolCallArguments::Truncated {
            truncation: ExecutedToolCallTruncation {
                original_bytes,
                max_bytes,
                omitted_calls,
                original_name_bytes,
            },
        };
    }
}

impl ResponseItem {
    /// Attaches model-attempted tool invocations without replacing existing item metadata.
    pub fn append_executed_tool_calls(&mut self, calls: Vec<ExecutedToolCall>) {
        if calls.is_empty() {
            return;
        }
        let Some(metadata) = self.internal_chat_message_metadata_passthrough_mut() else {
            return;
        };
        metadata
            .get_or_insert_with(InternalChatMessageMetadataPassthrough::default)
            .executed_tool_calls
            .get_or_insert_with(Vec::new)
            .extend(calls);
    }

    /// Returns warehouse-only attempted-tool metadata for any supported item variant.
    pub fn executed_tool_call_metadata(&self) -> Option<&InternalChatMessageMetadataPassthrough> {
        self.internal_chat_message_metadata_passthrough()
    }

    /// Bounds one request item's attempted calls to its share of the prompt budget.
    fn bound_executed_tool_calls_with_budget(&mut self, max_metadata_bytes: usize) {
        let Some(metadata) = self.internal_chat_message_metadata_passthrough_mut() else {
            return;
        };
        let Some(passthrough) = metadata.as_mut() else {
            return;
        };
        let Some(calls) = passthrough.executed_tool_calls.as_mut() else {
            return;
        };

        let mut serialized_bytes = 2;
        let mut retained_calls = 0;
        calls.retain_mut(|call| {
            let separator_bytes = usize::from(retained_calls > 0);
            let remaining_bytes = max_metadata_bytes
                .saturating_sub(serialized_bytes)
                .saturating_sub(separator_bytes);
            let argument_bytes = serde_json::to_vec(&call.arguments)
                .map(|bytes| bytes.len())
                .unwrap_or(usize::MAX);
            let call_bytes = serde_json::to_vec(&*call)
                .map(|bytes| bytes.len())
                .unwrap_or(usize::MAX);

            if call_bytes > remaining_bytes
                || argument_bytes > MAX_EXECUTED_TOOL_CALL_ARGUMENT_BYTES
            {
                let original_bytes = call
                    .truncation()
                    .map(|truncation| truncation.original_bytes)
                    .unwrap_or(argument_bytes);
                let omitted_calls = call
                    .truncation()
                    .and_then(|truncation| truncation.omitted_calls);
                call.set_truncation(
                    original_bytes,
                    remaining_bytes.min(MAX_EXECUTED_TOOL_CALL_ARGUMENT_BYTES),
                    omitted_calls,
                );
            }

            let call_bytes = serde_json::to_vec(&*call)
                .map(|bytes| bytes.len())
                .unwrap_or(usize::MAX);
            if call_bytes > remaining_bytes {
                return false;
            }

            serialized_bytes = serialized_bytes
                .saturating_add(separator_bytes)
                .saturating_add(call_bytes);
            retained_calls += 1;
            true
        });

        if calls.is_empty() {
            passthrough.executed_tool_calls = None;
            if *passthrough == InternalChatMessageMetadataPassthrough::default() {
                *metadata = None;
            }
        }
    }

    /// Removes untrusted warehouse-only tool records without changing the turn ID.
    pub fn clear_executed_tool_calls(&mut self) {
        let Some(metadata) = self.internal_chat_message_metadata_passthrough_mut() else {
            return;
        };
        let Some(passthrough) = metadata.as_mut() else {
            return;
        };
        passthrough.executed_tool_calls = None;
        if *passthrough == InternalChatMessageMetadataPassthrough::default() {
            *metadata = None;
        }
    }
}

#[cfg(test)]
#[path = "executed_tool_calls_tests.rs"]
mod tests;

use std::collections::HashSet;

use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;
use ts_rs::TS;

use super::InternalChatMessageMetadataPassthrough;
use super::ResponseItem;

const MAX_EXECUTED_TOOL_CALL_ARGUMENT_BYTES: usize = 8 * 1024;
/// Maximum distinct result sources retained for one tool invocation.
const MAX_TOOL_RESULT_SOURCES: usize = 32;
/// Maximum UTF-8 bytes for each source's `type` and `id` separately, not the source list.
pub const MAX_TOOL_RESULT_SOURCE_FIELD_BYTES: usize = 128;
/// Maximum serialized warehouse-only attempted-tool metadata in one request.
const MAX_EXECUTED_TOOL_CALL_METADATA_BYTES: usize = 32 * 1024;
const EXECUTED_TOOL_CALL_METADATA_FIELD_BYTES: usize = b"\"executed_tool_calls\":".len();
const INTERNAL_CHAT_MESSAGE_METADATA_PASSTHROUGH_FIELD_BYTES: usize =
    b"\"internal_chat_message_metadata_passthrough\":".len();

fn executed_tool_call_metadata_field_bytes(
    metadata: &InternalChatMessageMetadataPassthrough,
) -> usize {
    let fields = InternalChatMessageMetadataPassthrough {
        cell_id: metadata.cell_id.clone(),
        tool_calls_complete: metadata.tool_calls_complete,
        ..Default::default()
    };
    let mut bytes =
        serde_json::to_vec(&fields).map_or(usize::MAX, |fields| fields.len().saturating_sub(2));
    if metadata.executed_tool_calls.is_some() {
        bytes = bytes
            .saturating_add(usize::from(bytes > 0))
            .saturating_add(EXECUTED_TOOL_CALL_METADATA_FIELD_BYTES);
    }
    if bytes == 0 {
        0
    } else if metadata.turn_id.is_some()
        || metadata.create_time.is_some()
        || metadata.content_item_kinds.is_some()
    {
        bytes + 1
    } else {
        bytes + INTERNAL_CHAT_MESSAGE_METADATA_PASSTHROUGH_FIELD_BYTES + 3
    }
}

/// Returns the exact serialized wire size of an item's attempted-tool metadata.
pub fn executed_tool_call_metadata_bytes(item: &ResponseItem) -> usize {
    let Some(metadata) = item.executed_tool_call_metadata() else {
        return 0;
    };
    metadata
        .executed_tool_calls
        .as_ref()
        .map_or(0, |calls| {
            serde_json::to_vec(calls)
                .map(|calls| calls.len())
                .unwrap_or(usize::MAX)
        })
        .saturating_add(executed_tool_call_metadata_field_bytes(metadata))
}

impl InternalChatMessageMetadataPassthrough {
    /// Compares call order, names and arguments, ignoring optional result metadata.
    pub fn has_same_tool_calls(&self, calls: &[ExecutedToolCall]) -> bool {
        self.executed_tool_calls.as_ref().is_some_and(|recorded| {
            recorded.len() == calls.len()
                && recorded.iter().zip(calls).all(|(recorded, call)| {
                    recorded.name == call.name && recorded.arguments() == call.arguments()
                })
        })
    }
}

/// Bounds attempted-tool metadata fairly across the complete serialized request.
pub fn bound_executed_tool_calls_for_prompt(items: &mut [ResponseItem]) {
    bound_executed_tool_calls_for_prompt_with_priority(items, /*prioritize_recent*/ false);
}

/// Bounds retained history without letting older calls displace the newest calls.
pub fn bound_executed_tool_calls_for_prompt_prioritizing_recent(items: &mut [ResponseItem]) {
    items.reverse();
    bound_executed_tool_calls_for_prompt_with_priority(items, /*prioritize_recent*/ true);
    items.reverse();
}

fn bound_executed_tool_calls_for_prompt_with_priority(
    items: &mut [ResponseItem],
    prioritize_recent: bool,
) {
    let mut damaged_cells = HashSet::new();
    for item in items.iter_mut() {
        let Some(metadata) = item
            .internal_chat_message_metadata_passthrough_mut()
            .and_then(Option::as_mut)
        else {
            continue;
        };
        let mut truncated = false;
        for call in metadata.executed_tool_calls.iter_mut().flatten() {
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
            truncated |= call.truncation().is_some();
        }
        if truncated {
            metadata.tool_calls_complete = None;
            damaged_cells.extend(metadata.cell_id.clone());
        }
    }
    clear_damaged_cell_completeness(items, &damaged_cells);

    let metadata_bytes = |items: &[ResponseItem]| {
        items.iter().fold(0_usize, |bytes, item| {
            bytes.saturating_add(executed_tool_call_metadata_bytes(item))
        })
    };
    if metadata_bytes(items) <= MAX_EXECUTED_TOOL_CALL_METADATA_BYTES {
        return;
    }
    // Raw result metadata must not displace existing source evidence, calls or completion proof.
    for item in items.iter_mut() {
        if let Some(metadata) = item
            .internal_chat_message_metadata_passthrough_mut()
            .and_then(Option::as_mut)
        {
            for call in metadata.executed_tool_calls.iter_mut().flatten() {
                if call.tool_result_metadata.is_some() {
                    call.tool_result_metadata = ToolResultMetadata::omitted_due_to_size_limit();
                }
            }
        }
    }

    if metadata_bytes(items) <= MAX_EXECUTED_TOOL_CALL_METADATA_BYTES {
        return;
    }
    // Omission markers are optional too; keep the original call budget if they cannot fit.
    for item in items.iter_mut() {
        item.clear_tool_result_metadata();
    }

    if metadata_bytes(items) <= MAX_EXECUTED_TOOL_CALL_METADATA_BYTES {
        return;
    }
    // Source evidence is optional; dropping it must not discard calls or their completion proof.
    for item in items.iter_mut() {
        if let Some(metadata) = item
            .internal_chat_message_metadata_passthrough_mut()
            .and_then(Option::as_mut)
        {
            for call in metadata.executed_tool_calls.iter_mut().flatten() {
                call.tool_result_sources = None;
            }
        }
    }
    if metadata_bytes(items) <= MAX_EXECUTED_TOOL_CALL_METADATA_BYTES {
        return;
    }

    let mut remaining_items = items
        .iter()
        .filter(|item| executed_tool_call_metadata_bytes(item) > 0)
        .count();
    let mut remaining_bytes = MAX_EXECUTED_TOOL_CALL_METADATA_BYTES;
    for item in items.iter_mut() {
        let item_bytes = executed_tool_call_metadata_bytes(item);
        if item_bytes == 0 {
            continue;
        }
        let item_budget = if prioritize_recent {
            remaining_bytes
        } else {
            remaining_bytes / remaining_items
        };
        if item_bytes > item_budget {
            // Remember the cell before a too-small share removes its metadata entirely.
            damaged_cells.extend(
                item.executed_tool_call_metadata()
                    .and_then(|metadata| metadata.cell_id.clone()),
            );
            item.clear_tool_calls_complete();
            item.bound_executed_tool_calls_with_budget(item_budget);
        }
        remaining_bytes = remaining_bytes.saturating_sub(executed_tool_call_metadata_bytes(item));
        remaining_items -= 1;
    }
    clear_damaged_cell_completeness(items, &damaged_cells);
}

fn clear_damaged_cell_completeness(items: &mut [ResponseItem], damaged_cells: &HashSet<String>) {
    for item in items {
        if item.executed_tool_call_metadata().is_some_and(|metadata| {
            metadata
                .cell_id
                .as_ref()
                .is_some_and(|cell_id| damaged_cells.contains(cell_id))
        }) {
            item.clear_tool_calls_complete();
        }
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
    /// Host-generated analytics only: ignore input JSON rather than accepting caller-supplied
    /// evidence, and keep this out of public schemas and generated clients.
    #[serde(default, skip_deserializing, skip_serializing_if = "Option::is_none")]
    #[schemars(skip)]
    #[ts(skip)]
    tool_result_sources: Option<Vec<ToolResultSource>>,
    /// Raw MCP result metadata is host-recorded only, never trusted from input JSON or exposed
    /// in public schemas. Its Debug implementation also prevents raw values reaching logs.
    #[serde(
        default,
        skip_deserializing,
        skip_serializing_if = "ToolResultMetadata::is_none"
    )]
    #[schemars(skip)]
    #[ts(skip)]
    tool_result_metadata: ToolResultMetadata,
}

/// An entire MCP result's `_meta`, or a string marker when omitted due to the size limit.
#[derive(Clone, Default, Serialize, PartialEq, Eq)]
#[serde(transparent)]
pub struct ToolResultMetadata(Option<serde_json::Value>);

impl std::fmt::Debug for ToolResultMetadata {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("ToolResultMetadata([redacted])")
    }
}

impl ToolResultMetadata {
    /// Bounds serialization before cloning arbitrary MCP metadata; no keys are filtered.
    pub fn new(metadata: &serde_json::Value) -> Self {
        let mut limit = MetadataSizeLimit(MAX_EXECUTED_TOOL_CALL_METADATA_BYTES);
        if serde_json::to_writer(&mut limit, metadata).is_ok() {
            Self(Some(metadata.clone()))
        } else {
            Self::omitted_due_to_size_limit()
        }
    }

    fn omitted_due_to_size_limit() -> Self {
        // MCP `_meta` is an object; this string is a harness omission status, not provider data.
        Self(Some(serde_json::Value::String(
            "omitted_due_to_size_limit".to_string(),
        )))
    }

    fn is_none(&self) -> bool {
        self.0.is_none()
    }

    /// Whether the bounded snapshot contains metadata or an omission marker.
    pub fn is_some(&self) -> bool {
        self.0.is_some()
    }
}

struct MetadataSizeLimit(usize);

impl std::io::Write for MetadataSizeLimit {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0 = self
            .0
            .checked_sub(bytes.len())
            .ok_or_else(|| std::io::Error::other("tool result metadata exceeds the byte limit"))?;
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// A bounded capture update. Omitted updates still clear any previously recorded evidence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolResultSources(Option<Vec<ToolResultSource>>);

impl ToolResultSources {
    /// Deduplicates a captured snapshot, discarding all sources if it exceeds a limit.
    /// Capture rules determine coverage; this is not a resource or permission inventory.
    pub fn new(sources: Vec<ToolResultSource>) -> Self {
        let mut unique_sources = Vec::new();
        for source in sources {
            if unique_sources.contains(&source) {
                continue;
            }
            if unique_sources.len() == MAX_TOOL_RESULT_SOURCES
                || source.r#type.len() > MAX_TOOL_RESULT_SOURCE_FIELD_BYTES
                || source.id.len() > MAX_TOOL_RESULT_SOURCE_FIELD_BYTES
            {
                return Self(None);
            }
            unique_sources.push(source);
        }
        Self(Some(unique_sources))
    }

    /// Records a failed parse using the receiver's existing array-of-sources shape.
    /// `parse_failed` is a status marker, not a resource type; the required ID is empty.
    pub fn parse_failed() -> Self {
        Self(Some(vec![ToolResultSource {
            r#type: "parse_failed".to_string(),
            id: String::new(),
        }]))
    }
}

/// A captured source ID, or a `parse_failed` status marker with an empty ID.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ToolResultSource {
    #[serde(rename = "type")]
    pub r#type: String,
    pub id: String,
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
            tool_result_sources: None,
            tool_result_metadata: ToolResultMetadata::default(),
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

    /// Replaces this invocation's capture outcome, including clearing omitted evidence.
    pub fn set_tool_result_sources(&mut self, sources: ToolResultSources) -> bool {
        self.tool_result_sources = sources.0;
        self.tool_result_sources.is_some()
    }

    /// Replaces the entire `_meta` snapshot, including with an omission marker if oversized.
    pub fn set_tool_result_metadata(&mut self, metadata: ToolResultMetadata) {
        self.tool_result_metadata = metadata;
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
    fn ensure_tool_call_metadata(&mut self) -> Option<&mut InternalChatMessageMetadataPassthrough> {
        self.internal_chat_message_metadata_passthrough_mut()
            .map(Option::get_or_insert_default)
    }

    /// Associates host-recorded calls and completeness with their Code Mode cell.
    pub fn set_tool_call_cell_id(&mut self, cell_id: &str) {
        if let Some(metadata) = self.ensure_tool_call_metadata() {
            metadata.cell_id = Some(cell_id.to_string());
        }
    }

    /// Attaches model-attempted tool invocations without replacing existing item metadata.
    pub fn append_executed_tool_calls(&mut self, calls: Vec<ExecutedToolCall>) {
        if calls.is_empty() {
            return;
        }
        let Some(metadata) = self.ensure_tool_call_metadata() else {
            return;
        };
        metadata
            .executed_tool_calls
            .get_or_insert_with(Vec::new)
            .extend(calls);
    }

    /// Marks a host-owned cell's recorded tool calls as complete.
    pub fn mark_tool_calls_complete(&mut self) {
        if let Some(metadata) = self.ensure_tool_call_metadata() {
            metadata.tool_calls_complete = Some(true);
        }
    }

    /// Discards a completion claim when the host cannot retain its full evidence.
    pub fn clear_tool_calls_complete(&mut self) {
        if let Some(metadata) = self
            .internal_chat_message_metadata_passthrough_mut()
            .and_then(Option::as_mut)
        {
            metadata.tool_calls_complete = None;
        }
    }

    /// Returns warehouse-only attempted-tool metadata for any supported item variant.
    pub fn executed_tool_call_metadata(&self) -> Option<&InternalChatMessageMetadataPassthrough> {
        self.internal_chat_message_metadata_passthrough()
    }

    /// Omits raw tool results without changing existing call, source or completion metadata.
    pub fn clear_tool_result_metadata(&mut self) {
        if let Some(metadata) = self
            .internal_chat_message_metadata_passthrough_mut()
            .and_then(Option::as_mut)
        {
            for call in metadata.executed_tool_calls.iter_mut().flatten() {
                call.tool_result_metadata = ToolResultMetadata::default();
            }
        }
    }

    /// Replaces an over-budget output's calls with its own omission marker.
    fn bound_executed_tool_calls_with_budget(&mut self, max_metadata_bytes: usize) {
        let Some(metadata) = self.executed_tool_call_metadata() else {
            return;
        };
        let max_call_bytes =
            max_metadata_bytes.saturating_sub(executed_tool_call_metadata_field_bytes(metadata));
        let Some(calls) = self
            .internal_chat_message_metadata_passthrough_mut()
            .and_then(Option::as_mut)
            .and_then(|metadata| metadata.executed_tool_calls.as_mut())
            .filter(|calls| !calls.is_empty())
        else {
            self.clear_executed_tool_calls();
            return;
        };
        let represented_calls = calls.iter().fold(0_usize, |count, call| {
            count.saturating_add(1).saturating_add(
                call.truncation()
                    .and_then(|truncation| truncation.omitted_calls)
                    .unwrap_or_default(),
            )
        });
        calls.truncate(1);
        let call = &mut calls[0];
        let original_bytes = call
            .truncation()
            .map(|truncation| truncation.original_bytes)
            .unwrap_or_else(|| {
                serde_json::to_vec(&call.arguments)
                    .map(|bytes| bytes.len())
                    .unwrap_or(usize::MAX)
            });
        let original_name_bytes = call
            .truncation()
            .and_then(|truncation| truncation.original_name_bytes);
        let omitted_calls = (represented_calls > 1).then_some(represented_calls - 1);
        call.set_truncation_with_name(
            original_bytes,
            max_call_bytes.min(MAX_EXECUTED_TOOL_CALL_ARGUMENT_BYTES),
            omitted_calls,
            original_name_bytes,
        );
        let serialized_bytes = |calls: &[ExecutedToolCall]| {
            serde_json::to_vec(calls)
                .map(|bytes| bytes.len())
                .unwrap_or(usize::MAX)
        };
        if serialized_bytes(calls) > max_call_bytes {
            let call = &mut calls[0];
            call.set_truncation_with_name(
                original_bytes,
                max_call_bytes.min(MAX_EXECUTED_TOOL_CALL_ARGUMENT_BYTES),
                omitted_calls,
                Some(original_name_bytes.unwrap_or(call.name.len())),
            );
            // Removing UTF-8 name bytes saves at least that many serialized JSON bytes.
            let excess_bytes = serialized_bytes(calls).saturating_sub(max_call_bytes);
            let name = &mut calls[0].name;
            name.truncate(name.floor_char_boundary(name.len().saturating_sub(excess_bytes)));
        }
        if serialized_bytes(calls) > max_call_bytes {
            self.clear_executed_tool_calls();
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
        passthrough.cell_id = None;
        passthrough.executed_tool_calls = None;
        passthrough.tool_calls_complete = None;
        if *passthrough == InternalChatMessageMetadataPassthrough::default() {
            *metadata = None;
        }
    }
}

#[cfg(test)]
#[path = "executed_tool_calls_tests.rs"]
mod tests;

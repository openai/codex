use crate::models::ResponseInputItem;
use crate::models::ResponseItem;
use crate::models::ResponseItemMessageMetadata;
use crate::models::ResponseItemMetadata;
use crate::models::UserMessageType;

pub fn stamp_user_message_type_on_input_item(item: &mut ResponseInputItem, kind: UserMessageType) {
    let metadata = user_message_metadata_patch(kind);
    stamp_message_metadata_on_input_item(item, &metadata);
}

pub fn stamp_message_metadata_on_input_item(
    item: &mut ResponseInputItem,
    patch: &ResponseItemMessageMetadata,
) {
    let ResponseInputItem::Message { role, metadata, .. } = item else {
        return;
    };
    if role != "user" {
        return;
    }
    match metadata {
        Some(existing) => {
            if patch.user_message_type.is_some() {
                existing.user_message_type = patch.user_message_type.clone();
            }
        }
        None => {
            *metadata = Some(patch.clone());
        }
    }
}

pub fn user_message_metadata_patch(kind: UserMessageType) -> ResponseItemMessageMetadata {
    ResponseItemMessageMetadata {
        user_message_type: Some(kind),
        ..ResponseItemMessageMetadata::new(/*user_message_type*/ None)
    }
}

pub fn response_item_tool_call_id(item: &ResponseItem) -> Option<&str> {
    match item {
        ResponseItem::LocalShellCall {
            call_id: Some(call_id),
            ..
        } => Some(call_id),
        ResponseItem::FunctionCall { call_id, .. } => Some(call_id),
        ResponseItem::CustomToolCall { call_id, .. } => Some(call_id),
        _ => None,
    }
}

fn tool_call_metadata_slot_mut(
    item: &mut ResponseItem,
) -> Option<&mut Option<ResponseItemMetadata>> {
    match item {
        ResponseItem::LocalShellCall { metadata, .. }
        | ResponseItem::FunctionCall { metadata, .. }
        | ResponseItem::CustomToolCall { metadata, .. } => Some(metadata),
        _ => None,
    }
}

pub fn tool_call_metadata_or_default(item: &ResponseItem) -> Option<ResponseItemMetadata> {
    match item {
        ResponseItem::LocalShellCall { metadata, .. }
        | ResponseItem::FunctionCall { metadata, .. }
        | ResponseItem::CustomToolCall { metadata, .. } => {
            Some(metadata.clone().unwrap_or_default())
        }
        _ => None,
    }
}

pub fn stamp_tool_metadata_on_response_item(
    mut item: ResponseItem,
    patch: ResponseItemMetadata,
) -> ResponseItem {
    if patch.is_empty() {
        return item;
    }
    let Some(metadata_slot) = tool_call_metadata_slot_mut(&mut item) else {
        return item;
    };
    let mut metadata = metadata_slot.take().unwrap_or_default();
    metadata.merge_from(patch);
    *metadata_slot = (!metadata.is_empty()).then_some(metadata);
    item
}

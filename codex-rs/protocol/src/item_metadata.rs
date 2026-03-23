use crate::models::ResponseInputItem;
use crate::models::ResponseItemMessageMetadata;
use crate::models::UserMessageType;

pub fn stamp_user_message_type_on_input_item(item: &mut ResponseInputItem, kind: UserMessageType) {
    let ResponseInputItem::Message { role, metadata, .. } = item else {
        return;
    };
    if role != "user" {
        return;
    }
    let mut metadata_value = metadata
        .take()
        .unwrap_or_else(|| ResponseItemMessageMetadata::new(/*user_message_type*/ None));
    metadata_value.user_message_type = Some(kind);
    *metadata = Some(metadata_value);
}

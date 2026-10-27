use codex_extension_items::ExtensionItem;
use codex_extension_items::image_generation::GeneratedImageRef;
use codex_extension_items::image_generation::ImageGenerationItem;
use codex_protocol::ThreadId;
use codex_protocol::items::TurnItem;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::ImageGenerationEndEvent;
use codex_protocol::protocol::ItemCompletedEvent;
use codex_protocol::protocol::ThreadHistoryMode;

use super::persisted_event_msg;

fn image_item(image: GeneratedImageRef) -> ImageGenerationItem {
    ImageGenerationItem {
        id: "image-1".to_string(),
        status: "completed".to_string(),
        revised_prompt: Some("paint a blue whale".to_string()),
        image,
        transparent_background: Some(false),
        failure: None,
        saved_path: None,
        input_images: Vec::new(),
        imagegen_request_id: None,
        generation_id: None,
    }
}

#[test]
fn paginated_image_generation_preserves_file_reference() {
    let event = EventMsg::ItemCompleted(ItemCompletedEvent {
        thread_id: ThreadId::new(),
        turn_id: "turn-1".to_string(),
        item: TurnItem::Extension(ExtensionItem::ImageGeneration(image_item(
            GeneratedImageRef::File {
                file_id: "file-1".to_string(),
            },
        ))),
        started_at_ms: Some(1),
        completed_at_ms: 2,
    });

    let persisted = persisted_event_msg(&event, ThreadHistoryMode::Paginated)
        .expect("expected persisted image-generation item");
    let EventMsg::ItemCompleted(persisted) = persisted.as_ref() else {
        panic!("expected persisted image-generation item");
    };
    let TurnItem::Extension(ExtensionItem::ImageGeneration(image)) = &persisted.item else {
        panic!("expected image-generation extension item");
    };

    assert_eq!(
        image,
        &image_item(GeneratedImageRef::File {
            file_id: "file-1".to_string(),
        })
    );
}

#[test]
fn legacy_image_generation_preserves_file_reference() {
    let event = EventMsg::ImageGenerationEnd(ImageGenerationEndEvent {
        call_id: "image-1".to_string(),
        status: "completed".to_string(),
        revised_prompt: Some("paint a blue whale".to_string()),
        image: GeneratedImageRef::File {
            file_id: "file-1".to_string(),
        },
        transparent_background: Some(false),
        failure: None,
        saved_path: None,
        input_images: Vec::new(),
        generation_id: None,
    });

    let persisted = persisted_event_msg(&event, ThreadHistoryMode::Legacy)
        .expect("expected persisted legacy image-generation event");
    let EventMsg::ImageGenerationEnd(persisted) = persisted.as_ref() else {
        panic!("expected persisted legacy image-generation event");
    };

    assert_eq!(
        persisted.image,
        GeneratedImageRef::File {
            file_id: "file-1".to_string(),
        }
    );
}

#[test]
fn inline_image_generation_result_is_preserved() {
    let event = EventMsg::ItemCompleted(ItemCompletedEvent {
        thread_id: ThreadId::new(),
        turn_id: "turn-1".to_string(),
        item: TurnItem::Extension(ExtensionItem::ImageGeneration(image_item(
            GeneratedImageRef::Inline {
                result: "base64-image".to_string(),
            },
        ))),
        started_at_ms: Some(1),
        completed_at_ms: 2,
    });

    let persisted = persisted_event_msg(&event, ThreadHistoryMode::Paginated)
        .expect("expected persisted inline image-generation item");
    let EventMsg::ItemCompleted(persisted) = persisted.as_ref() else {
        panic!("expected persisted inline image-generation item");
    };
    let TurnItem::Extension(ExtensionItem::ImageGeneration(image)) = &persisted.item else {
        panic!("expected inline image-generation extension item");
    };

    assert_eq!(
        image,
        &image_item(GeneratedImageRef::Inline {
            result: "base64-image".to_string(),
        })
    );
}

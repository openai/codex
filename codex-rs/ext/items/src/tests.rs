use pretty_assertions::assert_eq;
use serde_json::json;

use super::ExtensionItem;
use super::ExtensionItemPayload;
use super::image_generation::ImageGenerationPayload;

fn completed_image_generation_item() -> ExtensionItem {
    ExtensionItem {
        id: "image-1".to_string(),
        payload: ExtensionItemPayload::ImageGeneration(ImageGenerationPayload {
            status: "completed".to_string(),
            revised_prompt: Some("A blue square".to_string()),
            result: "cG5n".to_string(),
            saved_path: None,
        }),
    }
}

#[test]
fn image_generation_item_preserves_stable_wire_shape() {
    let item = completed_image_generation_item();
    let value = serde_json::to_value(&item).expect("serialize extension item");

    assert_eq!(
        value,
        json!({
            "id": "image-1",
            "kind": "image_gen.generation",
            "payload": {
                "status": "completed",
                "revised_prompt": "A blue square",
                "result": "cG5n",
                "saved_path": null,
            },
        })
    );
    assert_eq!(
        serde_json::from_value::<ExtensionItem>(value).expect("deserialize extension item"),
        item
    );
}

#[test]
fn unknown_extension_kind_is_rejected() {
    let value = json!({
        "id": "image-1",
        "kind": "image_gen.unknown",
        "payload": {},
    });

    assert!(serde_json::from_value::<ExtensionItem>(value).is_err());
}

#[test]
fn malformed_known_extension_payload_is_rejected() {
    let value = json!({
        "id": "image-1",
        "kind": "image_gen.generation",
        "payload": {
            "status": "completed",
        },
    });

    assert!(serde_json::from_value::<ExtensionItem>(value).is_err());
}

use pretty_assertions::assert_eq;
use serde_json::json;

use super::ExtensionItem;
use super::image_generation::ImageGenerationItem;

fn completed_image_generation_item() -> ExtensionItem {
    ExtensionItem::ImageGeneration(ImageGenerationItem {
        id: "image-1".to_string(),
        status: "completed".to_string(),
        revised_prompt: Some("A blue square".to_string()),
        result: "cG5n".to_string(),
        saved_path: None,
    })
}

#[test]
fn image_generation_item_preserves_stable_wire_shape() {
    let item = completed_image_generation_item();
    let value = serde_json::to_value(&item).expect("serialize extension item");

    assert_eq!(
        value,
        json!({
            "kind": "image_gen.generation",
            "id": "image-1",
            "status": "completed",
            "revisedPrompt": "A blue square",
            "result": "cG5n",
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
        "kind": "image_gen.unknown",
        "id": "image-1",
    });

    assert!(serde_json::from_value::<ExtensionItem>(value).is_err());
}

#[test]
fn malformed_known_extension_payload_is_rejected() {
    let value = json!({
        "kind": "image_gen.generation",
        "id": "image-1",
        "status": "completed",
    });

    assert!(serde_json::from_value::<ExtensionItem>(value).is_err());
}

use pretty_assertions::assert_eq;
use serde_json::json;
use ts_rs::TS;

use super::ExtensionItem;
use super::image_generation::GeneratedImageRef;
use super::image_generation::ImageGenerationInputImage;
use super::image_generation::ImageGenerationItem;
use super::sleep::SleepItem;
use super::web_search::WebSearchAction;
use super::web_search::WebSearchItem;
use codex_utils_absolute_path::AbsolutePathBuf;

fn completed_image_generation_item() -> ExtensionItem {
    ExtensionItem::ImageGeneration(ImageGenerationItem {
        id: "image-1".to_string(),
        status: "completed".to_string(),
        revised_prompt: Some("A blue square".to_string()),
        image: GeneratedImageRef::Inline {
            result: "cG5n".to_string(),
        },
        transparent_background: None,
        failure: None,
        saved_path: None,
        input_images: Vec::new(),
        imagegen_request_id: None,
        generation_id: None,
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
            "transparentBackground": null,
            "failure": null,
            "generationId": null,
        })
    );
    assert_eq!(
        serde_json::from_value::<ExtensionItem>(value).expect("deserialize extension item"),
        item
    );
    assert_eq!(
        serde_json::from_value::<ExtensionItem>(json!({
            "kind": "image_gen.generation",
            "id": "image-1",
            "status": "completed",
            "revisedPrompt": "A blue square",
            "result": "cG5n",
        }))
        .expect("deserialize legacy image-generation item without transparency metadata"),
        item
    );
    assert_eq!(
        serde_json::from_value::<ExtensionItem>(json!({
            "kind": "image_gen.generation",
            "id": "image-1",
            "status": "completed",
            "revisedPrompt": "A blue square",
            "result": "cG5n",
            "fileId": null,
        }))
        .expect("deserialize transitional inline image-generation item"),
        item
    );
}

/// Transitional rollouts containing both fields must retain the durable file reference.
#[test]
fn image_generation_item_reads_transitional_file_wire_shape() {
    let item = ExtensionItem::ImageGeneration(ImageGenerationItem {
        id: "image-1".to_string(),
        status: "completed".to_string(),
        revised_prompt: Some("A blue square".to_string()),
        image: GeneratedImageRef::File {
            file_id: "file-1".to_string(),
        },
        transparent_background: None,
        failure: None,
        saved_path: None,
        input_images: Vec::new(),
        imagegen_request_id: None,
        generation_id: None,
    });

    assert_eq!(
        serde_json::from_value::<ExtensionItem>(json!({
            "kind": "image_gen.generation",
            "id": "image-1",
            "status": "completed",
            "revisedPrompt": "A blue square",
            "result": "",
            "fileId": "file-1",
        }))
        .expect("deserialize transitional file-backed image-generation item"),
        item
    );
    assert_eq!(
        serde_json::from_value::<ExtensionItem>(json!({
            "kind": "image_gen.generation",
            "id": "image-1",
            "status": "completed",
            "revisedPrompt": "A blue square",
            "result": "stale-inline-bytes",
            "fileId": "file-1",
        }))
        .expect("prefer file ID when both image representations are populated"),
        item
    );
    assert_eq!(
        serde_json::to_value(&item).expect("serialize file-backed extension item"),
        json!({
            "kind": "image_gen.generation",
            "id": "image-1",
            "status": "completed",
            "revisedPrompt": "A blue square",
            "fileId": "file-1",
            "transparentBackground": null,
            "failure": null,
            "generationId": null,
        })
    );
}

#[test]
fn image_generation_input_images_survive_serialization() {
    let ExtensionItem::ImageGeneration(mut image) = completed_image_generation_item() else {
        panic!("expected image-generation item");
    };
    let path = AbsolutePathBuf::current_dir()
        .expect("absolute cwd")
        .join("reference.png");
    image.input_images = vec![ImageGenerationInputImage {
        path: path.clone(),
        sha256: "ab".repeat(32),
    }];
    let item = ExtensionItem::ImageGeneration(image);
    let value = serde_json::to_value(&item).expect("serialize image generation");
    assert_eq!(
        value["inputImages"],
        json!([{"path": path, "sha256": "ab".repeat(32)}])
    );
    assert_eq!(
        serde_json::from_value::<ExtensionItem>(value).expect("deserialize image generation"),
        item
    );
}

#[test]
fn image_generation_item_preserves_authoritative_transparency() {
    let ExtensionItem::ImageGeneration(mut image) = completed_image_generation_item() else {
        panic!("expected image-generation item");
    };
    image.transparent_background = Some(true);
    let item = ExtensionItem::ImageGeneration(image);
    let value = serde_json::to_value(&item).expect("serialize extension item");

    assert_eq!(
        value,
        json!({
            "kind": "image_gen.generation",
            "id": "image-1",
            "status": "completed",
            "revisedPrompt": "A blue square",
            "result": "cG5n",
            "transparentBackground": true,
            "failure": null,
            "generationId": null,
        })
    );
    assert_eq!(
        serde_json::from_value::<ExtensionItem>(value).expect("deserialize extension item"),
        item
    );
}

#[test]
fn image_generation_typescript_fields_preserve_wire_contracts() {
    assert!(
        ImageGenerationItem::inline().contains("{ fileId: string, } | { result: string, }")
            || ImageGenerationItem::inline().contains("{ result: string, } | { fileId: string, }"),
        "v2 image-generation items must require exactly one image reference"
    );
    assert!(
        ImageGenerationItem::inline().contains("transparentBackground?: boolean"),
        "image-generation transparency must remain optional for existing TypeScript clients"
    );
}

#[test]
fn image_generation_request_id_stays_internal() {
    let ExtensionItem::ImageGeneration(mut image) = completed_image_generation_item() else {
        panic!("expected image-generation item");
    };
    image.imagegen_request_id = Some("req-imagegen-123".to_string());
    image.generation_id = Some("gen-image-123".to_string());
    let item = ExtensionItem::ImageGeneration(image);
    let value = serde_json::to_value(&item).expect("serialize extension item");

    assert!(value.get("imagegenRequestId").is_none());
    assert_eq!(value["generationId"], json!("gen-image-123"));
    let ExtensionItem::ImageGeneration(round_tripped) =
        serde_json::from_value::<ExtensionItem>(value).expect("deserialize extension item")
    else {
        panic!("expected image-generation item");
    };
    assert_eq!(round_tripped.imagegen_request_id, None);
    assert_eq!(
        round_tripped.generation_id.as_deref(),
        Some("gen-image-123")
    );
    assert!(!ImageGenerationItem::inline().contains("imagegenRequestId"));
    assert!(ImageGenerationItem::inline().contains("generationId: string | null"));
}

#[test]
fn web_search_item_preserves_stable_wire_shape() {
    let item = ExtensionItem::WebSearch(WebSearchItem {
        id: "search-1".to_string(),
        query: "docs".to_string(),
        action: Some(WebSearchAction::Search {
            query: Some("docs".to_string()),
            queries: None,
        }),
        results: None,
    });
    let value = serde_json::to_value(&item).expect("serialize extension item");

    assert_eq!(
        value,
        json!({
            "kind": "web.search",
            "id": "search-1",
            "query": "docs",
            "action": {
                "type": "search",
                "query": "docs",
                "queries": null,
            },
            "results": null,
        })
    );
    assert_eq!(
        serde_json::from_value::<ExtensionItem>(value).expect("deserialize extension item"),
        item
    );
    assert_eq!(
        serde_json::from_value::<ExtensionItem>(json!({
            "kind": "web.search",
            "id": "search-1",
            "query": "docs",
            "action": {
                "type": "search",
                "query": "docs",
                "queries": null,
            },
        }))
        .expect("deserialize legacy extension item without results"),
        item
    );
}

#[test]
fn sleep_item_preserves_stable_wire_shape() {
    let item = ExtensionItem::Sleep(SleepItem {
        id: "sleep-1".to_string(),
        duration_ms: 1_000,
    });
    let value = serde_json::to_value(&item).expect("serialize extension item");

    assert_eq!(
        value,
        json!({
            "kind": "clock.sleep",
            "id": "sleep-1",
            "durationMs": 1_000,
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

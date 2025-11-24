use rmcp::model::{AnnotateAble, Content, Meta, RawContent, ResourceContents};
use serde_json::json;

#[test]
fn serialize_embedded_text_resource_with_meta() {
    // Inner contents meta
    let mut resource_content_meta = Meta::new();
    resource_content_meta.insert("inner".to_string(), json!(2));

    // Top-level embedded resource meta
    let mut resource_meta = Meta::new();
    resource_meta.insert("top".to_string(), json!(1));

    let content: Content = RawContent::Resource(rmcp::model::RawEmbeddedResource {
        meta: Some(resource_meta),
        resource: ResourceContents::TextResourceContents {
            uri: "str://example".to_string(),
            mime_type: Some("text/plain".to_string()),
            text: "hello".to_string(),
            meta: Some(resource_content_meta),
        },
    })
    .no_annotation();

    let v = serde_json::to_value(&content).unwrap();

    let expected = json!({
        "type": "resource",
        "_meta": {"top": 1},
        "resource": {
            "uri": "str://example",
            "mimeType": "text/plain",
            "text": "hello",
            "_meta": {"inner": 2}
        }
    });

    assert_eq!(v, expected);
}

#[test]
fn serialize_embedded_text_resource_without_meta_omits_fields() {
    let content: Content = RawContent::Resource(rmcp::model::RawEmbeddedResource {
        meta: None,
        resource: ResourceContents::TextResourceContents {
            uri: "str://no-meta".to_string(),
            mime_type: Some("text/plain".to_string()),
            text: "hi".to_string(),
            meta: None,
        },
    })
    .no_annotation();

    let v = serde_json::to_value(&content).unwrap();

    assert_eq!(v.get("_meta"), None);
    let inner = v.get("resource").and_then(|r| r.as_object()).unwrap();
    assert_eq!(inner.get("_meta"), None);
}

#[test]
fn deserialize_embedded_text_resource_with_meta() {
    let raw = json!({
        "type": "resource",
        "_meta": {"x": true},
        "resource": {
            "uri": "str://from-json",
            "text": "ok",
            "_meta": {"y": 42}
        }
    });

    let content: Content = serde_json::from_value(raw).unwrap();

    let raw = match &content.raw {
        RawContent::Resource(er) => er,
        _ => panic!("expected resource"),
    };

    // top-level _meta
    let top = raw.meta.as_ref().expect("top-level meta missing");
    assert_eq!(top.get("x").unwrap(), &json!(true));

    // inner contents _meta
    match &raw.resource {
        ResourceContents::TextResourceContents {
            meta, uri, text, ..
        } => {
            assert_eq!(uri, "str://from-json");
            assert_eq!(text, "ok");
            let inner = meta.as_ref().expect("inner meta missing");
            assert_eq!(inner.get("y").unwrap(), &json!(42));
        }
        _ => panic!("expected text resource contents"),
    }
}

#[test]
fn serialize_embedded_blob_resource_with_meta() {
    let mut resource_content_meta = Meta::new();
    resource_content_meta.insert("blob_inner".to_string(), json!(true));

    let mut resource_meta = Meta::new();
    resource_meta.insert("blob_top".to_string(), json!("t"));

    let content: Content = RawContent::Resource(rmcp::model::RawEmbeddedResource {
        meta: Some(resource_meta),
        resource: ResourceContents::BlobResourceContents {
            uri: "str://blob".to_string(),
            mime_type: Some("application/octet-stream".to_string()),
            blob: "Zm9v".to_string(),
            meta: Some(resource_content_meta),
        },
    })
    .no_annotation();

    let v = serde_json::to_value(&content).unwrap();

    assert_eq!(v.get("_meta").unwrap(), &json!({"blob_top": "t"}));
    let inner = v.get("resource").unwrap();
    assert_eq!(inner.get("_meta").unwrap(), &json!({"blob_inner": true}));
}

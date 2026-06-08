use super::value_from_content;
use pretty_assertions::assert_eq;
use rmcp::model::Annotated;
use rmcp::model::Annotations;
use rmcp::model::Icon;
use rmcp::model::IconTheme;
use rmcp::model::Meta;
use rmcp::model::RawAudioContent;
use rmcp::model::RawContent;
use rmcp::model::RawEmbeddedResource;
use rmcp::model::RawImageContent;
use rmcp::model::RawResource;
use rmcp::model::RawTextContent;
use rmcp::model::ResourceContents;
use rmcp::model::Role;

fn meta(value: serde_json::Value) -> Meta {
    Meta(value.as_object().expect("metadata object").clone())
}

#[test]
fn converts_all_rmcp_content_variants_structurally() {
    let mut annotations = Annotations::default();
    annotations.audience = Some(vec![Role::User, Role::Assistant]);
    annotations.priority = Some(0.75);
    annotations.last_modified = Some("2026-06-07T12:34:56.123Z".parse().expect("valid timestamp"));

    let content = vec![
        Annotated::new(
            RawContent::Text(RawTextContent {
                text: "hello".to_string(),
                meta: Some(meta(serde_json::json!({"textMeta": true}))),
            }),
            Some(annotations),
        ),
        Annotated::new(
            RawContent::Image(RawImageContent {
                data: "base64-image".to_string(),
                mime_type: "image/png".to_string(),
                meta: Some(meta(serde_json::json!({"imageMeta": 1}))),
            }),
            None,
        ),
        Annotated::new(
            RawContent::Resource(RawEmbeddedResource {
                meta: Some(meta(serde_json::json!({"contentMeta": "outer"}))),
                resource: ResourceContents::TextResourceContents {
                    uri: "file:///example.txt".to_string(),
                    mime_type: Some("text/plain".to_string()),
                    text: "resource text".to_string(),
                    meta: Some(meta(serde_json::json!({"resourceMeta": "inner"}))),
                },
            }),
            None,
        ),
        Annotated::new(
            RawContent::Audio(RawAudioContent {
                data: "base64-audio".to_string(),
                mime_type: "audio/wav".to_string(),
            }),
            None,
        ),
        Annotated::new(
            RawContent::ResourceLink(RawResource {
                uri: "https://example.com/report".to_string(),
                name: "report".to_string(),
                title: Some("Report".to_string()),
                description: Some("Quarterly report".to_string()),
                mime_type: Some("application/pdf".to_string()),
                size: Some(42),
                icons: Some(vec![
                    Icon::new("https://example.com/icon.svg")
                        .with_mime_type("image/svg+xml")
                        .with_sizes(vec!["any".to_string()])
                        .with_theme(IconTheme::Light),
                ]),
                meta: Some(meta(serde_json::json!({"linkMeta": true}))),
            }),
            None,
        ),
    ];

    assert_eq!(
        content
            .into_iter()
            .map(value_from_content)
            .collect::<Vec<_>>(),
        vec![
            serde_json::json!({
                "type": "text",
                "text": "hello",
                "_meta": {"textMeta": true},
                "annotations": {
                    "audience": ["user", "assistant"],
                    "priority": 0.75,
                    "lastModified": "2026-06-07T12:34:56.123Z",
                },
            }),
            serde_json::json!({
                "type": "image",
                "data": "base64-image",
                "mimeType": "image/png",
                "_meta": {"imageMeta": 1},
            }),
            serde_json::json!({
                "type": "resource",
                "_meta": {"contentMeta": "outer"},
                "resource": {
                    "uri": "file:///example.txt",
                    "mimeType": "text/plain",
                    "text": "resource text",
                    "_meta": {"resourceMeta": "inner"},
                },
            }),
            serde_json::json!({
                "type": "audio",
                "data": "base64-audio",
                "mimeType": "audio/wav",
            }),
            serde_json::json!({
                "type": "resource_link",
                "uri": "https://example.com/report",
                "name": "report",
                "title": "Report",
                "description": "Quarterly report",
                "mimeType": "application/pdf",
                "size": 42,
                "icons": [{
                    "src": "https://example.com/icon.svg",
                    "mimeType": "image/svg+xml",
                    "sizes": ["any"],
                    "theme": "light",
                }],
                "_meta": {"linkMeta": true},
            }),
        ]
    );
}

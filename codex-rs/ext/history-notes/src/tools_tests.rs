use codex_http_client::HttpClientFactory;
use codex_http_client::OutboundProxyPolicy;
use codex_model_provider::create_model_provider;
use codex_model_provider_info::ModelProviderInfo;
use codex_protocol::models::FunctionCallOutputContentItem;
use codex_protocol::models::FunctionCallOutputPayload;
use codex_protocol::models::ImageDetail;
use codex_protocol::models::ImageReference;
use codex_protocol::models::ResponseInputItem;
use codex_tools::ToolOutput;
use codex_tools::ToolPayload;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::sync::Arc;

use super::HistoryNotesToolOutput;
use super::history_notes_tools;
use crate::backend::HttpBackend;
use crate::backend::HttpContext;

#[test]
fn preserves_encrypted_history_output() {
    let result = HistoryNotesToolOutput::new(json!({"encrypted_output": "enc_payload"}))
        .expect("valid output")
        .to_response_item(
            "call-1",
            &ToolPayload::Function {
                arguments: "{}".to_string(),
            },
        );

    let ResponseInputItem::FunctionCallOutput { output, .. } = result else {
        panic!("expected function-call output");
    };
    assert_eq!(
        output.content_items(),
        Some(
            [FunctionCallOutputContentItem::EncryptedContent {
                encrypted_content: "enc_payload".to_string(),
            }]
            .as_slice()
        )
    );
}

#[test]
fn preserves_images_as_separate_output_items_without_logging_bytes() {
    let output = HistoryNotesToolOutput::new(json!({
        "encrypted_output": "enc_payload",
        "images": [
            {"data": "cG5n", "mime_type": "image/png", "detail": "original"},
            {"data": "anBlZw==", "mime_type": "image/jpeg", "detail": "low"},
            {"data": "Z2lm", "mime_type": "image/gif"},
            {"data": "d2VicA==", "mime_type": "image/webp", "detail": null}
        ]
    }))
    .expect("valid image output");
    assert_eq!(
        output.log_output(),
        json!({"encrypted_output": "enc_payload"}).to_string()
    );
    assert_eq!(
        output.post_tool_use_response(
            "call-1",
            &ToolPayload::Function {
                arguments: "{}".to_string()
            }
        ),
        Some(json!({"encrypted_output": "enc_payload"}))
    );
    assert_eq!(
        output.to_response_item(
            "call-1",
            &ToolPayload::Function {
                arguments: "{}".to_string()
            }
        ),
        ResponseInputItem::FunctionCallOutput {
            call_id: "call-1".to_string(),
            output: FunctionCallOutputPayload::from_content_items(vec![
                FunctionCallOutputContentItem::EncryptedContent {
                    encrypted_content: "enc_payload".to_string()
                },
                FunctionCallOutputContentItem::InputImage {
                    image: ImageReference::Inline {
                        image_url: "data:image/png;base64,cG5n".to_string()
                    },
                    detail: Some(ImageDetail::Original)
                },
                FunctionCallOutputContentItem::InputImage {
                    image: ImageReference::Inline {
                        image_url: "data:image/jpeg;base64,anBlZw==".to_string()
                    },
                    detail: Some(ImageDetail::Low)
                },
                FunctionCallOutputContentItem::InputImage {
                    image: ImageReference::Inline {
                        image_url: "data:image/gif;base64,Z2lm".to_string()
                    },
                    detail: None
                },
                FunctionCallOutputContentItem::InputImage {
                    image: ImageReference::Inline {
                        image_url: "data:image/webp;base64,d2VicA==".to_string()
                    },
                    detail: None
                },
            ])
        }
    );
}

#[test]
fn accepts_empty_attachments_and_legacy_plaintext_results() {
    for (result, expected) in [
        (
            json!({"encrypted_output": "enc_payload", "images": []}),
            FunctionCallOutputPayload::from_content_items(vec![
                FunctionCallOutputContentItem::EncryptedContent {
                    encrypted_content: "enc_payload".to_string(),
                },
            ]),
        ),
        (
            json!({"text": "legacy result"}),
            FunctionCallOutputPayload::from_text(json!({"text": "legacy result"}).to_string()),
        ),
    ] {
        let output = HistoryNotesToolOutput::new(result).expect("valid output");
        assert_eq!(
            output.to_response_item(
                "call-1",
                &ToolPayload::Function {
                    arguments: "{}".to_string()
                }
            ),
            ResponseInputItem::FunctionCallOutput {
                call_id: "call-1".to_string(),
                output: expected
            }
        );
    }
}

#[test]
fn rejects_malformed_attachments_instead_of_silently_dropping_them() {
    for images in [
        json!(null),
        json!({}),
        json!([null]),
        json!([{"mime_type": "image/png"}]),
        json!([{"data": "private-image-bytes"}]),
        json!([{"data": "private-image-bytes", "mime_type": "image/png", "detail": "invalid"}]),
    ] {
        let result = HistoryNotesToolOutput::new(
            json!({"encrypted_output": "enc_payload", "images": images}),
        );
        let Err(codex_extension_api::FunctionCallError::RespondToModel(message)) = result else {
            panic!("expected a model-facing image error");
        };
        assert_eq!(message, "History backend returned invalid image content.");
    }
}

#[test]
fn sparse_catalog_messages_preserve_defaults_and_explicit_empty_descriptions() {
    let spec = super::tool_spec::<crate::requests::NotesReadFile, serde_json::Value>;
    let mut expected = spec(/*namespace_description*/ None, /*message*/ None);
    let codex_tools::ToolSpec::Namespace(namespace) = &mut expected else {
        panic!("expected namespace");
    };
    namespace.description.clear();
    let codex_tools::ResponsesApiNamespaceTool::Function(tool) = &mut namespace.tools[0] else {
        panic!("expected function");
    };
    tool.description.clear();
    let messages = codex_protocol::openai_models::NotesToolMessages {
        namespace_description: Some(String::new()),
        read_file: Some(codex_protocol::openai_models::ToolMessage {
            description: Some(String::new()),
            parameters: Some(String::new()),
        }),
        ..Default::default()
    };
    assert_eq!(
        spec(
            messages.namespace_description.as_deref(),
            messages.read_file.as_ref()
        ),
        expected
    );
}

#[test]
fn bundled_tool_contracts() {
    let backend = HttpBackend::new(
        create_model_provider(
            ModelProviderInfo::create_openai_provider(/*base_url*/ None),
            /*auth_manager*/ None,
        ),
        HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault),
        Arc::new(HttpContext {
            session_id: "session-123".to_owned(),
            current_agent_name: "/root".to_owned(),
        }),
    );
    let specs: Vec<_> = history_notes_tools(backend, /*history*/ None, /*notes*/ None)
        .iter()
        .map(|tool| tool.spec())
        .collect();
    insta::assert_snapshot!(serde_json::to_string_pretty(&specs).unwrap());
}

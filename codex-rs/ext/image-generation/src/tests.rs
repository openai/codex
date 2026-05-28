use codex_api::ImageBackground;
use codex_api::ImageEditRequest;
use codex_api::ImageGenerationRequest;
use codex_api::ImageQuality;
use codex_api::ImageUrl;
use codex_extension_api::ToolOutput;
use codex_extension_api::ToolPayload;
use codex_extension_api::ToolSpec;
use codex_protocol::models::ContentItem;
use codex_protocol::models::DEFAULT_IMAGE_DETAIL;
use codex_protocol::models::FunctionCallOutputBody;
use codex_protocol::models::FunctionCallOutputContentItem;
use codex_protocol::models::FunctionCallOutputPayload;
use codex_protocol::models::ResponseInputItem;
use codex_protocol::models::ResponseItem;
use codex_tools::ResponsesApiNamespaceTool;
use pretty_assertions::assert_eq;

use super::GeneratedImageOutput;
use super::ImageRequest;
use super::ImagegenAction;
use super::ImagegenArgs;
use super::imagegen_tool_spec;
use super::request_for_action;
use crate::IMAGE_GEN_NAMESPACE;
use crate::IMAGEGEN_TOOL_NAME;

const RESULT: &str = "cG5n";
const OUTPUT_HINT: &str = "Generated images are saved to /tmp as /tmp/call-1.png by default.";

#[test]
fn uses_reserved_image_gen_namespace() {
    let ToolSpec::Namespace(spec) = imagegen_tool_spec() else {
        panic!("imagegen should advertise a namespace tool");
    };
    assert_eq!(spec.name, IMAGE_GEN_NAMESPACE);
    let ResponsesApiNamespaceTool::Function(function) = &spec.tools[0];
    assert_eq!(function.name, IMAGEGEN_TOOL_NAME);
}

#[test]
fn generate_uses_fixed_request_defaults() {
    assert_eq!(
        request_for_action(&args(ImagegenAction::Generate, "paint a moonlit lake"), &[])
            .expect("generation request should build"),
        ImageRequest::Generate(ImageGenerationRequest {
            prompt: "paint a moonlit lake".to_string(),
            background: Some(ImageBackground::Auto),
            model: "gpt-image-2".to_string(),
            n: None,
            quality: Some(ImageQuality::Auto),
            size: Some("auto".to_string()),
        })
    );
}

#[test]
fn generated_output_returns_image_input_and_output_hint() {
    let output = GeneratedImageOutput {
        result: RESULT.to_string(),
        output_hint: Some(OUTPUT_HINT.to_string()),
    };

    let ResponseInputItem::FunctionCallOutput {
        output: response_output,
        ..
    } = output.to_response_item("call-1", &function_payload())
    else {
        panic!("imagegen should return function tool output");
    };
    let FunctionCallOutputBody::ContentItems(content_items) = response_output.body else {
        panic!("imagegen output should contain generated image bytes");
    };
    assert_eq!(
        content_items,
        vec![
            FunctionCallOutputContentItem::InputImage {
                image_url: format!("data:image/png;base64,{RESULT}"),
                detail: Some(DEFAULT_IMAGE_DETAIL),
            },
            FunctionCallOutputContentItem::InputText {
                text: OUTPUT_HINT.to_string(),
            },
        ]
    );
}

#[test]
fn edit_matches_context_selector_for_generated_images_after_latest_user_anchor() {
    let history = vec![
        generated_item("g1"),
        generated_item("g2"),
        generated_item("g3"),
        ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![
                ContentItem::InputImage {
                    image_url: "data:image/png;base64,u1".to_string(),
                    detail: None,
                },
                ContentItem::InputImage {
                    image_url: "data:image/png;base64,u2".to_string(),
                    detail: None,
                },
            ],
            phase: None,
        },
        generated_item("g4"),
        generated_item("g5"),
        generated_item("g6"),
        generated_item("g7"),
    ];

    assert_eq!(
        edit_request("change the lighting", &history),
        expected_edit_request(
            "change the lighting",
            &[
                "data:image/png;base64,u1",
                "data:image/png;base64,u2",
                "data:image/png;base64,g5",
                "data:image/png;base64,g6",
                "data:image/png;base64,g7",
            ]
        )
    );
}

#[test]
fn edit_preserves_a_generated_image_when_user_anchor_fills_the_limit() {
    let history = vec![
        ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: ["a", "b", "c", "d", "e"]
                .into_iter()
                .map(|image| ContentItem::InputImage {
                    image_url: format!("data:image/png;base64,{image}"),
                    detail: None,
                })
                .collect(),
            phase: None,
        },
        generated_item("generated"),
    ];

    assert_eq!(
        edit_request("edit the last generated image", &history),
        expected_edit_request(
            "edit the last generated image",
            &[
                "data:image/png;base64,b",
                "data:image/png;base64,c",
                "data:image/png;base64,d",
                "data:image/png;base64,e",
                "data:image/png;base64,generated",
            ]
        )
    );
}

#[test]
fn edit_uses_latest_user_upload_before_a_text_only_follow_up() {
    let history = vec![
        ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![ContentItem::InputImage {
                image_url: "data:image/png;base64,user".to_string(),
                detail: None,
            }],
            phase: None,
        },
        ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![ContentItem::InputText {
                text: "edit this image".to_string(),
            }],
            phase: None,
        },
    ];

    assert_eq!(
        edit_request("change the lighting", &history),
        expected_edit_request("change the lighting", &["data:image/png;base64,user"])
    );
}

#[test]
fn edit_reuses_images_from_prior_standalone_imagegen_calls() {
    let history = vec![
        ResponseItem::FunctionCall {
            id: None,
            name: IMAGEGEN_TOOL_NAME.to_string(),
            namespace: Some(IMAGE_GEN_NAMESPACE.to_string()),
            arguments: "{}".to_string(),
            call_id: "imagegen-1".to_string(),
        },
        generated_function_output("imagegen-1", "standalone"),
    ];

    assert_eq!(
        edit_request("change the lighting", &history),
        expected_edit_request("change the lighting", &["data:image/png;base64,standalone"])
    );
}

#[test]
fn edit_keeps_newest_standalone_generated_images_when_over_limit() {
    let history = (1..=6)
        .flat_map(|index| {
            let call_id = format!("imagegen-{index}");
            vec![
                ResponseItem::FunctionCall {
                    id: None,
                    name: IMAGEGEN_TOOL_NAME.to_string(),
                    namespace: Some(IMAGE_GEN_NAMESPACE.to_string()),
                    arguments: "{}".to_string(),
                    call_id: call_id.clone(),
                },
                generated_function_output(&call_id, &index.to_string()),
            ]
        })
        .collect::<Vec<_>>();

    assert_eq!(
        edit_request("change the lighting", &history),
        expected_edit_request(
            "change the lighting",
            &[
                "data:image/png;base64,2",
                "data:image/png;base64,3",
                "data:image/png;base64,4",
                "data:image/png;base64,5",
                "data:image/png;base64,6",
            ]
        )
    );
}

#[test]
fn edit_without_image_history_returns_tool_error() {
    let error = request_for_action(&args(ImagegenAction::Edit, "change the lighting"), &[])
        .expect_err("edit should require image context");

    assert_eq!(
        error.to_string(),
        "image edit requested without any usable image in conversation history"
    );
}

fn args(action: ImagegenAction, prompt: &str) -> ImagegenArgs {
    ImagegenArgs {
        prompt: prompt.to_string(),
        action,
    }
}

fn edit_request(prompt: &str, history: &[ResponseItem]) -> ImageEditRequest {
    let ImageRequest::Edit(request) =
        request_for_action(&args(ImagegenAction::Edit, prompt), history)
            .expect("edit request should build")
    else {
        panic!("expected edit request");
    };
    request
}

fn expected_edit_request(prompt: &str, images: &[&str]) -> ImageEditRequest {
    ImageEditRequest {
        images: images
            .iter()
            .map(|image_url| ImageUrl {
                image_url: (*image_url).to_string(),
            })
            .collect(),
        prompt: prompt.to_string(),
        background: Some(ImageBackground::Auto),
        model: "gpt-image-2".to_string(),
        n: None,
        quality: Some(ImageQuality::Auto),
        size: Some("auto".to_string()),
    }
}

fn generated_item(result: &str) -> ResponseItem {
    ResponseItem::ImageGenerationCall {
        id: format!("id-{result}"),
        status: "completed".to_string(),
        revised_prompt: None,
        result: result.to_string(),
    }
}

fn generated_function_output(call_id: &str, result: &str) -> ResponseItem {
    ResponseItem::FunctionCallOutput {
        call_id: call_id.to_string(),
        output: FunctionCallOutputPayload {
            body: FunctionCallOutputBody::ContentItems(vec![
                FunctionCallOutputContentItem::InputImage {
                    image_url: format!("data:image/png;base64,{result}"),
                    detail: Some(DEFAULT_IMAGE_DETAIL),
                },
                FunctionCallOutputContentItem::InputText {
                    text: "generated image save hint".to_string(),
                },
            ]),
            success: Some(true),
        },
    }
}

fn function_payload() -> ToolPayload {
    ToolPayload::Function {
        arguments: "{}".to_string(),
    }
}

use codex_api::ImageBackground;
use codex_api::ImageEditRequest;
use codex_api::ImageGenerationRequest;
use codex_api::ImageQuality;
use codex_api::ImageUrl;
use codex_core::context::extension_image_generation_output_hint;
use codex_extension_api::ToolOutput;
use codex_extension_api::ToolPayload;
use codex_extension_api::ToolSpec;
use codex_protocol::models::DEFAULT_IMAGE_DETAIL;
use codex_protocol::models::FunctionCallOutputBody;
use codex_protocol::models::FunctionCallOutputContentItem;
use codex_protocol::models::ResponseInputItem;
use codex_tools::ResponsesApiNamespaceTool;
use pretty_assertions::assert_eq;

use super::GeneratedImageOutput;
use super::ImageRequest;
use super::ImagegenArgs;
use super::imagegen_tool_spec;
use super::request_for_args;
use crate::IMAGE_GEN_NAMESPACE;
use crate::IMAGEGEN_TOOL_NAME;

const RESULT: &str = "cG5n";
const TINY_PNG_DATA_URL: &str = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAAC0lEQVR4nGNgAAIAAAUAAXpeqz8AAAAASUVORK5CYII=";
const TINY_PNG_BYTES: &[u8] = &[
    137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1, 0, 0, 0, 1, 8, 6, 0,
    0, 0, 31, 21, 196, 137, 0, 0, 0, 11, 73, 68, 65, 84, 120, 156, 99, 96, 0, 2, 0, 0, 5, 0, 1,
    122, 94, 171, 63, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96, 130,
];

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
fn omitted_or_empty_references_generate_with_fixed_defaults() {
    for referenced_image_paths in [None, Some(Vec::new())] {
        assert_eq!(
            request_for_args(&ImagegenArgs {
                prompt: "paint a moonlit lake".to_string(),
                referenced_image_paths,
            })
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
}

#[test]
fn referenced_paths_build_edit_request() {
    let path = std::env::temp_dir().join(format!(
        "codex-imagegen-reference-test-{}.png",
        std::process::id()
    ));
    std::fs::write(&path, TINY_PNG_BYTES).expect("test image should be written");

    let request = request_for_args(&ImagegenArgs {
        prompt: "change the lighting".to_string(),
        referenced_image_paths: Some(vec![path.display().to_string()]),
    });
    std::fs::remove_file(path).expect("test image should be removed");

    assert_eq!(
        request.expect("edit request should build"),
        ImageRequest::Edit(ImageEditRequest {
            images: vec![ImageUrl {
                image_url: TINY_PNG_DATA_URL.to_string(),
            }],
            prompt: "change the lighting".to_string(),
            background: Some(ImageBackground::Auto),
            model: "gpt-image-2".to_string(),
            n: None,
            quality: Some(ImageQuality::Auto),
            size: Some("auto".to_string()),
        })
    );
}

#[test]
fn unreadable_referenced_path_returns_tool_error() {
    let path = std::env::temp_dir().join(format!(
        "codex-imagegen-missing-test-{}.png",
        std::process::id()
    ));
    let error = request_for_args(&ImagegenArgs {
        prompt: "change the lighting".to_string(),
        referenced_image_paths: Some(vec![path.display().to_string()]),
    })
    .expect_err("missing reference should fail");

    assert!(error.to_string().starts_with(&format!(
        "unable to read referenced image at `{}`:",
        path.display()
    )));
}

#[test]
fn generated_output_returns_image_input_and_output_hint() {
    let output_hint =
        extension_image_generation_output_hint("/tmp", "/tmp/call-1.png").expect("hint should fit");
    let output = GeneratedImageOutput {
        result: RESULT.to_string(),
        output_hint: Some(output_hint.clone()),
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
            FunctionCallOutputContentItem::InputText { text: output_hint },
        ]
    );
}

#[test]
fn generated_output_returns_generated_image_helper_input_in_code_mode() {
    let output = GeneratedImageOutput {
        result: RESULT.to_string(),
        output_hint: Some("generated image save hint".to_string()),
    };

    assert_eq!(
        output.code_mode_result(&function_payload()),
        serde_json::json!({
            "image_url": format!("data:image/png;base64,{RESULT}"),
            "output_hint": "generated image save hint",
        })
    );
}

#[test]
fn generated_output_omits_oversized_output_hint() {
    let long_path = "x".repeat(1024);
    let output = GeneratedImageOutput {
        result: RESULT.to_string(),
        output_hint: extension_image_generation_output_hint("/tmp", long_path),
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
        vec![FunctionCallOutputContentItem::InputImage {
            image_url: format!("data:image/png;base64,{RESULT}"),
            detail: Some(DEFAULT_IMAGE_DETAIL),
        }]
    );
}

fn function_payload() -> ToolPayload {
    ToolPayload::Function {
        arguments: "{}".to_string(),
    }
}

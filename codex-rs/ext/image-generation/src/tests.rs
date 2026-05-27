use std::sync::Arc;
use std::sync::Mutex;

use codex_api::ImageBackground;
use codex_api::ImageData;
use codex_api::ImageEditRequest;
use codex_api::ImageGenerationRequest;
use codex_api::ImageQuality;
use codex_api::ImageResponse;
use codex_api::ImageUrl;
use codex_extension_api::ConversationHistory;
use codex_extension_api::ToolCall;
use codex_extension_api::ToolExecutor;
use codex_extension_api::ToolName;
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
use codex_tools::ToolExposure;
use codex_utils_output_truncation::TruncationPolicy;
use pretty_assertions::assert_eq;
use serde_json::json;

use crate::IMAGE_GEN_NAMESPACE;
use crate::IMAGEGEN_TOOL_NAME;
use crate::backend::ImageGenerationBackend;
use crate::tool::ImageGenerationTool;
use crate::tool::generated_image_output_dir;

const RESULT: &str = "cG5n";

#[derive(Clone)]
struct CapturingBackend {
    generated: Arc<Mutex<Vec<ImageGenerationRequest>>>,
    edited: Arc<Mutex<Vec<ImageEditRequest>>>,
}

impl CapturingBackend {
    fn new() -> Self {
        Self {
            generated: Arc::new(Mutex::new(Vec::new())),
            edited: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn response() -> ImageResponse {
        ImageResponse {
            created: 1,
            data: vec![ImageData {
                b64_json: RESULT.to_string(),
            }],
            background: None,
            quality: None,
            size: None,
        }
    }
}

impl ImageGenerationBackend for CapturingBackend {
    async fn generate(&self, request: ImageGenerationRequest) -> Result<ImageResponse, String> {
        self.generated.lock().expect("generated lock").push(request);
        Ok(Self::response())
    }

    async fn edit(&self, request: ImageEditRequest) -> Result<ImageResponse, String> {
        self.edited.lock().expect("edited lock").push(request);
        Ok(Self::response())
    }
}

#[test]
fn uses_reserved_image_gen_namespace() {
    let (tool, _tempdir) = test_tool(CapturingBackend::new());

    assert_eq!(
        tool.tool_name(),
        ToolName::namespaced("image_gen", "imagegen")
    );
    assert_eq!(tool.exposure(), ToolExposure::DirectModelOnly);
    let ToolSpec::Namespace(spec) = tool.spec() else {
        panic!("imagegen should advertise a namespace tool");
    };
    assert_eq!(spec.name, "image_gen");
    let ResponsesApiNamespaceTool::Function(function) = &spec.tools[0];
    assert_eq!(function.name, "imagegen");
    assert!(function.description.contains("Set `action` to `generate`"));
    assert!(function.description.contains("previously generated image"));
}

#[tokio::test]
async fn generate_uses_defaults_and_returns_image_input_to_the_model() {
    let backend = CapturingBackend::new();
    let (tool, tempdir) = test_tool(backend.clone());

    let output = tool
        .handle(tool_call(
            json!({"prompt": "paint a moonlit lake", "action": "generate"}),
            Vec::new(),
        ))
        .await
        .expect("generation should succeed");

    assert_eq!(
        backend.generated.lock().expect("generated lock").as_slice(),
        &[ImageGenerationRequest {
            prompt: "paint a moonlit lake".to_string(),
            background: Some(ImageBackground::Auto),
            model: "gpt-image-2".to_string(),
            n: None,
            quality: Some(ImageQuality::Auto),
            size: Some("auto".to_string()),
        }]
    );
    let ResponseInputItem::FunctionCallOutput { output, .. } =
        output.to_response_item("call-1", &function_payload(json!({})))
    else {
        panic!("imagegen should return function tool output");
    };
    let FunctionCallOutputBody::ContentItems(output) = output.body else {
        panic!("imagegen output should contain generated image bytes");
    };
    assert_eq!(
        output,
        vec![
            FunctionCallOutputContentItem::InputImage {
                image_url: format!("data:image/png;base64,{RESULT}"),
                detail: Some(DEFAULT_IMAGE_DETAIL),
            },
            FunctionCallOutputContentItem::InputText {
                text: format!(
                    "Generated images are saved to {} as {} by default.\n\
                     If you need to use a generated image at another path, copy it and leave the original in place unless the user explicitly asks you to delete it.",
                    tempdir.path().display(),
                    tempdir.path().join("_image_id_.png").display(),
                ),
            },
        ]
    );
    assert_eq!(
        std::fs::read(tempdir.path().join("call-1.png")).expect("saved generated image"),
        b"png"
    );
}

#[tokio::test]
async fn edit_prefers_images_from_latest_user_message_then_generated_images_after_it() {
    let backend = CapturingBackend::new();
    let (tool, _tempdir) = test_tool(backend.clone());
    let history = vec![
        generated_item("old"),
        ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![ContentItem::InputImage {
                image_url: "data:image/png;base64,user".to_string(),
                detail: None,
            }],
            phase: None,
        },
        generated_item("new"),
    ];

    tool.handle(tool_call(
        json!({"prompt": "change the lighting", "action": "edit"}),
        history,
    ))
    .await
    .expect("edit should succeed");

    assert_eq!(
        backend.edited.lock().expect("edited lock").as_slice(),
        &[ImageEditRequest {
            images: vec![
                ImageUrl {
                    image_url: "data:image/png;base64,user".to_string(),
                },
                ImageUrl {
                    image_url: "data:image/png;base64,new".to_string(),
                },
            ],
            prompt: "change the lighting".to_string(),
            background: Some(ImageBackground::Auto),
            model: "gpt-image-2".to_string(),
            n: None,
            quality: Some(ImageQuality::Auto),
            size: Some("auto".to_string()),
        }]
    );
}

#[tokio::test]
async fn edit_anchors_on_latest_user_image_turn_then_takes_newest_generated_images() {
    let backend = CapturingBackend::new();
    let (tool, _tempdir) = test_tool(backend.clone());
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

    tool.handle(tool_call(
        json!({"prompt": "change the lighting", "action": "edit"}),
        history,
    ))
    .await
    .expect("edit should succeed");

    assert_eq!(
        backend.edited.lock().expect("edited lock").as_slice(),
        &[ImageEditRequest {
            images: vec![
                ImageUrl {
                    image_url: "data:image/png;base64,u1".to_string(),
                },
                ImageUrl {
                    image_url: "data:image/png;base64,u2".to_string(),
                },
                ImageUrl {
                    image_url: "data:image/png;base64,g7".to_string(),
                },
                ImageUrl {
                    image_url: "data:image/png;base64,g6".to_string(),
                },
                ImageUrl {
                    image_url: "data:image/png;base64,g5".to_string(),
                },
            ],
            prompt: "change the lighting".to_string(),
            background: Some(ImageBackground::Auto),
            model: "gpt-image-2".to_string(),
            n: None,
            quality: Some(ImageQuality::Auto),
            size: Some("auto".to_string()),
        }]
    );
}

#[tokio::test]
async fn edit_uses_latest_user_upload_before_a_text_only_follow_up() {
    let backend = CapturingBackend::new();
    let (tool, _tempdir) = test_tool(backend.clone());
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

    tool.handle(tool_call(
        json!({"prompt": "change the lighting", "action": "edit"}),
        history,
    ))
    .await
    .expect("edit should succeed");

    assert_eq!(
        backend.edited.lock().expect("edited lock").as_slice(),
        &[ImageEditRequest {
            images: vec![ImageUrl {
                image_url: "data:image/png;base64,user".to_string(),
            }],
            prompt: "change the lighting".to_string(),
            background: Some(ImageBackground::Auto),
            model: "gpt-image-2".to_string(),
            n: None,
            quality: Some(ImageQuality::Auto),
            size: Some("auto".to_string()),
        }]
    );
}

#[tokio::test]
async fn edit_reuses_images_from_prior_standalone_imagegen_calls() {
    let backend = CapturingBackend::new();
    let (tool, _tempdir) = test_tool(backend.clone());
    let history = vec![
        ResponseItem::FunctionCall {
            id: None,
            name: "view_image".to_string(),
            namespace: None,
            arguments: "{}".to_string(),
            call_id: "view-1".to_string(),
        },
        generated_function_output("view-1", "viewed"),
        ResponseItem::FunctionCall {
            id: None,
            name: IMAGEGEN_TOOL_NAME.to_string(),
            namespace: Some(IMAGE_GEN_NAMESPACE.to_string()),
            arguments: "{}".to_string(),
            call_id: "imagegen-1".to_string(),
        },
        generated_function_output("imagegen-1", "standalone"),
    ];

    tool.handle(tool_call(
        json!({"prompt": "change the lighting", "action": "edit"}),
        history,
    ))
    .await
    .expect("edit should succeed");

    assert_eq!(
        backend.edited.lock().expect("edited lock").as_slice(),
        &[ImageEditRequest {
            images: vec![ImageUrl {
                image_url: "data:image/png;base64,standalone".to_string(),
            }],
            prompt: "change the lighting".to_string(),
            background: Some(ImageBackground::Auto),
            model: "gpt-image-2".to_string(),
            n: None,
            quality: Some(ImageQuality::Auto),
            size: Some("auto".to_string()),
        }]
    );
}

#[tokio::test]
async fn edit_keeps_newest_standalone_generated_images_when_over_limit() {
    let backend = CapturingBackend::new();
    let (tool, _tempdir) = test_tool(backend.clone());
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
        .collect();

    tool.handle(tool_call(
        json!({"prompt": "change the lighting", "action": "edit"}),
        history,
    ))
    .await
    .expect("edit should succeed");

    assert_eq!(
        backend.edited.lock().expect("edited lock").as_slice(),
        &[ImageEditRequest {
            images: vec![
                ImageUrl {
                    image_url: "data:image/png;base64,6".to_string(),
                },
                ImageUrl {
                    image_url: "data:image/png;base64,5".to_string(),
                },
                ImageUrl {
                    image_url: "data:image/png;base64,4".to_string(),
                },
                ImageUrl {
                    image_url: "data:image/png;base64,3".to_string(),
                },
                ImageUrl {
                    image_url: "data:image/png;base64,2".to_string(),
                },
            ],
            prompt: "change the lighting".to_string(),
            background: Some(ImageBackground::Auto),
            model: "gpt-image-2".to_string(),
            n: None,
            quality: Some(ImageQuality::Auto),
            size: Some("auto".to_string()),
        }]
    );
}

#[tokio::test]
async fn edit_without_image_history_returns_tool_error() {
    let (tool, _tempdir) = test_tool(CapturingBackend::new());

    let error = match tool
        .handle(tool_call(
            json!({"prompt": "change the lighting", "action": "edit"}),
            Vec::new(),
        ))
        .await
    {
        Ok(_) => panic!("edit should require image context"),
        Err(error) => error,
    };

    assert_eq!(
        error.to_string(),
        "image edit requested without any usable image in conversation history"
    );
}

#[test]
fn generated_image_output_dir_is_scoped_to_sanitized_thread_id() {
    assert_eq!(
        generated_image_output_dir(std::path::Path::new("/tmp/codex-home"), "thread/1"),
        std::path::PathBuf::from("/tmp/codex-home/generated_images/thread_1")
    );
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

fn test_tool(
    backend: CapturingBackend,
) -> (ImageGenerationTool<CapturingBackend>, tempfile::TempDir) {
    let tempdir = tempfile::tempdir().expect("tempdir");
    let tool = ImageGenerationTool::new(backend, tempdir.path().to_path_buf());
    (tool, tempdir)
}

fn tool_call(arguments: serde_json::Value, history: Vec<ResponseItem>) -> ToolCall {
    ToolCall {
        turn_id: "turn-1".to_string(),
        call_id: "call-1".to_string(),
        tool_name: ToolName::namespaced(IMAGE_GEN_NAMESPACE, IMAGEGEN_TOOL_NAME),
        truncation_policy: TruncationPolicy::Bytes(1024),
        conversation_history: ConversationHistory::new(history),
        payload: function_payload(arguments),
    }
}

fn function_payload(arguments: serde_json::Value) -> ToolPayload {
    ToolPayload::Function {
        arguments: arguments.to_string(),
    }
}

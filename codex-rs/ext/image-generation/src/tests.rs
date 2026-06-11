use async_trait::async_trait;
use codex_api::ImageBackground;
use codex_api::ImageEditRequest;
use codex_api::ImageGenerationRequest;
use codex_api::ImageQuality;
use codex_api::ImageUrl;
use codex_core::context::extension_image_generation_output_hint;
use codex_extension_api::ToolEnvironment;
use codex_extension_api::ToolOutput;
use codex_extension_api::ToolPayload;
use codex_extension_api::ToolSpec;
use codex_file_system::CopyOptions;
use codex_file_system::CreateDirectoryOptions;
use codex_file_system::ExecutorFileSystem;
use codex_file_system::FileMetadata;
use codex_file_system::FileSystemResult;
use codex_file_system::FileSystemSandboxContext;
use codex_file_system::ReadDirectoryEntry;
use codex_file_system::RemoveOptions;
use codex_protocol::models::ContentItem;
use codex_protocol::models::DEFAULT_IMAGE_DETAIL;
use codex_protocol::models::FunctionCallOutputBody;
use codex_protocol::models::FunctionCallOutputContentItem;
use codex_protocol::models::FunctionCallOutputPayload;
use codex_protocol::models::PermissionProfile;
use codex_protocol::models::ResponseInputItem;
use codex_protocol::models::ResponseItem;
use codex_tools::ResponsesApiNamespaceTool;
use codex_utils_absolute_path::AbsolutePathBuf;
use pretty_assertions::assert_eq;
use std::io;
use std::path::Path as StdPath;
use std::sync::Arc;
use std::sync::Mutex;

use super::GeneratedImageOutput;
use super::ImageRequest;
use super::ImagegenArgs;
use super::imagegen_tool_spec;
use super::request_for_call_args;
use crate::IMAGE_GEN_NAMESPACE;
use crate::IMAGEGEN_TOOL_NAME;

const RESULT: &str = "cG5n";
const TINY_PNG_BYTES: &[u8] = &[
    137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1, 0, 0, 0, 1, 8, 6, 0,
    0, 0, 31, 21, 196, 137, 0, 0, 0, 11, 73, 68, 65, 84, 120, 156, 99, 96, 0, 2, 0, 0, 5, 0, 1,
    122, 94, 171, 63, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96, 130,
];
const TINY_PNG_DATA_URL: &str = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAAC0lEQVR4nGNgAAIAAAUAAXpeqz8AAAAASUVORK5CYII=";

#[test]
fn uses_reserved_image_gen_namespace() {
    let ToolSpec::Namespace(spec) = imagegen_tool_spec() else {
        panic!("imagegen should advertise a namespace tool");
    };
    assert_eq!(spec.name, IMAGE_GEN_NAMESPACE);
    let ResponsesApiNamespaceTool::Function(function) = &spec.tools[0];
    assert_eq!(function.name, IMAGEGEN_TOOL_NAME);
}

#[tokio::test]
async fn omitted_references_generate_with_fixed_defaults() {
    assert_eq!(
        request_for_call_args(
            &ImagegenArgs {
                prompt: "paint a moonlit lake".to_string(),
                referenced_image_paths: None,
                num_last_images_to_include: None,
            },
            &[],
            /*environments*/ None,
        )
        .await
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

#[tokio::test]
async fn recent_image_fallback_selects_newest_images_in_chronological_order() {
    let history = vec![
        ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![
                input_image("user-1"),
                input_image("user-2"),
                ContentItem::InputText {
                    text: "edit these".to_string(),
                },
            ],
            phase: None,
        },
        ResponseItem::FunctionCall {
            id: None,
            name: "mcp_image".to_string(),
            namespace: None,
            arguments: "{}".to_string(),
            call_id: "mcp-call".to_string(),
        },
        ResponseItem::FunctionCallOutput {
            call_id: "mcp-call".to_string(),
            output: image_output("mcp"),
        },
        ResponseItem::CustomToolCall {
            id: None,
            status: Some("completed".to_string()),
            call_id: "code-mode-call".to_string(),
            name: "exec".to_string(),
            input: String::new(),
        },
        ResponseItem::CustomToolCallOutput {
            call_id: "code-mode-call".to_string(),
            name: Some("exec".to_string()),
            output: image_output("code-mode"),
        },
        ResponseItem::ImageGenerationCall {
            id: "generated-call".to_string(),
            status: "completed".to_string(),
            revised_prompt: None,
            result: "generated".to_string(),
        },
        ResponseItem::FunctionCallOutput {
            call_id: "orphan-call".to_string(),
            output: image_output("orphan"),
        },
    ];

    assert_eq!(
        request_for_call_args(
            &ImagegenArgs {
                prompt: "change the lighting".to_string(),
                referenced_image_paths: None,
                num_last_images_to_include: Some(4),
            },
            &history,
            /*environments*/ None,
        )
        .await
        .expect("history-backed edit request should build"),
        ImageRequest::Edit(expected_edit_request(
            "change the lighting",
            &["user-2", "mcp", "code-mode", "generated"],
        ))
    );
}

#[tokio::test]
async fn conflicting_image_selectors_return_tool_error() {
    let error = request_for_call_args(
        &ImagegenArgs {
            prompt: "change the lighting".to_string(),
            referenced_image_paths: Some(vec![
                "/tmp/image.png"
                    .try_into()
                    .expect("test path should be absolute"),
            ]),
            num_last_images_to_include: Some(1),
        },
        &[],
        /*environments*/ None,
    )
    .await
    .expect_err("conflicting selectors should fail");

    assert_eq!(
        error.to_string(),
        "provide only one of `referenced_image_paths` or `num_last_images_to_include`"
    );
}

#[tokio::test]
async fn too_many_referenced_image_paths_return_tool_error() {
    let error = request_for_call_args(
        &ImagegenArgs {
            prompt: "change the lighting".to_string(),
            referenced_image_paths: Some(
                (0..6)
                    .map(|index| {
                        format!("/tmp/image-{index}.png")
                            .try_into()
                            .expect("test path should be absolute")
                    })
                    .collect(),
            ),
            num_last_images_to_include: None,
        },
        &[],
        /*environments*/ None,
    )
    .await
    .expect_err("too many paths should fail before reading files");

    assert_eq!(
        error.to_string(),
        "`referenced_image_paths` must contain at most 5 paths"
    );
}

#[tokio::test]
async fn recent_image_fallback_requires_requested_count() {
    let error = request_for_call_args(
        &ImagegenArgs {
            prompt: "change the lighting".to_string(),
            referenced_image_paths: None,
            num_last_images_to_include: Some(2),
        },
        &[ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![input_image("only-image")],
            phase: None,
        }],
        /*environments*/ None,
    )
    .await
    .expect_err("history-backed edit should require the requested image count");

    assert_eq!(
        error.to_string(),
        "requested the last 2 conversation images, but only 1 were available"
    );
}

#[tokio::test]
async fn referenced_image_uses_environment_file_system_and_sandbox() {
    let path = absolute_path("/virtual/image.png");
    let sandbox = FileSystemSandboxContext::from_permission_profile(PermissionProfile::read_only());
    let file_system = Arc::new(RecordingFileSystem::success(TINY_PNG_BYTES));
    let environment = tool_environment(file_system.clone(), sandbox.clone());

    assert_eq!(
        request_for_call_args(
            &ImagegenArgs {
                prompt: "change the lighting".to_string(),
                referenced_image_paths: Some(vec![path.clone()]),
                num_last_images_to_include: None,
            },
            &[],
            Some(&[environment]),
        )
        .await
        .expect("referenced image request should build"),
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
    assert_eq!(file_system.reads(), vec![(path, sandbox)]);
}

#[tokio::test]
async fn referenced_image_read_failure_returns_tool_error() {
    let path = absolute_path("/virtual/denied.png");
    let sandbox = FileSystemSandboxContext::from_permission_profile(PermissionProfile::read_only());
    let file_system = Arc::new(RecordingFileSystem::denied());
    let environment = tool_environment(file_system.clone(), sandbox.clone());

    let error = request_for_call_args(
        &ImagegenArgs {
            prompt: "change the lighting".to_string(),
            referenced_image_paths: Some(vec![path.clone()]),
            num_last_images_to_include: None,
        },
        &[],
        Some(&[environment]),
    )
    .await
    .expect_err("denied image read should fail");

    assert_eq!(
        error.to_string(),
        "unable to read referenced image at `/virtual/denied.png`: denied"
    );
    assert_eq!(file_system.reads(), vec![(path, sandbox)]);
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

fn input_image(image: &str) -> ContentItem {
    ContentItem::InputImage {
        image_url: format!("data:image/png;base64,{image}"),
        detail: None,
    }
}

fn image_output(image: &str) -> FunctionCallOutputPayload {
    FunctionCallOutputPayload::from_content_items(vec![FunctionCallOutputContentItem::InputImage {
        image_url: format!("data:image/png;base64,{image}"),
        detail: None,
    }])
}

fn expected_edit_request(prompt: &str, images: &[&str]) -> ImageEditRequest {
    ImageEditRequest {
        images: images
            .iter()
            .map(|image| ImageUrl {
                image_url: format!("data:image/png;base64,{image}"),
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

fn function_payload() -> ToolPayload {
    ToolPayload::Function {
        arguments: "{}".to_string(),
    }
}

struct RecordingFileSystem {
    contents: Option<Vec<u8>>,
    reads: Mutex<Vec<(AbsolutePathBuf, FileSystemSandboxContext)>>,
}

impl RecordingFileSystem {
    fn success(contents: &[u8]) -> Self {
        Self {
            contents: Some(contents.to_vec()),
            reads: Mutex::new(Vec::new()),
        }
    }

    fn denied() -> Self {
        Self {
            contents: None,
            reads: Mutex::new(Vec::new()),
        }
    }

    fn reads(&self) -> Vec<(AbsolutePathBuf, FileSystemSandboxContext)> {
        self.reads.lock().expect("reads mutex poisoned").clone()
    }
}

#[async_trait]
impl ExecutorFileSystem for RecordingFileSystem {
    async fn canonicalize(
        &self,
        _path: &AbsolutePathBuf,
        _sandbox: Option<&FileSystemSandboxContext>,
    ) -> FileSystemResult<AbsolutePathBuf> {
        unimplemented!("test filesystem only supports reads")
    }

    async fn join(
        &self,
        _base_path: &AbsolutePathBuf,
        _path: &StdPath,
    ) -> FileSystemResult<AbsolutePathBuf> {
        unimplemented!("test filesystem only supports reads")
    }

    async fn parent(&self, _path: &AbsolutePathBuf) -> FileSystemResult<Option<AbsolutePathBuf>> {
        unimplemented!("test filesystem only supports reads")
    }

    async fn read_file(
        &self,
        path: &AbsolutePathBuf,
        sandbox: Option<&FileSystemSandboxContext>,
    ) -> FileSystemResult<Vec<u8>> {
        self.reads.lock().expect("reads mutex poisoned").push((
            path.clone(),
            sandbox.expect("sandbox context required").clone(),
        ));
        self.contents
            .clone()
            .ok_or_else(|| io::Error::new(io::ErrorKind::PermissionDenied, "denied"))
    }

    async fn write_file(
        &self,
        _path: &AbsolutePathBuf,
        _contents: Vec<u8>,
        _sandbox: Option<&FileSystemSandboxContext>,
    ) -> FileSystemResult<()> {
        unimplemented!("test filesystem only supports reads")
    }

    async fn create_directory(
        &self,
        _path: &AbsolutePathBuf,
        _options: CreateDirectoryOptions,
        _sandbox: Option<&FileSystemSandboxContext>,
    ) -> FileSystemResult<()> {
        unimplemented!("test filesystem only supports reads")
    }

    async fn get_metadata(
        &self,
        _path: &AbsolutePathBuf,
        _sandbox: Option<&FileSystemSandboxContext>,
    ) -> FileSystemResult<FileMetadata> {
        unimplemented!("test filesystem only supports reads")
    }

    async fn read_directory(
        &self,
        _path: &AbsolutePathBuf,
        _sandbox: Option<&FileSystemSandboxContext>,
    ) -> FileSystemResult<Vec<ReadDirectoryEntry>> {
        unimplemented!("test filesystem only supports reads")
    }

    async fn remove(
        &self,
        _path: &AbsolutePathBuf,
        _options: RemoveOptions,
        _sandbox: Option<&FileSystemSandboxContext>,
    ) -> FileSystemResult<()> {
        unimplemented!("test filesystem only supports reads")
    }

    async fn copy(
        &self,
        _source_path: &AbsolutePathBuf,
        _destination_path: &AbsolutePathBuf,
        _options: CopyOptions,
        _sandbox: Option<&FileSystemSandboxContext>,
    ) -> FileSystemResult<()> {
        unimplemented!("test filesystem only supports reads")
    }
}

fn absolute_path(path: &str) -> AbsolutePathBuf {
    path.try_into().expect("test path should be absolute")
}

fn tool_environment(
    file_system: Arc<dyn ExecutorFileSystem>,
    file_system_sandbox_context: FileSystemSandboxContext,
) -> ToolEnvironment {
    ToolEnvironment {
        environment_id: "test".to_string(),
        cwd: absolute_path("/virtual"),
        file_system,
        file_system_sandbox_context,
    }
}

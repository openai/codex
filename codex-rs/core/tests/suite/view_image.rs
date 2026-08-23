#![cfg(not(target_os = "windows"))]

use anyhow::Context;
use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use codex_core::TurnInputRequest;
use codex_exec_server::CreateDirectoryOptions;
use codex_exec_server::LOCAL_ENVIRONMENT_ID;
use codex_exec_server::REMOTE_ENVIRONMENT_ID;
use codex_exec_server::RemoveOptions;
use codex_features::Feature;
use codex_login::CodexAuth;
use codex_protocol::config_types::CollaborationMode;
use codex_protocol::config_types::ModeKind;
use codex_protocol::config_types::ReasoningSummary;
use codex_protocol::config_types::Settings;
use codex_protocol::models::PermissionProfile;
use codex_protocol::openai_models::ConfigShellToolType;
use codex_protocol::openai_models::InputModality;
use codex_protocol::openai_models::ModelInfo;
use codex_protocol::openai_models::ModelVisibility;
use codex_protocol::openai_models::ModelsResponse;
use codex_protocol::openai_models::ReasoningEffort;
use codex_protocol::openai_models::ReasoningEffortPreset;
use codex_protocol::openai_models::TruncationPolicyConfig;
use codex_protocol::permissions::FileSystemAccessMode;
use codex_protocol::permissions::FileSystemPath;
use codex_protocol::permissions::FileSystemSandboxEntry;
use codex_protocol::permissions::FileSystemSandboxPolicy;
use codex_protocol::permissions::NetworkSandboxPolicy;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::EnvironmentConfigState;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::ThreadSettingsOverrides;
use codex_protocol::protocol::TurnEnvironmentSelection;
use codex_protocol::user_input::UserInput;
use codex_utils_path_uri::PathUri;
use core_test_support::PathExt;
use core_test_support::is_remote_test_environment;
use core_test_support::responses;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_function_call;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_models_once;
use core_test_support::responses::mount_sse_sequence;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::skip_if_no_remote_env;
use core_test_support::test_codex::TestCodex;
use core_test_support::test_codex::local;
use core_test_support::test_codex::test_codex;
use core_test_support::test_codex::turn_permission_fields;
use core_test_support::test_target_os;
use core_test_support::wait_for_event_with_timeout;
use image::DynamicImage;
use image::GenericImageView;
use image::ImageBuffer;
use image::Rgba;
use image::load_from_memory;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use std::fs;
use std::io::Cursor;
use std::path::PathBuf;
use tempfile::TempDir;
use tokio::time::Duration;
use wiremock::BodyPrintLimit;
use wiremock::MockServer;

const VIEW_IMAGE_TURN_COMPLETE_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Clone, Copy)]
enum ResizeNoticeExpectation {
    Disabled,
    Enabled,
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum ImageBudgetPolicy {
    DetailBased,
    Unified,
    UnifiedResponsesLiteWithoutOriginalSupport,
}

fn disabled_user_turn(test: &TestCodex, items: Vec<UserInput>, model: String) -> TurnInputRequest {
    let (sandbox_policy, permission_profile) =
        turn_permission_fields(PermissionProfile::Disabled, test.config.cwd.as_path());
    TurnInputRequest::user_input(items).with_thread_settings(ThreadSettingsOverrides {
        approval_policy: Some(AskForApproval::Never),
        sandbox_policy: Some(sandbox_policy),
        permission_profile,
        collaboration_mode: Some(CollaborationMode {
            mode: ModeKind::Default,
            settings: Settings {
                model,
                reasoning_effort: None,
                developer_instructions: None,
            },
        }),
        ..Default::default()
    })
}

fn image_messages(body: &Value) -> Vec<&Value> {
    body.get("input")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter(|item| {
                    item.get("type").and_then(Value::as_str) == Some("message")
                        && item
                            .get("content")
                            .and_then(Value::as_array)
                            .map(|content| {
                                content.iter().any(|span| {
                                    span.get("type").and_then(Value::as_str) == Some("input_image")
                                })
                            })
                            .unwrap_or(false)
                })
                .collect()
        })
        .unwrap_or_default()
}

fn find_image_message(body: &Value) -> Option<&Value> {
    image_messages(body).into_iter().next()
}

fn message_has_text_with_prefix(item: &Value, prefix: &str) -> bool {
    item.get("content")
        .and_then(Value::as_array)
        .is_some_and(|content| {
            content.iter().any(|span| {
                span.get("text")
                    .and_then(Value::as_str)
                    .is_some_and(|text| text.starts_with(prefix))
            })
        })
}

fn assert_developer_text_message(item: &Value, expected_text: &str) {
    assert_eq!(item.get("role").and_then(Value::as_str), Some("developer"));
    assert_eq!(
        item.get("content").and_then(Value::as_array),
        Some(&vec![json!({
            "type": "input_text",
            "text": expected_text,
        })])
    );
}

fn png_bytes(width: u32, height: u32, rgba: [u8; 4]) -> anyhow::Result<Vec<u8>> {
    let image = ImageBuffer::from_pixel(width, height, Rgba(rgba));
    let mut cursor = Cursor::new(Vec::new());
    DynamicImage::ImageRgba8(image).write_to(&mut cursor, image::ImageFormat::Png)?;
    Ok(cursor.into_inner())
}

async fn create_workspace_directory(test: &TestCodex, rel_path: &str) -> anyhow::Result<PathUri> {
    let abs_path_uri = test.workspace_path_uri(rel_path)?;
    test.fs()
        .create_directory(
            &abs_path_uri,
            CreateDirectoryOptions {
                recursive: true,
                follow_symlinks: true,
            },
            /*sandbox*/ None,
        )
        .await?;
    Ok(abs_path_uri)
}

async fn write_workspace_file(
    test: &TestCodex,
    rel_path: &str,
    contents: Vec<u8>,
) -> anyhow::Result<PathBuf> {
    let abs_path_uri = test.workspace_path_uri(rel_path)?;
    if let Some(parent_uri) = abs_path_uri.parent() {
        test.fs()
            .create_directory(
                &parent_uri,
                CreateDirectoryOptions {
                    recursive: true,
                    follow_symlinks: true,
                },
                /*sandbox*/ None,
            )
            .await?;
    }
    test.fs()
        .write_file(
            &abs_path_uri,
            contents,
            Default::default(),
            /*sandbox*/ None,
        )
        .await?;
    Ok(abs_path_uri.to_path_buf())
}

async fn write_workspace_png(
    test: &TestCodex,
    rel_path: &str,
    width: u32,
    height: u32,
    rgba: [u8; 4],
) -> anyhow::Result<PathBuf> {
    write_workspace_file(test, rel_path, png_bytes(width, height, rgba)?).await
}

async fn assert_user_turn_local_image_resizes_to(
    original_dimensions: (u32, u32),
    expected_dimensions: (u32, u32),
    image_budget_policy: ImageBudgetPolicy,
    resize_notice_expectation: ResizeNoticeExpectation,
) -> anyhow::Result<()> {
    let server = start_mock_server().await;

    let builder = match image_budget_policy {
        ImageBudgetPolicy::DetailBased | ImageBudgetPolicy::Unified => test_codex(),
        ImageBudgetPolicy::UnifiedResponsesLiteWithoutOriginalSupport => test_codex()
            .with_model_info_override("gpt-5.4", |model_info| {
                model_info.supports_image_detail_original = false;
                model_info.use_responses_lite = true;
            }),
    };
    let mut builder = builder.with_config(move |config| {
        if image_budget_policy != ImageBudgetPolicy::DetailBased {
            let _ = config.features.enable(Feature::UnifiedImageBudget);
        }
        if matches!(resize_notice_expectation, ResizeNoticeExpectation::Enabled) {
            let _ = config.features.enable(Feature::ImageResizeNotice);
        }
    });
    let test = builder.build_with_auto_env(&server).await?;
    let TestCodex {
        codex,
        session_configured,
        ..
    } = &test;

    let (original_width, original_height) = original_dimensions;
    let local_image_dir = tempfile::tempdir()?;
    let abs_path = local_image_dir.path().join("example.png");
    let image = ImageBuffer::from_pixel(original_width, original_height, Rgba([20u8, 40, 60, 255]));
    image.save(&abs_path)?;

    let response = sse(vec![
        ev_response_created("resp-1"),
        ev_assistant_message("msg-1", "done"),
        ev_completed("resp-1"),
    ]);
    let mock = responses::mount_sse_once(&server, response).await;

    let session_model = session_configured.model.clone();

    codex
        .start_or_steer_turn(disabled_user_turn(
            &test,
            vec![UserInput::LocalImage {
                path: abs_path.clone(),
                detail: None,
            }],
            session_model,
        ))
        .await?;

    wait_for_event_with_timeout(
        codex,
        |event| matches!(event, EventMsg::TurnComplete(_)),
        // Empirically, image attachment can be slow under Bazel/RBE.
        VIEW_IMAGE_TURN_COMPLETE_TIMEOUT,
    )
    .await;

    let request = mock.single_request();
    assert!(request.has_content_kinds(&["user.text", "user.image", "user.text"]));
    let body = request.body_json();
    let input = body
        .get("input")
        .and_then(Value::as_array)
        .context("request input")?;
    let image_message =
        find_image_message(&body).context("pending input image message not included in request")?;
    let image_message_index = input
        .iter()
        .position(|item| std::ptr::eq(item, image_message))
        .context("image message index")?;
    let resize_notice_indices = input
        .iter()
        .enumerate()
        .filter_map(|(index, item)| {
            message_has_text_with_prefix(item, "<image_resize_notice>").then_some(index)
        })
        .collect::<Vec<_>>();
    match resize_notice_expectation {
        ResizeNoticeExpectation::Disabled => {
            assert_eq!(resize_notice_indices, Vec::<usize>::new());
        }
        ResizeNoticeExpectation::Enabled => {
            assert!(request.has_content_kinds(&["images.resize_notice"]));
            assert_eq!(resize_notice_indices, vec![image_message_index + 1]);
            assert_developer_text_message(
                &input[image_message_index + 1],
                &format!(
                    concat!(
                        "<image_resize_notice>\n",
                        "Image 1 of 1 in the preceding user message was resized from {}x{} to {}x{} pixels.\n",
                        "</image_resize_notice>"
                    ),
                    original_width, original_height, expected_dimensions.0, expected_dimensions.1
                ),
            );
        }
    }
    let image_url = image_message
        .get("content")
        .and_then(Value::as_array)
        .and_then(|content| {
            content.iter().find_map(|span| {
                if span.get("type").and_then(Value::as_str) == Some("input_image") {
                    span.get("image_url").and_then(Value::as_str)
                } else {
                    None
                }
            })
        })
        .context("image_url present")?;

    let (prefix, encoded) = image_url
        .split_once(',')
        .context("image url contains data prefix")?;
    assert_eq!(prefix, "data:image/png;base64");

    let decoded = BASE64_STANDARD
        .decode(encoded)
        .context("image data decodes from base64 for request")?;
    let resized = load_from_memory(&decoded).context("load resized image")?;
    let (width, height) = resized.dimensions();
    assert_eq!((width, height), expected_dimensions);

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn user_turn_with_local_image_attaches_image() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    assert_user_turn_local_image_resizes_to(
        (2304, 864),
        (2048, 768),
        ImageBudgetPolicy::DetailBased,
        ResizeNoticeExpectation::Disabled,
    )
    .await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn user_turn_with_vertical_local_image_resizes_to_square_bounds() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    assert_user_turn_local_image_resizes_to(
        (1024, 4096),
        (512, 2048),
        ImageBudgetPolicy::DetailBased,
        ResizeNoticeExpectation::Disabled,
    )
    .await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn user_turn_local_image_applies_patch_budget_and_reports_resize() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    assert_user_turn_local_image_resizes_to(
        (2048, 2048),
        (1600, 1600),
        ImageBudgetPolicy::DetailBased,
        ResizeNoticeExpectation::Enabled,
    )
    .await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn user_turn_unified_image_budget_enforces_dimension_and_patch_limits() -> anyhow::Result<()>
{
    skip_if_no_network!(Ok(()));

    assert_user_turn_local_image_resizes_to(
        (6401, 1),
        (6000, 1),
        ImageBudgetPolicy::Unified,
        ResizeNoticeExpectation::Enabled,
    )
    .await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn user_turn_unified_image_budget_supports_responses_lite_without_original_detail()
-> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    assert_user_turn_local_image_resizes_to(
        (2304, 864),
        (2304, 864),
        ImageBudgetPolicy::UnifiedResponsesLiteWithoutOriginalSupport,
        ResizeNoticeExpectation::Disabled,
    )
    .await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn view_image_tool_attaches_local_image() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let mut builder = test_codex().with_config(|config| {
        let _ = config.features.enable(Feature::ImageResizeNotice);
    });
    let test = builder.build_with_auto_env(&server).await?;
    let TestCodex {
        codex,
        session_configured,
        ..
    } = &test;
    let rel_path = "assets/example.png";
    let path_uri = test.workspace_path_uri(rel_path)?;
    let original_width = 2304;
    let original_height = 864;
    write_workspace_png(
        &test,
        rel_path,
        original_width,
        original_height,
        [255u8, 0, 0, 255],
    )
    .await?;

    let call_id = "view-image-call";
    let arguments = serde_json::json!({ "path": rel_path }).to_string();

    let first_response = sse(vec![
        ev_response_created("resp-1"),
        ev_function_call(call_id, "view_image", &arguments),
        ev_completed("resp-1"),
    ]);
    responses::mount_sse_once(&server, first_response).await;

    let second_response = sse(vec![
        ev_assistant_message("msg-1", "done"),
        ev_completed("resp-2"),
    ]);
    let mock = responses::mount_sse_once(&server, second_response).await;

    let session_model = session_configured.model.clone();

    codex
        .start_or_steer_turn(disabled_user_turn(
            &test,
            vec![UserInput::Text {
                text: "please add the screenshot".into(),
                text_elements: Vec::new(),
            }],
            session_model,
        ))
        .await?;

    let mut item_started = None;
    let mut item_completed = None;
    let mut legacy_event = None;
    wait_for_event_with_timeout(
        codex,
        |event| match event {
            EventMsg::ItemStarted(event) => {
                if matches!(&event.item, codex_protocol::items::TurnItem::ImageView(_)) {
                    item_started = Some(event.item.clone());
                }
                false
            }
            EventMsg::ItemCompleted(event) => {
                if matches!(&event.item, codex_protocol::items::TurnItem::ImageView(_)) {
                    item_completed = Some(event.item.clone());
                }
                false
            }
            EventMsg::ViewImageToolCall(event) => {
                legacy_event = Some(event.clone());
                false
            }
            EventMsg::TurnComplete(_) => true,
            _ => false,
        },
        // Empirically, we have seen this run slow when run under
        // Bazel on arm Linux.
        VIEW_IMAGE_TURN_COMPLETE_TIMEOUT,
    )
    .await;

    match item_started.expect("view image item started event emitted") {
        codex_protocol::items::TurnItem::ImageView(item) => {
            assert_eq!(item.id, call_id);
            assert_eq!(item.path, path_uri);
        }
        other => panic!("expected ImageView item, got {other:?}"),
    }
    match item_completed.expect("view image item completed event emitted") {
        codex_protocol::items::TurnItem::ImageView(item) => {
            assert_eq!(item.id, call_id);
            assert_eq!(item.path, path_uri);
        }
        other => panic!("expected ImageView item, got {other:?}"),
    }
    let legacy_event = legacy_event.expect("legacy view image event emitted");
    assert_eq!(legacy_event.call_id, call_id);
    assert_eq!(legacy_event.path, path_uri);

    let req = mock.single_request();
    let body = req.body_json();
    assert!(
        find_image_message(&body).is_none(),
        "view_image tool should not inject a separate image message"
    );

    let function_output = req.function_call_output(call_id);
    let output_items = function_output
        .get("output")
        .and_then(Value::as_array)
        .expect("function_call_output should be a content item array");
    assert_eq!(
        output_items.len(),
        1,
        "view_image tool output should remain unchanged apart from image preparation"
    );
    assert_eq!(
        output_items[0].get("type").and_then(Value::as_str),
        Some("input_image"),
        "view_image should return only its input_image content item"
    );
    let input = body
        .get("input")
        .and_then(Value::as_array)
        .expect("request input");
    let function_output_index = input
        .iter()
        .position(|item| {
            item.get("type").and_then(Value::as_str) == Some("function_call_output")
                && item.get("call_id").and_then(Value::as_str) == Some(call_id)
        })
        .expect("function call output index");
    assert_developer_text_message(
        &input[function_output_index + 1],
        concat!(
            "<image_resize_notice>\n",
            "Image 1 of 1 in the preceding tool output was resized from 2304x864 to 2048x768 pixels.\n",
            "</image_resize_notice>"
        ),
    );
    let image_url = output_items[0]
        .get("image_url")
        .and_then(Value::as_str)
        .expect("image_url present");

    let (prefix, encoded) = image_url
        .split_once(',')
        .expect("image url contains data prefix");
    assert_eq!(prefix, "data:image/png;base64");

    let decoded = BASE64_STANDARD
        .decode(encoded)
        .expect("image data decodes from base64 for request");
    let resized = load_from_memory(&decoded).expect("load resized image");
    let (resized_width, resized_height) = resized.dimensions();
    assert_eq!((resized_width, resized_height), (2048, 768));

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn view_image_routes_to_selected_local_environment() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let mut builder = test_codex();
    let test = builder.build(&server).await?;
    write_workspace_file(
        &test,
        "local.png",
        png_bytes(/*width*/ 1, /*height*/ 1, [0, 255, 0, 255])?,
    )
    .await?;
    let call_id = "call-view-image-local-env";
    let response_mock = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                ev_function_call(
                    call_id,
                    "view_image",
                    &json!({
                        "path": "local.png",
                        "environment_id": LOCAL_ENVIRONMENT_ID,
                    })
                    .to_string(),
                ),
                ev_completed("resp-1"),
            ]),
            sse(vec![
                ev_response_created("resp-2"),
                ev_assistant_message("msg-1", "done"),
                ev_completed("resp-2"),
            ]),
        ],
    )
    .await;

    test.submit_turn_with_environments(
        "route local view image",
        Some(vec![local(test.config.cwd.clone())]),
    )
    .await?;

    let output = response_mock
        .last_request()
        .context("missing request containing local view_image output")?
        .function_call_output(call_id);
    let output_items = output
        .get("output")
        .and_then(Value::as_array)
        .context("view_image output should be content items")?;
    assert_eq!(output_items.len(), 1);
    let image_url = output_items[0]
        .get("image_url")
        .and_then(Value::as_str)
        .context("view_image output should include image_url")?;
    assert!(
        image_url.starts_with("data:image/png;base64,"),
        "unexpected image_url: {image_url}",
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn view_image_tool_applies_local_sandbox_read_denies() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let mut builder = test_codex();
    let test = builder.build(&server).await?;
    let rel_path = "denied.png";
    let denied_path = test.config.cwd.join(rel_path);
    write_workspace_file(
        &test,
        rel_path,
        png_bytes(/*width*/ 1, /*height*/ 1, [0, 255, 0, 255])?,
    )
    .await?;
    let call_id = "call-view-image-outside-cwd";
    let response_mock = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                ev_function_call(
                    call_id,
                    "view_image",
                    &json!({ "path": rel_path }).to_string(),
                ),
                ev_completed("resp-1"),
            ]),
            sse(vec![
                ev_response_created("resp-2"),
                ev_assistant_message("msg-1", "done"),
                ev_completed("resp-2"),
            ]),
        ],
    )
    .await;

    let mut file_system_sandbox_policy = FileSystemSandboxPolicy::default();
    file_system_sandbox_policy
        .entries
        .push(FileSystemSandboxEntry {
            path: FileSystemPath::Path {
                path: denied_path.clone().into(),
            },
            access: FileSystemAccessMode::Deny,
            missing_path_behavior: None,
        });
    let permission_profile = PermissionProfile::from_runtime_permissions(
        &file_system_sandbox_policy,
        NetworkSandboxPolicy::Restricted,
    );

    test.submit_turn_with_permission_profile("attach the denied image", permission_profile)
        .await?;

    let request = response_mock
        .last_request()
        .context("missing request containing sandboxed view_image output")?;
    assert!(
        request.inputs_of_type("input_image").is_empty(),
        "sandboxed local view_image should not attach denied images"
    );
    let output_text = request
        .function_call_output_content_and_success(call_id)
        .and_then(|(content, _)| content)
        .context("sandboxed view_image error text present")?;
    let denied_path_display =
        PathUri::from_host_native_path(&denied_path)?.inferred_native_path_string();
    let expected_locate_prefix = format!("unable to locate image at `{denied_path_display}`:");
    let expected_read_prefix = format!("unable to read image at `{denied_path_display}`:");
    assert!(
        output_text.starts_with(&expected_locate_prefix)
            || output_text.starts_with(&expected_read_prefix),
        "expected error to start with `{expected_locate_prefix}` or `{expected_read_prefix}` but got `{output_text}`"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn view_image_routes_to_selected_remote_environment() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));
    skip_if_no_remote_env!(Ok(()));

    let server = start_mock_server().await;
    let mut builder = test_codex();
    let test = builder.build_with_remote_and_local_env(&server).await?;
    let local_cwd = TempDir::new()?;
    fs::write(local_cwd.path().join("remote.png"), b"not a remote image")?;
    let local_selection = local(local_cwd.path().abs());
    let remote_cwd_uri = test.executor_environment().selection().cwd.clone();
    let image_path_uri = remote_cwd_uri.join("remote.png")?;
    let png = png_bytes(/*width*/ 1, /*height*/ 1, [0, 255, 0, 255])?;
    test.fs()
        .write_file(
            &image_path_uri,
            png,
            Default::default(),
            /*sandbox*/ None,
        )
        .await?;
    let absolute_image_path = image_path_uri.inferred_native_path_string();
    let remote_selection = TurnEnvironmentSelection {
        environment_id: REMOTE_ENVIRONMENT_ID.to_string(),
        cwd: remote_cwd_uri.clone(),
        workspace_roots: vec![remote_cwd_uri],
        config: EnvironmentConfigState::FromThread,
    };
    let relative_call_id = "call-view-image-relative-multi-env";
    let absolute_call_id = "call-view-image-absolute-multi-env";
    let response_mock = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                ev_function_call(
                    relative_call_id,
                    "view_image",
                    &json!({
                        "path": "remote.png",
                        "environment_id": REMOTE_ENVIRONMENT_ID,
                    })
                    .to_string(),
                ),
                ev_completed("resp-1"),
            ]),
            sse(vec![
                ev_response_created("resp-2"),
                ev_function_call(
                    absolute_call_id,
                    "view_image",
                    &json!({
                        "path": absolute_image_path,
                        "environment_id": REMOTE_ENVIRONMENT_ID,
                    })
                    .to_string(),
                ),
                ev_completed("resp-2"),
            ]),
            sse(vec![
                ev_response_created("resp-3"),
                ev_assistant_message("msg-1", "done"),
                ev_completed("resp-3"),
            ]),
        ],
    )
    .await;

    test.submit_turn_with_environments(
        "route view image",
        Some(vec![local_selection, remote_selection]),
    )
    .await?;

    let request = response_mock
        .last_request()
        .context("missing request containing view_image output")?
        .clone();
    for call_id in [relative_call_id, absolute_call_id] {
        let output = request.function_call_output(call_id);
        let output_items = output
            .get("output")
            .and_then(Value::as_array)
            .context("view_image output should be content items")?;
        assert_eq!(output_items.len(), 1);
        let image_url = output_items[0]
            .get("image_url")
            .and_then(Value::as_str)
            .context("view_image output should include image_url")?;
        assert!(
            image_url.starts_with("data:image/png;base64,"),
            "unexpected image_url: {image_url}",
        );
    }

    test.fs()
        .remove(
            &image_path_uri,
            RemoveOptions {
                recursive: false,
                force: true,
                follow_symlinks: true,
            },
            /*sandbox*/ None,
        )
        .await?;

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn view_image_tool_can_preserve_original_resolution_when_requested_on_gpt5_4()
-> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let mut builder = test_codex().with_model("gpt-5.4");
    let test = builder.build_with_auto_env(&server).await?;
    let TestCodex {
        codex,
        session_configured,
        ..
    } = &test;

    let rel_path = "assets/original-example.png";
    let original_width = 2304;
    let original_height = 864;
    write_workspace_png(
        &test,
        rel_path,
        original_width,
        original_height,
        [0u8, 80, 255, 255],
    )
    .await?;

    let call_id = "view-image-original";
    let arguments = serde_json::json!({ "path": rel_path, "detail": "original" }).to_string();

    let first_response = sse(vec![
        ev_response_created("resp-1"),
        ev_function_call(call_id, "view_image", &arguments),
        ev_completed("resp-1"),
    ]);
    responses::mount_sse_once(&server, first_response).await;

    let second_response = sse(vec![
        ev_assistant_message("msg-1", "done"),
        ev_completed("resp-2"),
    ]);
    let mock = responses::mount_sse_once(&server, second_response).await;

    let session_model = session_configured.model.clone();

    codex
        .start_or_steer_turn(disabled_user_turn(
            &test,
            vec![UserInput::Text {
                text: "please add the original screenshot".into(),
                text_elements: Vec::new(),
            }],
            session_model,
        ))
        .await?;

    wait_for_event_with_timeout(
        codex,
        |event| matches!(event, EventMsg::TurnComplete(_)),
        VIEW_IMAGE_TURN_COMPLETE_TIMEOUT,
    )
    .await;

    let req = mock.single_request();
    let function_output = req.function_call_output(call_id);
    let output_items = function_output
        .get("output")
        .and_then(Value::as_array)
        .expect("function_call_output should be a content item array");
    assert_eq!(output_items.len(), 1);
    assert_eq!(
        output_items[0].get("detail").and_then(Value::as_str),
        Some("original")
    );
    let image_url = output_items[0]
        .get("image_url")
        .and_then(Value::as_str)
        .expect("image_url present");

    let (_, encoded) = image_url
        .split_once(',')
        .expect("image url contains data prefix");
    let decoded = BASE64_STANDARD
        .decode(encoded)
        .expect("image data decodes from base64 for request");
    let preserved = load_from_memory(&decoded).expect("load preserved image");
    let (width, height) = preserved.dimensions();
    assert_eq!(width, original_width);
    assert_eq!(height, original_height);

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn view_image_unified_budget_hides_detail_but_accepts_legacy_hints() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let mut builder = test_codex().with_model("gpt-5.4").with_config(|config| {
        let _ = config.features.enable(Feature::UnifiedImageBudget);
    });
    let test = builder.build_with_auto_env(&server).await?;
    let rel_path = "assets/unified-example.png";
    write_workspace_png(
        &test,
        rel_path,
        /*width*/ 2304,
        /*height*/ 864,
        [0u8, 80, 255, 255],
    )
    .await?;

    let call_id = "view-image-unified";
    let first_mock = responses::mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-1"),
            ev_function_call(
                call_id,
                "view_image",
                &serde_json::json!({ "path": rel_path, "detail": "high" }).to_string(),
            ),
            ev_completed("resp-1"),
        ]),
    )
    .await;
    let second_mock = responses::mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("msg-1", "done"),
            ev_completed("resp-2"),
        ]),
    )
    .await;

    test.submit_turn("show the screenshot").await?;

    let first_request = first_mock.single_request().body_json();
    let view_image_tool = first_request["tools"]
        .as_array()
        .and_then(|tools| tools.iter().find(|tool| tool["name"] == "view_image"))
        .context("view_image tool should be available")?;
    assert!(
        view_image_tool["parameters"]["properties"]
            .get("detail")
            .is_none(),
        "the unified image budget should not advertise detail"
    );

    let request = second_mock.single_request();
    let output = request.function_call_output(call_id);
    let output_items = output["output"]
        .as_array()
        .context("view_image should return image content")?;
    assert_eq!(output_items.len(), 1);
    assert_eq!(output_items[0]["detail"], "original");

    let image_url = output_items[0]["image_url"]
        .as_str()
        .context("view_image output should include image_url")?;
    let (_, payload) = image_url
        .split_once(',')
        .context("view_image image_url should include a base64 payload")?;
    let image = load_from_memory(&BASE64_STANDARD.decode(payload)?)?;
    assert_eq!(image.dimensions(), (2304, 864));

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn view_image_tool_errors_clearly_for_unsupported_detail_values() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let mut builder = test_codex().with_model("gpt-5.4");
    let test = builder.build_with_auto_env(&server).await?;
    let TestCodex {
        codex,
        session_configured,
        ..
    } = &test;

    let rel_path = "assets/unsupported-detail.png";
    write_workspace_png(
        &test,
        rel_path,
        /*width*/ 256,
        /*height*/ 128,
        [0u8, 80, 255, 255],
    )
    .await?;

    let call_id = "view-image-unsupported-detail";
    let arguments = serde_json::json!({ "path": rel_path, "detail": "low" }).to_string();

    let first_response = sse(vec![
        ev_response_created("resp-1"),
        ev_function_call(call_id, "view_image", &arguments),
        ev_completed("resp-1"),
    ]);
    responses::mount_sse_once(&server, first_response).await;

    let second_response = sse(vec![
        ev_assistant_message("msg-1", "done"),
        ev_completed("resp-2"),
    ]);
    let mock = responses::mount_sse_once(&server, second_response).await;

    let session_model = session_configured.model.clone();

    codex
        .start_or_steer_turn(disabled_user_turn(
            &test,
            vec![UserInput::Text {
                text: "please attach the image at low detail".into(),
                text_elements: Vec::new(),
            }],
            session_model,
        ))
        .await?;

    wait_for_event_with_timeout(
        codex,
        |event| matches!(event, EventMsg::TurnComplete(_)),
        VIEW_IMAGE_TURN_COMPLETE_TIMEOUT,
    )
    .await;

    let req = mock.single_request();
    let body_with_tool_output = req.body_json();
    let output_text = req
        .function_call_output_content_and_success(call_id)
        .and_then(|(content, _)| content)
        .expect("output text present");
    assert_eq!(
        output_text,
        "view_image.detail only supports `high` or `original`; omit `detail` for default high resized behavior, got `low`"
    );

    assert!(
        find_image_message(&body_with_tool_output).is_none(),
        "unsupported detail values should not produce an input_image message"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn view_image_tool_treats_null_detail_as_omitted() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let mut builder = test_codex().with_model("gpt-5.4");
    let test = builder.build_with_auto_env(&server).await?;
    let TestCodex {
        codex,
        session_configured,
        ..
    } = &test;

    let rel_path = "assets/null-detail.png";
    let original_width = 2304;
    let original_height = 864;
    write_workspace_png(
        &test,
        rel_path,
        original_width,
        original_height,
        [0u8, 80, 255, 255],
    )
    .await?;

    let call_id = "view-image-null-detail";
    let arguments = serde_json::json!({ "path": rel_path, "detail": null }).to_string();

    let first_response = sse(vec![
        ev_response_created("resp-1"),
        ev_function_call(call_id, "view_image", &arguments),
        ev_completed("resp-1"),
    ]);
    responses::mount_sse_once(&server, first_response).await;

    let second_response = sse(vec![
        ev_assistant_message("msg-1", "done"),
        ev_completed("resp-2"),
    ]);
    let mock = responses::mount_sse_once(&server, second_response).await;

    let session_model = session_configured.model.clone();

    codex
        .start_or_steer_turn(disabled_user_turn(
            &test,
            vec![UserInput::Text {
                text: "please attach the image with a null detail".into(),
                text_elements: Vec::new(),
            }],
            session_model,
        ))
        .await?;

    wait_for_event_with_timeout(
        codex,
        |event| matches!(event, EventMsg::TurnComplete(_)),
        VIEW_IMAGE_TURN_COMPLETE_TIMEOUT,
    )
    .await;

    let req = mock.single_request();
    let function_output = req.function_call_output(call_id);
    let output_items = function_output
        .get("output")
        .and_then(Value::as_array)
        .expect("function_call_output should be a content item array");
    assert_eq!(output_items.len(), 1);
    assert_eq!(
        output_items[0].get("detail").and_then(Value::as_str),
        Some("high")
    );
    let image_url = output_items[0]
        .get("image_url")
        .and_then(Value::as_str)
        .expect("image_url present");

    let (_, encoded) = image_url
        .split_once(',')
        .expect("image url contains data prefix");
    let decoded = BASE64_STANDARD
        .decode(encoded)
        .expect("image data decodes from base64 for request");
    let resized = load_from_memory(&decoded).expect("load resized image");
    let (width, height) = resized.dimensions();
    assert_eq!((width, height), (2048, 768));

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn view_image_tool_resizes_when_model_lacks_original_detail_support() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    assert_view_image_tool_resizes_without_original_support(ImageBudgetPolicy::DetailBased).await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn view_image_unified_budget_stays_disabled_for_unsupported_model() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    assert_view_image_tool_resizes_without_original_support(ImageBudgetPolicy::Unified).await
}

async fn assert_view_image_tool_resizes_without_original_support(
    image_budget_policy: ImageBudgetPolicy,
) -> anyhow::Result<()> {
    let server = start_mock_server().await;
    let mut builder = test_codex()
        .with_model("gpt-5.2")
        .with_config(move |config| {
            if image_budget_policy == ImageBudgetPolicy::Unified {
                let _ = config.features.enable(Feature::UnifiedImageBudget);
            }
        });
    let test = builder.build_with_auto_env(&server).await?;
    let TestCodex {
        codex,
        session_configured,
        ..
    } = &test;

    let rel_path = "assets/original-example-lower-model.png";
    let original_width = 2304;
    let original_height = 864;
    write_workspace_png(
        &test,
        rel_path,
        original_width,
        original_height,
        [0u8, 80, 255, 255],
    )
    .await?;

    let call_id = "view-image-original-lower-model";
    let arguments = serde_json::json!({ "path": rel_path }).to_string();

    let first_response = sse(vec![
        ev_response_created("resp-1"),
        ev_function_call(call_id, "view_image", &arguments),
        ev_completed("resp-1"),
    ]);
    responses::mount_sse_once(&server, first_response).await;

    let second_response = sse(vec![
        ev_assistant_message("msg-1", "done"),
        ev_completed("resp-2"),
    ]);
    let mock = responses::mount_sse_once(&server, second_response).await;

    let session_model = session_configured.model.clone();

    codex
        .start_or_steer_turn(disabled_user_turn(
            &test,
            vec![UserInput::Text {
                text: "please add the screenshot".into(),
                text_elements: Vec::new(),
            }],
            session_model,
        ))
        .await?;

    wait_for_event_with_timeout(
        codex,
        |event| matches!(event, EventMsg::TurnComplete(_)),
        VIEW_IMAGE_TURN_COMPLETE_TIMEOUT,
    )
    .await;

    let req = mock.single_request();
    let function_output = req.function_call_output(call_id);
    let output_items = function_output
        .get("output")
        .and_then(Value::as_array)
        .expect("function_call_output should be a content item array");
    assert_eq!(output_items.len(), 1);
    assert_eq!(
        output_items[0].get("detail").and_then(Value::as_str),
        Some("high")
    );

    let image_url = output_items[0]
        .get("image_url")
        .and_then(Value::as_str)
        .expect("image_url present");

    let (prefix, encoded) = image_url
        .split_once(',')
        .expect("image url contains data prefix");
    assert_eq!(prefix, "data:image/png;base64");

    let decoded = BASE64_STANDARD
        .decode(encoded)
        .expect("image data decodes from base64 for request");
    let resized = load_from_memory(&decoded).expect("load resized image");
    let (resized_width, resized_height) = resized.dimensions();
    assert_eq!((resized_width, resized_height), (2048, 768));

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn view_image_tool_does_not_force_original_resolution_with_capability_only()
-> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let mut builder = test_codex().with_model("gpt-5.4");
    let test = builder.build_with_auto_env(&server).await?;
    let TestCodex {
        codex,
        session_configured,
        ..
    } = &test;

    let rel_path = "assets/original-example-capability-only.png";
    let original_width = 2304;
    let original_height = 864;
    write_workspace_png(
        &test,
        rel_path,
        original_width,
        original_height,
        [0u8, 80, 255, 255],
    )
    .await?;

    let call_id = "view-image-capability-only";
    let arguments = serde_json::json!({ "path": rel_path }).to_string();

    let first_response = sse(vec![
        ev_response_created("resp-1"),
        ev_function_call(call_id, "view_image", &arguments),
        ev_completed("resp-1"),
    ]);
    responses::mount_sse_once(&server, first_response).await;

    let second_response = sse(vec![
        ev_assistant_message("msg-1", "done"),
        ev_completed("resp-2"),
    ]);
    let mock = responses::mount_sse_once(&server, second_response).await;

    let session_model = session_configured.model.clone();

    codex
        .start_or_steer_turn(disabled_user_turn(
            &test,
            vec![UserInput::Text {
                text: "please add the screenshot".into(),
                text_elements: Vec::new(),
            }],
            session_model,
        ))
        .await?;

    wait_for_event_with_timeout(
        codex,
        |event| matches!(event, EventMsg::TurnComplete(_)),
        VIEW_IMAGE_TURN_COMPLETE_TIMEOUT,
    )
    .await;

    let req = mock.single_request();
    let function_output = req.function_call_output(call_id);
    let output_items = function_output
        .get("output")
        .and_then(Value::as_array)
        .expect("function_call_output should be a content item array");
    assert_eq!(output_items.len(), 1);
    assert_eq!(
        output_items[0].get("detail").and_then(Value::as_str),
        Some("high")
    );
    let image_url = output_items[0]
        .get("image_url")
        .and_then(Value::as_str)
        .expect("image_url present");

    let (_, encoded) = image_url
        .split_once(',')
        .expect("image url contains data prefix");
    let decoded = BASE64_STANDARD
        .decode(encoded)
        .expect("image data decodes from base64 for request");
    let resized = load_from_memory(&decoded).expect("load resized image");
    let (resized_width, resized_height) = resized.dimensions();
    assert_eq!((resized_width, resized_height), (2048, 768));

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn view_image_tool_errors_when_path_is_directory() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;

    let mut builder = test_codex();
    let test = builder.build_with_auto_env(&server).await?;
    let TestCodex {
        codex,
        session_configured,
        ..
    } = &test;

    let rel_path = "assets";
    let abs_path = create_workspace_directory(&test, rel_path).await?;

    let call_id = "view-image-directory";
    let arguments = serde_json::json!({ "path": rel_path }).to_string();

    let first_response = sse(vec![
        ev_response_created("resp-1"),
        ev_function_call(call_id, "view_image", &arguments),
        ev_completed("resp-1"),
    ]);
    responses::mount_sse_once(&server, first_response).await;

    let second_response = sse(vec![
        ev_assistant_message("msg-1", "done"),
        ev_completed("resp-2"),
    ]);
    let mock = responses::mount_sse_once(&server, second_response).await;

    let session_model = session_configured.model.clone();

    codex
        .start_or_steer_turn(disabled_user_turn(
            &test,
            vec![UserInput::Text {
                text: "please attach the folder".into(),
                text_elements: Vec::new(),
            }],
            session_model,
        ))
        .await?;

    wait_for_event_with_timeout(
        codex,
        |event| matches!(event, EventMsg::TurnComplete(_)),
        VIEW_IMAGE_TURN_COMPLETE_TIMEOUT,
    )
    .await;

    let req = mock.single_request();
    let body_with_tool_output = req.body_json();
    let output_text = req
        .function_call_output_content_and_success(call_id)
        .and_then(|(content, _)| content)
        .expect("output text present");
    let expected_path = abs_path.inferred_native_path_string();
    let expected_message = format!("image path `{expected_path}` is not a file");
    assert_eq!(output_text, expected_message);

    assert!(
        find_image_message(&body_with_tool_output).is_none(),
        "directory path should not produce an input_image message"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn view_image_tool_rejects_invalid_image_before_tool_output() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let mut builder = test_codex();
    let test = builder.build_with_auto_env(&server).await?;
    let TestCodex {
        codex,
        session_configured,
        ..
    } = &test;

    let rel_path = "assets/invalid-image.json";
    write_workspace_file(&test, rel_path, br#"{ "message": "hello" }"#.to_vec()).await?;
    let call_id = "view-image-invalid-placeholder";
    let arguments = serde_json::json!({ "path": rel_path }).to_string();

    responses::mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-1"),
            ev_function_call(call_id, "view_image", &arguments),
            ev_completed("resp-1"),
        ]),
    )
    .await;
    let second_mock = responses::mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("msg-1", "done"),
            ev_completed("resp-2"),
        ]),
    )
    .await;

    codex
        .start_or_steer_turn(disabled_user_turn(
            &test,
            vec![UserInput::Text {
                text: "please inspect the image".into(),
                text_elements: Vec::new(),
            }],
            session_configured.model.clone(),
        ))
        .await?;
    wait_for_event_with_timeout(
        codex,
        |event| matches!(event, EventMsg::TurnComplete(_)),
        VIEW_IMAGE_TURN_COMPLETE_TIMEOUT,
    )
    .await;

    let request = second_mock.single_request();
    let output_text = request
        .function_call_output_content_and_success(call_id)
        .and_then(|(content, _)| content)
        .context("invalid view_image error text present")?;
    assert_eq!(
        output_text,
        "unable to process image: invalid or unsupported image data"
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn view_image_tool_errors_when_file_missing() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    println!(
        "view_image missing-file test target: {:?}, remote: {}",
        test_target_os(),
        is_remote_test_environment()
    );

    let server = start_mock_server().await;

    let mut builder = test_codex();
    let test = builder.build_with_auto_env(&server).await?;
    let TestCodex {
        codex,
        session_configured,
        ..
    } = &test;

    let rel_path = "missing/example.png";
    let expected_path = test
        .workspace_path_uri(rel_path)?
        .inferred_native_path_string();

    let call_id = "view-image-missing";
    let arguments = serde_json::json!({ "path": rel_path }).to_string();

    let first_response = sse(vec![
        ev_response_created("resp-1"),
        ev_function_call(call_id, "view_image", &arguments),
        ev_completed("resp-1"),
    ]);
    responses::mount_sse_once(&server, first_response).await;

    let second_response = sse(vec![
        ev_assistant_message("msg-1", "done"),
        ev_completed("resp-2"),
    ]);
    let mock = responses::mount_sse_once(&server, second_response).await;

    let session_model = session_configured.model.clone();

    codex
        .start_or_steer_turn(disabled_user_turn(
            &test,
            vec![UserInput::Text {
                text: "please attach the missing image".into(),
                text_elements: Vec::new(),
            }],
            session_model,
        ))
        .await?;

    wait_for_event_with_timeout(
        codex,
        |event| matches!(event, EventMsg::TurnComplete(_)),
        VIEW_IMAGE_TURN_COMPLETE_TIMEOUT,
    )
    .await;

    let req = mock.single_request();
    let body_with_tool_output = req.body_json();
    let output_text = req
        .function_call_output_content_and_success(call_id)
        .and_then(|(content, _)| content)
        .expect("output text present");
    let expected_prefix = format!("unable to locate image at `{expected_path}`:");
    assert!(
        output_text.starts_with(&expected_prefix),
        "expected error to start with `{expected_prefix}` but got `{output_text}`"
    );

    assert!(
        find_image_message(&body_with_tool_output).is_none(),
        "missing file should not produce an input_image message"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn view_image_tool_returns_unsupported_message_for_text_only_model() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    // Use MockServer directly (not start_mock_server) so the first /models request returns our
    // text-only model. start_mock_server mounts empty models first, causing get_model_info to
    // fall back to model_info_from_slug with default_input_modalities (Text+Image), which would
    // incorrectly allow view_image.
    let server = MockServer::builder()
        .body_print_limit(BodyPrintLimit::Limited(80_000))
        .start()
        .await;
    let model_slug = "text-only-view-image-test-model";
    let text_only_model = ModelInfo {
        slug: model_slug.to_string(),
        display_name: "Text-only view_image test model".to_string(),
        description: Some("Remote model for view_image unsupported-path coverage".to_string()),
        default_reasoning_level: Some(ReasoningEffort::Medium),
        supported_reasoning_levels: vec![ReasoningEffortPreset {
            effort: ReasoningEffort::Medium,
            description: ReasoningEffort::Medium.to_string(),
        }],
        shell_type: ConfigShellToolType::UnifiedExec,
        visibility: ModelVisibility::List,
        supported_in_api: true,
        input_modalities: vec![InputModality::Text],
        used_fallback_model_metadata: false,
        supports_search_tool: false,
        use_responses_lite: false,
        node_repl_auto_review_required: false,
        node_repl_disabled: false,
        auto_review_model_override: None,
        model_specialty: None,
        tool_mode: None,
        multi_agent_version: None,
        priority: 1,
        additional_speed_tiers: Vec::new(),
        service_tiers: Vec::new(),
        default_service_tier: None,
        upgrade: None,
        model_messages: None,
        include_skills_usage_instructions: false,
        include_plugin_usage_instructions: false,
        include_apps_usage_instructions: false,
        supports_reasoning_summary_parameter: true,
        default_reasoning_summary: ReasoningSummary::Auto,
        support_verbosity: false,
        default_verbosity: None,
        availability_nux: None,
        apply_patch_tool_type: None,
        web_search_tool_type: Default::default(),
        truncation_policy: TruncationPolicyConfig::bytes(/*limit*/ 10_000),
        supports_image_detail_original: false,
        context_window: Some(272_000),
        max_context_window: None,
        auto_compact_token_limit: None,
        comp_hash: None,
        effective_context_window_percent: 95,
        experimental_supported_tools: Vec::new(),
    };
    mount_models_once(
        &server,
        ModelsResponse {
            models: vec![text_only_model],
        },
    )
    .await;

    let mut builder = test_codex()
        .with_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing())
        .with_config(|config| {
            config.model = Some(model_slug.to_string());
        });
    let test = builder.build_with_auto_env(&server).await?;
    let TestCodex { codex, .. } = &test;

    let rel_path = "assets/example.png";
    write_workspace_png(
        &test,
        rel_path,
        /*width*/ 20,
        /*height*/ 20,
        [255u8, 0, 0, 255],
    )
    .await?;

    let call_id = "view-image-unsupported-model";
    let arguments = serde_json::json!({ "path": rel_path }).to_string();
    let first_response = sse(vec![
        ev_response_created("resp-1"),
        ev_function_call(call_id, "view_image", &arguments),
        ev_completed("resp-1"),
    ]);
    responses::mount_sse_once(&server, first_response).await;

    let second_response = sse(vec![
        ev_assistant_message("msg-1", "done"),
        ev_completed("resp-2"),
    ]);
    let mock = responses::mount_sse_once(&server, second_response).await;

    codex
        .start_or_steer_turn(disabled_user_turn(
            &test,
            vec![UserInput::Text {
                text: "please attach the image".into(),
                text_elements: Vec::new(),
            }],
            model_slug.to_string(),
        ))
        .await?;

    wait_for_event_with_timeout(
        codex,
        |event| matches!(event, EventMsg::TurnComplete(_)),
        VIEW_IMAGE_TURN_COMPLETE_TIMEOUT,
    )
    .await;

    let output_text = mock
        .single_request()
        .function_call_output_content_and_success(call_id)
        .and_then(|(content, _)| content)
        .expect("output text present");
    assert_eq!(
        output_text,
        "view_image is not allowed because you do not support image inputs"
    );

    Ok(())
}

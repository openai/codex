use anyhow::Context;
use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use codex_core::TurnInputRequest;
use codex_features::Feature;
use codex_history::RolloutItem;
use codex_history::RolloutLine;
use codex_protocol::config_types::CollaborationMode;
use codex_protocol::config_types::ModeKind;
use codex_protocol::config_types::Settings;
use codex_protocol::models::ContentItem;
use codex_protocol::models::DEFAULT_IMAGE_DETAIL;
use codex_protocol::models::ImageDetail;
use codex_protocol::models::PermissionProfile;
use codex_protocol::models::ResponseInputItem;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::ThreadSettingsOverrides;
use codex_protocol::user_input::UserInput;
use codex_utils_image::data_url_from_bytes;
use core_test_support::TempDirExt;
use core_test_support::responses;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::responses::strip_metadata;
use core_test_support::responses::strip_response_item_id;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::TestCodex;
use core_test_support::test_codex::local_selections;
use core_test_support::test_codex::test_codex;
use core_test_support::test_codex::turn_permission_fields;
use core_test_support::wait_for_event;
use image::GenericImageView;
use image::ImageBuffer;
use image::Rgba;
use pretty_assertions::assert_eq;
use serde_json::Value;
use std::fs;
use std::path::Path;
use std::time::Duration;

fn find_user_message_with_image(text: &str) -> Option<ResponseItem> {
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let rollout: RolloutLine = match serde_json::from_str(trimmed) {
            Ok(rollout) => rollout,
            Err(_) => continue,
        };
        if let RolloutItem::ResponseItem(envelope) = &rollout.item
            && let ResponseItem::Message { role, content, .. } = &envelope.item
            && role == "user"
            && content
                .iter()
                .any(|span| matches!(span, ContentItem::InputImage { .. }))
        {
            return Some(envelope.item.clone());
        }
    }
    None
}

fn extract_image_url(item: &ResponseItem) -> Option<String> {
    match item {
        ResponseItem::Message { content, .. } => content.iter().find_map(|span| match span {
            ContentItem::InputImage { image_url, .. } => Some(image_url.clone()),
            _ => None,
        }),
        _ => None,
    }
}

async fn read_rollout_text(path: &Path) -> anyhow::Result<String> {
    for _ in 0..50 {
        if path.exists()
            && let Ok(text) = std::fs::read_to_string(path)
            && !text.trim().is_empty()
        {
            return Ok(text);
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    std::fs::read_to_string(path)
        .with_context(|| format!("read rollout file at {}", path.display()))
}

fn write_test_png(path: &Path, color: [u8; 4]) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let image = ImageBuffer::from_pixel(2, 2, Rgba(color));
    image.save(path)?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn copy_paste_local_image_persists_rollout_request_shape() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;

    let TestCodex {
        codex,
        cwd,
        session_configured,
        home: _home,
        ..
    } = test_codex().build(&server).await?;

    let rel_path = "images/paste.png";
    let abs_path = cwd.path().join(rel_path);
    write_test_png(&abs_path, [12, 34, 56, 255])?;

    let response = sse(vec![
        ev_response_created("resp-1"),
        ev_assistant_message("msg-1", "done"),
        ev_completed("resp-1"),
    ]);
    responses::mount_sse_once(&server, response).await;

    let session_model = session_configured.model.clone();
    let (sandbox_policy, permission_profile) =
        turn_permission_fields(PermissionProfile::Disabled, cwd.path());

    codex
        .start_or_steer_turn(
            TurnInputRequest::user_input(vec![
                UserInput::LocalImage {
                    path: abs_path.clone(),
                    detail: None,
                },
                UserInput::Text {
                    text: "pasted image".to_string(),
                    text_elements: Vec::new(),
                },
            ])
            .with_thread_settings(ThreadSettingsOverrides {
                environments: Some(local_selections(cwd.abs())),
                approval_policy: Some(AskForApproval::Never),
                sandbox_policy: Some(sandbox_policy),
                permission_profile,
                collaboration_mode: Some(CollaborationMode {
                    mode: ModeKind::Default,
                    settings: Settings {
                        model: session_model,
                        reasoning_effort: None,
                        developer_instructions: None,
                    },
                }),
                ..Default::default()
            }),
        )
        .await?;

    wait_for_event(&codex, |event| matches!(event, EventMsg::TurnComplete(_))).await;
    codex.submit(Op::Shutdown).await?;
    wait_for_event(&codex, |event| matches!(event, EventMsg::ShutdownComplete)).await;

    let rollout_path = codex.rollout_path().expect("rollout path");
    let rollout_text = read_rollout_text(&rollout_path).await?;
    let actual = find_user_message_with_image(&rollout_text)
        .expect("expected user message with input image in rollout");

    let image_url = extract_image_url(&actual).expect("expected image url in rollout");
    let expected = ResponseItem::Message {
        id: None,
        role: "user".to_string(),
        content: vec![
            ContentItem::InputText {
                text: codex_protocol::models::local_image_open_tag_text_with_path(
                    /*label_number*/ 1, &abs_path,
                ),
            },
            ContentItem::InputImage {
                image_url,
                detail: Some(DEFAULT_IMAGE_DETAIL),
            },
            ContentItem::InputText {
                text: codex_protocol::models::image_close_tag_text(),
            },
            ContentItem::InputText {
                text: "pasted image".to_string(),
            },
        ],
        phase: None,
        internal_chat_message_metadata_passthrough: None,
    };

    assert_eq!(strip_response_item_id(strip_metadata(actual)), expected);

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn drag_drop_image_persists_rollout_request_shape() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;

    let TestCodex {
        codex,
        cwd,
        session_configured,
        home: _home,
        ..
    } = test_codex().build(&server).await?;

    let image_url = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGP4z8DwHwAFAAH/iZk9HQAAAABJRU5ErkJggg==".to_string();

    let response = sse(vec![
        ev_response_created("resp-1"),
        ev_assistant_message("msg-1", "done"),
        ev_completed("resp-1"),
    ]);
    responses::mount_sse_once(&server, response).await;

    let session_model = session_configured.model.clone();
    let (sandbox_policy, permission_profile) =
        turn_permission_fields(PermissionProfile::Disabled, cwd.path());

    codex
        .start_or_steer_turn(
            TurnInputRequest::user_input(vec![
                UserInput::Image {
                    image_url: image_url.clone(),
                    detail: None,
                },
                UserInput::Text {
                    text: "dropped image".to_string(),
                    text_elements: Vec::new(),
                },
            ])
            .with_thread_settings(ThreadSettingsOverrides {
                environments: Some(local_selections(cwd.abs())),
                approval_policy: Some(AskForApproval::Never),
                sandbox_policy: Some(sandbox_policy),
                permission_profile,
                collaboration_mode: Some(CollaborationMode {
                    mode: ModeKind::Default,
                    settings: Settings {
                        model: session_model,
                        reasoning_effort: None,
                        developer_instructions: None,
                    },
                }),
                ..Default::default()
            }),
        )
        .await?;

    wait_for_event(&codex, |event| matches!(event, EventMsg::TurnComplete(_))).await;
    codex.submit(Op::Shutdown).await?;
    wait_for_event(&codex, |event| matches!(event, EventMsg::ShutdownComplete)).await;

    let rollout_path = codex.rollout_path().expect("rollout path");
    let rollout_text = read_rollout_text(&rollout_path).await?;
    let actual = find_user_message_with_image(&rollout_text)
        .expect("expected user message with input image in rollout");

    let image_url = extract_image_url(&actual).expect("expected image url in rollout");
    let expected = ResponseItem::Message {
        id: None,
        role: "user".to_string(),
        content: vec![
            ContentItem::InputImage {
                image_url,
                detail: Some(DEFAULT_IMAGE_DETAIL),
            },
            ContentItem::InputText {
                text: "dropped image".to_string(),
            },
        ],
        phase: None,
        internal_chat_message_metadata_passthrough: None,
    };

    assert_eq!(strip_response_item_id(strip_metadata(actual)), expected);

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn resumed_history_only_emits_resize_notices_for_new_images() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let initial = test_codex().build_with_auto_env(&server).await?;
    responses::mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-initial"),
            ev_assistant_message("msg-initial", "recorded"),
            ev_completed("resp-initial"),
        ]),
    )
    .await;
    initial.submit_turn("historical image").await?;

    let rollout_path = initial
        .session_configured
        .rollout_path
        .clone()
        .context("initial rollout path")?;
    initial.codex.shutdown_and_wait().await?;

    let image_path = initial.cwd.path().join("large-image.png");
    ImageBuffer::from_pixel(
        /*width*/ 2304,
        /*height*/ 864,
        Rgba([12u8, 34, 56, 255]),
    )
    .save(&image_path)?;
    let original_image_url = data_url_from_bytes("image/png", &fs::read(&image_path)?);

    let mut rollout_lines = fs::read_to_string(&rollout_path)?
        .lines()
        .map(serde_json::from_str::<RolloutLine>)
        .collect::<serde_json::Result<Vec<_>>>()?;
    let historical_content = rollout_lines
        .iter_mut()
        .find_map(|line| {
            let RolloutItem::ResponseItem(envelope) = &mut line.item else {
                return None;
            };
            let ResponseItem::Message { role, content, .. } = &mut envelope.item else {
                return None;
            };
            (role == "user"
                && content.iter().any(|item| {
                    matches!(item, ContentItem::InputText { text } if text == "historical image")
                }))
            .then_some(content)
        })
        .context("historical user message in rollout")?;
    historical_content.insert(
        /*index*/ 0,
        ContentItem::InputImage {
            image_url: original_image_url.clone(),
            detail: Some(ImageDetail::High),
        },
    );
    let rollout = rollout_lines
        .iter()
        .map(serde_json::to_string)
        .collect::<serde_json::Result<Vec<_>>>()?
        .join("\n");
    fs::write(&rollout_path, format!("{rollout}\n"))?;

    let mut resume_builder = test_codex().with_config(|config| {
        let _ = config.features.enable(Feature::ImageResizeNotice);
        let _ = config
            .features
            .enable(Feature::RetainClientDeveloperMessages);
    });
    let resumed = resume_builder
        .resume(&server, initial.home.clone(), rollout_path.clone())
        .await?;
    let resumed_mock = responses::mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-resumed"),
            ev_assistant_message("msg-resumed", "done"),
            ev_completed("resp-resumed"),
        ]),
    )
    .await;
    resumed
        .codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Image {
            image_url: original_image_url.clone(),
            detail: Some(ImageDetail::High),
        }]))
        .await?;
    wait_for_event(&resumed.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;

    let request = resumed_mock.single_request();
    let input = request.input();
    let image_message_indices = input
        .iter()
        .enumerate()
        .filter_map(|(index, item)| {
            (item.get("type").and_then(Value::as_str) == Some("message")
                && item
                    .get("content")
                    .and_then(Value::as_array)
                    .is_some_and(|content| {
                        content.iter().any(|item| {
                            item.get("type").and_then(Value::as_str) == Some("input_image")
                        })
                    }))
            .then_some(index)
        })
        .collect::<Vec<_>>();
    assert_eq!(image_message_indices.len(), 2);

    let historical_image_url = input[image_message_indices[0]]
        .get("content")
        .and_then(Value::as_array)
        .and_then(|content| {
            content
                .iter()
                .find(|item| item.get("type").and_then(Value::as_str) == Some("input_image"))
        })
        .and_then(|item| item.get("image_url"))
        .and_then(Value::as_str)
        .context("historical image URL in resumed request")?;
    let (_, encoded_image) = historical_image_url
        .split_once(',')
        .context("historical image data URL")?;
    let historical_image = image::load_from_memory(&BASE64_STANDARD.decode(encoded_image)?)?;
    assert_eq!(historical_image.dimensions(), (2048, 768));

    let expected_notice = concat!(
        "<image_resize_notice>\n",
        "Image 1 of 1 in the preceding user message was resized from 2304x864 to 2048x768 pixels.\n",
        "</image_resize_notice>"
    );
    let resize_notices = request
        .message_input_texts("developer")
        .into_iter()
        .filter(|text| text.starts_with("<image_resize_notice>"))
        .collect::<Vec<_>>();
    assert_eq!(resize_notices, vec![expected_notice.to_string()]);
    assert_eq!(
        input[image_message_indices[1] + 1]
            .get("content")
            .and_then(Value::as_array)
            .and_then(|content| content.first())
            .and_then(|item| item.get("text"))
            .and_then(Value::as_str),
        Some(expected_notice)
    );

    resumed.codex.shutdown_and_wait().await?;
    let replayed = resume_builder
        .resume(&server, resumed.home.clone(), rollout_path.clone())
        .await?;
    let replayed_mock = responses::mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-replayed"),
            ev_assistant_message("msg-replayed", "done"),
            ev_completed("resp-replayed"),
        ]),
    )
    .await;
    replayed.submit_turn("preserve recorded notices").await?;
    let replayed_notices = replayed_mock
        .single_request()
        .message_input_texts("developer")
        .into_iter()
        .filter(|text| text.starts_with("<image_resize_notice>"))
        .collect::<Vec<_>>();
    assert_eq!(replayed_notices, vec![expected_notice.to_string()]);

    let replayed = test_codex()
        .with_config(|config| {
            let _ = config.features.enable(Feature::ImageResizeNotice);
            let _ = config
                .features
                .enable(Feature::RetainClientDeveloperMessages);
        })
        .restart(&server, &replayed)
        .await?;
    let existing_rollout_lines = fs::read_to_string(&rollout_path)?.lines().count();
    replayed
        .codex
        .inject_response_items(vec![
            ResponseInputItem::Message {
                role: "user".to_string(),
                content: vec![ContentItem::InputImage {
                    image_url: original_image_url,
                    detail: Some(ImageDetail::High),
                }],
                phase: None,
            }
            .into(),
            ResponseInputItem::Message {
                role: "developer".to_string(),
                content: vec![ContentItem::InputText {
                    text: "<image_resize_notice>client message</image_resize_notice>".to_string(),
                }],
                phase: None,
            }
            .into(),
        ])
        .await?;
    let persisted_developer_metadata = fs::read_to_string(&rollout_path)?
        .lines()
        .skip(existing_rollout_lines)
        .filter_map(|line| {
            let RolloutItem::ResponseItem(envelope) = serde_json::from_str::<RolloutLine>(line)
                .expect("new rollout line should deserialize")
                .item
            else {
                return None;
            };
            matches!(envelope.item, ResponseItem::Message { role, .. } if role == "developer")
                .then(|| envelope.metadata.map(|metadata| metadata.client_authored))
        })
        .collect::<Vec<_>>();
    assert_eq!(persisted_developer_metadata, vec![None, Some(true)]);

    Ok(())
}

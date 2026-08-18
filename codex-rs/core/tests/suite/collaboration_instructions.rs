use anyhow::Result;
use codex_core::TurnInputRequest;
use codex_features::Feature;
use codex_history::RolloutItem;
use codex_history::RolloutLine;
use codex_login::CodexAuth;
use codex_models_manager::model_info::model_info_from_slug;
use codex_protocol::config_types::CollaborationMode;
use codex_protocol::config_types::ModeKind;
use codex_protocol::config_types::Settings;
use codex_protocol::openai_models::CollaborationModeMessages;
use codex_protocol::openai_models::ModelMessages;
use codex_protocol::openai_models::ModelsResponse;
use codex_protocol::protocol::COLLABORATION_MODE_CLOSE_TAG;
use codex_protocol::protocol::COLLABORATION_MODE_OPEN_TAG;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::ThreadSettingsOverrides;
use codex_protocol::user_input::UserInput;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_models_once_with_etag;
use core_test_support::responses::mount_response_once;
use core_test_support::responses::mount_sse_once;
use core_test_support::responses::sse;
use core_test_support::responses::sse_response;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::local_selections;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use test_case::test_case;
use wiremock::MockServer;

fn collab_mode_with_mode_and_instructions(
    mode: ModeKind,
    instructions: Option<&str>,
) -> CollaborationMode {
    CollaborationMode {
        mode,
        settings: Settings {
            model: "gpt-5.4".to_string(),
            reasoning_effort: None,
            developer_instructions: instructions.map(str::to_string),
        },
    }
}

fn collab_mode_with_instructions(instructions: Option<&str>) -> CollaborationMode {
    collab_mode_with_mode_and_instructions(ModeKind::Default, instructions)
}

fn collab_mode_for_model(
    mode: ModeKind,
    model: &str,
    instructions: Option<&str>,
) -> CollaborationMode {
    CollaborationMode {
        mode,
        settings: Settings {
            model: model.to_string(),
            reasoning_effort: None,
            developer_instructions: instructions.map(str::to_string),
        },
    }
}

fn model_with_collaboration_messages(
    slug: &str,
    default: Option<&str>,
    plan: Option<&str>,
) -> codex_protocol::openai_models::ModelInfo {
    let mut model = model_info_from_slug(slug);
    let model_messages = model.model_messages.get_or_insert(ModelMessages {
        instructions_template: None,
        instructions_variables: None,
        approvals: None,
        collaboration_modes: None,
        auto_review: None,
        permissions: None,
        multi_agent: None,
        token_budget: None,
        guardian_v2: None,
    });
    model_messages.collaboration_modes = Some(CollaborationModeMessages {
        default: default.map(str::to_string),
        plan: plan.map(str::to_string),
    });
    model
}

fn developer_texts(input: &[Value]) -> Vec<String> {
    input
        .iter()
        .filter(|item| item.get("role").and_then(Value::as_str) == Some("developer"))
        .filter_map(|item| item.get("content")?.as_array().cloned())
        .flatten()
        .filter_map(|content| {
            let text = content.get("text")?.as_str()?;
            Some(text.to_string())
        })
        .collect()
}

fn collab_xml(text: &str) -> String {
    format!("{COLLABORATION_MODE_OPEN_TAG}{text}{COLLABORATION_MODE_CLOSE_TAG}")
}

fn count_messages_containing(texts: &[String], target: &str) -> usize {
    texts.iter().filter(|text| text.contains(target)).count()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn no_collaboration_instructions_by_default() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let req = mount_sse_once(
        &server,
        sse(vec![ev_response_created("resp-1"), ev_completed("resp-1")]),
    )
    .await;

    let test = test_codex().build(&server).await?;

    test.codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: "hello".into(),
            text_elements: Vec::new(),
        }]))
        .await?;
    wait_for_event(&test.codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    let input = req.single_request().input();
    let dev_texts = developer_texts(&input);
    assert!(
        dev_texts
            .iter()
            .any(|text| text.contains("<permissions instructions>")),
        "expected permissions instructions in developer messages, got {dev_texts:?}"
    );
    assert_eq!(
        count_messages_containing(&dev_texts, COLLABORATION_MODE_OPEN_TAG),
        0
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn catalog_collaboration_messages_track_mode_changes() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let req1 = mount_sse_once(
        &server,
        sse(vec![ev_response_created("resp-1"), ev_completed("resp-1")]),
    )
    .await;
    let req2 = mount_sse_once(
        &server,
        sse(vec![ev_response_created("resp-2"), ev_completed("resp-2")]),
    )
    .await;

    let model_slug = "catalog-collaboration-model";
    let default_text = "catalog default instructions";
    let plan_text = "catalog plan instructions";
    let model = model_with_collaboration_messages(model_slug, Some(default_text), Some(plan_text));
    let mut builder = test_codex()
        .with_model(model_slug)
        .with_config(move |config| {
            config.model_catalog = Some(ModelsResponse {
                models: vec![model],
            });
        });
    let test = builder.build(&server).await?;

    core_test_support::submit_thread_settings(
        &test.codex,
        ThreadSettingsOverrides {
            collaboration_mode: Some(collab_mode_for_model(
                ModeKind::Default,
                model_slug,
                Some("legacy default instructions"),
            )),
            ..Default::default()
        },
    )
    .await?;
    test.submit_text_turn("default turn").await?;

    let first_dev_texts = developer_texts(&req1.single_request().input());
    assert_eq!(
        count_messages_containing(&first_dev_texts, &collab_xml(default_text)),
        1
    );
    assert_eq!(
        count_messages_containing(&first_dev_texts, "legacy default instructions"),
        0
    );

    core_test_support::submit_thread_settings(
        &test.codex,
        ThreadSettingsOverrides {
            collaboration_mode: Some(collab_mode_for_model(
                ModeKind::Plan,
                model_slug,
                Some("legacy plan instructions"),
            )),
            ..Default::default()
        },
    )
    .await?;
    test.submit_text_turn("plan turn").await?;

    let second_dev_texts = developer_texts(&req2.single_request().input());
    assert_eq!(
        count_messages_containing(&second_dev_texts, &collab_xml(default_text)),
        1
    );
    assert_eq!(
        count_messages_containing(&second_dev_texts, &collab_xml(plan_text)),
        1
    );
    assert_eq!(
        count_messages_containing(&second_dev_texts, "legacy plan instructions"),
        0
    );

    Ok(())
}

#[test_case(ModeKind::Default; "default")]
#[test_case(ModeKind::Plan; "plan")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn catalog_collaboration_messages_refresh_without_mode_or_model_change(
    mode: ModeKind,
) -> Result<()> {
    skip_if_no_network!(Ok(()));

    const ETAG_1: &str = "\"collaboration-models-1\"";
    const ETAG_2: &str = "\"collaboration-models-2\"";
    const ETAG_3: &str = "\"collaboration-models-3\"";
    const MODEL: &str = "catalog-collaboration-refresh-model";
    const ORIGINAL: &str = "original catalog collaboration instructions";
    const UPDATED: &str = "updated catalog collaboration instructions";
    const INACTIVE: &str = "inactive mode instructions";

    let catalog = |instructions: &str| ModelsResponse {
        models: vec![match mode {
            ModeKind::Default => {
                model_with_collaboration_messages(MODEL, Some(instructions), Some(INACTIVE))
            }
            ModeKind::Plan => {
                model_with_collaboration_messages(MODEL, Some(INACTIVE), Some(instructions))
            }
        }],
    };
    let server = MockServer::start().await;
    let mut models_mocks =
        vec![mount_models_once_with_etag(&server, catalog(ORIGINAL), ETAG_1).await];
    let test = test_codex()
        .with_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing())
        .with_model(MODEL)
        .with_config(|config| {
            config.model_provider.request_max_retries = Some(0);
            config.model_provider.stream_max_retries = Some(1);
            config
                .features
                .disable(Feature::Apps)
                .expect("test config should allow feature update");
        })
        .build_with_auto_env(&server)
        .await?;

    core_test_support::submit_thread_settings(
        &test.codex,
        ThreadSettingsOverrides {
            collaboration_mode: Some(collab_mode_for_model(
                mode,
                MODEL,
                Some("legacy fallback instructions"),
            )),
            ..Default::default()
        },
    )
    .await?;

    let history = [ORIGINAL, UPDATED, ""];
    let mut requests = Vec::new();
    for (turn, etag, refreshed_instructions, expected) in [
        ("original", ETAG_2, Some(UPDATED), &history[..1]),
        ("updated", ETAG_2, None, &history[..2]),
        ("unchanged", ETAG_3, Some(""), &history[..2]),
        ("cleared", ETAG_3, None, &history[..]),
        ("still-cleared", ETAG_3, None, &history[..]),
    ] {
        if let Some(instructions) = refreshed_instructions {
            models_mocks
                .push(mount_models_once_with_etag(&server, catalog(instructions), etag).await);
        }
        let response = mount_response_once(
            &server,
            sse_response(sse(vec![ev_response_created(turn), ev_completed(turn)]))
                .insert_header("X-Models-Etag", etag),
        )
        .await;

        test.submit_text_turn(turn).await?;
        let request = response.single_request();
        let dev_texts = request.message_input_texts("developer");
        let collaboration_instructions = dev_texts
            .iter()
            .flat_map(|text| text.split(COLLABORATION_MODE_OPEN_TAG).skip(1))
            .map(|text| {
                text.split_once(COLLABORATION_MODE_CLOSE_TAG)
                    .expect("collaboration fragment should have a closing tag")
                    .0
            })
            .collect::<Vec<_>>();
        assert_eq!(collaboration_instructions.as_slice(), expected, "{turn}");
        requests.push(request);
    }

    assert_eq!(
        models_mocks
            .iter()
            .map(|mock| mock.requests().len())
            .collect::<Vec<_>>(),
        [1, 1, 1]
    );
    for pair in requests.windows(2) {
        let previous_input = pair[0].input();
        assert_eq!(
            pair[1].input().get(..previous_input.len()),
            Some(previous_input.as_slice())
        );
        assert_eq!(pair[0].instructions_text(), pair[1].instructions_text());
    }

    Ok(())
}

#[test_case(None; "missing instructions")]
#[test_case(Some(""); "explicit empty suppresses fallback")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn empty_collaboration_guidance_clears_prior_instructions_once(
    empty_instructions: Option<&str>,
) -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let model_slug = "catalog-collaboration-clear-model";
    let empty_model_slug = "catalog-collaboration-empty-model";
    let default_text = "catalog default instructions";
    let fallback_text = "legacy fallback instructions";
    let fallback = empty_instructions.map(|_| fallback_text);
    let model =
        model_with_collaboration_messages(model_slug, Some(default_text), empty_instructions);
    let empty_model =
        model_with_collaboration_messages(empty_model_slug, empty_instructions, empty_instructions);
    let mut builder = test_codex()
        .with_model(model_slug)
        .with_config(move |config| {
            config.model_catalog = Some(ModelsResponse {
                models: vec![model, empty_model],
            });
        });
    let test = builder.build_with_auto_env(&server).await?;

    for (turn, mode, model, clear_count) in [
        ("default", ModeKind::Default, model_slug, 0),
        ("clear", ModeKind::Plan, model_slug, 1),
        ("still-clear", ModeKind::Plan, model_slug, 1),
        ("change-model", ModeKind::Plan, empty_model_slug, 1),
        ("change-mode", ModeKind::Default, empty_model_slug, 1),
    ] {
        let response = mount_sse_once(
            &server,
            sse(vec![ev_response_created(turn), ev_completed(turn)]),
        )
        .await;
        core_test_support::submit_thread_settings(
            &test.codex,
            ThreadSettingsOverrides {
                collaboration_mode: Some(collab_mode_for_model(mode, model, fallback)),
                ..Default::default()
            },
        )
        .await?;
        test.submit_text_turn(turn).await?;

        let dev_texts = response.single_request().message_input_texts("developer");
        assert_eq!(
            (
                count_messages_containing(&dev_texts, &collab_xml(default_text)),
                count_messages_containing(&dev_texts, &collab_xml("")),
                count_messages_containing(&dev_texts, fallback_text),
            ),
            (1, clear_count, 0),
            "{turn}"
        );
    }

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn model_change_appends_new_catalog_collaboration_message() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let _req1 = mount_sse_once(
        &server,
        sse(vec![ev_response_created("resp-1"), ev_completed("resp-1")]),
    )
    .await;
    let req2 = mount_sse_once(
        &server,
        sse(vec![ev_response_created("resp-2"), ev_completed("resp-2")]),
    )
    .await;

    let first_slug = "catalog-collaboration-model-a";
    let second_slug = "catalog-collaboration-model-b";
    let first_text = "model A collaboration instructions";
    let second_text = "model B collaboration instructions";
    let first = model_with_collaboration_messages(first_slug, Some(first_text), /*plan*/ None);
    let second =
        model_with_collaboration_messages(second_slug, Some(second_text), /*plan*/ None);
    let mut builder = test_codex()
        .with_model(first_slug)
        .with_config(move |config| {
            config.model_catalog = Some(ModelsResponse {
                models: vec![first, second],
            });
        });
    let test = builder.build(&server).await?;

    core_test_support::submit_thread_settings(
        &test.codex,
        ThreadSettingsOverrides {
            collaboration_mode: Some(collab_mode_for_model(
                ModeKind::Default,
                first_slug,
                Some("legacy instructions"),
            )),
            ..Default::default()
        },
    )
    .await?;
    test.submit_text_turn("first").await?;

    core_test_support::submit_thread_settings(
        &test.codex,
        ThreadSettingsOverrides {
            model: Some(second_slug.to_string()),
            ..Default::default()
        },
    )
    .await?;
    test.submit_text_turn("second").await?;

    let dev_texts = developer_texts(&req2.single_request().input());
    assert_eq!(
        count_messages_containing(&dev_texts, &collab_xml(first_text)),
        1
    );
    assert_eq!(
        count_messages_containing(&dev_texts, &collab_xml(second_text)),
        1
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn user_input_includes_collaboration_instructions_after_override() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let req = mount_sse_once(
        &server,
        sse(vec![ev_response_created("resp-1"), ev_completed("resp-1")]),
    )
    .await;

    let test = test_codex().build(&server).await?;

    let collab_text = "collab instructions";
    let collaboration_mode = collab_mode_with_instructions(Some(collab_text));
    core_test_support::submit_thread_settings(
        &test.codex,
        ThreadSettingsOverrides {
            collaboration_mode: Some(collaboration_mode),
            ..Default::default()
        },
    )
    .await?;

    test.codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: "hello".into(),
            text_elements: Vec::new(),
        }]))
        .await?;
    wait_for_event(&test.codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    let input = req.single_request().input();
    let dev_texts = developer_texts(&input);
    let collab_text = collab_xml(collab_text);
    assert_eq!(count_messages_containing(&dev_texts, &collab_text), 1);

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn collaboration_instructions_added_on_user_turn() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let req = mount_sse_once(
        &server,
        sse(vec![ev_response_created("resp-1"), ev_completed("resp-1")]),
    )
    .await;

    let test = test_codex().build(&server).await?;
    let collab_text = "turn instructions";
    let collaboration_mode = collab_mode_with_instructions(Some(collab_text));

    test.codex
        .start_or_steer_turn(
            TurnInputRequest::user_input(vec![UserInput::Text {
                text: "hello".into(),
                text_elements: Vec::new(),
            }])
            .with_thread_settings(ThreadSettingsOverrides {
                environments: Some(local_selections(test.config.cwd.clone())),
                approval_policy: Some(test.config.permissions.approval_policy.value()),
                sandbox_policy: Some(test.config.legacy_sandbox_policy()),
                summary: Some(
                    test.config
                        .model_reasoning_summary
                        .unwrap_or(codex_protocol::config_types::ReasoningSummary::Auto),
                ),
                collaboration_mode: Some(collaboration_mode),
                ..Default::default()
            }),
        )
        .await?;
    wait_for_event(&test.codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    let input = req.single_request().input();
    let dev_texts = developer_texts(&input);
    let collab_text = collab_xml(collab_text);
    assert_eq!(count_messages_containing(&dev_texts, &collab_text), 1);

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn collaboration_instructions_omitted_when_disabled() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let req = mount_sse_once(
        &server,
        sse(vec![ev_response_created("resp-1"), ev_completed("resp-1")]),
    )
    .await;

    let mut builder = test_codex().with_config(|config| {
        config.include_collaboration_mode_instructions = false;
    });
    let test = builder.build(&server).await?;
    let collaboration_mode = collab_mode_with_instructions(Some("turn instructions"));

    test.codex
        .start_or_steer_turn(
            TurnInputRequest::user_input(vec![UserInput::Text {
                text: "hello".into(),
                text_elements: Vec::new(),
            }])
            .with_thread_settings(ThreadSettingsOverrides {
                environments: Some(local_selections(test.config.cwd.clone())),
                approval_policy: Some(test.config.permissions.approval_policy.value()),
                sandbox_policy: Some(test.config.legacy_sandbox_policy()),
                summary: Some(
                    test.config
                        .model_reasoning_summary
                        .unwrap_or(codex_protocol::config_types::ReasoningSummary::Auto),
                ),
                collaboration_mode: Some(collaboration_mode),
                ..Default::default()
            }),
        )
        .await?;
    wait_for_event(&test.codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    let input = req.single_request().input();
    let dev_texts = developer_texts(&input);
    assert_eq!(
        count_messages_containing(&dev_texts, COLLABORATION_MODE_OPEN_TAG),
        0
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn override_then_next_turn_uses_updated_collaboration_instructions() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let req = mount_sse_once(
        &server,
        sse(vec![ev_response_created("resp-1"), ev_completed("resp-1")]),
    )
    .await;

    let test = test_codex().build(&server).await?;
    let collab_text = "override instructions";
    let collaboration_mode = collab_mode_with_instructions(Some(collab_text));

    core_test_support::submit_thread_settings(
        &test.codex,
        ThreadSettingsOverrides {
            collaboration_mode: Some(collaboration_mode),
            ..Default::default()
        },
    )
    .await?;

    test.codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: "hello".into(),
            text_elements: Vec::new(),
        }]))
        .await?;
    wait_for_event(&test.codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    let input = req.single_request().input();
    let dev_texts = developer_texts(&input);
    let collab_text = collab_xml(collab_text);
    assert_eq!(count_messages_containing(&dev_texts, &collab_text), 1);

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn user_turn_overrides_collaboration_instructions_after_override() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let req = mount_sse_once(
        &server,
        sse(vec![ev_response_created("resp-1"), ev_completed("resp-1")]),
    )
    .await;

    let test = test_codex().build(&server).await?;
    let base_text = "base instructions";
    let base_mode = collab_mode_with_instructions(Some(base_text));
    let turn_text = "turn override";
    let turn_mode = collab_mode_with_instructions(Some(turn_text));

    core_test_support::submit_thread_settings(
        &test.codex,
        ThreadSettingsOverrides {
            collaboration_mode: Some(base_mode),
            ..Default::default()
        },
    )
    .await?;

    test.codex
        .start_or_steer_turn(
            TurnInputRequest::user_input(vec![UserInput::Text {
                text: "hello".into(),
                text_elements: Vec::new(),
            }])
            .with_thread_settings(ThreadSettingsOverrides {
                environments: Some(local_selections(test.config.cwd.clone())),
                approval_policy: Some(test.config.permissions.approval_policy.value()),
                sandbox_policy: Some(test.config.legacy_sandbox_policy()),
                summary: Some(
                    test.config
                        .model_reasoning_summary
                        .unwrap_or(codex_protocol::config_types::ReasoningSummary::Auto),
                ),
                collaboration_mode: Some(turn_mode),
                ..Default::default()
            }),
        )
        .await?;
    wait_for_event(&test.codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    let input = req.single_request().input();
    let dev_texts = developer_texts(&input);
    let base_text = collab_xml(base_text);
    let turn_text = collab_xml(turn_text);
    assert_eq!(count_messages_containing(&dev_texts, &base_text), 0);
    assert_eq!(count_messages_containing(&dev_texts, &turn_text), 1);

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn collaboration_mode_update_appends_instruction_changes_within_same_mode() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let _req1 = mount_sse_once(
        &server,
        sse(vec![ev_response_created("resp-1"), ev_completed("resp-1")]),
    )
    .await;
    let req2 = mount_sse_once(
        &server,
        sse(vec![ev_response_created("resp-2"), ev_completed("resp-2")]),
    )
    .await;

    let test = test_codex().build(&server).await?;
    let first_text = "first instructions";
    let second_text = "second instructions";

    core_test_support::submit_thread_settings(
        &test.codex,
        ThreadSettingsOverrides {
            collaboration_mode: Some(collab_mode_with_instructions(Some(first_text))),
            ..Default::default()
        },
    )
    .await?;

    test.codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: "hello 1".into(),
            text_elements: Vec::new(),
        }]))
        .await?;
    wait_for_event(&test.codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    core_test_support::submit_thread_settings(
        &test.codex,
        ThreadSettingsOverrides {
            collaboration_mode: Some(collab_mode_with_instructions(Some(second_text))),
            ..Default::default()
        },
    )
    .await?;

    test.codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: "hello 2".into(),
            text_elements: Vec::new(),
        }]))
        .await?;
    wait_for_event(&test.codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    let input = req2.single_request().input();
    let dev_texts = developer_texts(&input);
    let first_text = collab_xml(first_text);
    let second_text = collab_xml(second_text);
    assert_eq!(count_messages_containing(&dev_texts, &first_text), 1);
    assert_eq!(count_messages_containing(&dev_texts, &second_text), 1);

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn collaboration_mode_update_noop_does_not_append() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let _req1 = mount_sse_once(
        &server,
        sse(vec![ev_response_created("resp-1"), ev_completed("resp-1")]),
    )
    .await;
    let req2 = mount_sse_once(
        &server,
        sse(vec![ev_response_created("resp-2"), ev_completed("resp-2")]),
    )
    .await;

    let test = test_codex().build(&server).await?;
    let collab_text = "same instructions";

    core_test_support::submit_thread_settings(
        &test.codex,
        ThreadSettingsOverrides {
            collaboration_mode: Some(collab_mode_with_instructions(Some(collab_text))),
            ..Default::default()
        },
    )
    .await?;

    test.codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: "hello 1".into(),
            text_elements: Vec::new(),
        }]))
        .await?;
    wait_for_event(&test.codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    core_test_support::submit_thread_settings(
        &test.codex,
        ThreadSettingsOverrides {
            collaboration_mode: Some(collab_mode_with_instructions(Some(collab_text))),
            ..Default::default()
        },
    )
    .await?;

    test.codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: "hello 2".into(),
            text_elements: Vec::new(),
        }]))
        .await?;
    wait_for_event(&test.codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    let input = req2.single_request().input();
    let dev_texts = developer_texts(&input);
    let collab_text = collab_xml(collab_text);
    assert_eq!(count_messages_containing(&dev_texts, &collab_text), 1);

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn collaboration_mode_update_emits_new_instruction_message_when_mode_changes() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let _req1 = mount_sse_once(
        &server,
        sse(vec![ev_response_created("resp-1"), ev_completed("resp-1")]),
    )
    .await;
    let req2 = mount_sse_once(
        &server,
        sse(vec![ev_response_created("resp-2"), ev_completed("resp-2")]),
    )
    .await;

    let test = test_codex().build(&server).await?;
    let default_text = "default mode instructions";
    let plan_text = "plan mode instructions";

    core_test_support::submit_thread_settings(
        &test.codex,
        ThreadSettingsOverrides {
            collaboration_mode: Some(collab_mode_with_mode_and_instructions(
                ModeKind::Default,
                Some(default_text),
            )),
            ..Default::default()
        },
    )
    .await?;

    test.codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: "hello 1".into(),
            text_elements: Vec::new(),
        }]))
        .await?;
    wait_for_event(&test.codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    core_test_support::submit_thread_settings(
        &test.codex,
        ThreadSettingsOverrides {
            collaboration_mode: Some(collab_mode_with_mode_and_instructions(
                ModeKind::Plan,
                Some(plan_text),
            )),
            ..Default::default()
        },
    )
    .await?;

    test.codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: "hello 2".into(),
            text_elements: Vec::new(),
        }]))
        .await?;
    wait_for_event(&test.codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    let input = req2.single_request().input();
    let dev_texts = developer_texts(&input);
    let default_text = collab_xml(default_text);
    let plan_text = collab_xml(plan_text);
    assert_eq!(count_messages_containing(&dev_texts, &default_text), 1);
    assert_eq!(count_messages_containing(&dev_texts, &plan_text), 1);

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn collaboration_mode_update_noop_does_not_append_when_mode_is_unchanged() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let _req1 = mount_sse_once(
        &server,
        sse(vec![ev_response_created("resp-1"), ev_completed("resp-1")]),
    )
    .await;
    let req2 = mount_sse_once(
        &server,
        sse(vec![ev_response_created("resp-2"), ev_completed("resp-2")]),
    )
    .await;

    let test = test_codex().build(&server).await?;
    let collab_text = "mode-stable instructions";

    core_test_support::submit_thread_settings(
        &test.codex,
        ThreadSettingsOverrides {
            collaboration_mode: Some(collab_mode_with_mode_and_instructions(
                ModeKind::Default,
                Some(collab_text),
            )),
            ..Default::default()
        },
    )
    .await?;

    test.codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: "hello 1".into(),
            text_elements: Vec::new(),
        }]))
        .await?;
    wait_for_event(&test.codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    core_test_support::submit_thread_settings(
        &test.codex,
        ThreadSettingsOverrides {
            collaboration_mode: Some(collab_mode_with_mode_and_instructions(
                ModeKind::Default,
                Some(collab_text),
            )),
            ..Default::default()
        },
    )
    .await?;

    test.codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: "hello 2".into(),
            text_elements: Vec::new(),
        }]))
        .await?;
    wait_for_event(&test.codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    let input = req2.single_request().input();
    let dev_texts = developer_texts(&input);
    let collab_text = collab_xml(collab_text);
    assert_eq!(count_messages_containing(&dev_texts, &collab_text), 1);

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn resume_replays_collaboration_instructions() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let _req1 = mount_sse_once(
        &server,
        sse(vec![ev_response_created("resp-1"), ev_completed("resp-1")]),
    )
    .await;
    let req2 = mount_sse_once(
        &server,
        sse(vec![ev_response_created("resp-2"), ev_completed("resp-2")]),
    )
    .await;

    let mut builder = test_codex();
    let initial = builder.build(&server).await?;

    let collab_text = "resume instructions";
    core_test_support::submit_thread_settings(
        &initial.codex,
        ThreadSettingsOverrides {
            collaboration_mode: Some(collab_mode_with_instructions(Some(collab_text))),
            ..Default::default()
        },
    )
    .await?;

    initial
        .codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: "hello".into(),
            text_elements: Vec::new(),
        }]))
        .await?;
    wait_for_event(&initial.codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    let resumed = builder.restart(&server, &initial).await?;
    resumed
        .codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: "after resume".into(),
            text_elements: Vec::new(),
        }]))
        .await?;
    wait_for_event(&resumed.codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    let input = req2.single_request().input();
    let dev_texts = developer_texts(&input);
    let collab_text = collab_xml(collab_text);
    assert_eq!(count_messages_containing(&dev_texts, &collab_text), 1);

    Ok(())
}

#[test_case(json!("default"); "mode only")]
#[test_case(json!({"mode": "default", "model": "catalog-collaboration-resume-model"}); "mode and model")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cold_resume_refreshes_legacy_collaboration_snapshot_once(
    legacy_snapshot: Value,
) -> Result<()> {
    skip_if_no_network!(Ok(()));

    const MODEL: &str = "catalog-collaboration-resume-model";
    const ORIGINAL: &str = "original collaboration instructions";
    const CURRENT: &str = "current collaboration instructions";
    let builder_with_instructions = |instructions: &str| {
        let model =
            model_with_collaboration_messages(MODEL, Some(instructions), /*plan*/ None);
        test_codex().with_model(MODEL).with_config(move |config| {
            config.model_catalog = Some(ModelsResponse {
                models: vec![model],
            });
        })
    };
    let server = start_mock_server().await;
    let initial_response = mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("initial"),
            ev_assistant_message("initial-message", "original response"),
            ev_completed("initial"),
        ]),
    )
    .await;
    let initial = builder_with_instructions(ORIGINAL)
        .build_with_auto_env(&server)
        .await?;
    initial.submit_text_turn("before upgrading").await?;

    let home = initial.home.clone();
    let rollout_path = initial
        .session_configured
        .rollout_path
        .clone()
        .expect("initial session should have a rollout path");
    initial.codex.shutdown_and_wait().await?;
    let mut rewrote_snapshot = false;
    let legacy_rollout = std::fs::read_to_string(&rollout_path)?
        .lines()
        .map(|original_line| {
            let mut line = serde_json::from_str::<RolloutLine>(original_line)?;
            if let RolloutItem::WorldState(world_state) = &mut line.item
                && let Some(snapshot) = world_state.state.get_mut("collaboration_mode")
            {
                *snapshot = legacy_snapshot.clone();
                rewrote_snapshot = true;
                serde_json::to_string(&line)
            } else {
                Ok(original_line.to_string())
            }
        })
        .collect::<std::result::Result<Vec<_>, _>>()?
        .join("\n");
    assert!(
        rewrote_snapshot,
        "initial rollout should record collaboration mode"
    );
    std::fs::write(&rollout_path, format!("{legacy_rollout}\n"))?;

    let resumed = builder_with_instructions(CURRENT)
        .resume(&server, home, rollout_path)
        .await?;
    let mut previous_input = initial_response.single_request().input();
    for turn in ["first-resumed", "second-resumed"] {
        let response = mount_sse_once(
            &server,
            sse(vec![ev_response_created(turn), ev_completed(turn)]),
        )
        .await;
        resumed.submit_text_turn(turn).await?;
        let request = response.single_request();
        let input = request.input();
        assert_eq!(
            input.get(..previous_input.len()),
            Some(previous_input.as_slice())
        );
        let dev_texts = request.message_input_texts("developer");
        assert_eq!(
            (
                count_messages_containing(&dev_texts, &collab_xml(ORIGINAL)),
                count_messages_containing(&dev_texts, &collab_xml(CURRENT)),
                count_messages_containing(&dev_texts, COLLABORATION_MODE_OPEN_TAG),
            ),
            (1, 1, 2),
            "{turn}"
        );
        previous_input = input;
    }

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn empty_collaboration_instructions_are_ignored() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let req = mount_sse_once(
        &server,
        sse(vec![ev_response_created("resp-1"), ev_completed("resp-1")]),
    )
    .await;

    let test = test_codex().build(&server).await?;
    let current_model = test.session_configured.model.clone();

    core_test_support::submit_thread_settings(
        &test.codex,
        ThreadSettingsOverrides {
            collaboration_mode: Some(CollaborationMode {
                mode: ModeKind::Default,
                settings: Settings {
                    model: current_model,
                    reasoning_effort: None,
                    developer_instructions: Some("".to_string()),
                },
            }),
            ..Default::default()
        },
    )
    .await?;

    test.codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: "hello".into(),
            text_elements: Vec::new(),
        }]))
        .await?;
    wait_for_event(&test.codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    let input = req.single_request().input();
    let dev_texts = developer_texts(&input);
    let collab_text = collab_xml("");
    assert_eq!(count_messages_containing(&dev_texts, &collab_text), 0);

    Ok(())
}

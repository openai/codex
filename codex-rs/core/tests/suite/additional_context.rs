use anyhow::Result;
use codex_core::TurnInputRequest;
use codex_features::Feature;
use codex_otel::ADDITIONAL_CONTEXT_COUNT_METRIC;
use codex_otel::ADDITIONAL_CONTEXT_VALUE_BYTES_METRIC;
use codex_otel::MetricsClient;
use codex_otel::MetricsConfig;
use codex_protocol::error::CodexErrorDetails;
use codex_protocol::items::TurnItem;
use codex_protocol::protocol::AdditionalContextEntry;
use codex_protocol::protocol::AdditionalContextKind;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::ItemCompletedEvent;
use codex_protocol::protocol::ThreadSettingsOverrides;
use codex_protocol::user_input::UserInput;
use core_test_support::context_snapshot;
use core_test_support::context_snapshot::ContextSnapshotOptions;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_sse_once;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event_match;
use opentelemetry_sdk::metrics::InMemoryMetricExporter;
use opentelemetry_sdk::metrics::data::AggregatedMetrics;
use opentelemetry_sdk::metrics::data::MetricData;
use opentelemetry_sdk::metrics::data::ScopeMetrics;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::collections::BTreeMap;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn additional_context_is_model_visible_but_not_a_user_message_item() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let request = mount_sse_once(
        &server,
        sse(vec![ev_response_created("resp-1"), ev_completed("resp-1")]),
    )
    .await;
    let test = test_codex()
        .with_config(|config| {
            config.update_plan_enabled = true;
            config.include_environment_context = false;
        })
        .build(&server)
        .await?;

    test.codex
        .start_or_steer_turn(
            TurnInputRequest::user_input(vec![UserInput::Text {
                text: "inspect the active tab".to_string(),
                text_elements: Vec::new(),
            }])
            .with_additional_context(BTreeMap::from([
                (
                    "browser_info".to_string(),
                    AdditionalContextEntry::new(
                        "tab one".to_string(),
                        AdditionalContextKind::Untrusted,
                    ),
                ),
                (
                    "automation_info".to_string(),
                    AdditionalContextEntry::new(
                        "run one".to_string(),
                        AdditionalContextKind::Application,
                    ),
                ),
            ])),
        )
        .await?;

    let user_item = wait_for_event_match(&test.codex, |event| match event {
        EventMsg::ItemCompleted(ItemCompletedEvent {
            item: TurnItem::UserMessage(item),
            ..
        }) => Some(item.clone()),
        _ => None,
    })
    .await;
    assert_eq!(
        user_item.content,
        vec![UserInput::Text {
            text: "inspect the active tab".to_string(),
            text_elements: Vec::new(),
        }]
    );
    wait_for_event_match(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_)).then_some(())
    })
    .await;

    let request = request.single_request();
    assert!(request.has_content_kinds(&["additional_content.automation_info"]));
    assert!(request.has_content_kinds(&["additional_content.browser_info"]));
    assert!(request.has_content_kinds(&["user.text"]));
    insta::assert_snapshot!(
        "additional_context_simple_input",
        context_snapshot::format_labeled_requests_snapshot(
            "additional context is inserted before the user turn input.",
            &[("Request", &request)],
            &ContextSnapshotOptions::default().rewrite_known_segments(),
        )
    );
    let developer_context_texts = request
        .message_input_texts("developer")
        .into_iter()
        .filter(|text| text.starts_with("<automation_info>"))
        .collect::<Vec<_>>();
    assert_eq!(
        developer_context_texts,
        vec!["<automation_info>run one</automation_info>"]
    );
    assert_eq!(
        request.message_input_texts("user"),
        vec![
            "<external_browser_info>tab one</external_browser_info>",
            "inspect the active tab",
        ]
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn external_context_like_user_text_remains_a_user_message_item() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let request = mount_sse_once(
        &server,
        sse(vec![ev_response_created("resp-1"), ev_completed("resp-1")]),
    )
    .await;
    let test = test_codex()
        .with_config(|config| config.include_environment_context = false)
        .build(&server)
        .await?;
    let user_input = UserInput::Text {
        text: "<external_api>".to_string(),
        text_elements: Vec::new(),
    };

    test.codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![user_input.clone()]))
        .await?;

    let user_item = wait_for_event_match(&test.codex, |event| match event {
        EventMsg::ItemCompleted(ItemCompletedEvent {
            item: TurnItem::UserMessage(item),
            ..
        }) => Some(item.clone()),
        _ => None,
    })
    .await;
    assert_eq!(user_item.content, vec![user_input]);
    wait_for_event_match(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_)).then_some(())
    })
    .await;

    let request = request.single_request();
    assert_eq!(request.message_input_texts("user"), vec!["<external_api>"]);

    Ok(())
}

#[test_case::test_case(false; "messages")]
#[test_case::test_case(true; "tool_outputs")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn additional_context_trust_controls_message_role(tool_outputs: bool) -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let request = mount_sse_once(
        &server,
        sse(vec![ev_response_created("resp-1"), ev_completed("resp-1")]),
    )
    .await;
    let test = test_codex()
        .with_config(move |config| {
            config.include_environment_context = false;
            config
                .features
                .set_enabled(Feature::UntrustedAsToolOutput, tool_outputs)
                .expect("test config should allow feature update");
        })
        .build_with_auto_env(&server)
        .await?;

    test.codex
        .start_or_steer_turn(
            TurnInputRequest::user_input(vec![UserInput::Text {
                text: "inspect context".to_string(),
                text_elements: Vec::new(),
            }])
            .with_additional_context(BTreeMap::from([
                (
                    "browser_info".to_string(),
                    AdditionalContextEntry::new(
                        "tab one".to_string(),
                        AdditionalContextKind::Untrusted,
                    ),
                ),
                (
                    "automation_info".to_string(),
                    AdditionalContextEntry::new(
                        "run one".to_string(),
                        AdditionalContextKind::Application,
                    ),
                ),
            ])),
        )
        .await?;
    wait_for_event_match(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_)).then_some(())
    })
    .await;

    let request = request.single_request();
    let developer_context_texts = request
        .message_input_texts("developer")
        .into_iter()
        .filter(|text| text.starts_with("<automation_info>"))
        .collect::<Vec<_>>();
    assert_eq!(
        developer_context_texts,
        vec!["<automation_info>run one</automation_info>"]
    );
    if tool_outputs {
        assert_eq!(request.message_input_texts("user"), vec!["inspect context"]);
        let outputs = request.inputs_of_type("function_call_output");
        assert_eq!(
            outputs
                .iter()
                .map(|output| (
                    output["name"].clone(),
                    output["namespace"].clone(),
                    output["output"].clone(),
                    output.get("call_id").cloned(),
                ))
                .collect::<Vec<_>>(),
            vec![(
                json!("browser_info"),
                json!("agent"),
                json!("tab one"),
                None
            )],
        );
        assert!(request.inputs_of_type("function_call").is_empty());
    } else {
        assert_eq!(
            request.message_input_texts("user"),
            vec![
                "<external_browser_info>tab one</external_browser_info>",
                "inspect context"
            ],
        );
        assert!(request.inputs_of_type("function_call_output").is_empty());
    }

    Ok(())
}

#[test_case::test_case(None, None; "unchanged")]
#[test_case::test_case(Some("browser"), Some("editor"); "name_only_change")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn additional_context_is_deduplicated_between_turns_while_retained(
    first_name: Option<&str>,
    second_name: Option<&str>,
) -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let first_request = mount_sse_once(
        &server,
        sse(vec![ev_response_created("resp-1"), ev_completed("resp-1")]),
    )
    .await;
    let second_request = mount_sse_once(
        &server,
        sse(vec![ev_response_created("resp-2"), ev_completed("resp-2")]),
    )
    .await;
    let test = test_codex()
        .with_config(|config| config.include_environment_context = false)
        .build(&server)
        .await?;
    let mut additional_context = BTreeMap::from([(
        "browser_info".to_string(),
        AdditionalContextEntry {
            value: "same tab".to_string(),
            kind: AdditionalContextKind::Untrusted,
            name: first_name.map(str::to_owned),
        },
    )]);

    test.codex
        .start_or_steer_turn(
            TurnInputRequest::user_input(vec![UserInput::Text {
                text: "first turn".to_string(),
                text_elements: Vec::new(),
            }])
            .with_additional_context(additional_context.clone()),
        )
        .await?;
    wait_for_event_match(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_)).then_some(())
    })
    .await;

    additional_context
        .get_mut("browser_info")
        .expect("browser context")
        .name = second_name.map(str::to_owned);
    test.codex
        .start_or_steer_turn(
            TurnInputRequest::user_input(vec![UserInput::Text {
                text: "second turn".to_string(),
                text_elements: Vec::new(),
            }])
            .with_additional_context(additional_context),
        )
        .await?;
    wait_for_event_match(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_)).then_some(())
    })
    .await;

    assert_eq!(
        first_request.single_request().message_input_texts("user"),
        vec![
            "<external_browser_info>same tab</external_browser_info>",
            "first turn",
        ]
    );
    assert_eq!(
        second_request.single_request().message_input_texts("user"),
        vec![
            "<external_browser_info>same tab</external_browser_info>",
            "first turn",
            "second turn",
        ]
    );

    Ok(())
}

#[test_case::test_case(false; "messages")]
#[test_case::test_case(true; "tool_outputs")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn additional_context_removes_one_value_while_adding_another(
    tool_outputs: bool,
) -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let first_request = mount_sse_once(
        &server,
        sse(vec![ev_response_created("resp-1"), ev_completed("resp-1")]),
    )
    .await;
    let second_request = mount_sse_once(
        &server,
        sse(vec![ev_response_created("resp-2"), ev_completed("resp-2")]),
    )
    .await;
    let third_request = mount_sse_once(
        &server,
        sse(vec![ev_response_created("resp-3"), ev_completed("resp-3")]),
    )
    .await;
    let test = test_codex()
        .with_config(move |config| {
            config.include_environment_context = false;
            config
                .features
                .set_enabled(Feature::UntrustedAsToolOutput, tool_outputs)
                .expect("test config should allow feature update");
        })
        .build_with_auto_env(&server)
        .await?;

    test.codex
        .start_or_steer_turn(
            TurnInputRequest::user_input(vec![UserInput::Text {
                text: "first turn".to_string(),
                text_elements: Vec::new(),
            }])
            .with_additional_context(BTreeMap::from([
                (
                    "automation_info".to_string(),
                    AdditionalContextEntry::new(
                        "run one".to_string(),
                        AdditionalContextKind::Untrusted,
                    ),
                ),
                (
                    "browser_info".to_string(),
                    AdditionalContextEntry::new(
                        "tab one".to_string(),
                        AdditionalContextKind::Untrusted,
                    ),
                ),
            ])),
        )
        .await?;
    wait_for_event_match(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_)).then_some(())
    })
    .await;

    test.codex
        .start_or_steer_turn(
            TurnInputRequest::user_input(vec![UserInput::Text {
                text: "second turn".to_string(),
                text_elements: Vec::new(),
            }])
            .with_additional_context(BTreeMap::from([
                (
                    "automation_info".to_string(),
                    AdditionalContextEntry::new(
                        "run one".to_string(),
                        AdditionalContextKind::Untrusted,
                    ),
                ),
                (
                    "terminal_info".to_string(),
                    AdditionalContextEntry::new(
                        "pty one".to_string(),
                        AdditionalContextKind::Untrusted,
                    ),
                ),
            ])),
        )
        .await?;
    wait_for_event_match(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_)).then_some(())
    })
    .await;

    test.codex
        .start_or_steer_turn(
            TurnInputRequest::user_input(vec![UserInput::Text {
                text: "third turn".to_string(),
                text_elements: Vec::new(),
            }])
            .with_additional_context(BTreeMap::from([
                (
                    "automation_info".to_string(),
                    AdditionalContextEntry::new(
                        "run one".to_string(),
                        AdditionalContextKind::Untrusted,
                    ),
                ),
                (
                    "browser_info".to_string(),
                    AdditionalContextEntry::new(
                        "tab one".to_string(),
                        AdditionalContextKind::Untrusted,
                    ),
                ),
                (
                    "terminal_info".to_string(),
                    AdditionalContextEntry::new(
                        "pty one".to_string(),
                        AdditionalContextKind::Untrusted,
                    ),
                ),
            ])),
        )
        .await?;
    wait_for_event_match(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_)).then_some(())
    })
    .await;

    if tool_outputs {
        for (request, expected_outputs, expected_users) in [
            (
                first_request.single_request(),
                vec!["run one", "tab one"],
                vec!["first turn"],
            ),
            (
                second_request.single_request(),
                vec!["run one", "tab one", "pty one"],
                vec!["first turn", "second turn"],
            ),
            (
                third_request.single_request(),
                vec!["run one", "tab one", "pty one", "tab one"],
                vec!["first turn", "second turn", "third turn"],
            ),
        ] {
            let outputs = request.inputs_of_type("function_call_output");
            assert_eq!(
                outputs
                    .iter()
                    .map(|output| output["output"].as_str().expect("text output"))
                    .collect::<Vec<_>>(),
                expected_outputs,
            );
            assert_eq!(request.message_input_texts("user"), expected_users);
        }
        return Ok(());
    }

    assert_eq!(
        first_request.single_request().message_input_texts("user"),
        vec![
            "<external_automation_info>run one</external_automation_info>",
            "<external_browser_info>tab one</external_browser_info>",
            "first turn",
        ]
    );
    assert_eq!(
        second_request.single_request().message_input_texts("user"),
        vec![
            "<external_automation_info>run one</external_automation_info>",
            "<external_browser_info>tab one</external_browser_info>",
            "first turn",
            "<external_terminal_info>pty one</external_terminal_info>",
            "second turn",
        ]
    );
    assert_eq!(
        third_request.single_request().message_input_texts("user"),
        vec![
            "<external_automation_info>run one</external_automation_info>",
            "<external_browser_info>tab one</external_browser_info>",
            "first turn",
            "<external_terminal_info>pty one</external_terminal_info>",
            "second turn",
            "<external_browser_info>tab one</external_browser_info>",
            "third turn",
        ]
    );

    Ok(())
}

#[test_case::test_case(false; "messages")]
#[test_case::test_case(true; "tool_outputs")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn additional_context_values_are_truncated_before_model_input(
    tool_outputs: bool,
) -> Result<()> {
    skip_if_no_network!(Ok(()));

    const MAX_EXPECTED_EXTERNAL_CONTEXT_TEXT_BYTES: usize = 5 * 1024;

    let server = start_mock_server().await;
    let request = mount_sse_once(
        &server,
        sse(vec![ev_response_created("resp-1"), ev_completed("resp-1")]),
    )
    .await;
    let test = test_codex()
        .with_config(move |config| {
            config.include_environment_context = false;
            config
                .features
                .set_enabled(Feature::UntrustedAsToolOutput, tool_outputs)
                .expect("test config should allow feature update");
        })
        .build_with_auto_env(&server)
        .await?;
    let long_browser_value = format!("browser-head-{}browser-tail", "b".repeat(40_000));
    let long_automation_value = format!("automation-head-{}automation-tail", "a".repeat(40_000));
    let untruncated_browser_fragment =
        format!("<external_browser_info>{long_browser_value}</external_browser_info>");
    let untruncated_automation_fragment =
        format!("<automation_info>{long_automation_value}</automation_info>");

    test.codex
        .start_or_steer_turn(
            TurnInputRequest::user_input(vec![UserInput::Text {
                text: "summarize context".to_string(),
                text_elements: Vec::new(),
            }])
            .with_additional_context(BTreeMap::from([
                (
                    "automation_info".to_string(),
                    AdditionalContextEntry::new(
                        long_automation_value.clone(),
                        AdditionalContextKind::Application,
                    ),
                ),
                (
                    "browser_info".to_string(),
                    AdditionalContextEntry::new(
                        long_browser_value.clone(),
                        AdditionalContextKind::Untrusted,
                    ),
                ),
            ])),
        )
        .await?;
    wait_for_event_match(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_)).then_some(())
    })
    .await;

    let request = request.single_request();
    let developer_texts = request
        .message_input_texts("developer")
        .into_iter()
        .filter(|text| text.starts_with("<automation_info>"))
        .collect::<Vec<_>>();
    let [automation_text] = developer_texts.as_slice() else {
        panic!("expected application additional context, got {developer_texts:?}");
    };
    assert!(automation_text.starts_with(&format!(
        "<automation_info>automation-head-{}",
        "a".repeat(1024)
    )));
    assert!(automation_text.contains("tokens truncated"));
    assert!(automation_text.ends_with("automation-tail</automation_info>"));
    assert!(automation_text.len() < untruncated_automation_fragment.len());
    assert!(
        automation_text.len() <= MAX_EXPECTED_EXTERNAL_CONTEXT_TEXT_BYTES,
        "application additional context was not capped before model input: {} bytes",
        automation_text.len()
    );

    if tool_outputs {
        assert_eq!(
            request.message_input_texts("user"),
            vec!["summarize context"]
        );
        let outputs = request.inputs_of_type("function_call_output");
        assert_eq!(outputs.len(), 1);
        assert_eq!(outputs[0]["name"], "browser_info");
        assert_eq!(outputs[0]["namespace"], "agent");
        let body = outputs[0]["output"].as_str().expect("text output");
        assert!(body.starts_with(&format!("browser-head-{}", "b".repeat(1024))));
        assert!(body.ends_with("browser-tail"));
        assert!(body.contains("tokens truncated"));
        assert!(body.len() <= MAX_EXPECTED_EXTERNAL_CONTEXT_TEXT_BYTES);
        let mut changed_middle = long_browser_value.clone();
        changed_middle.replace_range(20_000..20_001, "c");
        let changed_prefix = changed_middle.replacen("browser-head", "browser-view", 1);
        for (value, expected_count) in [(changed_middle, 1), (changed_prefix, 2)] {
            let next = mount_sse_once(&server, sse(vec![ev_completed("updated-context")])).await;
            test.codex
                .start_or_steer_turn(
                    TurnInputRequest::user_input(vec![UserInput::Text {
                        text: "check updated context".to_owned(),
                        text_elements: Vec::new(),
                    }])
                    .with_additional_context(BTreeMap::from([
                        (
                            "automation_info".to_owned(),
                            AdditionalContextEntry {
                                name: None,
                                value: long_automation_value.clone(),
                                kind: AdditionalContextKind::Application,
                            },
                        ),
                        (
                            "browser_info".to_owned(),
                            AdditionalContextEntry {
                                name: None,
                                value,
                                kind: AdditionalContextKind::Untrusted,
                            },
                        ),
                    ])),
                )
                .await?;
            wait_for_event_match(&test.codex, |event| {
                matches!(event, EventMsg::TurnComplete(_)).then_some(())
            })
            .await;
            let next = next.single_request();
            let outputs = next.inputs_of_type("function_call_output");
            assert_eq!(outputs.len(), expected_count);
            assert_eq!(outputs[0]["output"], body);
            if expected_count == 2 {
                assert!(
                    outputs[1]["output"]
                        .as_str()
                        .expect("text output")
                        .starts_with("browser-view")
                );
            }
        }
        return Ok(());
    }

    let user_texts = request.message_input_texts("user");
    let [external_text, user_text] = user_texts.as_slice() else {
        panic!("expected external context plus user input, got {user_texts:?}");
    };
    assert_eq!(user_text, "summarize context");
    assert!(external_text.starts_with(&format!(
        "<external_browser_info>browser-head-{}",
        "b".repeat(1024)
    )));
    assert!(external_text.contains("tokens truncated"));
    assert!(external_text.ends_with("browser-tail</external_browser_info>"));
    assert!(external_text.len() < untruncated_browser_fragment.len());
    assert!(
        external_text.len() <= MAX_EXPECTED_EXTERNAL_CONTEXT_TEXT_BYTES,
        "untrusted additional context was not capped before model input: {} bytes",
        external_text.len()
    );

    Ok(())
}

#[test_case::test_case(false; "messages")]
#[test_case::test_case(true; "tool_outputs")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn additional_context_metrics_record_submitted_sizes_and_truncation(
    tool_outputs: bool,
) -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let metrics = MetricsClient::new(
        MetricsConfig::in_memory(
            "test",
            "additional-context",
            env!("CARGO_PKG_VERSION"),
            InMemoryMetricExporter::default(),
        )
        .with_runtime_reader(),
    )?;
    let test = test_codex()
        .with_model("gpt-6-sol")
        .with_config(move |config| {
            config
                .features
                .set_enabled(Feature::UntrustedAsToolOutput, tool_outputs)
                .expect("test config should allow feature update");
        })
        .build_with_auto_env(&server)
        .await?;
    let mut options = test.start_thread_options().await;
    options.thread_extension_init.insert(metrics.clone());
    let thread = test.thread_manager.start_thread(options).await?.thread;
    let application = AdditionalContextKind::Application;
    let untrusted = AdditionalContextKind::Untrusted;
    let cases = [
        (
            "application_limit",
            application,
            None,
            "a".repeat(4_000),
            false,
        ),
        (
            "application_over",
            application,
            None,
            "a".repeat(4_001),
            true,
        ),
        (
            "opaque_named_limit",
            untrusted,
            Some("browser_limit"),
            "b".repeat(4_000),
            false,
        ),
        (
            "opaque_named_over",
            untrusted,
            Some("browser_over"),
            "b".repeat(4_001),
            true,
        ),
        // Size is measured in UTF-8 bytes; the unnamed entry must not acquire a key-derived label.
        ("opaque_unnamed", untrusted, None, "é".repeat(2_001), true),
        ("opaque_empty", untrusted, None, String::new(), false),
    ];
    let values = cases
        .iter()
        .map(|(key, kind, name, value, _)| {
            (
                key.to_string(),
                AdditionalContextEntry {
                    name: name.map(str::to_owned),
                    kind: *kind,
                    value: value.clone(),
                },
            )
        })
        .collect::<BTreeMap<_, _>>();
    for turn in 0..2 {
        let request = mount_sse_once(&server, sse(vec![ev_completed("context-metrics")])).await;
        thread
            .start_or_steer_turn(
                TurnInputRequest::user_input(vec![UserInput::Text {
                    text: format!("inspect context {turn}"),
                    text_elements: Vec::new(),
                }])
                .with_additional_context(values.clone())
                .with_thread_settings(ThreadSettingsOverrides {
                    model: Some("gpt-5.5".to_string()),
                    ..Default::default()
                }),
            )
            .await?;
        wait_for_event_match(&thread, |event| {
            matches!(event, EventMsg::TurnComplete(_)).then_some(())
        })
        .await;
        let request = request.single_request();
        assert_eq!(request.body_json()["model"], "gpt-5.5");
        // Repeated submissions are measured even though retained context is deduplicated.
        assert_eq!(
            request.inputs_of_type("function_call_output").len(),
            if tool_outputs { 4 } else { 0 }
        );
        for (key, kind, _, _, _) in &cases {
            let role = match kind {
                AdditionalContextKind::Application => "developer",
                AdditionalContextKind::Untrusted if tool_outputs => continue,
                AdditionalContextKind::Untrusted => "user",
            };
            assert_eq!(
                request
                    .message_input_texts(role)
                    .iter()
                    .filter(|text| text.contains(key))
                    .count(),
                1,
            );
        }
    }

    let context_attribute = |attribute: &opentelemetry::KeyValue| {
        (!matches!(
            attribute.key.as_str(),
            "auth_mode" | "session_source" | "originator" | "service_name" | "app.version"
        ))
        .then(|| (attribute.key.to_string(), attribute.value.to_string()))
    };
    let snapshot = metrics.snapshot()?;
    let mut counts = BTreeMap::new();
    let mut sizes = BTreeMap::new();
    for metric in snapshot.scope_metrics().flat_map(ScopeMetrics::metrics) {
        match metric.name() {
            ADDITIONAL_CONTEXT_COUNT_METRIC => {
                let AggregatedMetrics::U64(MetricData::Sum(sum)) = metric.data() else {
                    panic!("additional context count must be a counter");
                };
                for point in sum.data_points() {
                    counts.insert(
                        point
                            .attributes()
                            .filter_map(context_attribute)
                            .collect::<BTreeMap<_, _>>(),
                        point.value(),
                    );
                }
            }
            ADDITIONAL_CONTEXT_VALUE_BYTES_METRIC => {
                let AggregatedMetrics::F64(MetricData::Histogram(histogram)) = metric.data() else {
                    panic!("additional context size must be a histogram");
                };
                for point in histogram.data_points() {
                    sizes.insert(
                        point
                            .attributes()
                            .filter_map(context_attribute)
                            .collect::<BTreeMap<_, _>>(),
                        (point.count(), point.sum()),
                    );
                }
            }
            _ => {}
        }
    }
    let mut expected_counts = BTreeMap::new();
    let mut expected_sizes = BTreeMap::new();
    for (_, kind, name, value, truncated) in cases {
        let kind = match kind {
            AdditionalContextKind::Application => "application",
            AdditionalContextKind::Untrusted => "untrusted",
        };
        let mut tags = BTreeMap::from([
            ("kind".to_string(), kind.to_string()),
            ("model".to_string(), "gpt-5.5".to_string()),
            ("truncated".to_string(), truncated.to_string()),
        ]);
        if let Some(name) = name {
            tags.insert("name".to_string(), name.to_string());
        }
        expected_counts.insert(tags.clone(), 2);
        // The marker is outside the retained-text budget, so a truncated value can grow.
        let after_bytes = if truncated {
            4_000 + "…1 tokens truncated…".len()
        } else {
            value.len()
        };
        for (stage, bytes) in [("before", value.len()), ("after", after_bytes)] {
            let mut tags = tags.clone();
            tags.insert("stage".to_string(), stage.to_string());
            expected_sizes.insert(tags, (2, 2.0 * bytes as f64));
        }
    }
    assert_eq!(counts, expected_counts);
    assert_eq!(sizes, expected_sizes);
    metrics.shutdown()?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn additional_context_rejects_invalid_names_before_admitting_input() -> Result<()> {
    skip_if_no_network!(Ok(()));
    let server = start_mock_server().await;
    let request = mount_sse_once(&server, sse(vec![ev_completed("valid-turn")])).await;
    let test = test_codex()
        .with_config(|config| config.include_environment_context = false)
        .build_with_auto_env(&server)
        .await?;

    for (kind, name) in [
        (AdditionalContextKind::Untrusted, String::new()),
        (AdditionalContextKind::Untrusted, "agent.browser".to_owned()),
        (AdditionalContextKind::Untrusted, "other.browser".to_owned()),
        (AdditionalContextKind::Untrusted, "1browser".to_owned()),
        (AdditionalContextKind::Untrusted, "Browser".to_owned()),
        (AdditionalContextKind::Untrusted, "é".to_owned()),
        (AdditionalContextKind::Untrusted, "a".repeat(65)),
        (AdditionalContextKind::Application, "browser".to_owned()),
    ] {
        let result = test
            .codex
            .start_or_steer_turn(
                TurnInputRequest::user_input(vec![UserInput::Text {
                    text: "rejected input".to_owned(),
                    text_elements: Vec::new(),
                }])
                .with_additional_context(BTreeMap::from([(
                    "source".to_owned(),
                    AdditionalContextEntry::new("rejected context".to_owned(), kind)
                        .with_name(name),
                )])),
            )
            .await;
        assert!(matches!(
            result
                .expect_err("invalid context must be rejected")
                .details(),
            CodexErrorDetails::InvalidRequest(_)
        ));
    }
    assert!(request.requests().is_empty());
    test.submit_text_turn("valid input").await?;
    assert_eq!(
        request.single_request().message_input_texts("user"),
        vec!["valid input"]
    );
    Ok(())
}

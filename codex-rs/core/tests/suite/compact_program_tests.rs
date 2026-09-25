//! Model-switch compaction must retain the historical model/program pair across replay.

use super::*;
use codex_core::CodexThread;
use codex_core::ForkSnapshot;
use codex_core::StartThreadOptions;
use codex_core::TurnStartOptions;
use codex_protocol::turn_input::CyberAccessProgram;
use pretty_assertions::assert_eq;
use test_case::test_case;

#[derive(Clone, Copy, Debug)]
enum History {
    Live,
    Resume,
    Fork,
    Rollback,
    IncompleteRollback,
    Checkpoint,
    ApiKeyResume,
}

#[derive(Clone, Copy, Debug)]
enum Compaction {
    RemoteHash,
    RemoteDownshift,
    LocalHash,
    LocalDownshift,
    Fallback,
}

async fn submit_pair(
    thread: &CodexThread,
    model: &str,
    program: Option<CyberAccessProgram>,
) -> Result<()> {
    thread
        .start_or_steer_turn(
            TurnInputRequest::user_input(vec![UserInput::Text {
                text: format!("turn with {model}"),
                text_elements: Vec::new(),
            }])
            .with_thread_settings(ThreadSettingsOverrides {
                model: Some(model.to_owned()),
                ..Default::default()
            })
            .on_start(TurnStartOptions {
                cyber_access_program: program,
                ..Default::default()
            }),
        )
        .await?;
    let event = wait_for_event(thread, |event| {
        matches!(event, EventMsg::TurnComplete(_) | EventMsg::Error(_))
    })
    .await;
    if let EventMsg::Error(error) = event {
        anyhow::bail!("turn failed: {error:?}");
    }
    Ok(())
}

#[test_case(Compaction::RemoteHash, History::Live, Some(CyberAccessProgram::DaybreakBlue); "remote hash")]
#[test_case(Compaction::RemoteDownshift, History::Live, Some(CyberAccessProgram::DaybreakBlue); "remote downshift")]
#[test_case(Compaction::LocalHash, History::Live, Some(CyberAccessProgram::DaybreakBlue); "local hash")]
#[test_case(Compaction::LocalDownshift, History::Live, Some(CyberAccessProgram::DaybreakBlue); "local downshift")]
#[test_case(Compaction::RemoteHash, History::Resume, Some(CyberAccessProgram::DaybreakBlue); "resume")]
#[test_case(Compaction::RemoteHash, History::Fork, Some(CyberAccessProgram::DaybreakBlue); "fork")]
#[test_case(Compaction::RemoteHash, History::Rollback, Some(CyberAccessProgram::DaybreakBlue); "rollback")]
#[test_case(Compaction::RemoteHash, History::IncompleteRollback, Some(CyberAccessProgram::DaybreakBlue); "incomplete rollback")]
#[test_case(Compaction::RemoteHash, History::Checkpoint, Some(CyberAccessProgram::DaybreakBlue); "checkpoint")]
#[test_case(Compaction::RemoteHash, History::Resume, None; "missing program")]
#[test_case(Compaction::RemoteHash, History::Resume, Some(CyberAccessProgram::Standard); "standard")]
#[test_case(Compaction::LocalHash, History::Live, None; "local missing program")]
#[test_case(Compaction::Fallback, History::Live, Some(CyberAccessProgram::DaybreakBlue); "fallback")]
#[test_case(Compaction::LocalHash, History::ApiKeyResume, Some(CyberAccessProgram::DaybreakBlue); "api key cannot inherit authorization")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn model_switch_program_pair(
    compaction: Compaction,
    history: History,
    previous_program: Option<CyberAccessProgram>,
) -> Result<()> {
    skip_if_no_network!(Ok(()));
    let server = start_mock_server().await;
    let previous_model = "gpt-5.5";
    let next_model = "gpt-5.6-cyber";
    let discarded_model = "discarded-model";
    let downshift = matches!(
        compaction,
        Compaction::RemoteDownshift | Compaction::LocalDownshift
    );
    let local = matches!(
        compaction,
        Compaction::LocalHash | Compaction::LocalDownshift
    );
    let mut old = model_info_with_context_window(previous_model, /*context_window*/ 273_000);
    let mut new = model_info_with_context_window(next_model, /*context_window*/ 125_000);
    if !downshift {
        old.comp_hash = Some("blue-hash".to_owned());
        new.comp_hash = Some("red-hash".to_owned());
    }
    let mut discarded = old.clone();
    discarded.slug = discarded_model.to_owned();
    let models = ModelsResponse {
        models: vec![old, new, discarded],
    };
    let provider = if local {
        local_compaction_provider(&server)
    } else {
        openai_model_provider(&server)
    };
    let mut builder = test_codex()
        .with_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing())
        .with_model(previous_model)
        .with_config(move |config| {
            config.model_catalog = Some(models.clone());
            config.model_provider = provider;
            set_test_compact_prompt(config);
        });
    let initial = builder.build_with_auto_env(&server).await?;
    let first = mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("first-message", "surviving history"),
            ev_completed_with_tokens("first", if downshift { 120_000 } else { 100 }),
        ]),
    )
    .await;
    submit_pair(&initial.codex, previous_model, previous_program).await?;
    let expected_program = previous_program
        .map(|program| json!({"cyber": program}))
        .unwrap_or(Value::Null);
    assert_eq!(
        first.single_request().body_json()["access_programs"],
        expected_program
    );

    if matches!(
        history,
        History::Fork | History::Rollback | History::IncompleteRollback
    ) {
        let discarded = mount_sse_once(
            &server,
            sse(vec![
                ev_assistant_message("discarded-message", "discarded history"),
                ev_completed("discarded"),
            ]),
        )
        .await;
        submit_pair(
            &initial.codex,
            discarded_model,
            Some(CyberAccessProgram::Standard),
        )
        .await?;
        assert_eq!(
            discarded.single_request().body_json()["model"],
            discarded_model
        );
    }
    if matches!(history, History::Checkpoint) {
        let compact = mount_sse_once(
            &server,
            sse(vec![
                json!({"type": "response.output_item.done", "item": {
                    "type": "compaction", "encrypted_content": "CHECKPOINT"
                }}),
                ev_completed("checkpoint"),
            ]),
        )
        .await;
        initial.codex.submit(Op::Compact).await?;
        wait_for_event(&initial.codex, |event| {
            matches!(event, EventMsg::TurnComplete(_))
        })
        .await;
        assert_eq!(
            compact
                .single_request()
                .inputs_of_type("compaction_trigger")
                .len(),
            1
        );
    }

    let resumed;
    let thread = match history {
        History::Live => Arc::clone(&initial.codex),
        History::Fork => {
            initial.codex.flush_rollout().await?;
            let mut config = initial.config.clone();
            config.model = Some(next_model.to_owned());
            initial
                .thread_manager
                .fork_legacy_thread(
                    ForkSnapshot::TruncateBeforeNthUserMessage(1),
                    StartThreadOptions::new(config),
                    initial.codex.rollout_path().expect("rollout"),
                )
                .await?
                .thread
        }
        History::Resume
        | History::Rollback
        | History::IncompleteRollback
        | History::Checkpoint
        | History::ApiKeyResume => {
            initial.codex.shutdown_and_wait().await?;
            let rollout_path = initial.codex.rollout_path().expect("rollout");
            if matches!(history, History::Rollback | History::IncompleteRollback) {
                // Live rollback is unsupported; replay its persisted marker, as older rollouts do.
                let text = fs::read_to_string(&rollout_path)?;
                let mut lines = text
                    .lines()
                    .map(serde_json::from_str::<Value>)
                    .collect::<std::result::Result<Vec<_>, _>>()?;
                if matches!(history, History::IncompleteRollback) {
                    let completion = lines
                        .iter()
                        .rposition(|line| {
                            line["type"] == "event_msg"
                                && line["payload"]["type"] == "task_complete"
                        })
                        .expect("last completion");
                    lines.truncate(completion);
                }
                lines.push(
                    json!({"timestamp": "2026-01-01T00:00:00Z", "type": "event_msg", "payload": {
                        "type": "thread_rolled_back", "num_turns": 1
                    }}),
                );
                fs::write(
                    &rollout_path,
                    lines
                        .iter()
                        .map(Value::to_string)
                        .collect::<Vec<_>>()
                        .join("\n")
                        + "\n",
                )?;
            }
            if matches!(history, History::ApiKeyResume) {
                builder = builder.with_auth(CodexAuth::from_api_key("test-key"));
            }
            let resume_config = initial.config.clone();
            builder = builder.with_config(move |config| {
                config.model_catalog = resume_config.model_catalog;
                config.model_provider = resume_config.model_provider;
                set_test_compact_prompt(config);
            });
            resumed = builder
                .resume(&server, Arc::clone(&initial.home), rollout_path)
                .await?;
            Arc::clone(&resumed.codex)
        }
    };
    let summary = if local {
        ev_assistant_message("summary", "summary of surviving history")
    } else {
        json!({"type": "response.output_item.done", "item": {
            "type": "compaction", "encrypted_content": "SWITCH_SUMMARY"
        }})
    };
    let mut replies = Vec::new();
    if matches!(compaction, Compaction::Fallback) {
        replies.push(invalid_request_response("previous model rejected"));
    }
    replies.push(sse_response(sse(vec![
        summary,
        ev_completed_with_tokens("compact", /*total_tokens*/ 10),
    ])));
    replies.push(sse_response(sse(vec![
        ev_assistant_message("next-message", "next answer"),
        ev_completed("next"),
    ])));
    let requests = mount_response_sequence(&server, replies).await;
    submit_pair(&thread, next_model, Some(CyberAccessProgram::DaybreakRed)).await?;
    thread.shutdown_and_wait().await?;
    let requests = requests.requests();
    let authorized = !matches!(history, History::ApiKeyResume);
    let old_program = if authorized {
        expected_program
    } else {
        Value::Null
    };
    let new_program = if authorized {
        json!({"cyber": "daybreak_red"})
    } else {
        Value::Null
    };
    let actual = requests
        .iter()
        .map(|request| {
            let body = request.body_json();
            json!([body["model"], body["access_programs"]])
        })
        .collect::<Vec<_>>();
    // Check fallback and subsequent sampling even when the historical request is wrong.
    let mut following = Vec::new();
    if matches!(compaction, Compaction::Fallback) {
        following.push(json!([next_model, new_program]));
    }
    following.push(json!([next_model, new_program]));
    assert_eq!(&actual[1..], following.as_slice());
    assert_eq!(actual[0], json!([previous_model, old_program]));
    if !local {
        assert_eq!(requests[0].inputs_of_type("compaction_trigger").len(), 1);
    }
    Ok(())
}

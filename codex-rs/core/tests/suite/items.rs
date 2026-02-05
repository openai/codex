#![cfg(not(target_os = "windows"))]

use anyhow::Ok;
use codex_core::protocol::EventMsg;
use codex_core::protocol::ItemCompletedEvent;
use codex_core::protocol::ItemStartedEvent;
use codex_core::protocol::Op;
use codex_protocol::config_types::CollaborationMode;
use codex_protocol::config_types::ModeKind;
use codex_protocol::config_types::Settings;
use codex_protocol::items::AgentMessageContent;
use codex_protocol::items::TurnItem;
use codex_protocol::models::WebSearchAction;
use codex_protocol::user_input::ByteRange;
use codex_protocol::user_input::TextElement;
use codex_protocol::user_input::UserInput;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_message_item_added;
use core_test_support::responses::ev_output_text_delta;
use core_test_support::responses::ev_reasoning_item;
use core_test_support::responses::ev_reasoning_item_added;
use core_test_support::responses::ev_reasoning_summary_text_delta;
use core_test_support::responses::ev_reasoning_text_delta;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::ev_web_search_call_added_partial;
use core_test_support::responses::ev_web_search_call_done;
use core_test_support::responses::mount_sse_once;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::TestCodex;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use core_test_support::wait_for_event_match;
use pretty_assertions::assert_eq;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn user_message_item_is_emitted() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;

    let TestCodex { codex, .. } = test_codex().build(&server).await?;

    let first_response = sse(vec![ev_response_created("resp-1"), ev_completed("resp-1")]);
    mount_sse_once(&server, first_response).await;

    let text_elements = vec![TextElement::new(
        ByteRange { start: 0, end: 6 },
        Some("<file>".into()),
    )];
    let expected_input = UserInput::Text {
        text: "please inspect sample.txt".into(),
        text_elements: text_elements.clone(),
    };

    codex
        .submit(Op::UserInput {
            items: vec![expected_input.clone()],
            final_output_json_schema: None,
        })
        .await?;

    let started_item = wait_for_event_match(&codex, |ev| match ev {
        EventMsg::ItemStarted(ItemStartedEvent {
            item: TurnItem::UserMessage(item),
            ..
        }) => Some(item.clone()),
        _ => None,
    })
    .await;
    let completed_item = wait_for_event_match(&codex, |ev| match ev {
        EventMsg::ItemCompleted(ItemCompletedEvent {
            item: TurnItem::UserMessage(item),
            ..
        }) => Some(item.clone()),
        _ => None,
    })
    .await;

    assert_eq!(started_item.id, completed_item.id);
    assert_eq!(started_item.content, vec![expected_input.clone()]);
    assert_eq!(completed_item.content, vec![expected_input]);

    let legacy_message = wait_for_event_match(&codex, |ev| match ev {
        EventMsg::UserMessage(event) => Some(event.clone()),
        _ => None,
    })
    .await;
    assert_eq!(legacy_message.message, "please inspect sample.txt");
    assert_eq!(legacy_message.text_elements, text_elements);
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn assistant_message_item_is_emitted() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;

    let TestCodex { codex, .. } = test_codex().build(&server).await?;

    let first_response = sse(vec![
        ev_response_created("resp-1"),
        ev_assistant_message("msg-1", "all done"),
        ev_completed("resp-1"),
    ]);
    mount_sse_once(&server, first_response).await;

    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "please summarize results".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
        })
        .await?;

    let started = wait_for_event_match(&codex, |ev| match ev {
        EventMsg::ItemStarted(ItemStartedEvent {
            item: TurnItem::AgentMessage(item),
            ..
        }) => Some(item.clone()),
        _ => None,
    })
    .await;
    let completed = wait_for_event_match(&codex, |ev| match ev {
        EventMsg::ItemCompleted(ItemCompletedEvent {
            item: TurnItem::AgentMessage(item),
            ..
        }) => Some(item.clone()),
        _ => None,
    })
    .await;

    assert_eq!(started.id, completed.id);
    let Some(codex_protocol::items::AgentMessageContent::Text { text }) = completed.content.first()
    else {
        panic!("expected agent message text content");
    };
    assert_eq!(text, "all done");

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn reasoning_item_is_emitted() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;

    let TestCodex { codex, .. } = test_codex().build(&server).await?;

    let reasoning_item = ev_reasoning_item(
        "reasoning-1",
        &["Consider inputs", "Compute output"],
        &["Detailed reasoning trace"],
    );

    let first_response = sse(vec![
        ev_response_created("resp-1"),
        reasoning_item,
        ev_completed("resp-1"),
    ]);
    mount_sse_once(&server, first_response).await;

    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "explain your reasoning".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
        })
        .await?;

    let started = wait_for_event_match(&codex, |ev| match ev {
        EventMsg::ItemStarted(ItemStartedEvent {
            item: TurnItem::Reasoning(item),
            ..
        }) => Some(item.clone()),
        _ => None,
    })
    .await;
    let completed = wait_for_event_match(&codex, |ev| match ev {
        EventMsg::ItemCompleted(ItemCompletedEvent {
            item: TurnItem::Reasoning(item),
            ..
        }) => Some(item.clone()),
        _ => None,
    })
    .await;

    assert_eq!(started.id, completed.id);
    assert_eq!(
        completed.summary_text,
        vec!["Consider inputs".to_string(), "Compute output".to_string()]
    );
    assert_eq!(
        completed.raw_content,
        vec!["Detailed reasoning trace".to_string()]
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn web_search_item_is_emitted() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;

    let TestCodex { codex, .. } = test_codex().build(&server).await?;

    let web_search_added = ev_web_search_call_added_partial("web-search-1", "in_progress");
    let web_search_done = ev_web_search_call_done("web-search-1", "completed", "weather seattle");

    let first_response = sse(vec![
        ev_response_created("resp-1"),
        web_search_added,
        web_search_done,
        ev_completed("resp-1"),
    ]);
    mount_sse_once(&server, first_response).await;

    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "find the weather".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
        })
        .await?;

    let begin = wait_for_event_match(&codex, |ev| match ev {
        EventMsg::WebSearchBegin(event) => Some(event.clone()),
        _ => None,
    })
    .await;
    let completed = wait_for_event_match(&codex, |ev| match ev {
        EventMsg::ItemCompleted(ItemCompletedEvent {
            item: TurnItem::WebSearch(item),
            ..
        }) => Some(item.clone()),
        _ => None,
    })
    .await;

    assert_eq!(begin.call_id, "web-search-1");
    assert_eq!(completed.id, begin.call_id);
    assert_eq!(
        completed.action,
        WebSearchAction::Search {
            query: Some("weather seattle".to_string()),
            queries: None,
        }
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn agent_message_content_delta_has_item_metadata() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;

    let TestCodex {
        codex,
        session_configured,
        ..
    } = test_codex().build(&server).await?;

    let stream = sse(vec![
        ev_response_created("resp-1"),
        ev_message_item_added("msg-1", ""),
        ev_output_text_delta("streamed response"),
        ev_assistant_message("msg-1", "streamed response"),
        ev_completed("resp-1"),
    ]);
    mount_sse_once(&server, stream).await;

    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "please stream text".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
        })
        .await?;

    let (started_turn_id, started_item) = wait_for_event_match(&codex, |ev| match ev {
        EventMsg::ItemStarted(ItemStartedEvent {
            turn_id,
            item: TurnItem::AgentMessage(item),
            ..
        }) => Some((turn_id.clone(), item.clone())),
        _ => None,
    })
    .await;

    let delta_event = wait_for_event_match(&codex, |ev| match ev {
        EventMsg::AgentMessageContentDelta(event) => Some(event.clone()),
        _ => None,
    })
    .await;
    let legacy_delta = wait_for_event_match(&codex, |ev| match ev {
        EventMsg::AgentMessageDelta(event) => Some(event.clone()),
        _ => None,
    })
    .await;
    let completed_item = wait_for_event_match(&codex, |ev| match ev {
        EventMsg::ItemCompleted(ItemCompletedEvent {
            item: TurnItem::AgentMessage(item),
            ..
        }) => Some(item.clone()),
        _ => None,
    })
    .await;

    let session_id = session_configured.session_id.to_string();
    assert_eq!(delta_event.thread_id, session_id);
    assert_eq!(delta_event.turn_id, started_turn_id);
    assert_eq!(delta_event.item_id, started_item.id);
    assert_eq!(delta_event.delta, "streamed response");
    assert_eq!(legacy_delta.delta, "streamed response");
    assert_eq!(completed_item.id, started_item.id);

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn plan_mode_emits_plan_item_from_proposed_plan_block() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;

    let TestCodex {
        codex,
        session_configured,
        ..
    } = test_codex().build(&server).await?;

    let plan_block = "<proposed_plan>\n- Step 1\n- Step 2\n</proposed_plan>\n";
    let full_message = format!("Intro\n{plan_block}Outro");
    let stream = sse(vec![
        ev_response_created("resp-1"),
        ev_message_item_added("msg-1", ""),
        ev_output_text_delta(&full_message),
        ev_assistant_message("msg-1", &full_message),
        ev_completed("resp-1"),
    ]);
    mount_sse_once(&server, stream).await;

    let collaboration_mode = CollaborationMode {
        mode: ModeKind::Plan,
        settings: Settings {
            model: session_configured.model.clone(),
            reasoning_effort: None,
            developer_instructions: None,
        },
    };

    codex
        .submit(Op::UserTurn {
            items: vec![UserInput::Text {
                text: "please plan".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
            cwd: std::env::current_dir()?,
            approval_policy: codex_core::protocol::AskForApproval::Never,
            sandbox_policy: codex_core::protocol::SandboxPolicy::DangerFullAccess,
            model: session_configured.model.clone(),
            effort: None,
            summary: codex_protocol::config_types::ReasoningSummary::Auto,
            collaboration_mode: Some(collaboration_mode),
            personality: None,
        })
        .await?;

    let plan_delta = wait_for_event_match(&codex, |ev| match ev {
        EventMsg::PlanDelta(event) => Some(event.clone()),
        _ => None,
    })
    .await;

    let plan_completed = wait_for_event_match(&codex, |ev| match ev {
        EventMsg::ItemCompleted(ItemCompletedEvent {
            item: TurnItem::Plan(item),
            ..
        }) => Some(item.clone()),
        _ => None,
    })
    .await;

    assert_eq!(
        plan_delta.thread_id,
        session_configured.session_id.to_string()
    );
    assert_eq!(plan_delta.delta, "- Step 1\n- Step 2\n");
    assert_eq!(plan_completed.text, "- Step 1\n- Step 2\n");

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn plan_mode_strips_plan_from_agent_messages() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;

    let TestCodex {
        codex,
        session_configured,
        ..
    } = test_codex().build(&server).await?;

    let plan_block = "<proposed_plan>\n- Step 1\n- Step 2\n</proposed_plan>\n";
    let full_message = format!("Intro\n{plan_block}Outro");
    let stream = sse(vec![
        ev_response_created("resp-1"),
        ev_message_item_added("msg-1", ""),
        ev_output_text_delta(&full_message),
        ev_assistant_message("msg-1", &full_message),
        ev_completed("resp-1"),
    ]);
    mount_sse_once(&server, stream).await;

    let collaboration_mode = CollaborationMode {
        mode: ModeKind::Plan,
        settings: Settings {
            model: session_configured.model.clone(),
            reasoning_effort: None,
            developer_instructions: None,
        },
    };

    codex
        .submit(Op::UserTurn {
            items: vec![UserInput::Text {
                text: "please plan".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
            cwd: std::env::current_dir()?,
            approval_policy: codex_core::protocol::AskForApproval::Never,
            sandbox_policy: codex_core::protocol::SandboxPolicy::DangerFullAccess,
            model: session_configured.model.clone(),
            effort: None,
            summary: codex_protocol::config_types::ReasoningSummary::Auto,
            collaboration_mode: Some(collaboration_mode),
            personality: None,
        })
        .await?;

    let mut agent_deltas = Vec::new();
    let mut plan_delta = None;
    let mut agent_item = None;
    let mut plan_item = None;

    while plan_delta.is_none() || agent_item.is_none() || plan_item.is_none() {
        let ev = wait_for_event(&codex, |_| true).await;
        match ev {
            EventMsg::AgentMessageContentDelta(event) => {
                agent_deltas.push(event.delta);
            }
            EventMsg::PlanDelta(event) => {
                plan_delta = Some(event.delta);
            }
            EventMsg::ItemCompleted(ItemCompletedEvent {
                item: TurnItem::AgentMessage(item),
                ..
            }) => {
                agent_item = Some(item);
            }
            EventMsg::ItemCompleted(ItemCompletedEvent {
                item: TurnItem::Plan(item),
                ..
            }) => {
                plan_item = Some(item);
            }
            _ => {}
        }
    }

    let agent_text = agent_deltas.concat();
    assert_eq!(agent_text, "Intro\nOutro");
    assert_eq!(plan_delta.unwrap(), "- Step 1\n- Step 2\n");
    assert_eq!(plan_item.unwrap().text, "- Step 1\n- Step 2\n");
    let agent_text_from_item: String = agent_item
        .unwrap()
        .content
        .iter()
        .map(|entry| match entry {
            AgentMessageContent::Text { text } => text.as_str(),
        })
        .collect();
    assert_eq!(agent_text_from_item, "Intro\nOutro");

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn plan_mode_handles_missing_plan_close_tag() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;

    let TestCodex {
        codex,
        session_configured,
        ..
    } = test_codex().build(&server).await?;

    let full_message = "Intro\n<proposed_plan>\n- Step 1\n";
    let stream = sse(vec![
        ev_response_created("resp-1"),
        ev_message_item_added("msg-1", ""),
        ev_output_text_delta(full_message),
        ev_assistant_message("msg-1", full_message),
        ev_completed("resp-1"),
    ]);
    mount_sse_once(&server, stream).await;

    let collaboration_mode = CollaborationMode {
        mode: ModeKind::Plan,
        settings: Settings {
            model: session_configured.model.clone(),
            reasoning_effort: None,
            developer_instructions: None,
        },
    };

    codex
        .submit(Op::UserTurn {
            items: vec![UserInput::Text {
                text: "please plan".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
            cwd: std::env::current_dir()?,
            approval_policy: codex_core::protocol::AskForApproval::Never,
            sandbox_policy: codex_core::protocol::SandboxPolicy::DangerFullAccess,
            model: session_configured.model.clone(),
            effort: None,
            summary: codex_protocol::config_types::ReasoningSummary::Auto,
            collaboration_mode: Some(collaboration_mode),
            personality: None,
        })
        .await?;

    let mut plan_delta = None;
    let mut plan_item = None;
    let mut agent_item = None;

    while plan_delta.is_none() || plan_item.is_none() || agent_item.is_none() {
        let ev = wait_for_event(&codex, |_| true).await;
        match ev {
            EventMsg::PlanDelta(event) => {
                plan_delta = Some(event.delta);
            }
            EventMsg::ItemCompleted(ItemCompletedEvent {
                item: TurnItem::Plan(item),
                ..
            }) => {
                plan_item = Some(item);
            }
            EventMsg::ItemCompleted(ItemCompletedEvent {
                item: TurnItem::AgentMessage(item),
                ..
            }) => {
                agent_item = Some(item);
            }
            _ => {}
        }
    }

    assert_eq!(plan_delta.unwrap(), "- Step 1\n");
    assert_eq!(plan_item.unwrap().text, "- Step 1\n");
    let agent_text_from_item: String = agent_item
        .unwrap()
        .content
        .iter()
        .map(|entry| match entry {
            AgentMessageContent::Text { text } => text.as_str(),
        })
        .collect();
    assert_eq!(agent_text_from_item, "Intro\n");

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn reasoning_content_delta_has_item_metadata() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;

    let TestCodex { codex, .. } = test_codex().build(&server).await?;

    let stream = sse(vec![
        ev_response_created("resp-1"),
        ev_reasoning_item_added("reasoning-1", &[""]),
        ev_reasoning_summary_text_delta("step one"),
        ev_reasoning_item("reasoning-1", &["step one"], &[]),
        ev_completed("resp-1"),
    ]);
    mount_sse_once(&server, stream).await;

    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "reason through it".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
        })
        .await?;

    let reasoning_item = wait_for_event_match(&codex, |ev| match ev {
        EventMsg::ItemStarted(ItemStartedEvent {
            item: TurnItem::Reasoning(item),
            ..
        }) => Some(item.clone()),
        _ => None,
    })
    .await;

    let delta_event = wait_for_event_match(&codex, |ev| match ev {
        EventMsg::ReasoningContentDelta(event) => Some(event.clone()),
        _ => None,
    })
    .await;
    let legacy_delta = wait_for_event_match(&codex, |ev| match ev {
        EventMsg::AgentReasoningDelta(event) => Some(event.clone()),
        _ => None,
    })
    .await;

    assert_eq!(delta_event.item_id, reasoning_item.id);
    assert_eq!(delta_event.delta, "step one");
    assert_eq!(legacy_delta.delta, "step one");

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn reasoning_raw_content_delta_respects_flag() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;

    let TestCodex { codex, .. } = test_codex()
        .with_config(|config| {
            config.show_raw_agent_reasoning = true;
        })
        .build(&server)
        .await?;

    let stream = sse(vec![
        ev_response_created("resp-1"),
        ev_reasoning_item_added("reasoning-raw", &[""]),
        ev_reasoning_text_delta("raw detail"),
        ev_reasoning_item("reasoning-raw", &["complete"], &["raw detail"]),
        ev_completed("resp-1"),
    ]);
    mount_sse_once(&server, stream).await;

    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "show raw reasoning".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
        })
        .await?;

    let reasoning_item = wait_for_event_match(&codex, |ev| match ev {
        EventMsg::ItemStarted(ItemStartedEvent {
            item: TurnItem::Reasoning(item),
            ..
        }) => Some(item.clone()),
        _ => None,
    })
    .await;

    let delta_event = wait_for_event_match(&codex, |ev| match ev {
        EventMsg::ReasoningRawContentDelta(event) => Some(event.clone()),
        _ => None,
    })
    .await;
    let legacy_delta = wait_for_event_match(&codex, |ev| match ev {
        EventMsg::AgentReasoningRawContentDelta(event) => Some(event.clone()),
        _ => None,
    })
    .await;

    assert_eq!(delta_event.item_id, reasoning_item.id);
    assert_eq!(delta_event.delta, "raw detail");
    assert_eq!(legacy_delta.delta, "raw detail");

    Ok(())
}

#![cfg(not(target_os = "windows"))]

use anyhow::Ok;
use codex_core::protocol::EventMsg;
use codex_core::protocol::ItemCompletedEvent;
use codex_core::protocol::ItemStartedEvent;
use codex_core::protocol::Op;
use codex_protocol::items::TurnItem;
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
use core_test_support::responses::ev_web_search_call_added;
use core_test_support::responses::ev_web_search_call_done;
use core_test_support::responses::mount_sse_once;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::TestCodex;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event_match;
use pretty_assertions::assert_eq;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn user_message_item_is_emitted() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;

    let TestCodex { codex, .. } = test_codex().build(&server).await?;

    let first_response = sse(vec![ev_response_created("resp-1"), ev_completed("resp-1")]);
    mount_sse_once(&server, first_response).await;

    codex
        .submit(Op::UserInput {
            items: (vec![UserInput::Text {
                text: "please inspect sample.txt".into(),
                text_elements: Vec::new(),
            }]),
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
    assert_eq!(
        started_item.content,
        vec![UserInput::Text {
            text: "please inspect sample.txt".into(),
            text_elements: Vec::new(),
        }]
    );
    assert_eq!(
        completed_item.content,
        vec![UserInput::Text {
            text: "please inspect sample.txt".into(),
            text_elements: Vec::new(),
        }]
    );
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

    let web_search_added =
        ev_web_search_call_added("web-search-1", "in_progress", "weather seattle");
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

    let started = wait_for_event_match(&codex, |ev| match ev {
        EventMsg::ItemStarted(ItemStartedEvent {
            item: TurnItem::WebSearch(item),
            ..
        }) => Some(item.clone()),
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

    assert_eq!(started.id, completed.id);
    assert_eq!(completed.query, "weather seattle");

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

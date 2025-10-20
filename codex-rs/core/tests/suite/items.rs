#![cfg(not(target_os = "windows"))]

use anyhow::Ok;
use codex_core::protocol::EventMsg;
use codex_core::protocol::Op;
use codex_protocol::items::TurnItem;
use codex_protocol::user_input::UserInput;
use core_test_support::responses;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::TestCodex;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event_match;
use pretty_assertions::assert_eq;
use wiremock::matchers::any;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn user_message_item_is_emitted() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;

    let TestCodex { codex, .. } = test_codex().build(&server).await?;

    let first_response = sse(vec![ev_response_created("resp-1"), ev_completed("resp-1")]);
    responses::mount_sse_once_match(&server, any(), first_response).await;

    codex
        .submit(Op::UserInput {
            items: (vec![UserInput::Text {
                text: "please inspect sample.txt".into(),
            }]),
        })
        .await?;

    let started = wait_for_event_match(&codex, |ev| match ev {
        EventMsg::ItemStarted(e) => Some(e.clone()),
        _ => None,
    })
    .await;

    let completed = wait_for_event_match(&codex, |ev| match ev {
        EventMsg::ItemCompleted(e) => Some(e.clone()),
        _ => None,
    })
    .await;

    let TurnItem::UserMessage(started_item) = started.item;
    let TurnItem::UserMessage(completed_item) = completed.item;

    assert_eq!(started_item.id, completed_item.id);
    assert_eq!(
        started_item.content,
        vec![UserInput::Text {
            text: "please inspect sample.txt".into(),
        }]
    );
    assert_eq!(
        completed_item.content,
        vec![UserInput::Text {
            text: "please inspect sample.txt".into(),
        }]
    );
    Ok(())
}

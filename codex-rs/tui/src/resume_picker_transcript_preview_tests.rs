use super::*;
use crate::legacy_core::config::ConfigBuilder;
use app_test_support::create_mock_responses_server_sequence;
use app_test_support::write_mock_responses_config_toml;
use codex_app_server_client::AppServerEvent;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ServerNotification;
use codex_app_server_protocol::SortDirection;
use codex_app_server_protocol::ThreadItemsListParams;
use codex_app_server_protocol::ThreadItemsListResponse;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::TurnStartResponse;
use codex_app_server_protocol::UserInput;
use core_test_support::responses;
use pretty_assertions::assert_eq;
use tempfile::tempdir;

fn preview_line(speaker: TranscriptPreviewSpeaker, text: &str) -> TranscriptPreviewLine {
    TranscriptPreviewLine {
        speaker,
        text: text.to_string(),
    }
}

#[test]
fn transcript_preview_reverse_scan_stops_before_older_items() {
    let newest = ThreadItem::AgentMessage {
        id: String::from("final"),
        text: String::from("one\ntwo\nthree\nfour\nfive\nsix"),
        phase: None,
        memory_citation: None,
    };
    let mut lines = Vec::new();

    append_transcript_preview_lines(
        &mut lines,
        std::iter::once(&newest).chain(std::iter::from_fn(|| {
            panic!("preview should stop after finding six lines")
        })),
        Path::new("/tmp"),
        /*inline_visualization_context*/ None,
    );

    assert_eq!(
        lines,
        vec![
            preview_line(TranscriptPreviewSpeaker::Assistant, "six"),
            preview_line(TranscriptPreviewSpeaker::Assistant, "five"),
            preview_line(TranscriptPreviewSpeaker::Assistant, "four"),
            preview_line(TranscriptPreviewSpeaker::Assistant, "three"),
            preview_line(TranscriptPreviewSpeaker::Assistant, "two"),
            preview_line(TranscriptPreviewSpeaker::Assistant, "one"),
        ]
    );
}

async fn transcript_preview_for_history_mode(
    history_mode: ThreadHistoryMode,
    invisible_newer_turns: usize,
) -> Vec<TranscriptPreviewLine> {
    let model_response = responses::sse(vec![
        responses::ev_response_created("resp-1"),
        responses::ev_assistant_message("commentary", "commentary one\ncommentary two"),
        responses::ev_assistant_message("final", "final one\nfinal two\nfinal three"),
        responses::ev_completed("resp-1"),
    ]);
    let model_responses = std::iter::once(model_response)
        .chain((0..invisible_newer_turns).map(|index| {
            responses::sse(vec![
                responses::ev_response_created(&format!("empty-resp-{index}")),
                responses::ev_completed(&format!("empty-resp-{index}")),
            ])
        }))
        .collect();
    let server = create_mock_responses_server_sequence(model_responses).await;
    let codex_home = tempdir().expect("tempdir");
    write_mock_responses_config_toml(
        codex_home.path(),
        &server.uri(),
        &Default::default(),
        /*auto_compact_limit*/ 100_000,
        /*requires_openai_auth*/ None,
        "mock_provider",
        "compact",
    )
    .expect("write mock config");
    let config = ConfigBuilder::default()
        .codex_home(codex_home.path().to_path_buf())
        .fallback_cwd(Some(codex_home.path().to_path_buf()))
        .build()
        .await
        .expect("build config");
    let mut app_server = crate::start_embedded_app_server_for_picker(&config)
        .await
        .expect("start app server");
    let request_handle = app_server.request_handle();
    let started: ThreadStartResponse = request_handle
        .request_typed(ClientRequest::ThreadStart {
            request_id: RequestId::String(String::from("preview-thread-start")),
            params: ThreadStartParams {
                model: Some(String::from("mock-model")),
                model_provider: Some(String::from("mock_provider")),
                cwd: Some(codex_home.path().display().to_string()),
                history_mode: Some(history_mode),
                ..Default::default()
            },
        })
        .await
        .expect("start thread");
    let thread_id = ThreadId::from_string(&started.thread.id).expect("thread id");
    for index in 0..=invisible_newer_turns {
        let input = if index == 0 {
            UserInput::Text {
                text: String::from("recent user"),
                text_elements: Vec::new(),
            }
        } else {
            UserInput::Image {
                url: String::from(
                    "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGP4z8DwHwAFAAH/iZk9HQAAAABJRU5ErkJggg==",
                ),
                detail: None,
            }
        };
        let _: TurnStartResponse = request_handle
            .request_typed(ClientRequest::TurnStart {
                request_id: RequestId::String(format!("preview-turn-start-{index}")),
                params: TurnStartParams {
                    thread_id: thread_id.to_string(),
                    input: vec![input],
                    ..Default::default()
                },
            })
            .await
            .expect("start turn");
        tokio::time::timeout(std::time::Duration::from_secs(10), async {
            loop {
                let Some(event) = app_server.next_event().await else {
                    panic!("app-server event stream closed before turn completed");
                };
                if matches!(
                    event,
                    AppServerEvent::ServerNotification(notification)
                        if matches!(notification.as_ref(), ServerNotification::TurnCompleted(_))
                ) {
                    break;
                }
            }
        })
        .await
        .expect("turn should complete");
    }

    if history_mode == ThreadHistoryMode::Paginated {
        let first_page: ThreadItemsListResponse = request_handle
            .request_typed(ClientRequest::ThreadItemsList {
                request_id: RequestId::String(String::from("preview-items-list")),
                params: ThreadItemsListParams {
                    thread_id: thread_id.to_string(),
                    turn_id: None,
                    cursor: None,
                    limit: Some(TRANSCRIPT_PREVIEW_ITEMS_PAGE_SIZE),
                    sort_direction: Some(SortDirection::Desc),
                },
            })
            .await
            .expect("list first items page");
        assert_eq!(first_page.data.len(), MAX_TRANSCRIPT_PREVIEW_LINES);
        let mut first_page_lines = Vec::new();
        append_transcript_preview_lines(
            &mut first_page_lines,
            first_page.data.iter().map(|entry| &entry.item),
            codex_home.path(),
            /*inline_visualization_context*/ None,
        );
        assert_eq!(first_page_lines, Vec::new());
        assert!(first_page.next_cursor.is_some());
    }

    load_transcript_preview(&mut app_server, thread_id, Some(&config))
        .await
        .expect("load transcript preview")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn transcript_preview_preserves_legacy_and_paginated_output() {
    let invisible_newer_turns = TRANSCRIPT_PREVIEW_ITEMS_PAGE_SIZE as usize;
    let legacy =
        transcript_preview_for_history_mode(ThreadHistoryMode::Legacy, invisible_newer_turns).await;
    let paginated =
        transcript_preview_for_history_mode(ThreadHistoryMode::Paginated, invisible_newer_turns)
            .await;

    assert_eq!(legacy, paginated);
    assert_eq!(
        paginated,
        vec![
            preview_line(TranscriptPreviewSpeaker::User, "recent user"),
            preview_line(TranscriptPreviewSpeaker::Assistant, "commentary one"),
            preview_line(TranscriptPreviewSpeaker::Assistant, "commentary two"),
            preview_line(TranscriptPreviewSpeaker::Assistant, "final one"),
            preview_line(TranscriptPreviewSpeaker::Assistant, "final two"),
            preview_line(TranscriptPreviewSpeaker::Assistant, "final three"),
        ]
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn paginated_transcript_preview_pages_past_old_item_budget() {
    let invisible_newer_turns = TRANSCRIPT_PREVIEW_ITEMS_PAGE_SIZE as usize * 4;

    let lines =
        transcript_preview_for_history_mode(ThreadHistoryMode::Paginated, invisible_newer_turns)
            .await;

    assert_eq!(
        lines,
        vec![
            preview_line(TranscriptPreviewSpeaker::User, "recent user"),
            preview_line(TranscriptPreviewSpeaker::Assistant, "commentary one"),
            preview_line(TranscriptPreviewSpeaker::Assistant, "commentary two"),
            preview_line(TranscriptPreviewSpeaker::Assistant, "final one"),
            preview_line(TranscriptPreviewSpeaker::Assistant, "final two"),
            preview_line(TranscriptPreviewSpeaker::Assistant, "final three"),
        ]
    );
}

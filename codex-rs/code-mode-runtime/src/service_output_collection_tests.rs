//! Real-runtime regressions for bounded output, observation and termination.

use std::sync::Arc;
use std::time::Duration;

use codex_code_mode_protocol::ExecuteRequest;
use codex_code_mode_protocol::FunctionCallOutputContentItem;
use codex_code_mode_protocol::NoopCodeModeSessionDelegate;
use codex_code_mode_protocol::RuntimeResponse;
use codex_code_mode_protocol::WaitOutcome;
use codex_code_mode_protocol::WaitRequest;
use pretty_assertions::assert_eq;

use super::InProcessCodeModeSession;
use crate::output::MAX_OUTPUT_ITEMS;
use crate::output::MAX_SERVER_DIAGNOSTIC_BYTES;
use codex_code_mode_protocol::CodeModeOutputTruncation;
use codex_code_mode_protocol::CodeModeSessionOptions;
use codex_code_mode_protocol::host::BOUNDED_OUTPUT_MAX_BYTES as MAX_OUTPUT_BYTES;
use codex_code_mode_protocol::host::BOUNDED_OUTPUT_MAX_IMAGE_BYTES as MAX_IMAGE_BYTES;

fn server_session() -> InProcessCodeModeSession {
    InProcessCodeModeSession::with_options(
        CodeModeSessionOptions {
            output_truncation: CodeModeOutputTruncation::Server,
            ..CodeModeSessionOptions::default()
        },
        /*task_failure_handler*/ None,
    )
}

async fn start(session: &InProcessCodeModeSession, source: &str) -> RuntimeResponse {
    session
        .execute(
            ExecuteRequest {
                tool_call_id: "bounded-output".to_owned(),
                enabled_tools: Vec::new(),
                source: source.to_owned(),
                yield_time_ms: Some(60_000),
                // These tests isolate collector overflow from the per-observation
                // formatter: request enough room to display every retained item.
                max_output_tokens: Some(MAX_OUTPUT_BYTES),
            },
            Arc::new(NoopCodeModeSessionDelegate),
            /*preempt*/ None,
        )
        .await
        .unwrap()
        .initial_response()
        .await
        .unwrap()
}

#[tokio::test]
async fn bounded_output_keeps_head_tail_without_stopping_the_cell() {
    let session = server_session();
    let emitted = MAX_OUTPUT_ITEMS + 100;
    let response = start(
        &session,
        &format!("text('begin'); for (let i = 0; i < {emitted}; i++) text(i); text('end');"),
    )
    .await;
    let RuntimeResponse::Result {
        content_items,
        error_text,
        ..
    } = response
    else {
        panic!("expected completed cell");
    };
    assert_eq!(error_text, None);
    assert_eq!(content_items.len(), MAX_OUTPUT_ITEMS);
    assert_eq!(
        content_items.first(),
        Some(&FunctionCallOutputContentItem::InputText {
            text: "begin".to_owned()
        })
    );
    assert_eq!(
        content_items.last(),
        Some(&FunctionCallOutputContentItem::InputText {
            text: "end".to_owned()
        })
    );
    assert!(matches!(
        &content_items[MAX_OUTPUT_ITEMS / 2],
        FunctionCallOutputContentItem::InputText { text }
            if text.starts_with("[Code Mode session output limit reached: omitted ")
    ));
}

#[tokio::test]
async fn huge_output_is_bounded_and_yield_drain_starts_a_new_budget() {
    let session = server_session();
    // Continue immediately after yielding to exercise the FIFO boundary as
    // well as discarding the already-delivered bounded snapshot.
    let response = start(
        &session,
        r#"text('😀'.repeat(1000000)); yield_control(); text('after yield');"#,
    )
    .await;
    let RuntimeResponse::Yielded {
        cell_id,
        content_items,
        ..
    } = response
    else {
        panic!("expected yielded cell");
    };
    assert_eq!(
        content_items,
        vec![
            FunctionCallOutputContentItem::InputText {
                text: "😀".repeat(MAX_OUTPUT_BYTES / 8)
            },
            FunctionCallOutputContentItem::InputText {
                text: format!(
                    "[Code Mode session output limit reached: omitted {} text bytes; discarded 0 text segments, 0 images, 0 audio items]",
                    4_000_000 - MAX_OUTPUT_BYTES
                )
            },
            FunctionCallOutputContentItem::InputText {
                text: "😀".repeat(MAX_OUTPUT_BYTES / 8)
            },
        ]
    );
    let response = session
        .wait(
            WaitRequest {
                cell_id,
                yield_time_ms: 60_000,
                max_output_tokens: None,
            },
            /*preempt*/ None,
        )
        .await
        .unwrap();
    let WaitOutcome::LiveCell(RuntimeResponse::Result {
        content_items,
        error_text,
        ..
    }) = response
    else {
        panic!("expected completed cell");
    };
    assert_eq!(
        (content_items, error_text),
        (
            vec![FunctionCallOutputContentItem::InputText {
                text: "after yield".to_owned()
            }],
            None
        )
    );
}

#[tokio::test]
async fn a_single_oversized_image_is_omitted_without_losing_surrounding_text() {
    let session = server_session();
    let response = start(
        &session,
        &format!("text('before'); image('data:image/png;base64,' + 'A'.repeat({MAX_IMAGE_BYTES})); text('after');"),
    )
    .await;
    let RuntimeResponse::Result {
        content_items,
        error_text,
        ..
    } = response
    else {
        panic!("expected completed cell");
    };
    assert_eq!(
        (content_items, error_text),
        (
            vec![
                FunctionCallOutputContentItem::InputText {
                    text: "before".to_owned()
                },
                FunctionCallOutputContentItem::InputText {
                    text: "[Code Mode session output limit reached: omitted 0 text bytes; discarded 0 text segments, 1 images, 0 audio items]".to_owned()
                },
                FunctionCallOutputContentItem::InputText {
                    text: "after".to_owned()
                }
            ],
            None
        )
    );
}

#[tokio::test]
async fn termination_releases_a_flooding_bounded_runtime() {
    let session = server_session();
    let emitted = MAX_OUTPUT_ITEMS + 100;
    let response = start(
        &session,
        &format!(
            "for (let i = 0; ; i++) {{
                text('output');
                if (i === {emitted}) yield_control();
            }}"
        ),
    )
    .await;
    let RuntimeResponse::Yielded {
        cell_id,
        content_items,
        ..
    } = response
    else {
        panic!("expected yielded cell");
    };
    assert_eq!(content_items.len(), MAX_OUTPUT_ITEMS);
    assert!(matches!(
        &content_items[MAX_OUTPUT_ITEMS / 2],
        FunctionCallOutputContentItem::InputText { text }
            if text.starts_with("[Code Mode session output limit reached: omitted ")
    ));
    let response = tokio::time::timeout(Duration::from_secs(5), session.terminate(cell_id.into()))
        .await
        .expect("termination must unblock output")
        .unwrap();
    assert!(matches!(
        response,
        WaitOutcome::LiveCell(RuntimeResponse::Terminated { .. })
    ));
    session.shutdown().await.unwrap();
}

#[tokio::test]
async fn terminal_error_shares_the_collector_limit_and_remains_a_failure() {
    let session = server_session();
    let response = start(
        &session,
        "text('before'); throw 'error-start:' + 'x'.repeat(2 * 1024 * 1024) + ':error-end';",
    )
    .await;
    let RuntimeResponse::Result {
        content_items,
        error_text,
        ..
    } = response
    else {
        panic!("expected failed cell");
    };
    let diagnostic = error_text.expect("overflowed diagnostics still report failure");
    // The model-output budget is generous, but the diagnostic has its own limit.
    assert!(diagnostic.len() <= MAX_SERVER_DIAGNOSTIC_BYTES);
    assert!(diagnostic.starts_with("error-start:"));
    assert!(diagnostic.ends_with(":error-end"));
    let notice = content_items
        .iter()
        .find_map(|item| match item {
            FunctionCallOutputContentItem::InputText { text }
                if text.starts_with("[Code Mode session output limit reached: omitted ") =>
            {
                Some(text)
            }
            _ => None,
        })
        .expect("terminal overflow notice");
    let total_text_bytes: usize = content_items
        .iter()
        .map(|item| match item {
            FunctionCallOutputContentItem::InputText { text } => text.len(),
            _ => 0,
        })
        .sum();
    assert!(total_text_bytes - notice.len() <= MAX_OUTPUT_BYTES);
    assert!(matches!(content_items.last(),
        Some(FunctionCallOutputContentItem::InputText { text }) if text.ends_with(":error-end")
    ));
    session.shutdown().await.unwrap();
}

#[tokio::test]
async fn wait_preserves_diagnostics_across_output_budgets() {
    let source = "yield_control(); text('before'); throw 'start:' + '🎷'.repeat(4096) + ':end';";
    let raw_error = format!("start:{}:end", "🎷".repeat(4096));
    for mode in [
        CodeModeOutputTruncation::Client,
        CodeModeOutputTruncation::Server,
    ] {
        let session = InProcessCodeModeSession::with_options(
            CodeModeSessionOptions {
                output_truncation: mode,
                ..CodeModeSessionOptions::default()
            },
            /*task_failure_handler*/ None,
        );
        // The error fits the diagnostic limit but exceeds both output budgets.
        for max_output_tokens in [0, 16] {
            let response = start(&session, source).await;
            let RuntimeResponse::Yielded { cell_id, .. } = response else {
                panic!("expected yield before error");
            };
            let WaitOutcome::LiveCell(RuntimeResponse::Result {
                content_items,
                error_text,
                ..
            }) = session
                .wait(
                    WaitRequest {
                        cell_id,
                        yield_time_ms: 60_000,
                        max_output_tokens: Some(max_output_tokens),
                    },
                    /*preempt*/ None,
                )
                .await
                .unwrap()
            else {
                panic!("expected completed cell");
            };
            let diagnostic = error_text.expect("errors remain failures at all budgets");
            assert_eq!(diagnostic, raw_error);
            match mode {
                CodeModeOutputTruncation::Client => {
                    assert_eq!(
                        content_items,
                        vec![FunctionCallOutputContentItem::InputText {
                            text: "before".to_owned()
                        }]
                    );
                }
                CodeModeOutputTruncation::Server => {
                    if max_output_tokens != 0 {
                        assert!(matches!(
                            content_items.first(),
                            Some(FunctionCallOutputContentItem::InputText { text })
                                if text.starts_with("Warning: truncated output")
                        ));
                    }
                }
            }
        }
        session.shutdown().await.unwrap();
    }
}

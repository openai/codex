use super::App;
use super::RECAP_DELAY;
use super::RECAP_HISTORY_MAX_TURNS;
use super::RECAP_MAX_CHARS;
use super::RECAP_PROMPT_MAX_BYTES;
use super::RECAP_RETRY_DELAY;
use super::RecapProgress;
use super::RecapRequest;
use super::RecapState;
use super::parse_recap;
use super::recap_history;
use super::recap_prompt;
use crate::app::test_support::make_test_app;
use crate::app_event::AppEvent;
use crate::app_event::RecapTrigger;
use crate::app_event_sender::AppEventSender;
use crate::history_cell::AgentMarkdownCell;
use crate::history_cell::AgentMessageCell;
use crate::history_cell::HistoryCell;
use crate::history_cell::PlainHistoryCell;
use crate::history_cell::ThreadRecapHistoryCell;
use crate::history_cell::ThreadRecapLoadingCell;
use crate::history_cell::UserHistoryCell;
use crate::line_truncation::line_width;
use codex_app_server_protocol::Turn;
use codex_app_server_protocol::TurnStatus;
use codex_protocol::ThreadId;
use pretty_assertions::assert_eq;
use ratatui::style::Modifier;
use ratatui::style::Stylize;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;
use tokio::sync::mpsc::error::TryRecvError;
use uuid::Uuid;

fn turn(status: TurnStatus) -> Turn {
    Turn {
        id: "turn".to_string(),
        items: Vec::new(),
        items_view: Default::default(),
        status,
        error: None,
        started_at: None,
        completed_at: None,
        duration_ms: None,
    }
}

fn user_history_cell(message: &str) -> Arc<dyn HistoryCell> {
    Arc::new(UserHistoryCell {
        message: message.to_string(),
        spoken: false,
        text_elements: Vec::new(),
        local_image_paths: Vec::new(),
        remote_image_urls: Vec::new(),
    })
}

fn assistant_history_cell(message: &str) -> Arc<dyn HistoryCell> {
    Arc::new(AgentMarkdownCell::new(message.to_string(), Path::new(".")))
}

#[test]
fn recap_history_preserves_chronological_user_and_assistant_messages() {
    let cells = vec![
        user_history_cell("First request"),
        assistant_history_cell("First response"),
        user_history_cell("Second request"),
        Arc::new(AgentMessageCell::new(
            vec!["Streaming response".into()],
            /*is_first_line*/ true,
        )),
    ];

    assert_eq!(
        recap_history(&cells),
        "User: First request\n\nAssistant: First response\n\nUser: Second request\n\nAssistant: Streaming response"
    );
}

#[test]
fn recap_history_keeps_only_the_most_recent_eight_user_turns() {
    let total_turns = RECAP_HISTORY_MAX_TURNS + 2;
    let mut cells = Vec::new();

    for index in 0..total_turns {
        cells.push(user_history_cell(&format!("question-{index}")));
        cells.push(assistant_history_cell(&format!("answer-{index}")));
    }

    let expected = (2..total_turns)
        .map(|index| format!("User: question-{index}\n\nAssistant: answer-{index}"))
        .collect::<Vec<_>>()
        .join("\n\n");

    assert_eq!(recap_history(&cells), expected);
}

#[test]
fn recap_history_ignores_activity_previous_recaps_and_empty_messages() {
    let cells = vec![
        user_history_cell("  "),
        Arc::new(PlainHistoryCell::new(vec!["tool output".into()])),
        user_history_cell("Implement recap"),
        assistant_history_cell(" \n "),
        Arc::new(ThreadRecapHistoryCell::new("Previous recap".to_string())),
        assistant_history_cell("Done"),
    ];

    assert_eq!(
        recap_history(&cells),
        "User: Implement recap\n\nAssistant: Done"
    );
}

#[test]
fn recap_history_preserves_latest_user_turn_when_latest_response_is_oversized() {
    let cells = vec![
        user_history_cell("Keep this latest request"),
        assistant_history_cell(&"🦀".repeat(RECAP_PROMPT_MAX_BYTES * 2)),
    ];
    let prompt = recap_prompt(&recap_history(&cells));

    assert!(prompt.len() <= RECAP_PROMPT_MAX_BYTES);
    assert!(prompt.contains("User: Keep this latest request"));
    assert!(prompt.contains("Assistant: 🦀"));
}

#[test]
fn recap_history_caps_utf8_bytes_without_splitting_characters() {
    let cells = vec![user_history_cell(
        &"最新の進捗🦀".repeat(RECAP_PROMPT_MAX_BYTES),
    )];
    let history = recap_history(&cells);

    assert!(recap_prompt(&history).len() <= RECAP_PROMPT_MAX_BYTES);
    assert!(history.starts_with("User: 最新の進捗🦀"));
}

#[test]
fn generated_recap_is_normalized_and_bounded() {
    let expected = "🚀".repeat(RECAP_MAX_CHARS);
    let cases = [
        (
            serde_json::json!({ "recap": "  Fixed the parser.  \n" }).to_string(),
            Some("Fixed the parser.".to_string()),
        ),
        (
            serde_json::json!({ "recap": format!("{expected}discarded") }).to_string(),
            Some(expected),
        ),
        ("not json".to_string(), None),
        (r#"{"recap":"  \t  "}"#.to_string(), None),
    ];

    for (response, expected) in cases {
        assert_eq!(parse_recap(&response), expected, "response: {response}");
    }
}

#[test]
fn recap_requires_focus_loss_and_three_completed_turns() {
    let now = Instant::now();
    let mut state = RecapState::default();

    state.note_turn_finished(&TurnStatus::Completed, now);
    state.note_turn_finished(&TurnStatus::Completed, now);
    assert!(!state.should_generate(now + RECAP_DELAY));

    state.note_focus_lost(now);
    assert!(!state.should_generate(now + RECAP_DELAY));

    state.note_turn_finished(&TurnStatus::Completed, now);
    assert!(state.should_generate(now + RECAP_DELAY));
}

#[test]
fn recap_waits_after_focus_loss_even_if_turn_completed_earlier() {
    let started = Instant::now();
    let mut state = RecapState::default();

    for _ in 0..3 {
        state.note_turn_finished(&TurnStatus::Completed, started);
    }

    let focus_lost = started + RECAP_DELAY;
    state.note_focus_lost(focus_lost);

    assert!(!state.should_generate(focus_lost));
    assert!(!state.should_generate(focus_lost + RECAP_DELAY - Duration::from_secs(/*secs*/ 1)));
    assert!(state.should_generate(focus_lost + RECAP_DELAY));
}

#[test]
fn completed_turn_resets_recap_deadline_while_unfocused() {
    let focus_lost = Instant::now();
    let mut state = RecapState::default();
    state.note_focus_lost(focus_lost);

    for _ in 0..3 {
        state.note_turn_finished(&TurnStatus::Completed, focus_lost);
    }

    let last_completed_turn_at = focus_lost + Duration::from_secs(/*secs*/ 30);
    state.note_turn_finished(&TurnStatus::Completed, last_completed_turn_at);

    assert!(!state.should_generate(focus_lost + RECAP_DELAY));
    assert!(state.should_generate(last_completed_turn_at + RECAP_DELAY));
}

#[test]
fn failed_turn_invalidates_ready_recap_without_counting_as_completed() {
    let focus_lost = Instant::now();
    let mut state = RecapState::default();
    state.note_focus_lost(focus_lost);
    for _ in 0..3 {
        state.note_turn_finished(&TurnStatus::Completed, focus_lost);
    }
    let failed_at = focus_lost + RECAP_DELAY;
    state.note_turn_finished(&TurnStatus::Failed, failed_at);

    assert_eq!(state.completed_turns, 3);
    assert_eq!(state.turn_revision, 4);
    assert!(!state.should_generate(failed_at));
    assert!(state.should_generate(failed_at + RECAP_DELAY));
}

#[test]
fn repeated_focus_loss_does_not_restart_recap_delay() {
    let focus_lost = Instant::now();
    let mut state = RecapState::default();
    state.note_focus_lost(focus_lost);

    for _ in 0..3 {
        state.note_turn_finished(&TurnStatus::Completed, focus_lost);
    }

    state.note_focus_lost(focus_lost + Duration::from_secs(/*secs*/ 30));

    assert!(state.should_generate(focus_lost + RECAP_DELAY));
}

#[test]
fn regaining_focus_prevents_recap_generation() {
    let now = Instant::now();
    let mut state = RecapState::default();
    state.note_focus_lost(now);

    for _ in 0..3 {
        state.note_turn_finished(&TurnStatus::Completed, now);
    }

    state.note_focus_gained();

    assert!(!state.should_generate(now + RECAP_DELAY));
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn regaining_focus_cancels_scheduled_check() {
    let mut state = RecapState::default();
    state.note_focus_lost(Instant::now());

    let (task_tx, mut task_rx) = tokio::sync::mpsc::unbounded_channel();
    state.scheduled_check = Some(tokio::spawn(async move {
        tokio::time::sleep(RECAP_DELAY).await;
        task_tx.send(()).expect("scheduled recap event");
    }));

    tokio::task::yield_now().await;

    state.note_focus_gained();
    tokio::time::advance(RECAP_DELAY).await;
    tokio::task::yield_now().await;

    assert!(state.scheduled_check.is_none());
    assert!(matches!(
        task_rx.try_recv(),
        Err(tokio::sync::mpsc::error::TryRecvError::Disconnected)
    ));
}

#[test]
fn regaining_focus_preserves_only_manual_in_flight_request() {
    let request_id = Uuid::new_v4();
    let thread_id = ThreadId::new();
    let mut state = RecapState::default();
    state.in_flight_request_id = Some(request_id);
    state.in_flight_trigger = Some(RecapTrigger::Manual);
    state.in_flight_thread_id = Some(thread_id);

    state.note_focus_gained();

    assert_eq!(
        (
            state.in_flight_request_id,
            state.in_flight_trigger,
            state.in_flight_thread_id,
        ),
        (
            Some(request_id),
            Some(RecapTrigger::Manual),
            Some(thread_id),
        )
    );

    state.in_flight_trigger = Some(RecapTrigger::Automatic);
    state.note_focus_gained();

    assert_eq!(
        (
            state.in_flight_request_id,
            state.in_flight_trigger,
            state.in_flight_thread_id,
        ),
        (None, None, None)
    );
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn scheduled_check_fires_after_thirty_minutes() {
    let thread_id = ThreadId::new();
    let now = Instant::now();
    let mut app = make_test_app().await;
    let (event_tx, mut event_rx) = tokio::sync::mpsc::unbounded_channel();
    app.app_event_tx = AppEventSender::new(event_tx);

    app.recap.note_focus_lost(now);
    for _ in 0..3 {
        app.recap.note_turn_finished(&TurnStatus::Completed, now);
    }
    app.schedule_recap_check(thread_id, now);
    tokio::task::yield_now().await;

    tokio::time::advance(Duration::from_secs(/*secs*/ 30 * 60 - 1)).await;
    tokio::task::yield_now().await;
    assert!(matches!(event_rx.try_recv(), Err(TryRecvError::Empty)));

    tokio::time::advance(Duration::from_secs(/*secs*/ 1)).await;
    tokio::task::yield_now().await;
    assert!(matches!(
        event_rx.try_recv(),
        Ok(AppEvent::CheckRecap {
            thread_id: event_thread_id,
        }) if event_thread_id == thread_id
    ));
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn rescheduling_check_cancels_the_earlier_timer() {
    let thread_id = ThreadId::new();
    let first_turn = Instant::now();
    let mut app = make_test_app().await;
    let (event_tx, mut event_rx) = tokio::sync::mpsc::unbounded_channel();
    app.app_event_tx = AppEventSender::new(event_tx);

    app.recap.note_focus_lost(first_turn);
    for _ in 0..3 {
        app.recap
            .note_turn_finished(&TurnStatus::Completed, first_turn);
    }
    app.schedule_recap_check(thread_id, first_turn);
    tokio::task::yield_now().await;

    let elapsed = Duration::from_secs(/*secs*/ 1);
    tokio::time::advance(elapsed).await;
    let later_turn = first_turn + elapsed;
    app.recap
        .note_turn_finished(&TurnStatus::Completed, later_turn);
    app.schedule_recap_check(thread_id, later_turn);
    tokio::task::yield_now().await;

    tokio::time::advance(RECAP_DELAY - elapsed).await;
    tokio::task::yield_now().await;
    assert!(matches!(event_rx.try_recv(), Err(TryRecvError::Empty)));

    tokio::time::advance(elapsed).await;
    tokio::task::yield_now().await;
    assert!(matches!(
        event_rx.try_recv(),
        Ok(AppEvent::CheckRecap {
            thread_id: event_thread_id,
        }) if event_thread_id == thread_id
    ));
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn auto_recap_opt_out_cancels_scheduling_after_restored_progress() {
    let thread_id = ThreadId::new();
    let now = Instant::now();
    let mut app = make_test_app().await;
    let (event_tx, mut event_rx) = tokio::sync::mpsc::unbounded_channel();
    app.app_event_tx = AppEventSender::new(event_tx);
    app.recap.note_focus_lost(now);
    let progress = RecapProgress {
        completed_turns: 3,
        last_recapped_turn_count: None,
    };
    app.recap.seed_from_progress(progress, now);
    app.schedule_recap_check(thread_id, now);
    tokio::task::yield_now().await;

    app.local_settings.tui.auto_recap = false;
    app.schedule_recap_check(thread_id, now);
    tokio::time::advance(RECAP_DELAY).await;
    tokio::task::yield_now().await;

    assert!(app.recap.scheduled_check.is_none());
    assert!(matches!(event_rx.try_recv(), Err(TryRecvError::Empty)));
    assert_eq!(app.recap.progress(), progress);
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn dropping_recap_state_cancels_timer() {
    let mut state = RecapState::default();

    let (scheduled_tx, mut scheduled_rx) = tokio::sync::mpsc::unbounded_channel();
    state.scheduled_check = Some(tokio::spawn(async move {
        tokio::time::sleep(RECAP_DELAY).await;
        scheduled_tx.send(()).expect("scheduled check");
    }));

    tokio::task::yield_now().await;
    drop(state);
    tokio::time::advance(RECAP_DELAY).await;
    tokio::task::yield_now().await;

    assert!(matches!(
        scheduled_rx.try_recv(),
        Err(TryRecvError::Disconnected)
    ));
}

#[test]
fn another_recap_requires_two_additional_completed_turns() {
    let now = Instant::now();
    let mut state = RecapState::default();
    state.note_focus_lost(now);
    state.seed_from_progress(
        RecapProgress {
            completed_turns: 3,
            last_recapped_turn_count: None,
        },
        now,
    );
    state.mark_recapped(/*completed_turn_count*/ 3);
    assert!(!state.should_generate(now + RECAP_DELAY));

    let fourth_turn = now + RECAP_DELAY;
    state.note_turn_finished(&TurnStatus::Completed, fourth_turn);
    assert!(!state.should_generate(fourth_turn + RECAP_DELAY));

    let fifth_turn = fourth_turn + Duration::from_secs(/*secs*/ 1);
    state.note_turn_finished(&TurnStatus::Completed, fifth_turn);

    assert!(state.should_generate(fifth_turn + RECAP_DELAY));
}

#[test]
fn replacing_the_primary_thread_resets_progress_but_preserves_focus() {
    let now = Instant::now();
    let mut state = RecapState::default();
    state.note_focus_lost(now);
    state.seed_from_progress(
        RecapProgress {
            completed_turns: 3,
            last_recapped_turn_count: Some(3),
        },
        now,
    );

    let replaced_at = now + Duration::from_secs(/*secs*/ 30);
    state.reset_for_new_thread(replaced_at);

    assert_eq!(state.progress(), RecapProgress::default());
    assert_eq!(state.unfocused_since, Some(replaced_at));
    assert!(!state.should_generate(replaced_at + RECAP_DELAY));
}

#[test]
fn restored_history_counts_only_completed_turns() {
    let now = Instant::now();
    let mut state = RecapState::default();
    state.note_focus_lost(now);
    state.seed_from_turns(
        &[
            turn(TurnStatus::Completed),
            turn(TurnStatus::Failed),
            turn(TurnStatus::Completed),
            turn(TurnStatus::Interrupted),
            turn(TurnStatus::InProgress),
            turn(TurnStatus::Completed),
        ],
        now,
    );

    assert_eq!(state.completed_turns, 3);
    assert!(state.should_generate(now + RECAP_DELAY));
}

#[test]
fn restored_history_never_reduces_observed_completed_turns() {
    let now = Instant::now();
    let mut state = RecapState::default();

    for _ in 0..4 {
        state.note_turn_finished(&TurnStatus::Completed, now);
    }

    state.seed_from_turns(
        &[
            turn(TurnStatus::Completed),
            turn(TurnStatus::Completed),
            turn(TurnStatus::Completed),
        ],
        now,
    );

    assert_eq!(state.completed_turns, 4);
}

#[test]
fn recap_history_cell_uses_hanging_indent_and_right_padding() {
    let cell =
        ThreadRecapHistoryCell::new("Automatic recaps stay compact on wide terminals.".to_string());
    let lines = cell.display_lines(/*width*/ 56);
    assert!(
        lines
            .iter()
            .all(|line| line.style.add_modifier.contains(Modifier::ITALIC))
    );
    assert_eq!(
        &lines[0].spans[..3],
        &["  ".into(), "↳ ".dim(), "Recap: ".bold()],
    );
    let rendered = lines
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n");

    insta::assert_snapshot!(rendered, @r"
      ↳ Recap: Automatic recaps stay compact on wide
               terminals.
    ");
}

#[test]
fn recap_history_cell_wraps_in_narrow_terminals() {
    let cell = ThreadRecapHistoryCell::new(
        "Keep conversation recaps readable in narrow terminals.".to_string(),
    );
    let rendered = cell
        .display_lines(/*width*/ 32)
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n");

    insta::assert_snapshot!(rendered, @r"
      ↳ Recap: Keep conversation
               recaps readable in
               narrow terminals.
    ");
}

#[test]
fn recap_history_cell_preserves_unicode_and_url_tokens() {
    let cell = ThreadRecapHistoryCell::new(
        "Résumé ready. See https://example.com/review/42 日本語 details.".to_string(),
    );
    let rendered = cell
        .display_lines(/*width*/ 48)
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n");

    insta::assert_snapshot!(rendered, @r"
      ↳ Recap: Résumé ready. See
               https://example.com/review/42
               日本語 details.
    ");
}

#[test]
fn recap_history_cell_wraps_long_urls_in_narrow_terminals() {
    let cell = ThreadRecapHistoryCell::new(
        "The café review is paused with the draft ready at https://example.com/review/42. 日本語"
            .to_string(),
    );
    let lines = cell.display_lines(/*width*/ 32);
    assert!(lines.iter().all(|line| line_width(line) <= 30));
    assert!(
        lines
            .iter()
            .skip(/*n*/ 1)
            .all(|line| line.to_string().starts_with("           "))
    );
    assert!(
        lines
            .iter()
            .all(|line| line.style.add_modifier.contains(Modifier::ITALIC))
    );
    let rendered = lines
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n");

    insta::assert_snapshot!(rendered, @"
    ↳ Recap: The café review is
             paused with the
             draft ready at
             https://example.com
             /review/42. 日本語
    ");
}

#[test]
fn recap_history_cell_splits_only_urls_wider_than_the_text_column() {
    for (url, expected) in [
        (
            "https://example.com",
            vec!["  ↳ Recap: https://example.com"],
        ),
        (
            "https://example.com/",
            vec!["  ↳ Recap: https://example.com", "           /"],
        ),
        (
            "https://a.co/ｶﾞｶﾞｶﾞｶﾞｶﾞｶﾞ",
            vec!["  ↳ Recap: https://a.co/ｶﾞｶﾞｶﾞ", "           ｶﾞｶﾞｶﾞ"],
        ),
    ] {
        let cell = ThreadRecapHistoryCell::new(url.to_string());
        let lines = cell.display_lines(/*width*/ 32);
        assert!(lines.iter().all(|line| line_width(line) <= 30));
        assert_eq!(
            lines.iter().map(ToString::to_string).collect::<Vec<_>>(),
            expected
        );
        let displayed_url = lines
            .iter()
            .map(|line| line.to_string().chars().skip(/*n*/ 11).collect::<String>())
            .collect::<String>();
        assert_eq!(displayed_url, url);
    }
}

#[test]
fn recap_history_cell_uses_available_space_below_indent_width() {
    let cell = ThreadRecapHistoryCell::new(
        "Resume this task.\nhttps://example.com/review/42.".to_string(),
    );
    let rendered = cell
        .display_lines(/*width*/ 12)
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n");

    insta::assert_snapshot!(rendered, @r"
    ↳ Recap:
    Resume
    this task.
    https://ex
    ample.com/
    review/42.
    ");
    assert_eq!(cell.display_lines(/*width*/ 0), Vec::new());
}

#[test]
fn recap_history_cell_preserves_heading_in_raw_history() {
    let cell = ThreadRecapHistoryCell::new("Resume this task.".to_string());
    let rendered = cell
        .raw_lines()
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n");

    insta::assert_snapshot!(rendered, @r"
    Conversation recap
    Resume this task.
    ");
}

#[test]
fn recap_history_cell_preserves_explicit_line_breaks() {
    let cell = ThreadRecapHistoryCell::new(
        "Finished the parser.\nNext: run the focused tests.".to_string(),
    );
    let displayed = cell
        .display_lines(/*width*/ 48)
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n");
    let raw = cell
        .raw_lines()
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n");

    insta::assert_snapshot!(displayed, @r"
      ↳ Recap: Finished the parser.
               Next: run the focused tests.
    ");
    insta::assert_snapshot!(raw, @r"
    Conversation recap
    Finished the parser.
    Next: run the focused tests.
    ");
}

fn track_in_flight_recap(app: &mut App, thread_id: ThreadId) -> (RecapRequest, ThreadId) {
    let request = RecapRequest {
        thread_id,
        request_id: Uuid::new_v4(),
        trigger: RecapTrigger::Automatic,
        completed_turn_count: 3,
        turn_revision: 3,
    };
    let temporary_thread_id = ThreadId::new();
    app.recap.completed_turns = request.completed_turn_count;
    app.recap.turn_revision = request.turn_revision;
    app.recap.in_flight_request_id = Some(request.request_id);
    app.recap.in_flight_trigger = Some(request.trigger);
    app.recap.in_flight_thread_id = Some(temporary_thread_id);
    app.recap.in_flight_request = Some(tokio::spawn(async {}));
    (request, temporary_thread_id)
}

async fn app_with_visible_thread(thread_id: ThreadId) -> App {
    let mut app = make_test_app().await;
    app.active_thread_id = Some(thread_id);
    app
}

#[tokio::test]
async fn generated_recap_is_returned_for_synchronous_insertion() {
    let thread_id = ThreadId::new();
    let mut app = app_with_visible_thread(thread_id).await;
    let (request, temporary_thread_id) = track_in_flight_recap(&mut app, thread_id);

    let cell = app
        .handle_generated_recap(
            request,
            temporary_thread_id,
            Ok(serde_json::json!({ "recap": "  Continue with focused tests.  " }).to_string()),
        )
        .expect("fresh recap");

    assert_eq!(app.recap.last_recapped_turn_count, Some(3));
    assert_eq!(
        cell.raw_lines()
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>(),
        vec!["Conversation recap", "Continue with focused tests."]
    );
}

#[tokio::test]
async fn auto_recap_opt_out_discards_results_without_retrying() {
    let thread_id = ThreadId::new();
    let mut app = app_with_visible_thread(thread_id).await;
    for result in [
        Ok(serde_json::json!({ "recap": "obsolete" }).to_string()),
        Err("temporary failure".to_string()),
    ] {
        let (request, temporary_thread_id) = track_in_flight_recap(&mut app, thread_id);
        app.local_settings.tui.auto_recap = false;

        assert!(
            app.handle_generated_recap(request, temporary_thread_id, result)
                .is_none()
        );
        assert!(app.recap.in_flight_request.is_none());
        assert!(app.recap.scheduled_check.is_none());
        assert_eq!(app.recap.last_recapped_turn_count, None);
    }
}

#[tokio::test]
async fn obsolete_recap_result_does_not_clear_the_current_request() {
    let thread_id = ThreadId::new();
    let mut app = app_with_visible_thread(thread_id).await;
    let (request, temporary_thread_id) = track_in_flight_recap(&mut app, thread_id);

    let cell = app.handle_generated_recap(
        RecapRequest {
            request_id: Uuid::new_v4(),
            ..request
        },
        temporary_thread_id,
        Ok(serde_json::json!({ "recap": "obsolete" }).to_string()),
    );

    assert!(cell.is_none());
    assert_eq!(app.recap.in_flight_request_id, Some(request.request_id));
    assert_eq!(app.recap.in_flight_thread_id, Some(temporary_thread_id));
}

#[tokio::test]
async fn newer_terminal_turn_invalidates_generated_recap() {
    let thread_id = ThreadId::new();
    let mut app = app_with_visible_thread(thread_id).await;
    let (request, temporary_thread_id) = track_in_flight_recap(&mut app, thread_id);
    app.recap
        .note_turn_finished(&TurnStatus::Failed, Instant::now());

    let cell = app.handle_generated_recap(
        request,
        temporary_thread_id,
        Ok(serde_json::json!({ "recap": "missing the failure" }).to_string()),
    );

    assert!(cell.is_none());
    assert_eq!(app.recap.last_recapped_turn_count, None);
    assert!(app.recap.in_flight_request.is_none());
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn manual_recap_failure_does_not_schedule_retry() {
    let thread_id = ThreadId::new();
    let mut app = app_with_visible_thread(thread_id).await;
    let (mut request, temporary_thread_id) = track_in_flight_recap(&mut app, thread_id);
    request.trigger = RecapTrigger::Manual;
    app.recap.in_flight_trigger = Some(RecapTrigger::Manual);
    app.transcript_cells
        .push(Arc::new(ThreadRecapLoadingCell::new(
            /*animations_enabled*/ false,
        )));

    assert!(
        app.handle_generated_recap(
            request,
            temporary_thread_id,
            Err("temporary failure".to_string()),
        )
        .is_none()
    );

    tokio::time::advance(RECAP_RETRY_DELAY).await;
    tokio::task::yield_now().await;

    assert_eq!(app.recap.retry_revision, None);
    assert!(app.recap.scheduled_check.is_none());
    assert!(
        !app.transcript_cells
            .iter()
            .any(|cell| cell.as_any().is::<ThreadRecapLoadingCell>())
    );
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn recap_failure_retries_once_for_the_same_turn_revision() {
    let thread_id = ThreadId::new();
    let mut app = app_with_visible_thread(thread_id).await;
    let (event_tx, mut event_rx) = tokio::sync::mpsc::unbounded_channel();
    app.app_event_tx = AppEventSender::new(event_tx);

    for attempt in 0..2 {
        let (request, temporary_thread_id) = track_in_flight_recap(&mut app, thread_id);
        assert!(
            app.handle_generated_recap(
                request,
                temporary_thread_id,
                Err("temporary failure".to_string()),
            )
            .is_none()
        );
        tokio::task::yield_now().await;
        tokio::time::advance(RECAP_RETRY_DELAY).await;
        tokio::task::yield_now().await;

        if attempt == 0 {
            assert!(matches!(
                event_rx.try_recv(),
                Ok(AppEvent::CheckRecap {
                    thread_id: event_thread_id,
                }) if event_thread_id == thread_id
            ));
        } else {
            assert!(matches!(event_rx.try_recv(), Err(TryRecvError::Empty)));
        }
    }
}

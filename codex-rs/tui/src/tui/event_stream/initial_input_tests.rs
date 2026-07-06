use std::time::Duration;

use crossterm::event::Event;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;
use crossterm::event::KeyModifiers;
use pretty_assertions::assert_eq;
use tokio::time::timeout;
use tokio_stream::StreamExt;

use super::*;
use crate::tui::TuiEvent;
use crate::tui::event_stream::InitialInputConfig;
use crate::tui::event_stream::tests::make_stream;
use crate::tui::event_stream::tests::setup;

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn initial_input_handoff_resets_quiet_for_legacy_repeats() {
    let (broker, handle, draw_tx, draw_rx, terminal_focused) = setup();
    let mut stream = make_stream(broker, draw_rx, terminal_focused)
        .with_enhanced_key_events(/*enhanced_key_events*/ false)
        .filtering_initial_input(InitialInputConfig::new(InitialInputPolicy::DiscardAll));
    handle.send(Ok(Event::Key(KeyEvent::new(
        KeyCode::Enter,
        KeyModifiers::NONE,
    ))));

    assert!(
        timeout(Duration::from_nanos(1), stream.next())
            .await
            .is_err()
    );
    tokio::time::advance(STARTUP_INPUT_QUIET_PERIOD / 2).await;
    handle.send(Ok(Event::Key(KeyEvent::new_with_kind(
        KeyCode::Enter,
        KeyModifiers::NONE,
        KeyEventKind::Repeat,
    ))));
    assert!(
        timeout(Duration::from_nanos(1), stream.next())
            .await
            .is_err()
    );
    tokio::time::advance(STARTUP_INPUT_QUIET_PERIOD / 2).await;
    assert!(
        timeout(Duration::from_nanos(1), stream.next())
            .await
            .is_err()
    );
    tokio::time::advance(STARTUP_INPUT_QUIET_PERIOD / 2).await;
    assert!(
        timeout(Duration::from_nanos(1), stream.next())
            .await
            .is_err()
    );

    let _ = draw_tx.send(());
    assert!(matches!(stream.next().await, Some(TuiEvent::Draw)));
    assert!(
        timeout(Duration::from_nanos(1), stream.next())
            .await
            .is_err()
    );
    tokio::time::advance(STARTUP_INPUT_QUIET_PERIOD).await;
    assert!(
        timeout(Duration::from_nanos(1), stream.next())
            .await
            .is_err()
    );

    let expected = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
    handle.send(Ok(Event::Key(expected)));
    match timeout(Duration::from_nanos(1), stream.next()).await {
        Ok(Some(TuiEvent::Key(actual))) => assert_eq!(actual, expected),
        other => panic!("expected post-handoff key event, saw {other:?}"),
    }
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn initial_input_handoff_releases_legacy_repeat_after_key_up() {
    let (broker, handle, _draw_tx, draw_rx, terminal_focused) = setup();
    let mut stream = make_stream(broker, draw_rx, terminal_focused)
        .filtering_initial_input(InitialInputConfig::new(InitialInputPolicy::DiscardAll));
    handle.send(Ok(Event::Key(KeyEvent::new(
        KeyCode::Enter,
        KeyModifiers::NONE,
    ))));
    assert!(
        timeout(Duration::from_nanos(1), stream.next())
            .await
            .is_err()
    );
    handle.send(Ok(Event::Key(KeyEvent::new_with_kind(
        KeyCode::Enter,
        KeyModifiers::NONE,
        KeyEventKind::Release,
    ))));
    assert!(
        timeout(Duration::from_nanos(1), stream.next())
            .await
            .is_err()
    );

    let expected = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
    handle.send(Ok(Event::Key(expected)));
    assert!(matches!(stream.next().await, Some(TuiEvent::Key(actual)) if actual == expected));
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn first_draw_is_followed_by_startup_input_settlement() {
    let (broker, _handle, draw_tx, draw_rx, terminal_focused) = setup();
    let mut stream = make_stream(broker, draw_rx, terminal_focused)
        .with_enhanced_key_events(/*enhanced_key_events*/ false)
        .filtering_initial_input(InitialInputConfig::new(InitialInputPolicy::PreserveText))
        .protecting_initial_submission_bindings(vec![key_hint::plain(KeyCode::Enter)]);
    let _ = draw_tx.send(());

    assert!(matches!(stream.next().await, Some(TuiEvent::Draw)));
    assert!(matches!(
        stream.next().await,
        Some(TuiEvent::StartupInputSettled)
    ));
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn tui_startup_policy_protects_a_handoff_without_captured_input() {
    let (broker, _handle, draw_tx, draw_rx, terminal_focused) = setup();
    let mut stream = crate::tui::configure_initial_input(
        make_stream(broker, draw_rx, terminal_focused),
        InitialInputPolicy::PreserveText,
        crate::tui::startup::StartupInputHandoff {
            claimed: true,
            ..Default::default()
        },
    );
    let _ = draw_tx.send(());

    assert!(matches!(stream.next().await, Some(TuiEvent::Draw)));
    assert!(matches!(
        stream.next().await,
        Some(TuiEvent::StartupInputSettled)
    ));
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn unknown_probe_action_settles_after_a_rendered_quiet_boundary() {
    let (broker, _handle, draw_tx, draw_rx, terminal_focused) = setup();
    let mut stream = crate::tui::configure_initial_input(
        make_stream(broker, draw_rx, terminal_focused),
        InitialInputPolicy::PreserveText,
        crate::tui::startup::StartupInputHandoff {
            claimed: true,
            unknown_action_seen: true,
            ..Default::default()
        },
    );
    let _ = draw_tx.send(());

    assert!(matches!(stream.next().await, Some(TuiEvent::Draw)));
    tokio::time::advance(STARTUP_INPUT_QUIET_PERIOD).await;
    assert!(matches!(
        stream.next().await,
        Some(TuiEvent::StartupInputSettled)
    ));
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn captured_printable_text_does_not_drop_a_boundary_duplicate() {
    let (broker, handle, draw_tx, draw_rx, terminal_focused) = setup();
    let mut stream = crate::tui::configure_initial_input(
        make_stream(broker, draw_rx, terminal_focused)
            .with_enhanced_key_events(/*enhanced_key_events*/ false),
        InitialInputPolicy::PreserveText,
        crate::tui::startup::StartupInputHandoff {
            claimed: true,
            restored_text: true,
            trailing_printable_action: Some((key_hint::plain(KeyCode::Char('o')), false)),
            ..Default::default()
        },
    );
    let _ = draw_tx.send(());
    let repeated = KeyEvent::new(KeyCode::Char('o'), KeyModifiers::NONE);
    handle.send(Ok(Event::Key(repeated)));

    assert!(
        matches!(stream.next().await, Some(TuiEvent::StartupComposerKey(actual)) if actual == repeated)
    );
    let repeat = KeyEvent::new(KeyCode::Char('o'), KeyModifiers::NONE);
    handle.send(Ok(Event::Key(repeat)));
    assert!(
        matches!(stream.next().await, Some(TuiEvent::StartupComposerKey(actual)) if actual == repeat)
    );
    assert!(matches!(stream.next().await, Some(TuiEvent::Draw)));
    tokio::time::advance(STARTUP_INPUT_QUIET_PERIOD).await;
    assert!(matches!(
        stream.next().await,
        Some(TuiEvent::StartupInputSettled)
    ));

    let enter = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
    handle.send(Ok(Event::Key(enter)));
    assert!(matches!(stream.next().await, Some(TuiEvent::Key(actual)) if actual == enter));
}

#[tokio::test(flavor = "current_thread")]
async fn an_unclaimed_runtime_event_stream_does_not_install_the_startup_filter() {
    let (broker, handle, _draw_tx, draw_rx, terminal_focused) = setup();
    let mut stream = crate::tui::configure_initial_input(
        make_stream(broker, draw_rx, terminal_focused),
        InitialInputPolicy::DiscardAll,
        crate::tui::startup::StartupInputHandoff::default(),
    );
    let key = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
    handle.send(Ok(Event::Key(key)));

    assert!(matches!(stream.next().await, Some(TuiEvent::Key(actual)) if actual == key));
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn post_draw_action_is_quarantined_until_composer_protection_expires() {
    let (broker, handle, draw_tx, draw_rx, terminal_focused) = setup();
    let mut stream = make_stream(broker, draw_rx, terminal_focused)
        .with_enhanced_key_events(/*enhanced_key_events*/ false)
        .filtering_initial_input(InitialInputConfig::new(InitialInputPolicy::PreserveText))
        .protecting_initial_submission_bindings(vec![key_hint::plain(KeyCode::Enter)]);
    let _ = draw_tx.send(());
    assert!(matches!(stream.next().await, Some(TuiEvent::Draw)));

    let enter = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
    handle.send(Ok(Event::Key(enter)));
    assert!(
        timeout(Duration::from_nanos(1), stream.next())
            .await
            .is_err()
    );
    tokio::time::advance(STARTUP_INPUT_QUIET_PERIOD).await;
    assert!(matches!(
        stream.next().await,
        Some(TuiEvent::StartupInputSettled)
    ));
    assert!(
        timeout(Duration::from_nanos(1), stream.next())
            .await
            .is_err()
    );

    tokio::time::advance(STARTUP_INPUT_QUIET_PERIOD).await;
    assert!(
        timeout(Duration::from_nanos(1), stream.next())
            .await
            .is_err()
    );
    handle.send(Ok(Event::Key(enter)));
    assert!(matches!(stream.next().await, Some(TuiEvent::Key(actual)) if actual == enter));
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn repeat_ready_at_the_quiet_deadline_extends_quarantine() {
    let (broker, handle, draw_tx, draw_rx, terminal_focused) = setup();
    let mut stream = make_stream(broker, draw_rx, terminal_focused)
        .with_enhanced_key_events(/*enhanced_key_events*/ false)
        .filtering_initial_input(InitialInputConfig {
            start_quiet: true,
            pending_plain_whitespace: "\n".to_string(),
            trailing_action: Some(key_hint::plain(KeyCode::Enter)),
            ..InitialInputConfig::new(InitialInputPolicy::PreserveText)
        })
        .protecting_initial_submission_bindings(vec![key_hint::plain(KeyCode::Enter)]);
    let _ = draw_tx.send(());
    assert!(matches!(stream.next().await, Some(TuiEvent::Draw)));

    tokio::time::advance(Duration::from_secs(60)).await;
    handle.send(Ok(Event::Key(KeyEvent::new_with_kind(
        KeyCode::Enter,
        KeyModifiers::NONE,
        KeyEventKind::Repeat,
    ))));
    assert!(
        timeout(Duration::from_nanos(1), stream.next())
            .await
            .is_err()
    );

    handle.send(Ok(Event::Key(KeyEvent::new_with_kind(
        KeyCode::Enter,
        KeyModifiers::NONE,
        KeyEventKind::Release,
    ))));
    assert!(matches!(
        stream.next().await,
        Some(TuiEvent::StartupInputSettled)
    ));
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn startup_handoff_waits_for_a_quiet_gap_after_legacy_submit_repeats() {
    let (broker, handle, draw_tx, draw_rx, terminal_focused) = setup();
    let mut stream = make_stream(broker, draw_rx, terminal_focused)
        .with_enhanced_key_events(/*enhanced_key_events*/ false)
        .filtering_initial_input(InitialInputConfig {
            start_quiet: true,
            pending_plain_whitespace: "\n".to_string(),
            trailing_action: Some(key_hint::plain(KeyCode::Enter)),
            ..InitialInputConfig::new(InitialInputPolicy::PreserveText)
        })
        .protecting_initial_submission_bindings(vec![key_hint::plain(KeyCode::Enter)]);
    let _ = draw_tx.send(());
    assert!(matches!(stream.next().await, Some(TuiEvent::Draw)));
    assert!(
        timeout(Duration::from_nanos(1), stream.next())
            .await
            .is_err()
    );

    tokio::time::advance(STARTUP_INPUT_QUIET_PERIOD).await;
    assert!(matches!(
        stream.next().await,
        Some(TuiEvent::StartupInputSettled)
    ));
    handle.send(Ok(Event::Key(KeyEvent::new(
        KeyCode::Enter,
        KeyModifiers::NONE,
    ))));
    assert!(
        timeout(Duration::from_nanos(1), stream.next())
            .await
            .is_err()
    );
    handle.send(Ok(Event::Key(KeyEvent::new(
        KeyCode::Enter,
        KeyModifiers::NONE,
    ))));
    assert!(
        timeout(Duration::from_nanos(1), stream.next())
            .await
            .is_err()
    );

    tokio::time::advance(STARTUP_INPUT_QUIET_PERIOD).await;
    assert!(
        timeout(Duration::from_nanos(1), stream.next())
            .await
            .is_err()
    );

    handle.send(Ok(Event::Key(KeyEvent::new(
        KeyCode::Enter,
        KeyModifiers::NONE,
    ))));
    assert!(matches!(
        stream.next().await,
        Some(TuiEvent::Key(KeyEvent {
            code: KeyCode::Enter,
            ..
        }))
    ));
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn restored_text_waits_for_post_render_quiet_without_a_captured_action() {
    let (broker, _handle, draw_tx, draw_rx, terminal_focused) = setup();
    let mut stream = make_stream(broker, draw_rx, terminal_focused).filtering_initial_input(
        InitialInputConfig {
            start_quiet: true,
            ..InitialInputConfig::new(InitialInputPolicy::PreserveText)
        },
    );
    let _ = draw_tx.send(());
    assert!(matches!(stream.next().await, Some(TuiEvent::Draw)));
    assert!(
        timeout(Duration::from_nanos(1), stream.next())
            .await
            .is_err()
    );

    tokio::time::advance(STARTUP_INPUT_QUIET_PERIOD).await;
    assert!(matches!(
        stream.next().await,
        Some(TuiEvent::StartupInputSettled)
    ));
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn distinct_post_draw_action_is_quarantined_then_settles() {
    let (broker, handle, draw_tx, draw_rx, terminal_focused) = setup();
    let mut stream = make_stream(broker, draw_rx, terminal_focused).filtering_initial_input(
        InitialInputConfig {
            start_quiet: true,
            pending_plain_whitespace: "\n".to_string(),
            trailing_action: Some(key_hint::plain(KeyCode::Enter)),
            ..InitialInputConfig::new(InitialInputPolicy::PreserveText)
        },
    );
    let _ = draw_tx.send(());
    assert!(matches!(stream.next().await, Some(TuiEvent::Draw)));

    let custom_submit = KeyEvent::new(KeyCode::Char('j'), KeyModifiers::CONTROL);
    handle.send(Ok(Event::Key(custom_submit)));
    assert!(
        timeout(Duration::from_nanos(1), stream.next())
            .await
            .is_err()
    );
    tokio::time::advance(STARTUP_INPUT_QUIET_PERIOD).await;
    assert!(matches!(
        stream.next().await,
        Some(TuiEvent::StartupInputSettled)
    ));
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn interrupt_remains_live_while_submit_actions_are_latched() {
    let (broker, handle, draw_tx, draw_rx, terminal_focused) = setup();
    let mut stream = make_stream(broker, draw_rx, terminal_focused).filtering_initial_input(
        InitialInputConfig {
            start_quiet: true,
            pending_plain_whitespace: "\n".to_string(),
            trailing_action: Some(key_hint::plain(KeyCode::Enter)),
            ..InitialInputConfig::new(InitialInputPolicy::PreserveText)
        },
    );
    let _ = draw_tx.send(());
    assert!(matches!(stream.next().await, Some(TuiEvent::Draw)));

    let interrupt = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
    handle.send(Ok(Event::Key(interrupt)));
    assert!(matches!(stream.next().await, Some(TuiEvent::Key(actual)) if actual == interrupt));
    handle.send(Ok(Event::Key(KeyEvent::new_with_kind(
        KeyCode::Char('c'),
        KeyModifiers::CONTROL,
        KeyEventKind::Repeat,
    ))));
    assert!(
        matches!(stream.next().await, Some(TuiEvent::Key(actual)) if actual.code == KeyCode::Char('c'))
    );

    handle.send(Ok(Event::Key(KeyEvent::new_with_kind(
        KeyCode::Char('c'),
        KeyModifiers::CONTROL,
        KeyEventKind::Release,
    ))));
    handle.send(Ok(Event::Key(KeyEvent::new_with_kind(
        KeyCode::Enter,
        KeyModifiers::NONE,
        KeyEventKind::Release,
    ))));
    assert!(matches!(
        stream.next().await,
        Some(TuiEvent::StartupInputSettled)
    ));
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn visible_edit_preserves_pending_whitespace_then_settles() {
    let (broker, handle, draw_tx, draw_rx, terminal_focused) = setup();
    let mut stream = make_stream(broker, draw_rx, terminal_focused).filtering_initial_input(
        InitialInputConfig {
            start_quiet: true,
            pending_plain_whitespace: "\n".to_string(),
            trailing_action: Some(key_hint::plain(KeyCode::Enter)),
            ..InitialInputConfig::new(InitialInputPolicy::PreserveText)
        },
    );
    let _ = draw_tx.send(());
    assert!(matches!(stream.next().await, Some(TuiEvent::Draw)));

    let visible_edit = KeyEvent::new(KeyCode::Char('d'), KeyModifiers::NONE);
    handle.send(Ok(Event::Key(visible_edit)));
    assert!(
        matches!(stream.next().await, Some(TuiEvent::StartupComposerPaste(text)) if text == "\n")
    );
    assert!(
        matches!(stream.next().await, Some(TuiEvent::StartupComposerKey(actual)) if actual == visible_edit)
    );
    assert!(matches!(stream.next().await, Some(TuiEvent::Draw)));
    handle.send(Ok(Event::Key(KeyEvent::new_with_kind(
        KeyCode::Enter,
        KeyModifiers::NONE,
        KeyEventKind::Release,
    ))));
    assert!(matches!(
        stream.next().await,
        Some(TuiEvent::StartupInputSettled)
    ));
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn trailing_startup_enter_cannot_submit_text_before_its_redraw() {
    let (broker, handle, draw_tx, draw_rx, terminal_focused) = setup();
    let mut stream = make_stream(broker, draw_rx, terminal_focused)
        .with_enhanced_key_events(/*enhanced_key_events*/ false)
        .filtering_initial_input(InitialInputConfig {
            start_quiet: true,
            trailing_action: Some(key_hint::plain(KeyCode::Enter)),
            ..InitialInputConfig::new(InitialInputPolicy::PreserveText)
        })
        .protecting_initial_submission_bindings(vec![key_hint::plain(KeyCode::Enter)]);
    let _ = draw_tx.send(());
    assert!(matches!(stream.next().await, Some(TuiEvent::Draw)));
    tokio::time::advance(STARTUP_INPUT_QUIET_PERIOD).await;
    assert!(matches!(
        stream.next().await,
        Some(TuiEvent::StartupInputSettled)
    ));

    let visible_edit = KeyEvent::new(KeyCode::Char('d'), KeyModifiers::NONE);
    handle.send(Ok(Event::Key(visible_edit)));
    assert!(
        matches!(stream.next().await, Some(TuiEvent::StartupComposerKey(actual)) if actual == visible_edit)
    );

    handle.send(Ok(Event::Key(KeyEvent::new(
        KeyCode::Enter,
        KeyModifiers::NONE,
    ))));
    assert!(matches!(stream.next().await, Some(TuiEvent::Draw)));
    assert!(
        timeout(Duration::from_nanos(1), stream.next())
            .await
            .is_err()
    );

    tokio::time::advance(STARTUP_INPUT_QUIET_PERIOD).await;
    assert!(
        timeout(Duration::from_nanos(1), stream.next())
            .await
            .is_err()
    );
    handle.send(Ok(Event::Key(KeyEvent::new(
        KeyCode::Enter,
        KeyModifiers::NONE,
    ))));
    assert!(matches!(
        stream.next().await,
        Some(TuiEvent::Key(KeyEvent {
            code: KeyCode::Enter,
            ..
        }))
    ));
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn visible_edit_and_redraw_release_a_stale_submit_latch() {
    let (broker, handle, draw_tx, draw_rx, terminal_focused) = setup();
    let mut stream = make_stream(broker, draw_rx, terminal_focused)
        .filtering_initial_input(InitialInputConfig {
            start_quiet: true,
            trailing_action: Some(key_hint::plain(KeyCode::Enter)),
            ..InitialInputConfig::new(InitialInputPolicy::PreserveText)
        })
        .protecting_initial_submission_bindings(vec![key_hint::plain(KeyCode::Enter)]);
    let _ = draw_tx.send(());
    assert!(matches!(stream.next().await, Some(TuiEvent::Draw)));
    tokio::time::advance(STARTUP_INPUT_QUIET_PERIOD).await;
    assert!(matches!(
        stream.next().await,
        Some(TuiEvent::StartupInputSettled)
    ));

    let visible_edit = KeyEvent::new(KeyCode::Char('d'), KeyModifiers::NONE);
    handle.send(Ok(Event::Key(visible_edit)));
    assert!(
        matches!(stream.next().await, Some(TuiEvent::StartupComposerKey(actual)) if actual == visible_edit)
    );
    assert!(matches!(stream.next().await, Some(TuiEvent::Draw)));
    assert!(
        timeout(Duration::from_nanos(1), stream.next())
            .await
            .is_err()
    );

    let submit = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
    handle.send(Ok(Event::Key(submit)));
    assert!(matches!(stream.next().await, Some(TuiEvent::Key(actual)) if actual == submit));
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn printable_screen_shortcut_is_preserved_after_quiet() {
    let (broker, handle, draw_tx, draw_rx, terminal_focused) = setup();
    let mut stream = make_stream(broker, draw_rx, terminal_focused)
        .filtering_initial_input(InitialInputConfig::new(InitialInputPolicy::PreserveText))
        .blocking_initial_actions(vec![crate::tui::startup::StartupBlockedAction {
            binding: key_hint::plain(KeyCode::Char('l')),
            from_raw_probe: false,
            release_observed: false,
            quiet_elapsed: true,
            preserve_after_quiet: true,
        }]);
    let _ = draw_tx.send(());
    assert!(matches!(stream.next().await, Some(TuiEvent::Draw)));

    let first_prompt_key = KeyEvent::new(KeyCode::Char('l'), KeyModifiers::NONE);
    handle.send(Ok(Event::Key(first_prompt_key)));
    assert!(matches!(
        stream.next().await,
        Some(TuiEvent::StartupComposerKey(actual)) if actual == first_prompt_key
    ));
    assert!(matches!(stream.next().await, Some(TuiEvent::Draw)));
    assert!(matches!(
        stream.next().await,
        Some(TuiEvent::StartupInputSettled)
    ));
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn stale_non_submit_actions_do_not_block_startup_settlement() {
    let (broker, handle, draw_tx, draw_rx, terminal_focused) = setup();
    let mut stream = make_stream(broker, draw_rx, terminal_focused)
        .filtering_initial_input(InitialInputConfig::new(InitialInputPolicy::PreserveText))
        .protecting_initial_submission_bindings(vec![key_hint::plain(KeyCode::Enter)])
        .blocking_initial_actions(vec![
            crate::tui::startup::StartupBlockedAction {
                binding: key_hint::plain(KeyCode::Down),
                from_raw_probe: false,
                release_observed: false,
                quiet_elapsed: true,
                preserve_after_quiet: false,
            },
            crate::tui::startup::StartupBlockedAction {
                binding: key_hint::plain(KeyCode::Enter),
                from_raw_probe: false,
                release_observed: false,
                quiet_elapsed: true,
                preserve_after_quiet: false,
            },
        ]);
    let _ = draw_tx.send(());
    assert!(matches!(stream.next().await, Some(TuiEvent::Draw)));
    assert!(matches!(
        stream.next().await,
        Some(TuiEvent::StartupInputSettled)
    ));

    handle.send(Ok(Event::Key(KeyEvent::new(
        KeyCode::Down,
        KeyModifiers::NONE,
    ))));
    assert!(
        timeout(Duration::from_nanos(1), stream.next())
            .await
            .is_err()
    );

    tokio::time::advance(STARTUP_INPUT_QUIET_PERIOD).await;
    assert!(
        timeout(Duration::from_nanos(1), stream.next())
            .await
            .is_err()
    );
    handle.send(Ok(Event::Key(KeyEvent::new(
        KeyCode::Down,
        KeyModifiers::NONE,
    ))));
    assert!(matches!(
        stream.next().await,
        Some(TuiEvent::Key(KeyEvent {
            code: KeyCode::Down,
            ..
        }))
    ));
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn legacy_submit_latch_expires_after_an_idle_quiet_period() {
    let (broker, handle, draw_tx, draw_rx, terminal_focused) = setup();
    let submit = key_hint::ctrl(KeyCode::Char('j'));
    let mut stream = make_stream(broker, draw_rx, terminal_focused)
        .with_enhanced_key_events(/*enhanced_key_events*/ false)
        .filtering_initial_input(InitialInputConfig::new(InitialInputPolicy::PreserveText))
        .protecting_initial_submission_bindings(vec![submit])
        .blocking_initial_actions(vec![crate::tui::startup::StartupBlockedAction {
            binding: submit,
            from_raw_probe: false,
            release_observed: false,
            quiet_elapsed: true,
            preserve_after_quiet: false,
        }]);
    let _ = draw_tx.send(());

    assert!(matches!(stream.next().await, Some(TuiEvent::Draw)));
    assert!(matches!(
        stream.next().await,
        Some(TuiEvent::StartupInputSettled)
    ));
    tokio::time::advance(STARTUP_INPUT_QUIET_PERIOD).await;
    assert!(
        timeout(Duration::from_nanos(1), stream.next())
            .await
            .is_err()
    );

    let submit_event = KeyEvent::new(KeyCode::Char('j'), KeyModifiers::CONTROL);
    handle.send(Ok(Event::Key(submit_event)));
    assert!(matches!(stream.next().await, Some(TuiEvent::Key(actual)) if actual == submit_event));
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn enhanced_submit_repeat_stays_quarantined_until_release() {
    let (broker, handle, draw_tx, draw_rx, terminal_focused) = setup();
    let submit = key_hint::ctrl(KeyCode::Char('j'));
    let mut stream = crate::tui::configure_initial_input(
        make_stream(broker, draw_rx, terminal_focused),
        InitialInputPolicy::PreserveText,
        crate::tui::startup::StartupInputHandoff {
            claimed: true,
            restored_text: true,
            submission_bindings: vec![submit],
            quarantined_actions: vec![crate::tui::startup::StartupBlockedAction::captured(
                submit, /*from_raw_probe*/ false,
            )],
            ..Default::default()
        },
    );
    let _ = draw_tx.send(());
    assert!(matches!(stream.next().await, Some(TuiEvent::Draw)));
    tokio::time::advance(STARTUP_INPUT_QUIET_PERIOD).await;
    assert!(matches!(
        stream.next().await,
        Some(TuiEvent::StartupInputSettled)
    ));
    tokio::time::advance(STARTUP_INPUT_QUIET_PERIOD).await;
    assert!(
        timeout(Duration::from_nanos(1), stream.next())
            .await
            .is_err()
    );

    handle.send(Ok(Event::Key(KeyEvent::new_with_kind(
        KeyCode::Char('j'),
        KeyModifiers::CONTROL,
        KeyEventKind::Repeat,
    ))));
    assert!(
        timeout(Duration::from_nanos(1), stream.next())
            .await
            .is_err()
    );

    handle.send(Ok(Event::Key(KeyEvent::new_with_kind(
        KeyCode::Char('j'),
        KeyModifiers::CONTROL,
        KeyEventKind::Release,
    ))));
    assert!(
        timeout(Duration::from_nanos(1), stream.next())
            .await
            .is_err()
    );
    let submit_press = KeyEvent::new(KeyCode::Char('j'), KeyModifiers::CONTROL);
    handle.send(Ok(Event::Key(submit_press)));
    assert!(matches!(stream.next().await, Some(TuiEvent::Key(actual)) if actual == submit_press));
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn enhanced_printable_repeat_cannot_escape_to_a_later_view() {
    let (broker, handle, draw_tx, draw_rx, terminal_focused) = setup();
    let printable = key_hint::plain(KeyCode::Char('y'));
    let mut stream = crate::tui::configure_initial_input(
        make_stream(broker, draw_rx, terminal_focused),
        InitialInputPolicy::PreserveText,
        crate::tui::startup::StartupInputHandoff {
            claimed: true,
            restored_text: true,
            trailing_printable_action: Some((printable, /*from_raw_probe*/ true)),
            ..Default::default()
        },
    );
    let _ = draw_tx.send(());
    assert!(matches!(stream.next().await, Some(TuiEvent::Draw)));
    tokio::time::advance(STARTUP_INPUT_QUIET_PERIOD).await;
    assert!(matches!(
        stream.next().await,
        Some(TuiEvent::StartupInputSettled)
    ));

    handle.send(Ok(Event::Key(KeyEvent::new_with_kind(
        KeyCode::Char('y'),
        KeyModifiers::NONE,
        KeyEventKind::Repeat,
    ))));
    assert!(
        timeout(Duration::from_nanos(1), stream.next())
            .await
            .is_err()
    );
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn enhanced_press_clears_stale_startup_repeat_provenance() {
    let (broker, handle, draw_tx, draw_rx, terminal_focused) = setup();
    let printable = key_hint::plain(KeyCode::Char('y'));
    let mut stream = crate::tui::configure_initial_input(
        make_stream(broker, draw_rx, terminal_focused)
            .with_enhanced_key_events(/*enhanced_key_events*/ true),
        InitialInputPolicy::PreserveText,
        crate::tui::startup::StartupInputHandoff {
            claimed: true,
            restored_text: true,
            repeat_actions: vec![crate::tui::startup::StartupBlockedAction::captured(
                printable, /*from_raw_probe*/ true,
            )],
            ..Default::default()
        },
    );
    let _ = draw_tx.send(());
    assert!(matches!(stream.next().await, Some(TuiEvent::Draw)));
    tokio::time::advance(STARTUP_INPUT_QUIET_PERIOD).await;
    assert!(matches!(
        stream.next().await,
        Some(TuiEvent::StartupInputSettled)
    ));

    let press = KeyEvent::new(KeyCode::Char('y'), KeyModifiers::NONE);
    handle.send(Ok(Event::Key(press)));
    assert!(matches!(stream.next().await, Some(TuiEvent::Key(actual)) if actual == press));

    let repeat =
        KeyEvent::new_with_kind(KeyCode::Char('y'), KeyModifiers::NONE, KeyEventKind::Repeat);
    handle.send(Ok(Event::Key(repeat)));
    assert!(matches!(stream.next().await, Some(TuiEvent::Key(actual)) if actual == repeat));
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn legacy_startup_submit_ready_at_the_deadline_extends_quarantine() {
    let (broker, handle, draw_tx, draw_rx, terminal_focused) = setup();
    let submit = key_hint::plain(KeyCode::Enter);
    let mut stream = crate::tui::configure_initial_input(
        make_stream(broker, draw_rx, terminal_focused)
            .with_enhanced_key_events(/*enhanced_key_events*/ false),
        InitialInputPolicy::PreserveText,
        crate::tui::startup::StartupInputHandoff {
            claimed: true,
            restored_text: true,
            repeat_actions: vec![crate::tui::startup::StartupBlockedAction::captured(
                submit, /*from_raw_probe*/ true,
            )],
            submission_bindings: vec![submit],
            ..Default::default()
        },
    );
    let _ = draw_tx.send(());
    assert!(matches!(stream.next().await, Some(TuiEvent::Draw)));
    tokio::time::advance(STARTUP_INPUT_QUIET_PERIOD).await;
    let press = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
    handle.send(Ok(Event::Key(press)));
    assert!(
        timeout(Duration::from_nanos(1), stream.next())
            .await
            .is_err()
    );

    tokio::time::advance(STARTUP_INPUT_QUIET_PERIOD).await;
    assert!(matches!(
        stream.next().await,
        Some(TuiEvent::StartupInputSettled)
    ));

    handle.send(Ok(Event::Key(press)));
    assert!(matches!(stream.next().await, Some(TuiEvent::Key(actual)) if actual == press));
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn remapped_enter_is_forwarded_as_literal_composer_text() {
    let (broker, handle, draw_tx, draw_rx, terminal_focused) = setup();
    let mut stream = crate::tui::configure_initial_input(
        make_stream(broker, draw_rx, terminal_focused),
        InitialInputPolicy::PreserveText,
        crate::tui::startup::StartupInputHandoff {
            claimed: true,
            submission_bindings: vec![key_hint::ctrl(KeyCode::Char('j'))],
            ..Default::default()
        },
    );
    let _ = draw_tx.send(());
    assert!(matches!(stream.next().await, Some(TuiEvent::Draw)));
    let enter = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
    handle.send(Ok(Event::Key(enter)));

    assert!(matches!(
        stream.next().await,
        Some(TuiEvent::StartupComposerPaste(text)) if text == "\n"
    ));
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn internal_raw_enter_does_not_suppress_a_literal_enter_across_the_handoff() {
    let (broker, handle, draw_tx, draw_rx, terminal_focused) = setup();
    let mut raw_enter = crate::tui::startup::StartupBlockedAction::captured(
        key_hint::plain(KeyCode::Enter),
        /*from_raw_probe*/ true,
    );
    raw_enter.preserve_after_quiet = true;
    let mut stream = crate::tui::configure_initial_input(
        make_stream(broker, draw_rx, terminal_focused)
            .with_enhanced_key_events(/*enhanced_key_events*/ false),
        InitialInputPolicy::PreserveText,
        crate::tui::startup::StartupInputHandoff {
            claimed: true,
            restored_text: true,
            repeat_actions: vec![raw_enter],
            submission_bindings: vec![key_hint::ctrl(KeyCode::Char('j'))],
            ..Default::default()
        },
    );
    let _ = draw_tx.send(());
    assert!(matches!(stream.next().await, Some(TuiEvent::Draw)));

    let enter = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
    handle.send(Ok(Event::Key(enter)));

    assert!(matches!(
        stream.next().await,
        Some(TuiEvent::StartupComposerPaste(text)) if text == "\n"
    ));
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn remapped_tab_is_forwarded_as_literal_composer_text() {
    let (broker, handle, draw_tx, draw_rx, terminal_focused) = setup();
    let mut stream = crate::tui::configure_initial_input(
        make_stream(broker, draw_rx, terminal_focused),
        InitialInputPolicy::PreserveText,
        crate::tui::startup::StartupInputHandoff {
            claimed: true,
            submission_bindings: vec![key_hint::ctrl(KeyCode::Char('j'))],
            ..Default::default()
        },
    );
    let _ = draw_tx.send(());
    assert!(matches!(stream.next().await, Some(TuiEvent::Draw)));
    handle.send(Ok(Event::Key(KeyEvent::new(
        KeyCode::Tab,
        KeyModifiers::NONE,
    ))));

    assert!(matches!(
        stream.next().await,
        Some(TuiEvent::StartupComposerPaste(text)) if text == "\t"
    ));
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn pending_raw_lf_becomes_internal_text_across_the_stream_handoff() {
    let (broker, handle, draw_tx, draw_rx, terminal_focused) = setup();
    let raw_enter = crate::tui::startup::StartupBlockedAction::captured(
        key_hint::plain(KeyCode::Enter),
        /*from_raw_probe*/ true,
    );
    let mut stream = crate::tui::configure_initial_input(
        make_stream(broker, draw_rx, terminal_focused),
        InitialInputPolicy::PreserveText,
        crate::tui::startup::StartupInputHandoff {
            claimed: true,
            restored_text: true,
            pending_plain_whitespace: "\n".to_string(),
            pending_plain_whitespace_actions: vec![raw_enter],
            submission_bindings: vec![key_hint::ctrl(KeyCode::Char('j'))],
            ..Default::default()
        },
    );

    let _ = draw_tx.send(());
    assert!(matches!(stream.next().await, Some(TuiEvent::Draw)));

    let visible_edit = KeyEvent::new(KeyCode::Char('b'), KeyModifiers::NONE);
    handle.send(Ok(Event::Key(visible_edit)));
    assert!(matches!(
        stream.next().await,
        Some(TuiEvent::StartupComposerPaste(text)) if text == "\n"
    ));
    assert!(matches!(
        stream.next().await,
        Some(TuiEvent::StartupComposerKey(actual)) if actual == visible_edit
    ));
    assert!(matches!(stream.next().await, Some(TuiEvent::Draw)));
    assert!(matches!(
        stream.next().await,
        Some(TuiEvent::StartupInputSettled)
    ));
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn printable_submit_binding_is_not_restored_as_visible_text() {
    let (broker, handle, draw_tx, draw_rx, terminal_focused) = setup();
    let submit = key_hint::plain(KeyCode::Char('x'));
    let mut stream = crate::tui::configure_initial_input(
        make_stream(broker, draw_rx, terminal_focused)
            .with_enhanced_key_events(/*enhanced_key_events*/ false),
        InitialInputPolicy::PreserveText,
        crate::tui::startup::StartupInputHandoff {
            claimed: true,
            submission_bindings: vec![submit],
            quarantined_actions: vec![crate::tui::startup::StartupBlockedAction {
                binding: submit,
                from_raw_probe: false,
                release_observed: false,
                quiet_elapsed: true,
                preserve_after_quiet: true,
            }],
            ..Default::default()
        },
    );
    let _ = draw_tx.send(());

    assert!(matches!(stream.next().await, Some(TuiEvent::Draw)));
    assert!(matches!(
        stream.next().await,
        Some(TuiEvent::StartupInputSettled)
    ));
    tokio::time::advance(STARTUP_INPUT_QUIET_PERIOD).await;
    assert!(
        timeout(Duration::from_nanos(1), stream.next())
            .await
            .is_err()
    );

    let submit_event = KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE);
    handle.send(Ok(Event::Key(submit_event)));
    assert!(matches!(stream.next().await, Some(TuiEvent::Key(actual)) if actual == submit_event));
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn live_printable_submit_binding_is_quarantined_before_settlement() {
    let (broker, handle, draw_tx, draw_rx, terminal_focused) = setup();
    let submit = key_hint::plain(KeyCode::Char('x'));
    let mut stream = crate::tui::configure_initial_input(
        make_stream(broker, draw_rx, terminal_focused)
            .with_enhanced_key_events(/*enhanced_key_events*/ false),
        InitialInputPolicy::PreserveText,
        crate::tui::startup::StartupInputHandoff {
            claimed: true,
            submission_bindings: vec![submit],
            ..Default::default()
        },
    );
    let _ = draw_tx.send(());
    assert!(matches!(stream.next().await, Some(TuiEvent::Draw)));

    let submit_event = KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE);
    handle.send(Ok(Event::Key(submit_event)));
    assert!(
        timeout(Duration::from_nanos(1), stream.next())
            .await
            .is_err()
    );

    tokio::time::advance(STARTUP_INPUT_QUIET_PERIOD).await;
    assert!(matches!(
        stream.next().await,
        Some(TuiEvent::StartupInputSettled)
    ));
    tokio::time::advance(STARTUP_INPUT_QUIET_PERIOD).await;
    assert!(
        timeout(Duration::from_nanos(1), stream.next())
            .await
            .is_err()
    );
    handle.send(Ok(Event::Key(submit_event)));
    assert!(matches!(stream.next().await, Some(TuiEvent::Key(actual)) if actual == submit_event));
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn raw_printable_submit_binding_stays_quarantined_at_the_stream_handoff() {
    let (broker, handle, draw_tx, draw_rx, terminal_focused) = setup();
    let submit = key_hint::plain(KeyCode::Char('x'));
    let mut stream = crate::tui::configure_initial_input(
        make_stream(broker, draw_rx, terminal_focused)
            .with_enhanced_key_events(/*enhanced_key_events*/ false),
        InitialInputPolicy::PreserveText,
        crate::tui::startup::StartupInputHandoff {
            claimed: true,
            restored_text: true,
            trailing_printable_action: Some((submit, true)),
            submission_bindings: vec![submit],
            ..Default::default()
        },
    );
    let _ = draw_tx.send(());

    assert!(matches!(stream.next().await, Some(TuiEvent::Draw)));
    tokio::time::advance(STARTUP_INPUT_QUIET_PERIOD).await;
    assert!(matches!(
        stream.next().await,
        Some(TuiEvent::StartupInputSettled)
    ));
    tokio::time::advance(STARTUP_INPUT_QUIET_PERIOD).await;
    assert!(
        timeout(Duration::from_nanos(1), stream.next())
            .await
            .is_err()
    );

    let submit_event = KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE);
    handle.send(Ok(Event::Key(submit_event)));
    assert!(matches!(stream.next().await, Some(TuiEvent::Key(actual)) if actual == submit_event));
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn pre_draw_input_is_drained_before_the_first_draw() {
    let (broker, handle, draw_tx, draw_rx, terminal_focused) = setup();
    let mut stream = make_stream(broker, draw_rx, terminal_focused)
        .filtering_initial_input(InitialInputConfig::new(InitialInputPolicy::PreserveText))
        .protecting_initial_submission_bindings(vec![key_hint::plain(KeyCode::Enter)]);
    let _ = draw_tx.send(());
    let visible_edit = KeyEvent::new(KeyCode::Char('d'), KeyModifiers::NONE);
    handle.send(Ok(Event::Key(visible_edit)));
    handle.send(Ok(Event::Key(KeyEvent::new(
        KeyCode::Enter,
        KeyModifiers::NONE,
    ))));

    assert!(
        matches!(stream.next().await, Some(TuiEvent::StartupComposerKey(actual)) if actual == visible_edit)
    );
    assert!(matches!(stream.next().await, Some(TuiEvent::Draw)));
    assert!(
        timeout(Duration::from_nanos(1), stream.next())
            .await
            .is_err()
    );
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn raw_enter_latch_matches_ctrl_j_after_protocol_activation() {
    let (broker, handle, draw_tx, draw_rx, terminal_focused) = setup();
    let mut stream = make_stream(broker, draw_rx, terminal_focused).filtering_initial_input(
        InitialInputConfig {
            start_quiet: true,
            pending_plain_whitespace: "\n".to_string(),
            trailing_action: Some(key_hint::plain(KeyCode::Enter)),
            trailing_action_from_raw_probe: true,
            ..InitialInputConfig::new(InitialInputPolicy::PreserveText)
        },
    );
    let _ = draw_tx.send(());
    assert!(matches!(stream.next().await, Some(TuiEvent::Draw)));

    handle.send(Ok(Event::Key(KeyEvent::new_with_kind(
        KeyCode::Char('j'),
        KeyModifiers::CONTROL,
        KeyEventKind::Repeat,
    ))));
    assert!(
        timeout(Duration::from_nanos(1), stream.next())
            .await
            .is_err()
    );
    handle.send(Ok(Event::Key(KeyEvent::new_with_kind(
        KeyCode::Char('j'),
        KeyModifiers::CONTROL,
        KeyEventKind::Release,
    ))));
    assert!(matches!(
        stream.next().await,
        Some(TuiEvent::StartupInputSettled)
    ));
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn enhanced_press_releases_a_quiet_raw_submit_latch() {
    let (broker, handle, draw_tx, draw_rx, terminal_focused) = setup();
    let submit = key_hint::KeyBinding::new(KeyCode::Char('j'), KeyModifiers::CONTROL);
    let mut stream = make_stream(broker, draw_rx, terminal_focused)
        .with_enhanced_key_events(/*enhanced_key_events*/ true)
        .filtering_initial_input(InitialInputConfig::new(InitialInputPolicy::PreserveText))
        .blocking_initial_actions(vec![StartupBlockedAction::captured(
            key_hint::plain(KeyCode::Enter),
            /*from_raw_probe*/ true,
        )])
        .protecting_initial_submission_bindings(vec![submit]);
    let _ = draw_tx.send(());

    assert!(matches!(stream.next().await, Some(TuiEvent::Draw)));
    tokio::time::advance(STARTUP_INPUT_QUIET_PERIOD).await;
    assert!(matches!(
        stream.next().await,
        Some(TuiEvent::StartupInputSettled)
    ));
    tokio::time::advance(STARTUP_INPUT_QUIET_PERIOD).await;
    assert!(
        timeout(Duration::from_nanos(1), stream.next())
            .await
            .is_err()
    );

    let press = KeyEvent::new_with_kind(
        KeyCode::Char('j'),
        KeyModifiers::CONTROL,
        KeyEventKind::Press,
    );
    handle.send(Ok(Event::Key(press)));
    assert!(matches!(stream.next().await, Some(TuiEvent::Key(actual)) if actual == press));
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn backspace_in_the_post_draw_gap_is_forwarded_before_settlement() {
    let (broker, handle, draw_tx, draw_rx, terminal_focused) = setup();
    let mut stream = make_stream(broker, draw_rx, terminal_focused)
        .filtering_initial_input(InitialInputConfig::new(InitialInputPolicy::PreserveText));
    let _ = draw_tx.send(());
    assert!(matches!(stream.next().await, Some(TuiEvent::Draw)));

    let backspace = KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE);
    handle.send(Ok(Event::Key(backspace)));
    assert!(
        matches!(stream.next().await, Some(TuiEvent::StartupComposerKey(actual)) if actual == backspace)
    );
    assert!(matches!(stream.next().await, Some(TuiEvent::Draw)));
    assert!(matches!(
        stream.next().await,
        Some(TuiEvent::StartupInputSettled)
    ));
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn paste_in_the_post_draw_gap_is_forwarded_before_settlement() {
    let (broker, handle, draw_tx, draw_rx, terminal_focused) = setup();
    let mut stream = make_stream(broker, draw_rx, terminal_focused)
        .filtering_initial_input(InitialInputConfig::new(InitialInputPolicy::PreserveText));
    let _ = draw_tx.send(());
    assert!(matches!(stream.next().await, Some(TuiEvent::Draw)));

    handle.send(Ok(Event::Paste("a\nb".to_string())));
    assert!(
        matches!(stream.next().await, Some(TuiEvent::StartupComposerPaste(text)) if text == "a\nb")
    );
    assert!(matches!(stream.next().await, Some(TuiEvent::Draw)));
    assert!(matches!(
        stream.next().await,
        Some(TuiEvent::StartupInputSettled)
    ));
}

#[cfg(windows)]
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn altgr_text_is_forwarded_before_startup_input_settlement() {
    let (broker, handle, draw_tx, draw_rx, terminal_focused) = setup();
    let mut stream = make_stream(broker, draw_rx, terminal_focused)
        .filtering_initial_input(InitialInputConfig::new(InitialInputPolicy::PreserveText));
    let _ = draw_tx.send(());
    assert!(matches!(stream.next().await, Some(TuiEvent::Draw)));

    let text = KeyEvent::new(
        KeyCode::Char('@'),
        KeyModifiers::CONTROL | KeyModifiers::ALT,
    );
    handle.send(Ok(Event::Key(text)));
    assert!(matches!(stream.next().await, Some(TuiEvent::Key(actual)) if actual == text));
    assert!(matches!(
        stream.next().await,
        Some(TuiEvent::StartupInputSettled)
    ));
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn startup_handoff_forwards_interrupt_only_once() {
    let (broker, handle, _draw_tx, draw_rx, terminal_focused) = setup();
    let mut stream = make_stream(broker, draw_rx, terminal_focused).filtering_initial_input(
        InitialInputConfig {
            start_quiet: true,
            pending_interrupt: true,
            trailing_action: Some(key_hint::ctrl(KeyCode::Char('c'))),
            ..InitialInputConfig::new(InitialInputPolicy::DiscardAll)
        },
    );

    let interrupt = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
    assert!(matches!(stream.next().await, Some(TuiEvent::Key(actual)) if actual == interrupt));
    handle.send(Ok(Event::Key(interrupt)));
    assert!(
        timeout(Duration::from_nanos(1), stream.next())
            .await
            .is_err()
    );
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn released_startup_interrupt_does_not_block_a_new_interrupt() {
    let (broker, handle, _draw_tx, draw_rx, terminal_focused) = setup();
    let mut stream = make_stream(broker, draw_rx, terminal_focused).filtering_initial_input(
        InitialInputConfig {
            start_quiet: true,
            pending_interrupt: true,
            ..InitialInputConfig::new(InitialInputPolicy::DiscardAll)
        },
    );

    let interrupt = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
    assert!(matches!(stream.next().await, Some(TuiEvent::Key(actual)) if actual == interrupt));
    handle.send(Ok(Event::Key(interrupt)));
    assert!(matches!(stream.next().await, Some(TuiEvent::Key(actual)) if actual == interrupt));
}

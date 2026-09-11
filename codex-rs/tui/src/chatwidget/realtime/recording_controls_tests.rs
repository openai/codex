//! Rendering and key routing regressions for live recording controls.

use super::*;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;
use crossterm::event::KeyModifiers;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn voice_mute_shortcut_only_handles_active_current_thread_presses() {
    let (mut chat, _sender, mut events, mut ops) = make_chatwidget_manual_with_sender().await;
    let thread_id = activate_voice(&mut chat);
    let shortcut = KeyEvent::new(KeyCode::Char('x'), KeyModifiers::CONTROL);

    assert!(!chat.handle_realtime_microphone_shortcut(KeyEvent::new(
        KeyCode::Char('m'),
        KeyModifiers::CONTROL,
    )));
    for kind in [KeyEventKind::Repeat, KeyEventKind::Release] {
        assert!(!chat.handle_realtime_microphone_shortcut(KeyEvent { kind, ..shortcut }));
    }
    for phase in [
        RealtimeConversationPhase::Inactive,
        RealtimeConversationPhase::Starting,
        RealtimeConversationPhase::Stopping,
    ] {
        chat.realtime_conversation.phase = phase;
        assert!(!chat.handle_realtime_microphone_shortcut(shortcut));
    }
    chat.realtime_conversation.phase = RealtimeConversationPhase::Active;
    chat.thread_id = Some(ThreadId::new());
    assert!(!chat.handle_realtime_microphone_shortcut(shortcut));
    chat.thread_id = Some(thread_id);
    assert!(events.try_recv().is_err());

    assert!(chat.handle_realtime_microphone_shortcut(shortcut));

    let Ok(AppEvent::InsertHistoryCell(cell)) = events.try_recv() else {
        panic!("a missing microphone handle should fail closed with the existing voice error");
    };
    assert!(
        cell.display_lines(/*width*/ 80)
            .iter()
            .any(|line| line.to_string().contains("Start voice mode before muting"))
    );
    assert!(!chat.realtime_conversation.microphone_muted);
    assert!(ops.try_recv().is_err());
}

#[tokio::test]
async fn voice_mute_shortcut_accepts_raw_terminal_control_bytes() {
    let (mut chat, _sender, mut events, mut ops) = make_chatwidget_manual_with_sender().await;
    activate_voice(&mut chat);

    assert!(chat.handle_realtime_microphone_shortcut(KeyEvent::new(
        KeyCode::Char('\u{18}'),
        KeyModifiers::NONE,
    )));
    assert!(events.try_recv().is_ok());
    assert!(!chat.realtime_conversation.microphone_muted);
    assert!(ops.try_recv().is_err());
}

#[tokio::test]
async fn voice_mute_keymap_updates_the_active_handler_and_composer_hint() {
    let (mut chat, _sender, mut events, _ops) = make_chatwidget_manual_with_sender().await;
    activate_voice(&mut chat);
    let custom = KeyEvent::new(KeyCode::F(8), KeyModifiers::NONE);
    for (binding, hint, handles_key) in [("'f8'", "f8 mute", true), ("[]", "/voice mute", false)] {
        let config = toml::from_str::<codex_config::types::TuiKeymap>(&format!(
            "[chat]\ntoggle_voice_mute = {binding}"
        ))
        .unwrap();
        let runtime = crate::keymap::RuntimeKeymap::from_config(&config).unwrap();
        chat.apply_keymap_update(config, &runtime);
        assert!(render_bottom_popup(&chat, /*width*/ 80).contains(hint));
        assert!(!chat.handle_realtime_microphone_shortcut(KeyEvent::new(
            KeyCode::Char('x'),
            KeyModifiers::CONTROL
        )));
        assert_eq!(
            chat.handle_realtime_microphone_shortcut(custom),
            handles_key
        );
        assert_eq!(events.try_recv().is_ok(), handles_key);
        assert!(!chat.realtime_conversation.microphone_muted);
    }
}

#[tokio::test]
async fn voice_composer_preserves_normal_colors_across_microphone_states() {
    use crate::render::renderable::Renderable;
    use ratatui::prelude::Buffer;
    use ratatui::prelude::Color;
    use ratatui::prelude::Rect;

    let (mut chat, _sender, _events, _ops) = make_chatwidget_manual_with_sender().await;
    let thread_id = activate_voice(&mut chat);
    chat.bottom_pane
        .set_composer_text("typed".to_string(), Vec::new(), Vec::new());
    let check = |chat: &mut ChatWidget, recording| {
        chat.update_realtime_footer();
        let area = Rect::new(
            /*x*/ 0,
            /*y*/ 0,
            /*width*/ 47,
            chat.bottom_pane.desired_height(/*width*/ 47),
        );
        let mut buffer = Buffer::empty(area);
        chat.bottom_pane.render(area, &mut buffer);
        assert_eq!(chat.realtime_microphone_is_listening(), recording);
        assert!(buffer.content.iter().all(|cell| cell.bg != Color::Red));
        if recording {
            let marker = buffer
                .content
                .iter()
                .find(|cell| cell.symbol() == "●")
                .expect("actual capture must show a recording marker");
            assert_eq!(marker.fg, Color::Red);
            assert_ne!(marker.bg, Color::Red);
        }
        if recording && chat.local_settings.tui.animations {
            let rows = buffer
                .content
                .chunks(/*chunk_size*/ 47)
                .take(/*n*/ 5)
                .enumerate()
                .map(|(index, cells)| {
                    let row = cells
                        .iter()
                        .map(ratatui::buffer::Cell::symbol)
                        .collect::<String>();
                    format!("{index}: {}", row.trim_end())
                        .trim_end()
                        .to_string()
                })
                .collect::<Vec<_>>()
                .join("\n");
            insta::assert_snapshot!(rows, @r"
            0:
            1:  voice ● listening ctrl+x mute     /voice stop
            2:    mic ▁▁▁▁▁▁  codex ▁▁▁▁▁▁
            3:
            4: › typed
            ");
        }
        buffer
            .content
            .windows(/*size*/ 5)
            .find(|cells| {
                cells
                    .iter()
                    .map(ratatui::buffer::Cell::symbol)
                    .collect::<String>()
                    == "typed"
            })
            .map(|cells| cells[0].fg)
            .unwrap()
    };
    let foreground = check(&mut chat, /*recording*/ true);
    chat.local_settings.tui.animations = false;
    assert_eq!(check(&mut chat, /*recording*/ true), foreground);
    chat.realtime_conversation.microphone_muted = true;
    assert_eq!(check(&mut chat, /*recording*/ false), foreground);
    chat.realtime_conversation.microphone_muted = false;
    chat.thread_id = Some(ThreadId::new());
    assert_eq!(check(&mut chat, /*recording*/ false), foreground);
    chat.thread_id = Some(thread_id);
    chat.realtime_conversation.phase = RealtimeConversationPhase::Stopping;
    assert_eq!(check(&mut chat, /*recording*/ false), foreground);
}

#[tokio::test]
async fn voice_preserves_the_normal_composer_prompt() {
    let (mut chat, _sender, _events, _ops) = make_chatwidget_manual_with_sender().await;
    let thread_id = activate_voice(&mut chat);
    let prompt = |chat: &mut ChatWidget| {
        chat.update_realtime_footer();
        render_bottom_popup(chat, /*width*/ 80)
            .lines()
            .filter_map(|line| line.chars().next())
            .find(|glyph| matches!(glyph, '›' | '!'))
    };
    for level in 0..=5 {
        chat.realtime_conversation.microphone_level = level;
        assert_eq!(prompt(&mut chat), Some('›'));
    }
    for (muted, animations) in [(true, true), (false, false), (false, true)] {
        chat.realtime_conversation.microphone_muted = muted;
        chat.local_settings.tui.animations = animations;
        assert_eq!(prompt(&mut chat), Some('›'));
    }
    chat.thread_id = Some(ThreadId::new());
    assert_eq!(prompt(&mut chat), Some('›'));
    chat.thread_id = Some(thread_id);
    chat.bottom_pane
        .set_composer_text("!".to_string(), Vec::new(), Vec::new());
    assert_eq!(prompt(&mut chat), Some('!'));
    chat.bottom_pane
        .handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert_eq!(prompt(&mut chat), Some('›'));
    chat.reset_realtime_conversation();
    assert_eq!(
        render_bottom_popup(&chat, /*width*/ 80).chars().next(),
        Some('›')
    );
}

#[tokio::test]
async fn voice_meters_do_not_sample_again_on_early_redraws() {
    let (mut chat, _sender, _events, _ops) = make_chatwidget_manual_with_sender().await;
    activate_voice(&mut chat);
    let now = std::time::Instant::now();
    let peaks = std::cell::Cell::new((0, 0));
    for elapsed_ms in [0, 100, 200, 300, 400, 500] {
        let sample_at = now + std::time::Duration::from_millis(elapsed_ms);
        peaks.set((8192, 4096));
        chat.refresh_realtime_audio_meters(sample_at, || peaks.replace((0, 0)));
        // Footer updates and transcript animation can redraw before more audio arrives.
        let (frame_requester, mut frame_requests) = crate::tui::FrameRequester::test_channel();
        chat.frame_requester = frame_requester;
        let before_redraw = std::time::Instant::now();
        chat.refresh_realtime_audio_meters(sample_at + std::time::Duration::from_millis(8), || {
            peaks.replace((0, 0))
        });
        let after_redraw = std::time::Instant::now();
        let remaining = std::time::Duration::from_millis(92);
        let deadline = frame_requests
            .try_recv()
            .expect("early redraw re-arms sampling");
        assert!((before_redraw + remaining..=after_redraw + remaining).contains(&deadline));
        assert!(frame_requests.try_recv().is_err());
    }
    assert_eq!(
        chat.realtime_conversation.audio_meter_history,
        std::collections::VecDeque::from([(255, 119); 6])
    );
    let render_meter = |chat: &ChatWidget, width| {
        render_bottom_popup(chat, width)
            .lines()
            .find(|line| line.contains("mic "))
            .unwrap()
            .trim()
            .to_string()
    };
    let mut meters = [80, 30].map(|width| render_meter(&chat, width)).to_vec();

    // An early redraw must also leave newly accumulated peaks for the next sample.
    peaks.set((4096, 8192));
    chat.refresh_realtime_audio_meters(now + std::time::Duration::from_millis(599), || {
        peaks.replace((0, 0))
    });
    chat.refresh_realtime_audio_meters(now + std::time::Duration::from_millis(600), || {
        peaks.replace((0, 0))
    });
    assert_eq!(
        chat.realtime_conversation.audio_meter_history,
        std::collections::VecDeque::from([
            (255, 119),
            (255, 119),
            (255, 119),
            (255, 119),
            (255, 119),
            (255, 119),
            (119, 255)
        ])
    );
    // Quiet samples scroll into each channel without erasing earlier speech.
    for (elapsed_ms, peaks) in [(700, (0, 8192)), (800, (8192, 0))] {
        chat.refresh_realtime_audio_meters(
            now + std::time::Duration::from_millis(elapsed_ms),
            || peaks,
        );
        meters.push(render_meter(&chat, /*width*/ 80));
    }
    for frame in 0..super::super::MAX_REALTIME_AUDIO_METER_FRAMES {
        chat.refresh_realtime_audio_meters(
            now + std::time::Duration::from_millis(900 + frame as u64 * 100),
            || (0, 0),
        );
        if frame == 0 {
            meters.push(render_meter(&chat, /*width*/ 80));
        }
    }
    assert_eq!(
        chat.realtime_conversation.audio_meter_history,
        std::collections::VecDeque::from([(0, 0); super::super::MAX_REALTIME_AUDIO_METER_FRAMES])
    );
    meters.push(render_meter(&chat, /*width*/ 80));
    insta::assert_snapshot!(meters.join("\n"));
}

#[tokio::test]
async fn voice_meters_preserve_silence_and_restart_sampling_after_reset() {
    let (mut chat, _sender, _events, _ops) = make_chatwidget_manual_with_sender().await;
    activate_voice(&mut chat);
    let now = std::time::Instant::now();
    chat.refresh_realtime_audio_meters(now, || (8192, 4096));
    // A delayed draw adds one real sample, not synthetic catch-up bars.
    chat.refresh_realtime_audio_meters(now + std::time::Duration::from_secs(1), || (0, 0));
    assert_eq!(
        chat.realtime_conversation.audio_meter_history,
        std::collections::VecDeque::from([(255, 119), (0, 0)])
    );
    chat.reset_realtime_conversation();
    activate_voice(&mut chat);
    chat.refresh_realtime_audio_meters(now + std::time::Duration::from_millis(1001), || {
        (4096, 8192)
    });
    assert_eq!(
        chat.realtime_conversation.audio_meter_history,
        std::collections::VecDeque::from([(119, 255)])
    );
}

#[tokio::test]
async fn voice_footer_renders_the_main_conversation_states() {
    let (mut chat, _sender, _events, _ops) = make_chatwidget_manual_with_sender().await;
    activate_voice(&mut chat);
    chat.thread_name = Some("status line stays visible".to_string());
    chat.local_settings.tui.status_line = Some(vec!["thread-title".to_string()]);
    chat.refresh_status_surfaces();
    let mut states = Vec::new();

    for (label, phase, muted, level, speaker_level, role, transcript) in [
        (
            "connecting",
            RealtimeConversationPhase::Starting,
            false,
            0,
            0,
            None,
            "",
        ),
        (
            "listening",
            RealtimeConversationPhase::Active,
            false,
            0,
            0,
            None,
            "",
        ),
        (
            "speaking",
            RealtimeConversationPhase::Active,
            false,
            4,
            5,
            None,
            "",
        ),
        (
            "muted",
            RealtimeConversationPhase::Active,
            true,
            4,
            4,
            None,
            "",
        ),
        (
            "transcript",
            RealtimeConversationPhase::Active,
            false,
            2,
            2,
            Some("assistant"),
            "Hello there",
        ),
    ] {
        chat.local_settings.tui.animations = label != "connecting";
        chat.realtime_conversation.phase = phase;
        chat.realtime_conversation.microphone_muted = muted;
        chat.realtime_conversation.microphone_level = level;
        chat.realtime_conversation.speaker_level = speaker_level;
        chat.realtime_conversation.microphone_intensity = (level * 255 / 5) as u8;
        chat.realtime_conversation.speaker_intensity = (speaker_level * 255 / 5) as u8;
        let intensities = (
            chat.realtime_conversation.microphone_intensity,
            chat.realtime_conversation.speaker_intensity,
        );
        chat.realtime_conversation.audio_meter_history = [intensities; 4].into();
        chat.realtime_conversation.microphone_history =
            super::super::VoiceAmplitudeHistory::default();
        chat.realtime_conversation.speaker_history = super::super::VoiceAmplitudeHistory::default();
        for _ in 0..4 {
            chat.realtime_conversation.microphone_history.push(level);
            chat.realtime_conversation
                .speaker_history
                .push(speaker_level);
        }
        chat.realtime_conversation.transcript_role = role.map(str::to_string);
        chat.realtime_conversation.transcript = transcript.to_string();
        chat.update_realtime_footer();
        let rendered = render_bottom_popup(&chat, /*width*/ 80);
        assert!(rendered.contains("status line stays visible"));
        states.push(format!("{label}:\n{rendered}"));
    }

    chat.realtime_conversation.speaker_level = 0;
    chat.realtime_conversation.speaker_intensity = 0;
    chat.realtime_conversation.speaker_history = super::super::VoiceAmplitudeHistory::default();
    chat.realtime_conversation.interruption_acknowledged_until =
        Some(std::time::Instant::now() + super::super::INTERRUPTION_ACKNOWLEDGMENT);
    chat.update_realtime_footer();
    states.push(format!(
        "interrupted:\n{}",
        render_bottom_popup(&chat, /*width*/ 80)
    ));

    insta::assert_snapshot!(states.join("\n\n"));
}

#[tokio::test]
async fn narrow_voice_footer_keeps_the_stop_control_before_meters() {
    let (mut chat, _sender, _events, _ops) = make_chatwidget_manual_with_sender().await;
    chat.local_settings.tui.animations = true;
    activate_voice(&mut chat);
    chat.realtime_conversation.speaker_active_until =
        Some(std::time::Instant::now() + super::super::SPEAKER_ACTIVITY_HOLD);
    chat.update_realtime_footer();

    let footer = render_bottom_popup(&chat, /*width*/ 46);
    assert!(footer.contains("voice ● speaking"));
    assert!(footer.contains("ctrl+x mute"));
    assert!(footer.contains("/voice stop"));
    chat.realtime_conversation.speaker_active_until = None;
    chat.realtime_conversation.speaker_level = 1;
    chat.realtime_conversation.speaker_intensity = 51;
    chat.realtime_conversation
        .audio_meter_history
        .push_back((99, 51));
    chat.on_realtime_transcript_delta("user".to_string(), "stop".to_string());
    assert_eq!(chat.realtime_conversation.speaker_level, 0);
    assert_eq!(chat.realtime_conversation.speaker_intensity, 0);
    assert_eq!(
        chat.realtime_conversation.audio_meter_history.back(),
        Some(&(99, 0))
    );
    assert_eq!(chat.realtime_conversation.speaker_active_until, None);
    let interrupted = render_bottom_popup(&chat, /*width*/ 45);
    assert!(interrupted.contains("voice ● heard"));
    assert!(interrupted.contains("ctrl+x mute"));
    assert!(interrupted.contains("/voice stop"));
    chat.realtime_conversation.interruption_acknowledged_until =
        Some(std::time::Instant::now() - super::super::INTERRUPTION_ACKNOWLEDGMENT);
    chat.realtime_conversation.speaker_active_until =
        Some(std::time::Instant::now() - super::super::SPEAKER_ACTIVITY_HOLD);
    chat.update_realtime_footer();
    assert!(render_bottom_popup(&chat, /*width*/ 47).contains("voice ● listening"));
    for (peak, expected) in [
        (0, 0),
        (1, 0),
        (511, 0),
        (512, 0),
        (513, 1),
        (4096, 3),
        (6144, 4),
        (6656, 4),
        (6657, 5),
        (8192, 5),
        (32768, 5),
        (u16::MAX, 5),
    ] {
        assert_eq!(
            super::super::recording_controls::audio_meter_level(peak),
            expected
        );
    }
    for (peak, expected) in [(0, 0), (512, 0), (8192, 255), (u16::MAX, 255)] {
        assert_eq!(
            super::super::recording_controls::audio_meter_intensity(peak),
            expected
        );
    }
    assert_eq!(
        super::super::recording_controls::audio_meter_level(/*peak*/ 542),
        super::super::recording_controls::audio_meter_level(/*peak*/ 543)
    );
    assert!(
        super::super::recording_controls::audio_meter_intensity(/*peak*/ 542)
            < super::super::recording_controls::audio_meter_intensity(/*peak*/ 543)
    );
}

#[tokio::test]
async fn voice_acknowledges_only_a_real_interruption_once() {
    let (mut chat, _sender, _events, _ops) = make_chatwidget_manual_with_sender().await;
    chat.local_settings.tui.animations = true;
    activate_voice(&mut chat);

    chat.on_realtime_transcript_delta("user".to_string(), "hello".to_string());
    assert!(
        chat.realtime_conversation
            .interruption_acknowledged_until
            .is_none()
    );
    chat.on_realtime_transcript_done("user".to_string(), "hello".to_string());
    chat.on_realtime_transcript_delta("assistant".to_string(), "speaking".to_string());
    chat.realtime_conversation.speaker_level = 3;
    chat.on_realtime_transcript_delta("user".to_string(), "wait".to_string());
    let acknowledged = chat.realtime_conversation.interruption_acknowledged_until;
    assert!(acknowledged.is_some());
    chat.on_realtime_transcript_delta("user".to_string(), " please".to_string());
    assert_eq!(
        chat.realtime_conversation.interruption_acknowledged_until,
        acknowledged
    );
    chat.on_realtime_transcript_done("user".to_string(), "wait please".to_string());
    chat.on_realtime_transcript_done("assistant".to_string(), "speaking".to_string());
    chat.on_realtime_transcript_delta("assistant".to_string(), "sure".to_string());
    assert!(
        chat.realtime_conversation
            .interruption_acknowledged_until
            .is_none()
    );

    chat.local_settings.tui.animations = false;
    chat.realtime_conversation.speaker_level = 3;
    chat.on_realtime_transcript_delta("user".to_string(), "stop".to_string());
    assert!(
        chat.realtime_conversation
            .interruption_acknowledged_until
            .is_none()
    );
}

#[tokio::test]
async fn voice_terminal_title_tracks_capture_activity_and_user_settings() {
    let (mut chat, _sender, _events, _ops) = make_chatwidget_manual_with_sender().await;
    let thread_id = activate_voice(&mut chat);
    chat.local_settings.tui.animations = false;
    let check = |chat: &mut ChatWidget, expected: &str| {
        chat.refresh_terminal_title();
        assert_eq!(chat.last_terminal_title.as_deref(), Some(expected));
    };
    check(&mut chat, "● project");
    chat.bottom_pane.set_task_running(/*running*/ true);
    chat.local_settings.tui.animations = true;
    chat.refresh_terminal_title();
    assert!(chat.last_terminal_title.as_deref().is_some_and(|title| {
        title.starts_with("● ") && title.ends_with(" project") && title != "● project"
    }));
    chat.local_settings.tui.animations = false;
    check(&mut chat, "● project");
    chat.local_settings.tui.terminal_title = Some(vec!["project-name".to_string()]);
    check(&mut chat, "project");
    chat.local_settings.tui.terminal_title = None;
    use RealtimeConversationPhase as Phase;
    for phase in [Phase::Starting, Phase::Stopping, Phase::Inactive] {
        chat.realtime_conversation.phase = phase;
        check(&mut chat, "project");
    }
    chat.realtime_conversation.phase = RealtimeConversationPhase::Active;
    chat.realtime_conversation.microphone_muted = true;
    check(&mut chat, "project");
    chat.realtime_conversation.microphone_muted = false;
    chat.thread_id = Some(ThreadId::new());
    check(&mut chat, "project");
    chat.thread_id = Some(thread_id);
    check(&mut chat, "● project");
    chat.reset_realtime_conversation();
    assert_eq!(chat.last_terminal_title.as_deref(), Some("project"));
    activate_voice(&mut chat);
    check(&mut chat, "● project");
    chat.clear_managed_terminal_title().expect("title clears");
    chat.reset_realtime_conversation();
    assert!(chat.last_terminal_title.is_none());
}

#[tokio::test]
async fn stopping_voice_clears_recording_title_without_restoring_cleared_titles() {
    for (thread_title, clear_title) in [(true, false), (false, false), (true, true)] {
        let (mut chat, _sender, _events, _ops) = make_chatwidget_manual_with_sender().await;
        chat.local_settings.tui.animations = false;
        chat.thread_name = Some("voice session".to_string());
        let mut items = vec!["activity".to_string()];
        if thread_title {
            items.push("thread-title".to_string());
        }
        chat.local_settings.tui.terminal_title = Some(items);
        activate_voice(&mut chat);
        chat.refresh_terminal_title();
        let recording = if thread_title {
            "● voice session"
        } else {
            "●"
        };
        assert_eq!(chat.last_terminal_title.as_deref(), Some(recording));
        if clear_title {
            chat.clear_managed_terminal_title().expect("title clears");
        }
        chat.realtime_conversation.phase = RealtimeConversationPhase::Stopping;
        chat.on_realtime_conversation_closed(/*reason*/ Some("requested".into()));
        assert_eq!(
            chat.last_terminal_title.as_deref(),
            (thread_title && !clear_title).then_some("voice session")
        );
    }
}

#[tokio::test]
async fn startup_timeout_clears_recording_title_before_backend_closes() {
    let (mut chat, _sender, _events, mut ops) = make_chatwidget_manual_with_sender().await;
    activate_voice(&mut chat);
    chat.local_settings.tui.animations = false;
    chat.refresh_terminal_title();
    assert_eq!(chat.last_terminal_title.as_deref(), Some("● project"));

    chat.realtime_conversation.phase = RealtimeConversationPhase::Starting;
    chat.on_realtime_webrtc_connected(
        /*attempt_id*/ chat.realtime_conversation.attempt_id,
        Err(codex_realtime_webrtc::ConnectionError::NegotiationTimedOut),
    );

    assert!(matches!(
        ops.try_recv(),
        Ok(AppCommand::RealtimeConversationStop { .. })
    ));
    assert_eq!(
        chat.realtime_conversation.phase,
        RealtimeConversationPhase::Stopping
    );
    assert_eq!(chat.last_terminal_title.as_deref(), Some("project"));
}

#[tokio::test]
async fn clipped_voice_composer_keeps_the_draft_and_cursor_visible() {
    use crate::render::renderable::Renderable;
    use ratatui::prelude::Buffer;
    use ratatui::prelude::Rect;

    let (mut chat, _sender, _events, _ops) = make_chatwidget_manual_with_sender().await;
    chat.local_settings.tui.animations = false;
    activate_voice(&mut chat);
    chat.bottom_pane
        .set_composer_text("typed".to_string(), Vec::new(), Vec::new());
    chat.update_realtime_footer();

    let mut layouts = Vec::new();
    for height in [5, 6, 8] {
        let area = Rect::new(/*x*/ 0, /*y*/ 0, /*width*/ 47, height);
        let mut buffer = Buffer::empty(area);
        chat.bottom_pane.render(area, &mut buffer);
        let rows = buffer
            .content
            .chunks(/*chunk_size*/ 47)
            .map(|cells| {
                cells
                    .iter()
                    .map(ratatui::buffer::Cell::symbol)
                    .collect::<String>()
                    .trim()
                    .to_string()
            })
            .filter(|line| line.contains("voice") || line.contains("typed"))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(rows.contains("› typed"));
        assert!(matches!(chat.bottom_pane.cursor_pos(area), Some((_, y)) if y < height));
        layouts.push(format!("{height} rows:\n{rows}"));
    }

    insta::assert_snapshot!(layouts.join("\n\n"), @r"
    5 rows:
    › typed

    6 rows:
    voice ● listening ctrl+x mute     /voice stop
    › typed

    8 rows:
    voice ● listening ctrl+x mute     /voice stop
    › typed
    ");
}

#[tokio::test]
async fn compact_voice_meters_keep_real_speaker_history_when_the_microphone_is_muted() {
    let (mut chat, _sender, _events, _ops) = make_chatwidget_manual_with_sender().await;
    chat.local_settings.tui.animations = false;
    let thread_id = activate_voice(&mut chat);
    chat.realtime_conversation.audio_meter_history = [
        (0, 255),
        (37, 183),
        (73, 128),
        (110, 64),
        (183, 1),
        (255, 0),
    ]
    .into();
    chat.update_realtime_footer();
    let live = render_bottom_popup(&chat, /*width*/ 45);
    let meters = live.lines().find(|line| line.contains("mic")).unwrap();
    assert!(meters.contains("mic ▁▃▄▅▇█"));
    assert!(meters.contains("codex █▇▅▃▂▁"));
    assert_eq!(
        live.lines().filter(|line| line.contains("codex")).count(),
        1
    );

    chat.realtime_conversation.microphone_muted = true;
    for role in ["user", "assistant"] {
        chat.realtime_conversation.transcript_role = Some(role.to_string());
        chat.update_realtime_footer();
        let muted = render_bottom_popup(&chat, /*width*/ 45);
        assert!(muted.contains("mic ▁▁▁▁▁▁"));
        assert!(muted.contains("codex █▇▅▃▂▁"));
    }
    chat.thread_id = Some(ThreadId::new());
    chat.update_realtime_footer();
    assert!(!render_bottom_popup(&chat, /*width*/ 45).contains("codex"));
    chat.thread_id = Some(thread_id);
    chat.realtime_conversation.phase = RealtimeConversationPhase::Stopping;
    chat.update_realtime_footer();
    assert!(!render_bottom_popup(&chat, /*width*/ 45).contains("codex"));
}

//! Voice picker rendering and selection routing without audio hardware.

use super::*;
use crate::chatwidget::tests::make_chatwidget_manual_with_sender;
use crate::chatwidget::tests::render_bottom_popup;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyModifiers;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn voice_settings_selects_a_voice_without_starting_audio() {
    let (mut chat, _sender, mut events, mut ops) = make_chatwidget_manual_with_sender().await;
    chat.config
        .features
        .enable(Feature::RealtimeConversation)
        .unwrap();
    chat.config.realtime.voice = Some(RealtimeVoice::Maple);
    chat.dispatch_command_with_args(SlashCommand::Voice, "settings".to_string(), Vec::new());
    assert!(
        std::iter::from_fn(|| events.try_recv().ok())
            .any(|event| matches!(event, AppEvent::OpenRealtimeSettings))
    );
    chat.open_realtime_settings(Some(RealtimeVoice::Maple), RealtimeVoicesList::builtin());
    insta::assert_snapshot!("voice_settings", render_bottom_popup(&chat, /*width*/ 80));
    chat.bottom_pane
        .handle_key_event(KeyEvent::new(KeyCode::Home, KeyModifiers::NONE));
    chat.bottom_pane
        .handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    let voice = std::iter::from_fn(|| events.try_recv().ok()).find_map(|event| match event {
        AppEvent::PersistRealtimeVoiceSelection { voice } => Some(voice),
        _ => None,
    });
    assert_eq!(voice, Some(RealtimeVoice::Juniper));
    assert_eq!(chat.config.realtime.voice, Some(RealtimeVoice::Maple));
    assert!(ops.try_recv().is_err());
    assert!(!chat.realtime_conversation_is_running());
}

#[tokio::test]
async fn saved_voice_confirmation_snapshot() {
    let (mut chat, _sender, mut events, _ops) = make_chatwidget_manual_with_sender().await;
    while events.try_recv().is_ok() {}
    chat.on_realtime_voice_saved(RealtimeVoice::Juniper);
    let lines = std::iter::from_fn(|| events.try_recv().ok())
        .filter_map(|event| match event {
            AppEvent::InsertHistoryCell(cell) => Some(cell.display_lines(/*width*/ 80)),
            _ => None,
        })
        .flatten()
        .map(|line| line.to_string())
        .collect::<Vec<_>>()
        .join("\n");
    insta::assert_snapshot!("saved_voice_confirmation", lines);
}

#[tokio::test]
async fn voice_settings_uses_server_catalog() {
    let (mut chat, _sender, _events, _ops) = make_chatwidget_manual_with_sender().await;
    chat.open_realtime_settings(
        Some(RealtimeVoice::Cove),
        RealtimeVoicesList {
            v1: vec![RealtimeVoice::Cove, RealtimeVoice::Maple],
            ..RealtimeVoicesList::builtin()
        },
    );
    insta::assert_snapshot!(
        "voice_settings_server_catalog",
        render_bottom_popup(&chat, /*width*/ 80)
    );
}

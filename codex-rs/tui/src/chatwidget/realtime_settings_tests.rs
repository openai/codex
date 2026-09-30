//! Voice settings hierarchy, parent navigation and selection routing without audio hardware.

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
    chat.open_realtime_settings();
    insta::assert_snapshot!("voice_settings", render_bottom_popup(&chat, /*width*/ 80));
    chat.bottom_pane
        .handle_key_event(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
    chat.bottom_pane
        .handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert!(
        std::iter::from_fn(|| events.try_recv().ok())
            .any(|event| matches!(event, AppEvent::OpenRealtimeVoices))
    );
    chat.open_realtime_voices(Some(RealtimeVoice::Maple), RealtimeVoicesList::builtin());
    insta::assert_snapshot!("voice_choices", render_bottom_popup(&chat, /*width*/ 80));
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
    chat.open_realtime_voices(
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

#[tokio::test]
async fn sound_devices_picker_routes_selection_without_starting_audio() {
    let (mut chat, _sender, mut events, mut ops) = make_chatwidget_manual_with_sender().await;
    chat.local_settings.audio.as_mut().unwrap().microphone =
        Some("Zen Go Synergy Core Playback".to_string());
    chat.open_realtime_settings();
    chat.bottom_pane
        .handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert!(
        std::iter::from_fn(|| events.try_recv().ok())
            .any(|event| matches!(event, AppEvent::OpenRealtimeSoundDevices))
    );
    chat.open_realtime_sound_devices();
    insta::assert_snapshot!(
        "voice_settings_input_channel",
        render_bottom_popup(&chat, /*width*/ 90)
    );
    chat.bottom_pane
        .handle_key_event(KeyEvent::new(KeyCode::Home, KeyModifiers::NONE));
    chat.bottom_pane
        .handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert!(
        std::iter::from_fn(|| events.try_recv().ok()).any(|event| matches!(
            event,
            AppEvent::OpenRealtimeDevicePicker {
                kind: AudioDeviceKind::Input
            }
        ))
    );
    assert!(ops.try_recv().is_err());
    assert!(!chat.realtime_conversation_is_running());
}

#[tokio::test]
async fn submenus_return_to_their_parent_with_back_or_escape() {
    let (mut chat, _sender, mut events, mut ops) = make_chatwidget_manual_with_sender().await;
    type OpenMenu = fn(&mut ChatWidget);
    let menus: [OpenMenu; 2] = [
        |chat| chat.open_realtime_voices(/*current*/ None, RealtimeVoicesList::builtin()),
        ChatWidget::open_realtime_sound_devices,
    ];
    for open in menus {
        for key in [KeyCode::Esc, KeyCode::Enter] {
            while events.try_recv().is_ok() {}
            open(&mut chat);
            if key == KeyCode::Enter {
                chat.bottom_pane
                    .handle_key_event(KeyEvent::new(KeyCode::End, KeyModifiers::NONE));
            }
            chat.bottom_pane
                .handle_key_event(KeyEvent::new(key, KeyModifiers::NONE));
            assert!(
                std::iter::from_fn(|| events.try_recv().ok())
                    .any(|event| matches!(event, AppEvent::OpenRealtimeSettings))
            );
        }
    }
    assert!(ops.try_recv().is_err());
    assert!(!chat.realtime_conversation_is_running());
}

#[tokio::test]
async fn device_pickers_route_selection() {
    let (mut chat, _sender, mut events, mut ops) = make_chatwidget_manual_with_sender().await;
    for (kind, channels, microphone, snapshot) in [
        (
            AudioDeviceKind::Input,
            2,
            None,
            "voice_stereo_input_devices",
        ),
        (
            AudioDeviceKind::Input,
            4,
            None,
            "voice_multichannel_input_devices",
        ),
        (
            AudioDeviceKind::Input,
            4,
            Some("Audio interface".into()),
            "voice_multichannel_selected_input_devices",
        ),
        (AudioDeviceKind::Output, 4, None, "voice_output_devices"),
    ] {
        chat.local_settings.audio.as_mut().unwrap().microphone = microphone;
        let device = AudioDevice {
            name: "Audio interface".into(),
            channels,
            is_default: true,
        };
        let devices = vec![
            device.clone(),
            AudioDevice {
                name: "Other device".into(),
                channels: 2,
                is_default: false,
            },
        ];
        chat.open_realtime_device_picker(kind, devices.clone());
        insta::assert_snapshot!(snapshot, render_bottom_popup(&chat, /*width*/ 80));
        chat.bottom_pane
            .handle_key_event(KeyEvent::new(KeyCode::Char('2'), KeyModifiers::NONE));
        let selected =
            std::iter::from_fn(|| events.try_recv().ok()).find_map(|event| match event {
                AppEvent::PersistRealtimeDevice { kind, name } => Some((kind, name)),
                _ => None,
            });
        assert_eq!(selected, Some((kind, Some(device.name.clone()))));
        chat.open_realtime_device_picker(kind, devices);
        chat.bottom_pane
            .handle_key_event(KeyEvent::new(KeyCode::Char('4'), KeyModifiers::NONE));
        assert!(
            std::iter::from_fn(|| events.try_recv().ok())
                .any(|event| matches!(event, AppEvent::OpenRealtimeSoundDevices))
        );
    }
    assert!(ops.try_recv().is_err());
    assert!(!chat.realtime_conversation_is_running());
}

#[tokio::test]
async fn ambiguous_device_names_cannot_select_the_wrong_device() {
    let (mut chat, _sender, mut events, _ops) = make_chatwidget_manual_with_sender().await;
    let devices = [2, 4]
        .map(|channels| AudioDevice {
            name: "USB microphone".into(),
            channels,
            is_default: false,
        })
        .to_vec();
    chat.open_realtime_device_picker(AudioDeviceKind::Input, devices);
    insta::assert_snapshot!(
        "voice_ambiguous_input_devices",
        render_bottom_popup(&chat, /*width*/ 80)
    );
    while events.try_recv().is_ok() {}
    chat.bottom_pane
        .handle_key_event(KeyEvent::new(KeyCode::Char('2'), KeyModifiers::NONE));
    assert!(
        !std::iter::from_fn(|| events.try_recv().ok())
            .any(|event| matches!(event, AppEvent::PersistRealtimeDevice { .. }))
    );
}

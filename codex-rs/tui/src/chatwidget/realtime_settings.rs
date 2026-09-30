//! Voice settings hierarchy with parent navigation; selections apply to subsequent conversations.

use super::*;
use codex_protocol::protocol::RealtimeVoice;
use codex_protocol::protocol::RealtimeVoicesList;
use codex_realtime_webrtc::AudioDevice;
use codex_realtime_webrtc::AudioDeviceKind;

fn back_item(parent: fn() -> AppEvent) -> SelectionItem {
    SelectionItem {
        name: "Back".to_string(),
        actions: vec![Box::new(move |tx| tx.send(parent()))],
        dismiss_on_select: true,
        ..Default::default()
    }
}

impl ChatWidget {
    pub(super) fn realtime_audio_settings(
        &mut self,
    ) -> Option<codex_config::config_toml::RealtimeAudioToml> {
        match self.local_settings.audio.clone() {
            Ok(audio) => Some(audio),
            Err(error) => {
                self.add_error_message(error);
                None
            }
        }
    }

    pub(crate) fn open_realtime_settings(&mut self) {
        self.bottom_pane.show_selection_view(SelectionViewParams {
            title: Some("Voice settings".to_string()),
            items: vec![
                SelectionItem {
                    name: "Set sound devices".to_string(),
                    actions: vec![Box::new(|tx| tx.send(AppEvent::OpenRealtimeSoundDevices))],
                    dismiss_on_select: true,
                    ..Default::default()
                },
                SelectionItem {
                    name: "Choose a voice".to_string(),
                    actions: vec![Box::new(|tx| tx.send(AppEvent::OpenRealtimeVoices))],
                    dismiss_on_select: true,
                    ..Default::default()
                },
            ],
            footer_hint: Some(standard_popup_hint_line()),
            ..SelectionViewParams::picker()
        });
    }

    pub(crate) fn open_realtime_voices(
        &mut self,
        current: Option<RealtimeVoice>,
        voices: RealtimeVoicesList,
    ) {
        // The TUI uses V3, which shares the V1 voice catalog.
        let mut items: Vec<SelectionItem> = voices
            .v1
            .into_iter()
            .map(|voice| SelectionItem {
                name: voice.wire_name().to_string(),
                is_current: Some(voice) == current,
                actions: vec![Box::new(move |tx| {
                    tx.send(AppEvent::PersistRealtimeVoiceSelection { voice });
                })],
                dismiss_on_select: true,
                ..Default::default()
            })
            .collect();
        items.push(back_item(|| AppEvent::OpenRealtimeSettings));
        self.bottom_pane.show_selection_view(SelectionViewParams {
            title: Some("Select voice".to_string()),
            subtitle: Some("Applies to your next voice conversation.".to_string()),
            footer_hint: Some(standard_popup_hint_line()),
            items,
            on_cancel: Some(Box::new(|tx| tx.send(AppEvent::OpenRealtimeSettings))),
            ..SelectionViewParams::picker()
        });
    }

    pub(crate) fn open_realtime_sound_devices(&mut self) {
        let Some(audio) = self.realtime_audio_settings() else {
            return;
        };
        let mut items = Vec::new();
        for (kind, label, selected) in [
            (AudioDeviceKind::Input, "Input device", audio.microphone),
            (AudioDeviceKind::Output, "Output device", audio.speaker),
        ] {
            items.push(SelectionItem {
                name: label.into(),
                description: Some(selected.unwrap_or_else(|| "System default".into())),
                actions: vec![Box::new(move |tx| {
                    tx.send(AppEvent::OpenRealtimeDevicePicker { kind })
                })],
                dismiss_on_select: true,
                ..Default::default()
            });
        }
        items.push(back_item(|| AppEvent::OpenRealtimeSettings));
        self.bottom_pane.show_selection_view(SelectionViewParams {
            title: Some("Sound devices".to_string()),
            subtitle: Some("Applies to your next voice conversation.".into()),
            footer_hint: Some(standard_popup_hint_line()),
            items,
            on_cancel: Some(Box::new(|tx| tx.send(AppEvent::OpenRealtimeSettings))),
            ..SelectionViewParams::picker()
        });
    }

    pub(crate) fn open_realtime_device_picker(
        &mut self,
        kind: AudioDeviceKind,
        devices: Vec<AudioDevice>,
    ) {
        let Some(audio) = self.realtime_audio_settings() else {
            return;
        };
        let (title, current) = match kind {
            AudioDeviceKind::Input => ("Input device", audio.microphone),
            AudioDeviceKind::Output => ("Output device", audio.speaker),
        };
        let ambiguous: std::collections::HashSet<_> = devices
            .iter()
            .filter(|device| {
                devices
                    .iter()
                    .filter(|other| other.name == device.name)
                    .count()
                    > 1
            })
            .map(|device| device.name.clone())
            .collect();
        let mut items: Vec<SelectionItem> = std::iter::once(/*value*/ None)
            .chain(devices.into_iter().map(Some))
            .map(|device| {
                let name = device.map(|device| device.name);
                let is_current = name == current;
                let disabled_reason = name
                    .as_ref()
                    .filter(|name| ambiguous.contains(*name))
                    .map(|_| "Identical device names; use System default.".into());
                SelectionItem {
                    name: name.clone().unwrap_or_else(|| "System default".into()),
                    is_current,
                    disabled_reason,
                    actions: vec![Box::new(move |tx| {
                        tx.send(AppEvent::PersistRealtimeDevice {
                            kind,
                            name: name.clone(),
                        })
                    })],
                    dismiss_on_select: true,
                    ..Default::default()
                }
            })
            .collect();
        items.push(back_item(|| AppEvent::OpenRealtimeSoundDevices));
        self.bottom_pane.show_selection_view(SelectionViewParams {
            title: Some(title.into()),
            subtitle: Some("Applies to your next voice conversation.".into()),
            items,
            footer_hint: Some(standard_popup_hint_line()),
            on_cancel: Some(Box::new(|tx| tx.send(AppEvent::OpenRealtimeSoundDevices))),
            ..SelectionViewParams::picker()
        });
    }

    pub(crate) fn set_realtime_voice(&mut self, voice: Option<RealtimeVoice>) {
        self.config.realtime.voice = voice;
    }

    pub(crate) fn on_realtime_voice_saved(&mut self, voice: RealtimeVoice) {
        self.set_realtime_voice(Some(voice));
        self.add_info_message(
            format!(
                "Voice set to {}. Applies to your next voice conversation.",
                voice.wire_name()
            ),
            /*hint*/ None,
        );
    }
}

#[cfg(test)]
#[path = "realtime_settings_tests.rs"]
mod tests;

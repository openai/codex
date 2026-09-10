//! Voice selection for subsequent conversations; saving never interrupts live audio.

use super::*;
use codex_protocol::protocol::RealtimeVoice;
use codex_protocol::protocol::RealtimeVoicesList;

impl ChatWidget {
    pub(crate) fn open_realtime_settings(
        &mut self,
        current: Option<RealtimeVoice>,
        voices: RealtimeVoicesList,
    ) {
        // The TUI uses V3, which shares the V1 voice catalog.
        let items = voices
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
        self.bottom_pane.show_selection_view(SelectionViewParams {
            title: Some("Select voice".to_string()),
            subtitle: Some("Applies to your next voice conversation.".to_string()),
            footer_hint: Some(standard_popup_hint_line()),
            items,
            ..Default::default()
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

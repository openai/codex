//! Dispatches voice controls through the app before task navigation can change their target.

use super::*;

impl App {
    pub(super) fn control_voice(&mut self, control: crate::app_event::VoiceControl) {
        use crate::app_event::VoiceControl;
        match control {
            VoiceControl::Toggle if !self.reconnect.offline => {
                self.chat_widget.toggle_realtime_conversation()
            }
            VoiceControl::Toggle => {}
            VoiceControl::Stop => self.chat_widget.stop_realtime_conversation(),
            VoiceControl::Mute => self.chat_widget.toggle_realtime_microphone(),
        }
    }
}

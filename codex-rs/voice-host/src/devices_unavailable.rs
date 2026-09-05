//! Keep non-native helper targets buildable without linking a host audio backend.

use std::io;

pub(super) struct Devices;

impl Devices {
    pub(super) fn take_state(&self) -> io::Result<codex_realtime_webrtc::AudioState> {
        Self::open().map(|_| codex_realtime_webrtc::AudioState::default())
    }

    pub(super) fn open() -> io::Result<Self> {
        Err(io::Error::other(
            "audio devices unavailable for this helper target",
        ))
    }

    pub(super) fn set_controls(&self, _: codex_realtime_webrtc::AudioControls) -> io::Result<()> {
        Self::open().map(|_| ())
    }

    pub(super) async fn service(
        &mut self,
        _: &mut crate::audio_track::AudioTrack,
    ) -> io::Result<usize> {
        Self::open().map(|_| 0)
    }
}

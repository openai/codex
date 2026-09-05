//! Keep non-native helper targets buildable without linking a host audio backend.

use std::io;

pub(super) struct Devices;

impl Devices {
    pub(super) fn open() -> io::Result<Self> {
        Err(io::Error::other(
            "audio devices unavailable for this helper target",
        ))
    }

    pub(super) fn set_controls(&self, _: codex_realtime_webrtc::AudioControls) -> io::Result<()> {
        Self::open().map(|_| ())
    }

    pub(super) fn service(&self) -> io::Result<()> {
        Self::open().map(|_| ())
    }
}

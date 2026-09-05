//! Bounded same-build controls and redacted signaling; audio never crosses this pipe.

use std::io;
use std::io::Read;

use serde::Deserialize;
use serde::Serialize;

pub const MAX_FRAME_BYTES: usize = 128 * 1024;

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AudioControls {
    pub microphone_muted: bool,
    pub speaker_suppressed: bool,
}

/// Peaks contain levels only, never retained audio or backend messages.
#[derive(Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AudioState {
    pub microphone_peak: u16,
    pub speaker_peak: u16,
}

/// SDP contains ICE credentials. Bound it at construction and never expose it in diagnostics.
#[derive(Deserialize, PartialEq, Serialize)]
#[serde(try_from = "String")]
pub struct SessionDescription(String);

impl TryFrom<String> for SessionDescription {
    type Error = &'static str;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        if value.is_empty() || value.len() > 64 * 1024 {
            return Err("invalid voice session description length");
        }
        Ok(Self(value))
    }
}

impl std::fmt::Debug for SessionDescription {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SessionDescription([REDACTED])")
    }
}

impl SessionDescription {
    pub fn into_sdp(self) -> String {
        self.0
    }
}

/// Fixed child settings prevent native initialization from scanning system plugins or caches.
pub const RUNTIME_ENVIRONMENT: [(&str, &str); 7] = [
    ("GST_PLUGIN_PATH", ""),
    ("GST_PLUGIN_PATH_1_0", ""),
    ("GST_PLUGIN_SYSTEM_PATH", ""),
    ("GST_PLUGIN_SYSTEM_PATH_1_0", ""),
    (
        "GST_REGISTRY",
        if cfg!(windows) { "NUL" } else { "/dev/null" },
    ),
    ("GST_REGISTRY_UPDATE", "no"),
    ("GST_REGISTRY_FORK", "no"),
];

#[derive(Debug, Deserialize, PartialEq, Serialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum Message {
    Hello { protocol: u32, build_commit: String },
    Ready {},
    InitializeRuntime {},
    RuntimeReady {},
    StartTransport {},
    Offer { sdp: SessionDescription },
    ApplyAnswer { sdp: SessionDescription },
    TransportReady {},
    OpenDevices {},
    DevicesOpened {},
    SetAudioControls { controls: AudioControls },
    AudioControlsApplied {},
    InspectAudio {},
    AudioState { state: AudioState },
    Close {},
    Closed {},
}

#[cfg(test)]
#[path = "protocol_tests.rs"]
mod tests;

pub fn encode_frame(message: &Message) -> io::Result<Vec<u8>> {
    let payload = serde_json::to_vec(message)?;
    if payload.len() > MAX_FRAME_BYTES {
        return Err(io::Error::other("voice frame exceeds limit"));
    }
    let mut frame = (payload.len() as u32).to_be_bytes().to_vec();
    frame.extend(payload);
    Ok(frame)
}

pub fn decode_frame(frame: &[u8]) -> io::Result<Option<Message>> {
    let [a, b, c, d, ..] = frame else {
        return Ok(None);
    };
    let length = u32::from_be_bytes([*a, *b, *c, *d]) as usize;
    if length > MAX_FRAME_BYTES || frame.len() > length + 4 {
        return Err(io::Error::other("invalid voice frame length"));
    }
    frame
        .get(4..length + 4)
        .map(serde_json::from_slice)
        .transpose()
        .map_err(|_| io::Error::other("invalid voice frame"))
}

pub fn read_message(reader: &mut impl Read) -> io::Result<Option<Message>> {
    let mut header = [0; 4];
    match reader.read_exact(&mut header[..1]) {
        Err(error) if error.kind() == io::ErrorKind::UnexpectedEof => return Ok(None),
        result => result?,
    }
    reader.read_exact(&mut header[1..])?;
    decode_frame(&header)?;
    let length = u32::from_be_bytes(header) as usize;
    let mut frame = header.to_vec();
    frame.resize(length + 4, /*value*/ 0);
    reader.read_exact(&mut frame[4..])?;
    decode_frame(&frame)
}

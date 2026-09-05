//! A single Opus RTP track. Elapsed mute time advances its clock without inventing packet loss.

use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;

use rtc::media::Sample;
use rtc::rtp_transceiver::rtp_sender::RTCRtpCodec;
use rtc::rtp_transceiver::rtp_sender::RTCRtpCodecParameters;
use rtc::rtp_transceiver::rtp_sender::RTCRtpCodingParameters;
use rtc::rtp_transceiver::rtp_sender::RTCRtpEncodingParameters;
use rtc::rtp_transceiver::rtp_sender::RtpCodecKind;
use webrtc::media_stream::MediaStreamTrack;
use webrtc::media_stream::track_local::static_sample::TrackLocalStaticSample;
use webrtc::peer_connection::MediaEngine;

pub(crate) const SEND_TIMEOUT: Duration = Duration::from_millis(/*millis*/ 100);

pub(crate) struct EncodedAudio {
    pub(crate) data: Vec<u8>,
    pub(crate) at: Instant,
}

pub(crate) struct AudioTrack {
    pub(crate) track: Arc<TrackLocalStaticSample>,
    ssrc: u32,
    end: Option<Instant>,
}

impl AudioTrack {
    pub(crate) fn new() -> Result<(MediaEngine, Self), &'static str> {
        let ssrc = rand::random();
        let codec = RTCRtpCodec {
            mime_type: "audio/opus".into(),
            clock_rate: 48_000,
            channels: 2,
            sdp_fmtp_line: "minptime=10;useinbandfec=1".into(),
            rtcp_feedback: vec![],
        };
        let mut media = MediaEngine::default();
        media
            .register_codec(
                RTCRtpCodecParameters {
                    rtp_codec: codec.clone(),
                    payload_type: 111,
                },
                RtpCodecKind::Audio,
            )
            .map_err(|_| "failed to register voice codec")?;
        let track = TrackLocalStaticSample::new(MediaStreamTrack::new(
            "realtime".into(),
            format!("audio-{ssrc}"),
            "microphone".into(),
            RtpCodecKind::Audio,
            vec![RTCRtpEncodingParameters {
                rtp_coding_parameters: RTCRtpCodingParameters {
                    ssrc: Some(ssrc),
                    ..Default::default()
                },
                codec,
                active: true,
                ..Default::default()
            }],
        ))
        .map_err(|_| "failed to create voice track")?;
        Ok((
            media,
            Self {
                track: Arc::new(track),
                ssrc,
                end: None,
            },
        ))
    }

    pub(crate) async fn send(&mut self, frame: EncodedAudio) -> Result<(), &'static str> {
        tokio::time::timeout(SEND_TIMEOUT, async {
            let mut gap = self
                .end
                .map(|end| frame.at.saturating_duration_since(end))
                .unwrap_or_default();
            while !gap.is_zero() {
                let duration = gap.min(Duration::from_secs(/*secs*/ 3600));
                self.track
                    .write_sample(
                        self.ssrc,
                        /*payload_type*/ 111,
                        &Sample {
                            duration,
                            ..Default::default()
                        },
                        &[],
                    )
                    .await
                    .map_err(|_| "failed to advance voice clock")?;
                gap -= duration;
            }
            let duration = Duration::from_millis(/*millis*/ 20);
            self.track
                .write_sample(
                    self.ssrc,
                    /*payload_type*/ 111,
                    &Sample {
                        data: frame.data.into(),
                        duration,
                        ..Default::default()
                    },
                    &[],
                )
                .await
                .map_err(|_| "failed to send voice audio")?;
            self.end = Some(self.end.unwrap_or(frame.at).max(frame.at) + duration);
            Ok(())
        })
        .await
        .map_err(|_| "voice audio sender stalled")?
    }
}

#[cfg(test)]
#[path = "audio_track_tests.rs"]
mod tests;

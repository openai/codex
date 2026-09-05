//! Owns one WebRTC peer and ordered event channel; the helper runtime bounds their lifetime.
//! Raw upstream errors and data messages are discarded; typed events use the existing sideband.
//! Candidate admission is bounded before an answer can mutate the peer or start ICE work.
//! Remotely opened channels are rejected; only the locally created event channel is used.

use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

use futures::future::BoxFuture;
use tokio::sync::Notify;
use tokio::sync::watch;
use tokio::task::JoinHandle;
use tokio::time::timeout;
use webrtc::data_channel::DataChannel;
use webrtc::data_channel::DataChannelEvent;
use webrtc::peer_connection::PeerConnection;
use webrtc::peer_connection::PeerConnectionBuilder;
use webrtc::peer_connection::PeerConnectionEventHandler;
use webrtc::peer_connection::RTCIceCandidateInit;
use webrtc::peer_connection::RTCIceGatheringState;
use webrtc::peer_connection::RTCSessionDescription;

const WAIT: Duration = Duration::from_secs(/*secs*/ 15);
const MAX_REMOTE_CANDIDATES: usize = 32;
type Result<T> = std::result::Result<T, &'static str>;

struct Events(Arc<Notify>);

// The upstream trait requires async-trait's boxed-future ABI.
impl PeerConnectionEventHandler for Events {
    fn on_data_channel<'a, 'async_trait>(
        &'a self,
        channel: Arc<dyn DataChannel>,
    ) -> BoxFuture<'async_trait, ()>
    where
        'a: 'async_trait,
        Self: 'async_trait,
    {
        Box::pin(async move {
            // Locally created channels do not invoke this callback.
            let _ = channel.close().await;
        })
    }

    fn on_ice_gathering_state_change<'a, 'async_trait>(
        &'a self,
        state: RTCIceGatheringState,
    ) -> BoxFuture<'async_trait, ()>
    where
        'a: 'async_trait,
        Self: 'async_trait,
    {
        Box::pin(async move {
            if state == RTCIceGatheringState::Complete {
                self.0.notify_one();
            }
        })
    }
}

pub(crate) struct Transport {
    pub(crate) incoming: crate::incoming::Incoming,
    pub(crate) audio: crate::audio_track::AudioTrack,
    connection: Arc<dyn PeerConnection>,
    gathered: Arc<Notify>,
    observer: JoinHandle<()>,
    pub(crate) ready: watch::Receiver<bool>,
}

impl Transport {
    pub(crate) async fn new() -> Result<Self> {
        Self::with_runtime(Arc::new(crate::transport_runtime::VoiceRuntime::default())).await
    }

    async fn with_runtime(runtime: Arc<dyn webrtc::runtime::Runtime>) -> Result<Self> {
        let (media, audio) = crate::audio_track::AudioTrack::new()?;
        let (incoming, ingress) = crate::incoming::Incoming::new();
        let gathered = Arc::new(Notify::new());
        // Upstream defaults exhaust checks after 1.4s, including checks sent before
        // a TCP connection exists. Keep probing throughout our negotiation deadline.
        let check_interval = Duration::from_millis(/*millis*/ 200);
        let attempts = u16::try_from(WAIT.as_millis() / check_interval.as_millis())
            .map_err(|_| "invalid voice ICE attempt budget")?;
        let mut settings = webrtc::peer_connection::SettingEngine::default();
        settings.set_ice_connection_attempts(Some(check_interval), Some(attempts));
        let connection: Arc<dyn PeerConnection> = Arc::new(
            PeerConnectionBuilder::new()
                .with_interceptor_registry(rtc::interceptor::Registry::from(ingress))
                .with_media_engine(media)
                .with_runtime(runtime)
                .with_setting_engine(settings)
                .with_handler(Arc::new(Events(gathered.clone())))
                .with_udp_addrs(vec!["0.0.0.0:0", "[::]:0"])
                .with_tcp_addrs(vec!["0.0.0.0:0", "[::]:0"])
                .with_data_channel_send_buffer_limit(/*bytes*/ 64 * 1024)
                .with_sctp_receive_buffer_size(/*size*/ 64 * 1024)
                .build()
                .await
                .map_err(|_| "failed to create voice peer")?,
        );
        if connection.add_track(audio.track.clone()).await.is_err() {
            let _ = timeout(Duration::from_secs(/*secs*/ 2), connection.close()).await;
            return Err("failed to attach voice track");
        }
        let channel = match connection
            .create_data_channel("oai-events", /*options*/ None)
            .await
        {
            Ok(channel) => channel,
            Err(_) => {
                let _ = timeout(Duration::from_secs(/*secs*/ 2), connection.close()).await;
                return Err("failed to create voice event channel");
            }
        };
        let (sender, ready) = watch::channel(false);
        let observer = tokio::spawn(async move {
            while let Some(event) = channel.poll().await {
                match event {
                    DataChannelEvent::OnOpen => {
                        sender.send_replace(true);
                    }
                    DataChannelEvent::OnError
                    | DataChannelEvent::OnClosing
                    | DataChannelEvent::OnClose => break,
                    DataChannelEvent::OnMessage(_)
                    | DataChannelEvent::OnBufferedAmountLow
                    | DataChannelEvent::OnBufferedAmountHigh => {}
                }
            }
            sender.send_replace(false);
        });
        Ok(Self {
            incoming,
            audio,
            connection,
            gathered,
            observer,
            ready,
        })
    }

    pub(crate) async fn offer(&self) -> Result<String> {
        timeout(WAIT, async {
            let offer = self
                .connection
                .create_offer(/*options*/ None)
                .await
                .map_err(|_| "failed to create voice offer")?;
            self.connection
                .set_local_description(offer)
                .await
                .map_err(|_| "failed to set voice offer")?;
            self.gathered.notified().await;
            self.connection
                .local_description()
                .await
                .map(|offer| offer.sdp)
                .ok_or("missing voice offer")
        })
        .await
        .map_err(|_| "timed out gathering voice candidates")?
    }

    pub(crate) async fn apply_answer(&self, sdp: String) -> Result<()> {
        let answer = RTCSessionDescription::answer(sdp).map_err(|_| "invalid voice answer")?;
        let parsed = answer.unmarshal().map_err(|_| "invalid voice answer")?;
        let mut seen = HashSet::new();
        let mut candidates = Vec::new();
        // Count all occurrences, including duplicates and other components/media sections.
        // set_remote_description itself installs ICE candidates, so validate before that call.
        for (index, attribute) in parsed
            .media_descriptions
            .iter()
            .flat_map(|media| &media.attributes)
            .filter(|attribute| attribute.key == "candidate")
            .enumerate()
        {
            if index >= MAX_REMOTE_CANDIDATES {
                return Err("too many voice candidates");
            }
            let candidate = attribute
                .value
                .as_deref()
                .ok_or("invalid voice candidate")?;
            let parsed = rtc::ice::candidate::unmarshal_candidate(candidate)
                .map_err(|_| "invalid voice candidate")?;
            if parsed.component() == 1 && seen.insert(candidate) {
                candidates.push(RTCIceCandidateInit {
                    candidate: format!("candidate:{candidate}"),
                    ..Default::default()
                });
            }
        }
        timeout(WAIT, async {
            self.connection
                .set_remote_description(answer)
                .await
                .map_err(|_| "failed to apply voice answer")?;
            // The async driver requires explicit candidates to start TCP dialing,
            // even when they were bundled in the non-trickle answer.
            for candidate in candidates {
                self.connection
                    .add_ice_candidate(candidate)
                    .await
                    .map_err(|_| "failed to apply voice candidate")?;
            }
            self.ready
                .clone()
                .wait_for(|ready| *ready)
                .await
                .map(|_| ())
                .map_err(|_| "voice event channel closed")
        })
        .await
        .map_err(|_| "timed out connecting voice peer")?
    }

    pub(crate) async fn close(&mut self) -> Result<()> {
        self.observer.abort();
        let _ = (&mut self.observer).await;
        timeout(Duration::from_secs(/*secs*/ 2), self.connection.close())
            .await
            .map_err(|_| "timed out closing voice peer")?
            .map_err(|_| "failed to close voice peer")
    }
}

impl Drop for Transport {
    fn drop(&mut self) {
        self.observer.abort();
        // The helper owns the runtime: destroying it cancels all remaining peer tasks.
    }
}

#[cfg(test)]
#[path = "transport_tests.rs"]
mod tests;

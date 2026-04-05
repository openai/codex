use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;

use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use opus_rs::OpusDecoder;
use serde_json::Value;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;
use tokio::sync::Notify;
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use tokio::time::timeout;
use tracing::debug;
use tracing::warn;
use webrtc::api::APIBuilder;
use webrtc::api::interceptor_registry::register_default_interceptors;
use webrtc::api::media_engine::MediaEngine;
use webrtc::data_channel::RTCDataChannel;
use webrtc::data_channel::data_channel_message::DataChannelMessage;
use webrtc::interceptor::registry::Registry;
use webrtc::peer_connection::RTCPeerConnection;
use webrtc::peer_connection::configuration::RTCConfiguration;
use webrtc::peer_connection::peer_connection_state::RTCPeerConnectionState;
use webrtc::peer_connection::sdp::session_description::RTCSessionDescription;
use webrtc::rtp_transceiver::rtp_codec::RTPCodecType;
use webrtc::track::track_remote::TrackRemote;

use super::WebSocketConnectionConfig;
use super::WebSocketHandshake;
use super::WebSocketRequest;

const HTTP_HEADER_TERMINATOR: &[u8] = b"\r\n\r\n";
const REALTIME_AUDIO_CHANNELS: u8 = 1;
const REALTIME_AUDIO_SAMPLE_RATE: u32 = 24_000;
const REALTIME_DATA_CHANNEL_TIMEOUT: Duration = Duration::from_secs(10);
const REALTIME_DATA_CHANNEL_FLUSH_DELAY: Duration = Duration::from_millis(25);
const REALTIME_MAX_DECODED_SAMPLES_PER_CHANNEL: usize = 5760;

pub(super) async fn accept_is_http_post(stream: &TcpStream) -> bool {
    let mut method = [0u8; 4];
    matches!(stream.peek(&mut method).await, Ok(4)) && method == *b"POST"
}

pub(super) async fn serve_connection(
    mut stream: TcpStream,
    connection: WebSocketConnectionConfig,
    connection_index: usize,
    requests: Arc<Mutex<Vec<Vec<WebSocketRequest>>>>,
    handshakes: Arc<Mutex<Vec<WebSocketHandshake>>>,
    request_log_updated: Arc<Notify>,
) {
    let Some(request) = read_http_request(&mut stream).await else {
        return;
    };

    handshakes.lock().unwrap().push(WebSocketHandshake {
        uri: request.uri,
        headers: request.headers,
    });

    let Some(offer_sdp) = parse_multipart_field(&request.body, &request.boundary, "sdp") else {
        let _ = stream
            .write_all(b"HTTP/1.1 400 Bad Request\r\nContent-Length: 0\r\n\r\n")
            .await;
        return;
    };

    let Some(session) = start_session(
        offer_sdp,
        connection,
        connection_index,
        requests,
        request_log_updated,
    )
    .await
    else {
        let _ = stream
            .write_all(b"HTTP/1.1 500 Internal Server Error\r\nContent-Length: 0\r\n\r\n")
            .await;
        return;
    };

    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/sdp\r\nContent-Length: {}\r\n\r\n{}",
        session.answer_sdp.len(),
        session.answer_sdp
    );
    if stream.write_all(response.as_bytes()).await.is_err() {
        return;
    }

    let _ = session.done_rx.await;
    let _ = session.peer_connection.close().await;
}

struct HttpRealtimeRequest {
    uri: String,
    headers: Vec<(String, String)>,
    boundary: String,
    body: Vec<u8>,
}

struct RealtimeSession {
    answer_sdp: String,
    peer_connection: Arc<RTCPeerConnection>,
    done_rx: oneshot::Receiver<()>,
}

async fn read_http_request(stream: &mut TcpStream) -> Option<HttpRealtimeRequest> {
    let mut received = Vec::new();
    let headers_end = loop {
        if let Some(headers_end) = received
            .windows(HTTP_HEADER_TERMINATOR.len())
            .position(|window| window == HTTP_HEADER_TERMINATOR)
        {
            break headers_end + HTTP_HEADER_TERMINATOR.len();
        }

        let mut chunk = [0u8; 1024];
        let read = stream.read(&mut chunk).await.ok()?;
        if read == 0 {
            return None;
        }
        received.extend_from_slice(&chunk[..read]);
    };

    let header_text = std::str::from_utf8(&received[..headers_end]).ok()?;
    let mut lines = header_text.split("\r\n").filter(|line| !line.is_empty());
    let request_line = lines.next()?;
    let mut request_line_parts = request_line.split_whitespace();
    if request_line_parts.next()? != "POST" {
        return None;
    }
    let uri = request_line_parts.next()?.to_string();

    let mut headers = Vec::new();
    let mut content_length = None;
    let mut boundary = None;
    for line in lines {
        let Some((name, value)) = line.split_once(':') else {
            continue;
        };
        let name = name.trim().to_string();
        let value = value.trim().to_string();
        if name.eq_ignore_ascii_case("content-length") {
            content_length = value.parse::<usize>().ok();
        }
        if name.eq_ignore_ascii_case("content-type") {
            boundary = value
                .split(';')
                .map(str::trim)
                .find_map(|part| part.strip_prefix("boundary="))
                .map(|boundary| boundary.trim_matches('"').to_string());
        }
        headers.push((name, value));
    }

    let content_length = content_length?;
    while received.len() - headers_end < content_length {
        let mut chunk = [0u8; 1024];
        let read = stream.read(&mut chunk).await.ok()?;
        if read == 0 {
            return None;
        }
        received.extend_from_slice(&chunk[..read]);
    }

    let body_end = headers_end + content_length;
    let body = received[headers_end..body_end].to_vec();
    Some(HttpRealtimeRequest {
        uri,
        headers,
        boundary: boundary?,
        body,
    })
}

fn parse_multipart_field(body: &[u8], boundary: &str, field_name: &str) -> Option<String> {
    let body = std::str::from_utf8(body).ok()?;
    let delimiter = format!("--{boundary}");
    body.split(&delimiter).find_map(|part| {
        let (headers, value) = part.split_once("\r\n\r\n")?;
        if !headers.contains(&format!("name=\"{field_name}\"")) {
            return None;
        }
        Some(value.trim_end_matches("\r\n").to_string())
    })
}

async fn start_session(
    offer_sdp: String,
    connection: WebSocketConnectionConfig,
    connection_index: usize,
    requests: Arc<Mutex<Vec<Vec<WebSocketRequest>>>>,
    request_log_updated: Arc<Notify>,
) -> Option<RealtimeSession> {
    let peer_connection = create_peer_connection().await?;
    let (tx_request, rx_request) = mpsc::unbounded_channel::<Value>();
    let (tx_data_channel, rx_data_channel) = oneshot::channel::<Arc<RTCDataChannel>>();
    let tx_data_channel = Mutex::new(Some(tx_data_channel));
    let tx_data_channel_request = tx_request.clone();
    let connection_closed = Arc::new(Notify::new());
    let on_data_channel_closed = Arc::clone(&connection_closed);
    let on_peer_connection_closed = Arc::clone(&connection_closed);

    peer_connection.on_data_channel(Box::new(move |data_channel| {
        let tx_request = tx_data_channel_request.clone();
        let on_data_channel_closed = Arc::clone(&on_data_channel_closed);
        if let Ok(mut tx_data_channel) = tx_data_channel.lock()
            && let Some(tx_data_channel) = tx_data_channel.take()
        {
            let _ = tx_data_channel.send(Arc::clone(&data_channel));
        }
        data_channel.on_close(Box::new(move || {
            let on_data_channel_closed = Arc::clone(&on_data_channel_closed);
            Box::pin(async move {
                on_data_channel_closed.notify_waiters();
            })
        }));
        data_channel.on_message(Box::new(move |message: DataChannelMessage| {
            let tx_request = tx_request.clone();
            Box::pin(async move {
                if !message.is_string {
                    return;
                }
                let Ok(text) = String::from_utf8(message.data.to_vec()) else {
                    return;
                };
                let Ok(body) = serde_json::from_str::<Value>(&text) else {
                    return;
                };
                let _ = tx_request.send(body);
            })
        }));
        Box::pin(async {})
    }));
    peer_connection.on_peer_connection_state_change(Box::new(move |state| {
        let on_peer_connection_closed = Arc::clone(&on_peer_connection_closed);
        Box::pin(async move {
            if matches!(
                state,
                RTCPeerConnectionState::Closed
                    | RTCPeerConnectionState::Disconnected
                    | RTCPeerConnectionState::Failed
            ) {
                on_peer_connection_closed.notify_waiters();
            }
        })
    }));

    register_remote_audio_handler(&peer_connection, tx_request.clone());

    let mut gather_complete = peer_connection.gathering_complete_promise().await;
    let offer = RTCSessionDescription::offer(offer_sdp).ok()?;
    peer_connection.set_remote_description(offer).await.ok()?;
    let answer = peer_connection.create_answer(None).await.ok()?;
    peer_connection.set_local_description(answer).await.ok()?;
    let _ = gather_complete.recv().await;
    let answer_sdp = peer_connection.local_description().await?.sdp;

    let (done_tx, done_rx) = oneshot::channel();
    tokio::spawn(async move {
        serve_scripted_requests(
            connection,
            connection_index,
            requests,
            request_log_updated,
            rx_request,
            rx_data_channel,
            connection_closed,
        )
        .await;
        let _ = done_tx.send(());
    });

    Some(RealtimeSession {
        answer_sdp,
        peer_connection,
        done_rx,
    })
}

async fn create_peer_connection() -> Option<Arc<RTCPeerConnection>> {
    let mut media_engine = MediaEngine::default();
    media_engine.register_default_codecs().ok()?;
    let registry = register_default_interceptors(Registry::new(), &mut media_engine).ok()?;
    let api = APIBuilder::new()
        .with_media_engine(media_engine)
        .with_interceptor_registry(registry)
        .build();
    api.new_peer_connection(RTCConfiguration::default())
        .await
        .map(Arc::new)
        .ok()
}

fn register_remote_audio_handler(
    peer_connection: &Arc<RTCPeerConnection>,
    tx_request: mpsc::UnboundedSender<Value>,
) {
    peer_connection.on_track(Box::new(move |track, _, _| {
        let tx_request = tx_request.clone();
        Box::pin(async move {
            if track.kind() != RTPCodecType::Audio {
                return;
            }
            pump_remote_audio_track(track, tx_request).await;
        })
    }));
}

async fn pump_remote_audio_track(
    track: Arc<TrackRemote>,
    tx_request: mpsc::UnboundedSender<Value>,
) {
    let mut decoder = match OpusDecoder::new(24_000, usize::from(REALTIME_AUDIO_CHANNELS)) {
        Ok(decoder) => decoder,
        Err(err) => {
            warn!(%err, "failed to initialize realtime Opus decoder in test server");
            return;
        }
    };
    debug!("initialized realtime Opus decoder in test server");
    let mut decoded = vec![0.0f32; REALTIME_MAX_DECODED_SAMPLES_PER_CHANNEL];

    while let Ok((packet, _)) = track.read_rtp().await {
        if packet.payload.is_empty() {
            continue;
        }
        let samples_per_channel = match decoder.decode(
            &packet.payload,
            REALTIME_MAX_DECODED_SAMPLES_PER_CHANNEL,
            &mut decoded,
        ) {
            Ok(samples_per_channel) => samples_per_channel,
            Err(err) => {
                warn!(
                    %err,
                    payload_len = packet.payload.len(),
                    "failed to decode realtime Opus packet in test server"
                );
                return;
            }
        };
        if samples_per_channel == 0 {
            continue;
        }

        let mut pcm_bytes = Vec::with_capacity(samples_per_channel * 2);
        for sample in &decoded[..samples_per_channel] {
            pcm_bytes.extend_from_slice(&f32_to_i16(*sample).to_le_bytes());
        }
        let _ = tx_request.send(serde_json::json!({
            "type": "input_audio_buffer.append",
            "audio": BASE64_STANDARD.encode(pcm_bytes),
            "sample_rate": REALTIME_AUDIO_SAMPLE_RATE,
            "channels": REALTIME_AUDIO_CHANNELS,
            "samples_per_channel": samples_per_channel,
        }));
    }
}

fn f32_to_i16(sample: f32) -> i16 {
    (sample.clamp(-1.0, 1.0) * i16::MAX as f32) as i16
}

async fn serve_scripted_requests(
    connection: WebSocketConnectionConfig,
    connection_index: usize,
    requests: Arc<Mutex<Vec<Vec<WebSocketRequest>>>>,
    request_log_updated: Arc<Notify>,
    mut rx_request: mpsc::UnboundedReceiver<Value>,
    rx_data_channel: oneshot::Receiver<Arc<RTCDataChannel>>,
    connection_closed: Arc<Notify>,
) {
    let Ok(Ok(data_channel)) = timeout(REALTIME_DATA_CHANNEL_TIMEOUT, rx_data_channel).await else {
        return;
    };

    let mut scripted_requests = VecDeque::from(connection.requests);
    while let Some(request_events) = scripted_requests.pop_front() {
        // WebRTC compact-remote tests often close the session before consuming every scripted
        // request slot. Treat transport closure as end-of-script instead of waiting forever for
        // another request that can no longer arrive.
        let body = tokio::select! {
            body = rx_request.recv() => body,
            _ = connection_closed.notified() => None,
        };
        let Some(body) = body else {
            break;
        };
        log_request(connection_index, body, &requests, &request_log_updated);
        for event in &request_events {
            let Ok(payload) = serde_json::to_string(event) else {
                continue;
            };
            if data_channel.send_text(payload).await.is_err() {
                return;
            }
        }
    }

    if connection.close_after_requests {
        tokio::time::sleep(REALTIME_DATA_CHANNEL_FLUSH_DELAY).await;
        let _ = data_channel.close().await;
    }
}

fn log_request(
    connection_index: usize,
    body: Value,
    requests: &Arc<Mutex<Vec<Vec<WebSocketRequest>>>>,
    request_log_updated: &Arc<Notify>,
) {
    let mut log = requests.lock().unwrap();
    if log.len() <= connection_index {
        log.resize_with(connection_index + 1, Vec::new);
    }
    if let Some(connection_log) = log.get_mut(connection_index) {
        connection_log.push(WebSocketRequest { body });
    }
    drop(log);
    request_log_updated.notify_waiters();
}

use crate::outgoing_message::OutgoingMessage;
use crate::transport::RemoteControlApnsRegistration;
use codex_app_server_protocol::JSONRPCMessage;
use serde::Deserialize;
use serde::Serialize;
use std::io;
use std::io::ErrorKind;
use url::Host;
use url::Url;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RemoteControlTarget {
    pub(super) websocket_url: String,
    pub(super) enroll_url: String,
    pub(super) refresh_url: String,
    pub(super) pair_url: String,
}

#[derive(Debug, Serialize)]
pub(super) struct EnrollRemoteServerRequest {
    pub(super) name: String,
    pub(super) os: &'static str,
    pub(super) arch: &'static str,
    pub(super) app_server_version: &'static str,
    pub(super) installation_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) apns_registration: Option<RemoteControlApnsRegistration>,
}

#[derive(Debug, Deserialize)]
pub(super) struct EnrollRemoteServerResponse {
    pub(super) server_id: String,
    pub(super) environment_id: String,
    pub(super) remote_control_token: String,
    pub(super) expires_at: String,
}

#[derive(Debug, Serialize)]
pub(super) struct RefreshRemoteServerRequest {
    pub(super) server_id: String,
    pub(super) installation_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) apns_registration: Option<RemoteControlApnsRegistration>,
}

#[derive(Debug, Serialize)]
pub(super) struct StartRemoteControlPairingRequest {
    pub(super) manual_code: bool,
}

#[derive(Debug, Deserialize)]
pub(super) struct StartRemoteControlPairingResponse {
    pub(super) pairing_code: String,
    pub(super) manual_pairing_code: Option<String>,
    pub(super) server_id: String,
    pub(super) environment_id: String,
    pub(super) expires_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ClientId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct StreamId(pub String);

impl StreamId {
    pub fn new_random() -> Self {
        Self(uuid::Uuid::now_v7().to_string())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientEvent {
    ClientMessage {
        message: JSONRPCMessage,
    },
    ClientMessageChunk {
        segment_id: usize,
        segment_count: usize,
        message_size_bytes: usize,
        message_chunk_base64: String,
    },
    /// Backend-generated acknowledgement for all server envelopes addressed to
    /// `client_id` and `stream_id` whose envelope `seq_id` is less than or equal
    /// to this ack's `seq_id`. Chunk acknowledgements carry `segment_id` so the
    /// sender can retain only the still-unacked wire chunks on reconnect.
    Ack {
        #[serde(skip_serializing_if = "Option::is_none")]
        segment_id: Option<usize>,
    },
    Ping,
    ClientClosed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) struct ClientEnvelope {
    #[serde(flatten)]
    pub(crate) event: ClientEvent,
    #[serde(rename = "client_id")]
    pub(crate) client_id: ClientId,
    #[serde(rename = "stream_id", skip_serializing_if = "Option::is_none")]
    pub(crate) stream_id: Option<StreamId>,
    /// For `Ack`, this is the backend-generated per-stream cursor over
    /// `ServerEnvelope.seq_id`.
    #[serde(rename = "seq_id", skip_serializing_if = "Option::is_none")]
    pub(crate) seq_id: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) cursor: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PongStatus {
    Active,
    Unknown,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerEvent {
    ServerMessage {
        message: Box<OutgoingMessage>,
    },
    ServerMessageChunk {
        segment_id: usize,
        segment_count: usize,
        message_size_bytes: usize,
        message_chunk_base64: String,
    },
    #[allow(dead_code)]
    Ack,
    Pong {
        status: PongStatus,
    },
}

impl ServerEvent {
    pub(crate) fn segment_id(&self) -> Option<usize> {
        match self {
            Self::ServerMessageChunk { segment_id, .. } => Some(*segment_id),
            Self::ServerMessage { .. } | Self::Ack | Self::Pong { .. } => None,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) struct ServerEnvelope {
    #[serde(flatten)]
    pub(crate) event: ServerEvent,
    #[serde(rename = "client_id")]
    pub(crate) client_id: ClientId,
    #[serde(rename = "stream_id")]
    pub(crate) stream_id: StreamId,
    #[serde(rename = "seq_id")]
    pub(crate) seq_id: u64,
}

fn is_allowed_remote_control_chatgpt_host(host: &Option<Host<&str>>) -> bool {
    let Some(Host::Domain(host)) = *host else {
        return false;
    };
    host == "chatgpt.com"
        || host == "chatgpt-staging.com"
        || host.ends_with(".chatgpt.com")
        || host.ends_with(".chatgpt-staging.com")
}

fn is_localhost(host: &Option<Host<&str>>) -> bool {
    match host {
        Some(Host::Domain("localhost")) => true,
        Some(Host::Ipv4(ip)) => ip.is_loopback(),
        Some(Host::Ipv6(ip)) => ip.is_loopback(),
        _ => false,
    }
}

pub(super) fn normalize_remote_control_url(
    remote_control_url: &str,
) -> io::Result<RemoteControlTarget> {
    let remote_control_url = normalize_remote_control_base_url(remote_control_url)?;
    let map_url_parse_error = |err: url::ParseError| -> io::Error {
        io::Error::new(
            ErrorKind::InvalidInput,
            format!("invalid remote control URL `{remote_control_url}`: {err}"),
        )
    };

    let enroll_url = remote_control_url
        .join("wham/remote/control/server/enroll")
        .map_err(map_url_parse_error)?;
    let refresh_url = remote_control_url
        .join("wham/remote/control/server/refresh")
        .map_err(map_url_parse_error)?;
    let pair_url = remote_control_url
        .join("wham/remote/control/server/pair")
        .map_err(map_url_parse_error)?;
    let mut websocket_url = remote_control_url
        .join("wham/remote/control/server")
        .map_err(map_url_parse_error)?;
    websocket_url
        .set_scheme(if enroll_url.scheme() == "https" {
            "wss"
        } else {
            "ws"
        })
        .map_err(|()| {
            io::Error::new(
                ErrorKind::InvalidInput,
                format!("invalid remote control URL `{remote_control_url}`"),
            )
        })?;

    Ok(RemoteControlTarget {
        websocket_url: websocket_url.to_string(),
        enroll_url: enroll_url.to_string(),
        refresh_url: refresh_url.to_string(),
        pair_url: pair_url.to_string(),
    })
}

pub(super) fn normalize_remote_control_base_url(remote_control_url: &str) -> io::Result<Url> {
    let map_url_parse_error = |err: url::ParseError| -> io::Error {
        io::Error::new(
            ErrorKind::InvalidInput,
            format!("invalid remote control URL `{remote_control_url}`: {err}"),
        )
    };
    let map_scheme_error = |_: ()| -> io::Error {
        io::Error::new(
            ErrorKind::InvalidInput,
            format!(
                "invalid remote control URL `{remote_control_url}`; expected HTTPS URL for chatgpt.com or chatgpt-staging.com, or HTTP/HTTPS URL for localhost"
            ),
        )
    };

    let mut remote_control_url = Url::parse(remote_control_url).map_err(map_url_parse_error)?;
    if !remote_control_url.path().ends_with('/') {
        let normalized_path = format!("{}/", remote_control_url.path());
        remote_control_url.set_path(&normalized_path);
    }

    let host = remote_control_url.host();
    match remote_control_url.scheme() {
        "https" if is_localhost(&host) || is_allowed_remote_control_chatgpt_host(&host) => {}
        "http" if is_localhost(&host) => {}
        _ => return Err(map_scheme_error(())),
    }

    Ok(remote_control_url)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn normalize_remote_control_url_accepts_chatgpt_https_urls() {
        assert_eq!(
            normalize_remote_control_url("https://chatgpt.com/backend-api")
                .expect("chatgpt.com URL should normalize"),
            RemoteControlTarget {
                websocket_url: "wss://chatgpt.com/backend-api/wham/remote/control/server"
                    .to_string(),
                enroll_url: "https://chatgpt.com/backend-api/wham/remote/control/server/enroll"
                    .to_string(),
                refresh_url: "https://chatgpt.com/backend-api/wham/remote/control/server/refresh"
                    .to_string(),
                pair_url: "https://chatgpt.com/backend-api/wham/remote/control/server/pair"
                    .to_string(),
            }
        );
        assert_eq!(
            normalize_remote_control_url("https://api.chatgpt-staging.com/backend-api")
                .expect("chatgpt-staging.com subdomain URL should normalize"),
            RemoteControlTarget {
                websocket_url:
                    "wss://api.chatgpt-staging.com/backend-api/wham/remote/control/server"
                        .to_string(),
                enroll_url:
                    "https://api.chatgpt-staging.com/backend-api/wham/remote/control/server/enroll"
                        .to_string(),
                refresh_url:
                    "https://api.chatgpt-staging.com/backend-api/wham/remote/control/server/refresh"
                        .to_string(),
                pair_url:
                    "https://api.chatgpt-staging.com/backend-api/wham/remote/control/server/pair"
                        .to_string(),
            }
        );
    }

    #[test]
    fn normalize_remote_control_url_accepts_localhost_urls() {
        assert_eq!(
            normalize_remote_control_url("http://localhost:8080/backend-api")
                .expect("localhost http URL should normalize"),
            RemoteControlTarget {
                websocket_url: "ws://localhost:8080/backend-api/wham/remote/control/server"
                    .to_string(),
                enroll_url: "http://localhost:8080/backend-api/wham/remote/control/server/enroll"
                    .to_string(),
                refresh_url: "http://localhost:8080/backend-api/wham/remote/control/server/refresh"
                    .to_string(),
                pair_url: "http://localhost:8080/backend-api/wham/remote/control/server/pair"
                    .to_string(),
            }
        );
        assert_eq!(
            normalize_remote_control_url("https://localhost:8443/backend-api")
                .expect("localhost https URL should normalize"),
            RemoteControlTarget {
                websocket_url: "wss://localhost:8443/backend-api/wham/remote/control/server"
                    .to_string(),
                enroll_url: "https://localhost:8443/backend-api/wham/remote/control/server/enroll"
                    .to_string(),
                refresh_url:
                    "https://localhost:8443/backend-api/wham/remote/control/server/refresh"
                        .to_string(),
                pair_url: "https://localhost:8443/backend-api/wham/remote/control/server/pair"
                    .to_string(),
            }
        );
    }

    #[test]
    fn normalize_remote_control_url_rejects_unsupported_urls() {
        for remote_control_url in [
            "http://chatgpt.com/backend-api",
            "http://example.com/backend-api",
            "https://example.com/backend-api",
            "https://chat.openai.com/backend-api",
            "https://chatgpt.com.evil.com/backend-api",
            "https://evilchatgpt.com/backend-api",
            "https://foo.localhost/backend-api",
        ] {
            let err = normalize_remote_control_url(remote_control_url)
                .expect_err("unsupported URL should be rejected");

            assert_eq!(err.kind(), ErrorKind::InvalidInput);
            assert_eq!(
                err.to_string(),
                format!(
                    "invalid remote control URL `{remote_control_url}`; expected HTTPS URL for chatgpt.com or chatgpt-staging.com, or HTTP/HTTPS URL for localhost"
                )
            );
        }
    }
}

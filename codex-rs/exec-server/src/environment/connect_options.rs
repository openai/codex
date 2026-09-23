//! Host-owned WebSocket connection options whose trusted headers remain private and redacted.

use std::collections::HashMap;
use std::time::Duration;

use http::HeaderMap;
use http::HeaderName;
use http::HeaderValue;

use crate::ExecServerError;
use crate::client_api::DEFAULT_REMOTE_EXEC_SERVER_CONNECT_TIMEOUT;
use crate::client_api::DEFAULT_REMOTE_EXEC_SERVER_INITIALIZE_TIMEOUT;
use crate::client_api::ExecServerTransportParams;
use crate::environment_provider::normalize_exec_server_url;

/// Host-owned connection settings for a named remote execution environment.
///
/// Headers are sent only on the direct WebSocket upgrade request and subsequent
/// reconnects. The embedding host must derive them from trusted request or
/// session context, including app-server environment registration or environment configuration.
#[derive(Clone, Eq, PartialEq)]
pub struct RemoteEnvironmentOptions {
    /// Direct `ws://` or `wss://` endpoint for the remote environment.
    pub exec_server_url: String,
    /// Optional connection timeout; the standard exec-server default applies when omitted.
    pub connect_timeout: Option<Duration>,
    /// Additional trusted headers for every physical WebSocket connection.
    pub http_headers: HashMap<String, String>,
}

impl std::fmt::Debug for RemoteEnvironmentOptions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RemoteEnvironmentOptions")
            .field("exec_server_url", &self.exec_server_url)
            .field("connect_timeout", &self.connect_timeout)
            .field("http_headers", &"<redacted>")
            .finish()
    }
}

impl RemoteEnvironmentOptions {
    pub(crate) fn into_transport_params(
        self,
    ) -> Result<ExecServerTransportParams, ExecServerError> {
        let (exec_server_url, disabled) = normalize_exec_server_url(Some(self.exec_server_url));
        if disabled {
            return Err(ExecServerError::Protocol(
                "remote environment cannot use disabled exec-server url".to_string(),
            ));
        }
        let exec_server_url = exec_server_url.ok_or_else(|| {
            ExecServerError::Protocol("remote environment requires an exec-server url".to_string())
        })?;

        if !self.http_headers.is_empty() {
            let url = url::Url::parse(&exec_server_url).map_err(|error| {
                ExecServerError::Protocol(format!("invalid exec-server WebSocket URL: {error}"))
            })?;
            let is_loopback = match url.host() {
                Some(url::Host::Domain(host)) => host.eq_ignore_ascii_case("localhost"),
                Some(url::Host::Ipv4(address)) => address.is_loopback(),
                Some(url::Host::Ipv6(address)) => address.is_loopback(),
                None => false,
            };
            if url.scheme() != "wss" && !is_loopback {
                return Err(ExecServerError::Protocol(
                    "exec-server WebSocket headers require wss:// or a loopback destination"
                        .to_string(),
                ));
            }
        }

        let mut http_headers = HeaderMap::with_capacity(self.http_headers.len());
        for (name, value) in self.http_headers {
            let header_name = HeaderName::from_bytes(name.as_bytes()).map_err(|_| {
                ExecServerError::Protocol(format!(
                    "invalid exec-server WebSocket header name `{name}`"
                ))
            })?;
            if matches!(
                header_name.as_str(),
                "connection" | "content-length" | "host" | "transfer-encoding" | "upgrade"
            ) || header_name.as_str().starts_with("sec-websocket-")
            {
                return Err(ExecServerError::Protocol(format!(
                    "exec-server WebSocket header `{header_name}` is controlled by the connection"
                )));
            }
            let mut header_value = HeaderValue::from_str(&value).map_err(|_| {
                ExecServerError::Protocol(format!(
                    "invalid value for exec-server WebSocket header `{header_name}`"
                ))
            })?;
            if http_headers.contains_key(&header_name) {
                return Err(ExecServerError::Protocol(format!(
                    "duplicate exec-server WebSocket header `{header_name}`"
                )));
            }
            header_value.set_sensitive(true);
            http_headers.insert(header_name, header_value);
        }

        Ok(ExecServerTransportParams::WebSocketUrl {
            websocket_url: exec_server_url,
            connect_timeout: self
                .connect_timeout
                .unwrap_or(DEFAULT_REMOTE_EXEC_SERVER_CONNECT_TIMEOUT),
            initialize_timeout: DEFAULT_REMOTE_EXEC_SERVER_INITIALIZE_TIMEOUT,
            http_headers,
        })
    }
}

#[cfg(test)]
#[path = "connect_options_tests.rs"]
mod tests;

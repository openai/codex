use std::fmt;
use std::sync::Arc;
use std::time::Duration;

#[derive(Clone, Debug)]
pub enum ExecServerError {
    Spawn(Arc<std::io::Error>),
    WebSocketConnectTimeout {
        url: String,
        timeout: Duration,
    },
    WebSocketConnect {
        url: String,
        source: Arc<tokio_tungstenite::tungstenite::Error>,
    },
    InitializeTimedOut {
        timeout: Duration,
    },
    Closed,
    Disconnected(String),
    Json(Arc<serde_json::Error>),
    HttpRequest(String),
    Protocol(String),
    Server {
        code: i64,
        message: String,
    },
    EnvironmentRegistryHttp {
        status: reqwest::StatusCode,
        code: Option<String>,
        message: String,
    },
    EnvironmentRegistryConfig(String),
    EnvironmentRegistryAuth(String),
    EnvironmentRegistryRequest(Arc<reqwest::Error>),
}

impl fmt::Display for ExecServerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Spawn(error) => write!(f, "failed to spawn exec-server: {error}"),
            Self::WebSocketConnectTimeout { url, timeout } => write!(
                f,
                "timed out connecting to exec-server websocket `{url}` after {timeout:?}"
            ),
            Self::WebSocketConnect { url, source } => {
                write!(
                    f,
                    "failed to connect to exec-server websocket `{url}`: {source}"
                )
            }
            Self::InitializeTimedOut { timeout } => write!(
                f,
                "timed out waiting for exec-server initialize handshake after {timeout:?}"
            ),
            Self::Closed => f.write_str("exec-server transport closed"),
            Self::Disconnected(message) => f.write_str(message),
            Self::Json(error) => {
                write!(
                    f,
                    "failed to serialize or deserialize exec-server JSON: {error}"
                )
            }
            Self::HttpRequest(message) => write!(f, "HTTP request failed: {message}"),
            Self::Protocol(message) => write!(f, "exec-server protocol error: {message}"),
            Self::Server { code, message } => {
                write!(f, "exec-server rejected request ({code}): {message}")
            }
            Self::EnvironmentRegistryHttp {
                status,
                code,
                message,
            } => {
                write!(f, "environment registry request failed ({status}")?;
                if let Some(code) = code {
                    write!(f, ", {code}")?;
                }
                write!(f, "): {message}")
            }
            Self::EnvironmentRegistryConfig(message) => {
                write!(f, "environment registry configuration error: {message}")
            }
            Self::EnvironmentRegistryAuth(message) => {
                write!(f, "environment registry authentication error: {message}")
            }
            Self::EnvironmentRegistryRequest(error) => {
                write!(f, "environment registry request failed: {error}")
            }
        }
    }
}

impl std::error::Error for ExecServerError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Spawn(error) => Some(error.as_ref()),
            Self::WebSocketConnect { source, .. } => Some(source.as_ref()),
            Self::Json(error) => Some(error.as_ref()),
            Self::EnvironmentRegistryRequest(error) => Some(error.as_ref()),
            Self::WebSocketConnectTimeout { .. }
            | Self::InitializeTimedOut { .. }
            | Self::Closed
            | Self::Disconnected(_)
            | Self::HttpRequest(_)
            | Self::Protocol(_)
            | Self::Server { .. }
            | Self::EnvironmentRegistryHttp { .. }
            | Self::EnvironmentRegistryConfig(_)
            | Self::EnvironmentRegistryAuth(_) => None,
        }
    }
}

impl From<serde_json::Error> for ExecServerError {
    fn from(error: serde_json::Error) -> Self {
        Self::Json(Arc::new(error))
    }
}

impl From<reqwest::Error> for ExecServerError {
    fn from(error: reqwest::Error) -> Self {
        Self::EnvironmentRegistryRequest(Arc::new(error))
    }
}

use crate::state::NetworkProxyState;
use rama_core::Service;
use rama_core::error::BoxError;
use rama_core::extensions::ExtensionsMut;
use rama_tcp::TcpStream;
use std::io;
use std::io::Write;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::AsyncReadExt;

/// Internal handoff from the trusted Linux proxy bridge.
#[doc(hidden)]
pub const PROXY_ATTRIBUTION_TOKEN_ENV_KEY: &str = "CODEX_NETWORK_PROXY_ATTRIBUTION";

const ATTRIBUTION_FRAME_MAGIC: &[u8; 8] = b"\0CDXPXY1";
const MAX_ATTRIBUTION_TOKEN_LEN: usize = 128;
const ATTRIBUTION_FRAME_TIMEOUT: Duration = Duration::from_secs(3);

pub(crate) struct BindConnectionAttribution<S> {
    inner: S,
    state: Arc<NetworkProxyState>,
    environment_id: Option<String>,
}

impl<S> BindConnectionAttribution<S> {
    pub(crate) fn new(
        inner: S,
        state: Arc<NetworkProxyState>,
        environment_id: Option<String>,
    ) -> Self {
        Self {
            inner,
            state,
            environment_id,
        }
    }
}

impl<S> Service<TcpStream> for BindConnectionAttribution<S>
where
    S: Service<TcpStream>,
    S::Error: Into<BoxError>,
{
    type Output = S::Output;
    type Error = BoxError;

    async fn serve(&self, mut stream: TcpStream) -> Result<Self::Output, Self::Error> {
        let state = match read_attribution_token(&mut stream).await? {
            Some(token) => self.state.for_execution_token(&token).ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "unknown network proxy attribution token",
                )
            })?,
            None => self.state.as_ref().clone(),
        };
        if let Some(expected_environment_id) = self.environment_id.as_deref()
            && state
                .environment_id()
                .is_some_and(|actual| actual != expected_environment_id)
        {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "network proxy attribution environment mismatch",
            )
            .into());
        }
        stream.extensions_mut().insert(Arc::new(state));
        self.inner.serve(stream).await.map_err(Into::into)
    }
}

async fn read_attribution_token(stream: &mut TcpStream) -> Result<Option<String>, BoxError> {
    let mut marker = [0_u8; 1];
    let read = stream.stream.peek(&mut marker).await?;
    if read == 0 {
        return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "empty proxy connection").into());
    }
    if marker[0] != ATTRIBUTION_FRAME_MAGIC[0] {
        return Ok(None);
    }

    let token = tokio::time::timeout(ATTRIBUTION_FRAME_TIMEOUT, async {
        let mut magic = [0_u8; ATTRIBUTION_FRAME_MAGIC.len()];
        stream.read_exact(&mut magic).await?;
        if &magic != ATTRIBUTION_FRAME_MAGIC {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "invalid network proxy attribution frame",
            ));
        }

        let token_len = stream.read_u16().await? as usize;
        if token_len == 0 || token_len > MAX_ATTRIBUTION_TOKEN_LEN {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "invalid network proxy attribution token length",
            ));
        }
        let mut token = vec![0_u8; token_len];
        stream.read_exact(&mut token).await?;
        String::from_utf8(token).map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "network proxy attribution token is not UTF-8",
            )
        })
    })
    .await
    .map_err(|_| {
        io::Error::new(
            io::ErrorKind::TimedOut,
            "network proxy attribution frame timed out",
        )
    })??;

    Ok(Some(token))
}

/// Writes the trusted bridge preface consumed by the shared proxy ingress.
#[doc(hidden)]
pub fn write_attribution_frame(writer: &mut impl Write, token: &str) -> io::Result<()> {
    if token.is_empty() || token.len() > MAX_ATTRIBUTION_TOKEN_LEN {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "invalid network proxy attribution token length",
        ));
    }
    let token_len = u16::try_from(token.len()).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "network proxy attribution token is too long",
        )
    })?;
    writer.write_all(ATTRIBUTION_FRAME_MAGIC)?;
    writer.write_all(&token_len.to_be_bytes())?;
    writer.write_all(token.as_bytes())
}

#[cfg(test)]
#[path = "attribution_tests.rs"]
mod tests;

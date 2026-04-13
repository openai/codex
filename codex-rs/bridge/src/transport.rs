use async_trait::async_trait;

use crate::BridgeEnvelope;
use crate::BridgeError;
use crate::BridgeFrame;
use crate::BridgeResult;

/// Transport used by [`crate::BridgeClient`] to send one complete bridge frame.
#[async_trait]
pub trait BridgeTransport: Send + Sync {
    /// Sends a request frame and returns the matching response envelope.
    async fn call(&self, request: BridgeFrame) -> BridgeResult<BridgeEnvelope>;
}

#[cfg(unix)]
mod unix {
    use std::path::PathBuf;

    use tokio::io::AsyncReadExt;
    use tokio::io::AsyncWriteExt;
    use tokio::net::UnixStream;

    use super::*;

    const MAX_FRAME_BYTES: usize = 128 * 1024 * 1024;

    /// Unix domain socket bridge transport.
    #[derive(Clone, Debug)]
    pub struct UnixSocketBridgeTransport {
        path: PathBuf,
    }

    impl UnixSocketBridgeTransport {
        /// Create a transport that opens a fresh Unix socket connection for each call.
        pub fn new(path: PathBuf) -> Self {
            Self { path }
        }
    }

    #[async_trait]
    impl BridgeTransport for UnixSocketBridgeTransport {
        async fn call(&self, request: BridgeFrame) -> BridgeResult<BridgeEnvelope> {
            let mut stream = UnixStream::connect(self.path.as_path())
                .await
                .map_err(transport_error)?;
            write_msgpack(&mut stream, &request).await?;
            let response = read_msgpack(&mut stream).await?;
            Ok(response)
        }
    }

    async fn write_msgpack<T>(stream: &mut UnixStream, value: &T) -> BridgeResult<()>
    where
        T: serde::Serialize,
    {
        let bytes = rmp_serde::to_vec_named(value).map_err(|err| BridgeError::Codec {
            message: err.to_string(),
        })?;
        let len = u32::try_from(bytes.len()).map_err(|_| BridgeError::InvalidResponse {
            message: format!("bridge request frame is too large: {} bytes", bytes.len()),
        })?;
        stream
            .write_all(&len.to_be_bytes())
            .await
            .map_err(transport_error)?;
        stream.write_all(&bytes).await.map_err(transport_error)?;
        stream.flush().await.map_err(transport_error)?;
        Ok(())
    }

    async fn read_msgpack<T>(stream: &mut UnixStream) -> BridgeResult<T>
    where
        T: serde::de::DeserializeOwned,
    {
        let mut len_bytes = [0u8; 4];
        stream
            .read_exact(&mut len_bytes)
            .await
            .map_err(transport_error)?;
        let len = u32::from_be_bytes(len_bytes) as usize;
        if len > MAX_FRAME_BYTES {
            return Err(BridgeError::InvalidResponse {
                message: format!("bridge response frame is too large: {len} bytes"),
            });
        }
        let mut bytes = vec![0u8; len];
        stream
            .read_exact(&mut bytes)
            .await
            .map_err(transport_error)?;
        rmp_serde::from_slice(bytes.as_slice()).map_err(|err| BridgeError::Codec {
            message: err.to_string(),
        })
    }

    fn transport_error(err: std::io::Error) -> BridgeError {
        BridgeError::Transport {
            message: err.to_string(),
        }
    }
}

#[cfg(unix)]
pub use unix::UnixSocketBridgeTransport;

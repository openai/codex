use std::io;
use std::mem::size_of;

use serde::Serialize;
use serde::de::DeserializeOwned;
use tokio::io::AsyncRead;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWrite;
use tokio::io::AsyncWriteExt;

/// Maximum JSON payload size accepted for one IPC frame.
pub const MAX_FRAME_BYTES: usize = 64 * 1024 * 1024;

/// A serialized IPC frame that has already passed the payload size limit.
#[derive(Clone, Debug)]
pub struct EncodedFrame {
    payload: Vec<u8>,
}

impl EncodedFrame {
    pub fn encode<T>(message: &T) -> io::Result<Self>
    where
        T: Serialize,
    {
        let payload = serde_json::to_vec(message).map_err(|err| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("failed to encode code-mode IPC frame: {err}"),
            )
        })?;
        if payload.len() > MAX_FRAME_BYTES {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "code-mode IPC frame length {} exceeds {MAX_FRAME_BYTES} bytes",
                    payload.len()
                ),
            ));
        }
        Ok(Self { payload })
    }
}

/// Decodes JSON messages prefixed by a four-byte little-endian payload length.
pub struct FramedReader<R> {
    reader: R,
}

impl<R> FramedReader<R>
where
    R: AsyncRead + Unpin,
{
    pub fn new(reader: R) -> Self {
        Self { reader }
    }

    /// Reads the next frame, returning `None` only for EOF at a frame boundary.
    pub async fn read<T>(&mut self) -> io::Result<Option<T>>
    where
        T: DeserializeOwned,
    {
        let mut length_bytes = [0_u8; size_of::<u32>()];
        if self.reader.read(&mut length_bytes[..1]).await? == 0 {
            return Ok(None);
        }
        self.reader.read_exact(&mut length_bytes[1..]).await?;

        let length = u32::from_le_bytes(length_bytes) as usize;
        if length > MAX_FRAME_BYTES {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("code-mode IPC frame length {length} exceeds {MAX_FRAME_BYTES} bytes"),
            ));
        }

        let mut payload = vec![0; length];
        self.reader.read_exact(&mut payload).await?;
        serde_json::from_slice(&payload).map(Some).map_err(|err| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("failed to decode code-mode IPC frame: {err}"),
            )
        })
    }
}

/// Encodes JSON messages with a four-byte little-endian payload length.
pub struct FramedWriter<W> {
    writer: W,
}

impl<W> FramedWriter<W>
where
    W: AsyncWrite + Unpin,
{
    pub fn new(writer: W) -> Self {
        Self { writer }
    }

    /// Writes and flushes one complete frame.
    pub async fn write<T>(&mut self, message: &T) -> io::Result<()>
    where
        T: Serialize,
    {
        self.write_frame(&EncodedFrame::encode(message)?).await
    }

    /// Writes and flushes a frame encoded before it entered an I/O queue.
    pub async fn write_frame(&mut self, frame: &EncodedFrame) -> io::Result<()> {
        let length = u32::try_from(frame.payload.len()).map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "code-mode IPC frame length exceeds u32",
            )
        })?;

        self.writer.write_all(&length.to_le_bytes()).await?;
        self.writer.write_all(&frame.payload).await?;
        self.writer.flush().await
    }
}

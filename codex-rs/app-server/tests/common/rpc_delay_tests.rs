use std::collections::VecDeque;
use std::io;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::task::Context;
use std::task::Poll;
use std::time::Duration;

use pretty_assertions::assert_eq;
use tokio::io::AsyncRead;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWrite;
use tokio::io::AsyncWriteExt;
use tokio::io::ReadBuf;
use tokio::io::duplex;
use tokio::net::TcpListener;
use tokio::net::TcpStream;
use tokio::time::advance;
use tokio::time::timeout;

use super::WebsocketDelayInterposer;
use super::forward_direction;
use super::websocket_authority;

#[tokio::test(start_paused = true)]
async fn delays_then_flushes_eof() -> anyhow::Result<()> {
    let delay = Duration::from_millis(15);
    let (mut input_writer, input_reader) = duplex(/*max_buf_size*/ 64);
    let (output_writer, mut output_reader) = duplex(/*max_buf_size*/ 64);
    let forward_task = tokio::spawn(forward_direction(input_reader, output_writer, delay));

    input_writer.write_all(b"payload").await?;
    input_writer.shutdown().await?;
    tokio::task::yield_now().await;

    let mut before_delay = [0; 1];
    assert!(
        timeout(Duration::ZERO, output_reader.read(&mut before_delay))
            .await
            .is_err()
    );

    advance(delay).await;
    let mut output = Vec::new();
    output_reader.read_to_end(&mut output).await?;
    assert_eq!(output, b"payload");
    forward_task.await??;
    Ok(())
}

#[tokio::test(start_paused = true)]
async fn burst_chunks_share_one_deadline() -> anyhow::Result<()> {
    let delay = Duration::from_millis(15);
    let (mut input_writer, input_reader) = duplex(/*max_buf_size*/ 1);
    let (output_writer, mut output_reader) = duplex(/*max_buf_size*/ 64);
    let forward_task = tokio::spawn(forward_direction(input_reader, output_writer, delay));

    input_writer.write_all(b"ab").await?;
    input_writer.shutdown().await?;
    tokio::task::yield_now().await;

    advance(delay).await;
    tokio::task::yield_now().await;
    let mut output = [0; 2];
    timeout(Duration::ZERO, output_reader.read_exact(&mut output)).await??;
    assert_eq!(output, *b"ab");
    forward_task.await??;
    Ok(())
}

#[tokio::test]
async fn write_error_cancels_reader() {
    let reader_dropped = Arc::new(AtomicBool::new(false));
    let reader = PendingAfterChunkReader::new(Arc::clone(&reader_dropped));

    let error = forward_direction(reader, FailingWriter, Duration::ZERO)
        .await
        .expect_err("failing writer should fail forwarding");
    assert_eq!(error.kind(), io::ErrorKind::BrokenPipe);

    for _ in 0..10 {
        if reader_dropped.load(Ordering::Acquire) {
            break;
        }
        tokio::task::yield_now().await;
    }
    assert!(reader_dropped.load(Ordering::Acquire));
}

#[tokio::test]
async fn zero_delay_loopback_forwards_and_closes_active_sockets() -> anyhow::Result<()> {
    let upstream_listener = TcpListener::bind("127.0.0.1:0").await?;
    let upstream_url = format!("ws://{}", upstream_listener.local_addr()?);
    let upstream_task = tokio::spawn(async move {
        let (mut upstream, _) = upstream_listener.accept().await?;
        let mut request = [0; 5];
        upstream.read_exact(&mut request).await?;
        assert_eq!(request, *b"hello");
        upstream.write_all(b"world").await?;

        let mut after_drop = [0; 1];
        let read = timeout(Duration::from_secs(1), upstream.read(&mut after_drop)).await??;
        Ok::<usize, anyhow::Error>(read)
    });

    let interposer = WebsocketDelayInterposer::start(&upstream_url, Duration::ZERO).await?;
    let mut downstream =
        TcpStream::connect(websocket_authority(interposer.websocket_url())?).await?;
    downstream.write_all(b"hello").await?;
    let mut response = [0; 5];
    downstream.read_exact(&mut response).await?;
    assert_eq!(response, *b"world");

    drop(interposer);
    tokio::task::yield_now().await;

    let mut after_drop = [0; 1];
    let read = timeout(Duration::from_secs(1), downstream.read(&mut after_drop)).await??;
    assert_eq!(read, 0);
    assert_eq!(upstream_task.await??, 0);
    Ok(())
}

struct PendingAfterChunkReader {
    chunks: VecDeque<Vec<u8>>,
    dropped: Arc<AtomicBool>,
}

impl PendingAfterChunkReader {
    fn new(dropped: Arc<AtomicBool>) -> Self {
        Self {
            chunks: VecDeque::from([b"x".to_vec()]),
            dropped,
        }
    }
}

impl AsyncRead for PendingAfterChunkReader {
    fn poll_read(
        mut self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let Some(chunk) = self.chunks.pop_front() else {
            return Poll::Pending;
        };
        buf.put_slice(&chunk);
        Poll::Ready(Ok(()))
    }
}

impl Drop for PendingAfterChunkReader {
    fn drop(&mut self) {
        self.dropped.store(true, Ordering::Release);
    }
}

struct FailingWriter;

impl AsyncWrite for FailingWriter {
    fn poll_write(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        _buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        Poll::Ready(Err(io::Error::new(
            io::ErrorKind::BrokenPipe,
            "synthetic write failure",
        )))
    }

    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }

    fn poll_shutdown(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }
}

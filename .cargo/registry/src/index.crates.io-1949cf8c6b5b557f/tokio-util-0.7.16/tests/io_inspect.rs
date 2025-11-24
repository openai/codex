use std::{
    future::poll_fn,
    io::IoSlice,
    pin::Pin,
    task::{Context, Poll},
};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, ReadBuf};
use tokio_util::io::{InspectReader, InspectWriter};

/// An AsyncRead implementation that works byte-by-byte, to catch out callers
/// who don't allow for `buf` being part-filled before the call
struct SmallReader {
    contents: Vec<u8>,
}

impl Unpin for SmallReader {}

impl AsyncRead for SmallReader {
    fn poll_read(
        mut self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        if let Some(byte) = self.contents.pop() {
            buf.put_slice(&[byte])
        }
        Poll::Ready(Ok(()))
    }
}

#[tokio::test]
async fn read_tee() {
    let contents = b"This could be really long, you know".to_vec();
    let reader = SmallReader {
        contents: contents.clone(),
    };
    let mut altout: Vec<u8> = Vec::new();
    let mut teeout = Vec::new();
    {
        let mut tee = InspectReader::new(reader, |bytes| altout.extend(bytes));
        tee.read_to_end(&mut teeout).await.unwrap();
    }
    assert_eq!(teeout, altout);
    assert_eq!(altout.len(), contents.len());
}

/// An AsyncWrite implementation that works byte-by-byte for poll_write, and
/// that reads the whole of the first buffer plus one byte from the second in
/// poll_write_vectored.
///
/// This is designed to catch bugs in handling partially written buffers
#[derive(Debug)]
struct SmallWriter {
    contents: Vec<u8>,
}

impl Unpin for SmallWriter {}

impl AsyncWrite for SmallWriter {
    fn poll_write(
        mut self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, std::io::Error>> {
        // Just write one byte at a time
        if buf.is_empty() {
            return Poll::Ready(Ok(0));
        }
        self.contents.push(buf[0]);
        Poll::Ready(Ok(1))
    }

    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Result<(), std::io::Error>> {
        Poll::Ready(Ok(()))
    }

    fn poll_shutdown(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
    ) -> Poll<Result<(), std::io::Error>> {
        Poll::Ready(Ok(()))
    }

    fn poll_write_vectored(
        mut self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        bufs: &[IoSlice<'_>],
    ) -> Poll<Result<usize, std::io::Error>> {
        // Write all of the first buffer, then one byte from the second buffer
        // This should trip up anything that doesn't correctly handle multiple
        // buffers.
        if bufs.is_empty() {
            return Poll::Ready(Ok(0));
        }
        let mut written_len = bufs[0].len();
        self.contents.extend_from_slice(&bufs[0]);

        if bufs.len() > 1 {
            let buf = bufs[1];
            if !buf.is_empty() {
                written_len += 1;
                self.contents.push(buf[0]);
            }
        }
        Poll::Ready(Ok(written_len))
    }

    fn is_write_vectored(&self) -> bool {
        true
    }
}

#[tokio::test]
async fn write_tee() {
    let mut altout: Vec<u8> = Vec::new();
    let mut writeout = SmallWriter {
        contents: Vec::new(),
    };
    {
        let mut tee = InspectWriter::new(&mut writeout, |bytes| altout.extend(bytes));
        tee.write_all(b"A testing string, very testing")
            .await
            .unwrap();
    }
    assert_eq!(altout, writeout.contents);
}

// This is inefficient, but works well enough for test use.
// If you want something similar for real code, you'll want to avoid all the
// fun of manipulating `bufs` - ideally, by the time you read this,
// IoSlice::advance_slices will be stable, and you can use that.
async fn write_all_vectored<W: AsyncWrite + Unpin>(
    mut writer: W,
    mut bufs: Vec<Vec<u8>>,
) -> Result<usize, std::io::Error> {
    let mut res = 0;
    while !bufs.is_empty() {
        let mut written = poll_fn(|cx| {
            let bufs: Vec<IoSlice> = bufs.iter().map(|v| IoSlice::new(v)).collect();
            Pin::new(&mut writer).poll_write_vectored(cx, &bufs)
        })
        .await?;
        res += written;
        while written > 0 {
            let buf_len = bufs[0].len();
            if buf_len <= written {
                bufs.remove(0);
                written -= buf_len;
            } else {
                let buf = &mut bufs[0];
                let drain_len = written.min(buf.len());
                buf.drain(..drain_len);
                written -= drain_len;
            }
        }
    }
    Ok(res)
}

#[tokio::test]
async fn write_tee_vectored() {
    let mut altout: Vec<u8> = Vec::new();
    let mut writeout = SmallWriter {
        contents: Vec::new(),
    };
    let original = b"A very long string split up";
    let bufs: Vec<Vec<u8>> = original
        .split(|b| b.is_ascii_whitespace())
        .map(Vec::from)
        .collect();
    assert!(bufs.len() > 1);
    let expected: Vec<u8> = {
        let mut out = Vec::new();
        for item in &bufs {
            out.extend_from_slice(item)
        }
        out
    };
    {
        let mut bufcount = 0;
        let tee = InspectWriter::new(&mut writeout, |bytes| {
            bufcount += 1;
            altout.extend(bytes)
        });

        assert!(tee.is_write_vectored());

        write_all_vectored(tee, bufs.clone()).await.unwrap();

        assert!(bufcount >= bufs.len());
    }
    assert_eq!(altout, writeout.contents);
    assert_eq!(writeout.contents, expected);
}

#![warn(rust_2018_idioms)]

use bytes::Bytes;
use futures_util::SinkExt;
use std::io::{self, Error, ErrorKind};
use tokio::io::AsyncWriteExt;
use tokio_util::codec::{Encoder, FramedWrite};
use tokio_util::io::{CopyToBytes, SinkWriter};
use tokio_util::sync::PollSender;

#[tokio::test]
async fn test_copied_sink_writer() -> Result<(), Error> {
    // Construct a channel pair to send data across and wrap a pollable sink.
    // Note that the sink must mimic a writable object, e.g. have `std::io::Error`
    // as its error type.
    // As `PollSender` requires an owned copy of the buffer, we wrap it additionally
    // with a `CopyToBytes` helper.
    let (tx, mut rx) = tokio::sync::mpsc::channel::<Bytes>(1);
    let mut writer = SinkWriter::new(CopyToBytes::new(
        PollSender::new(tx).sink_map_err(|_| io::Error::from(ErrorKind::BrokenPipe)),
    ));

    // Write data to our interface...
    let data: [u8; 4] = [1, 2, 3, 4];
    let _ = writer.write(&data).await;

    // ... and receive it.
    assert_eq!(data.to_vec(), rx.recv().await.unwrap().to_vec());

    Ok(())
}

/// A trivial encoder.
struct SliceEncoder;

impl SliceEncoder {
    fn new() -> Self {
        Self {}
    }
}

impl<'a> Encoder<&'a [u8]> for SliceEncoder {
    type Error = Error;

    fn encode(&mut self, item: &'a [u8], dst: &mut bytes::BytesMut) -> Result<(), Self::Error> {
        // This is where we'd write packet headers, lengths, etc. in a real encoder.
        // For simplicity and demonstration purposes, we just pack a copy of
        // the slice at the end of a buffer.
        dst.extend_from_slice(item);
        Ok(())
    }
}

#[tokio::test]
async fn test_direct_sink_writer() -> Result<(), Error> {
    // We define a framed writer which accepts byte slices
    // and 'reverse' this construction immediately.
    let framed_byte_lc = FramedWrite::new(Vec::new(), SliceEncoder::new());
    let mut writer = SinkWriter::new(framed_byte_lc);

    // Write multiple slices to the sink...
    let _ = writer.write(&[1, 2, 3]).await;
    let _ = writer.write(&[4, 5, 6]).await;

    // ... and compare it with the buffer.
    assert_eq!(
        writer.into_inner().write_buffer().to_vec().as_slice(),
        &[1, 2, 3, 4, 5, 6]
    );

    Ok(())
}

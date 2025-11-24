use crate::encode::IntoAsync;
use crate::error::LzwStatus;
use crate::error::StreamResult;
use crate::StreamBuf;
use std::io;

impl<'d, W: futures::io::AsyncWrite + core::marker::Unpin> IntoAsync<'d, W> {
    /// Encode data from a reader.
    ///
    /// This will drain the supplied reader. It will not encode an end marker after all data has
    /// been processed.
    pub async fn encode(&mut self, read: impl futures::io::AsyncBufRead) -> StreamResult {
        self.encode_part(read, false).await
    }

    /// Encode data from a reader and an end marker.
    pub async fn encode_all(mut self, read: impl futures::io::AsyncBufRead) -> StreamResult {
        self.encode_part(read, true).await
    }

    /// Set the size of the intermediate decode buffer.
    ///
    /// A buffer of this size is allocated to hold one part of the decoded stream when no buffer is
    /// available and any decoding method is called. No buffer is allocated if `set_buffer` has
    /// been called. The buffer is reused.
    ///
    /// # Panics
    /// This method panics if `size` is `0`.
    pub fn set_buffer_size(&mut self, size: usize) {
        assert_ne!(size, 0, "Attempted to set empty buffer");
        self.default_size = size;
    }

    /// Use a particular buffer as an intermediate decode buffer.
    ///
    /// Calling this sets or replaces the buffer. When a buffer has been set then it is used
    /// instead of dynamically allocating a buffer. Note that the size of the buffer is critical
    /// for efficient decoding. Some optimization techniques require the buffer to hold one or more
    /// previous decoded words. There is also additional overhead from `write` calls each time the
    /// buffer has been filled.
    ///
    /// # Panics
    /// This method panics if the `buffer` is empty.
    pub fn set_buffer(&mut self, buffer: &'d mut [u8]) {
        assert_ne!(buffer.len(), 0, "Attempted to set empty buffer");
        self.buffer = Some(StreamBuf::Borrowed(buffer));
    }

    async fn encode_part(
        &mut self,
        read: impl futures::io::AsyncBufRead,
        finish: bool,
    ) -> StreamResult {
        use futures::io::AsyncBufReadExt;
        use futures::io::AsyncWriteExt;

        let IntoAsync {
            encoder,
            writer,
            buffer,
            default_size,
        } = self;

        futures::pin_mut!(read);
        let mut read: core::pin::Pin<_> = read;

        let mut bytes_read = 0;
        let mut bytes_written = 0;

        // Converting to mutable refs to move into the `once` closure.
        let read_bytes = &mut bytes_read;
        let write_bytes = &mut bytes_written;

        let outbuf: &mut [u8] =
            match { buffer.get_or_insert_with(|| StreamBuf::Owned(vec![0u8; *default_size])) } {
                StreamBuf::Borrowed(slice) => &mut *slice,
                StreamBuf::Owned(vec) => &mut *vec,
            };
        assert!(!outbuf.is_empty());

        let status = loop {
            // Try to grab one buffer of input data.
            let mut filler = read.as_mut();
            let data = match filler.fill_buf().await {
                Ok(buf) => buf,
                Err(err) => break Err(err),
            };

            if data.is_empty() {
                if finish {
                    encoder.finish();
                } else {
                    break Ok(());
                }
            }

            // Decode as much of the buffer as fits.
            let result = encoder.encode_bytes(data, &mut outbuf[..]);
            // Do the bookkeeping and consume the buffer.
            *read_bytes += result.consumed_in;
            *write_bytes += result.consumed_out;
            read.as_mut().consume(result.consumed_in);

            // Handle an error status in the result.
            let done = match result.status {
                Ok(ok) => ok,
                Err(err) => {
                    break Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        &*format!("{:?}", err),
                    ));
                }
            };

            if let LzwStatus::Done = done {
                break writer.write_all(&outbuf[..result.consumed_out]).await;
            }

            if let LzwStatus::NoProgress = done {
                break Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "No more data but no end marker detected",
                ));
            }

            // And finish by writing our result.
            // TODO: we may lose data on error (also on status error above) which we might want to
            // deterministically handle so that we don't need to restart everything from scratch as
            // the only recovery strategy. Any changes welcome.
            match writer.write_all(&outbuf[..result.consumed_out]).await {
                Ok(_) => {}
                Err(err) => break Err(err),
            }
        };

        StreamResult {
            bytes_read,
            bytes_written,
            status,
        }
    }
}

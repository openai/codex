use std::collections::VecDeque;
use std::io::IoSlice;

use super::Stream;
use crate::utils::RawFdContainer;

#[derive(Debug)]
pub(super) struct WriteBuffer {
    data_buf: VecDeque<u8>,
    fd_buf: Vec<RawFdContainer>,
}

impl WriteBuffer {
    pub(super) fn new() -> Self {
        // Buffer size chosen by checking what libxcb does
        Self::with_capacity(16384)
    }

    fn with_capacity(capacity: usize) -> Self {
        Self {
            data_buf: VecDeque::with_capacity(capacity),
            fd_buf: Vec::new(),
        }
    }

    fn flush_buffer(&mut self, stream: &impl Stream) -> std::io::Result<()> {
        while self.needs_flush() {
            crate::trace!(
                "Trying to flush {} bytes of data and {} FDs",
                self.data_buf.len(),
                self.fd_buf.len()
            );
            let (data_buf_1, data_buf_2) = self.data_buf.as_slices();
            let data_bufs = [IoSlice::new(data_buf_1), IoSlice::new(data_buf_2)];
            match stream.write_vectored(&data_bufs, &mut self.fd_buf) {
                Ok(0) => {
                    if self.data_buf.is_empty() {
                        assert!(!self.fd_buf.is_empty());
                        return Err(std::io::Error::new(
                            std::io::ErrorKind::WriteZero,
                            "failed to write the buffered FDs",
                        ));
                    } else {
                        return Err(std::io::Error::new(
                            std::io::ErrorKind::WriteZero,
                            "failed to write the buffered data",
                        ));
                    }
                }
                Ok(n) => {
                    crate::trace!("Flushing wrote {} bytes of data", n);
                    let _ = self.data_buf.drain(..n);
                }
                Err(e) => return Err(e),
            }
        }
        Ok(())
    }

    fn write_helper<W: Stream, F, G>(
        &mut self,
        stream: &W,
        fds: &mut Vec<RawFdContainer>,
        write_buffer: F,
        write_inner: G,
        first_buffer: &[u8],
        to_write_length: usize,
    ) -> std::io::Result<usize>
    where
        F: FnOnce(&mut VecDeque<u8>),
        G: FnOnce(&W, &mut Vec<RawFdContainer>) -> std::io::Result<usize>,
    {
        crate::trace!(
            "Writing {} FDs and {} bytes of data",
            fds.len(),
            to_write_length
        );
        self.fd_buf.append(fds);

        // Is there enough buffer space left for this write?
        if (self.data_buf.capacity() - self.data_buf.len()) < to_write_length {
            // Not enough space, try to flush
            match self.flush_buffer(stream) {
                Ok(_) => {}
                Err(e) => {
                    if e.kind() == std::io::ErrorKind::WouldBlock {
                        let available_buf = self.data_buf.capacity() - self.data_buf.len();
                        if available_buf == 0 {
                            // Buffer filled and cannot flush anything without
                            // blocking, so return `WouldBlock`.
                            crate::trace!("Writing failed due to full buffer: {:?}", e);
                            return Err(e);
                        } else {
                            let n_to_write = first_buffer.len().min(available_buf);
                            self.data_buf.extend(&first_buffer[..n_to_write]);
                            // Return `Ok` because some or all data has been buffered,
                            // so from the outside it is seen as a successful write.
                            crate::trace!("Writing appended {} bytes to the buffer", n_to_write);
                            return Ok(n_to_write);
                        }
                    } else {
                        return Err(e);
                    }
                }
            }
        }

        if to_write_length >= self.data_buf.capacity() {
            // Write is larger than the buffer capacity, thus we just flushed the buffer. This
            // means that at this point the buffer is empty. Write directly to self.inner. No data
            // is copied into the buffer, since that would just mean that the large write gets
            // split into multiple smaller ones.
            assert!(self.data_buf.is_empty());
            crate::trace!("Large write is written directly to the stream");
            write_inner(stream, &mut self.fd_buf)
        } else {
            // At this point there is enough space available in the buffer.
            crate::trace!("Data to write is appended to the buffer");
            write_buffer(&mut self.data_buf);
            Ok(to_write_length)
        }
    }

    pub(super) fn write(
        &mut self,
        stream: &impl Stream,
        buf: &[u8],
        fds: &mut Vec<RawFdContainer>,
    ) -> std::io::Result<usize> {
        self.write_helper(
            stream,
            fds,
            |w| w.extend(buf),
            |w, fd| w.write(buf, fd),
            buf,
            buf.len(),
        )
    }

    pub(super) fn write_vectored(
        &mut self,
        stream: &impl Stream,
        bufs: &[IoSlice<'_>],
        fds: &mut Vec<RawFdContainer>,
    ) -> std::io::Result<usize> {
        let first_nonempty = bufs
            .iter()
            .find(|b| !b.is_empty())
            .map_or(&[][..], |b| &**b);
        let total_len = bufs.iter().map(|b| b.len()).sum();
        self.write_helper(
            stream,
            fds,
            |w| {
                for buf in bufs.iter() {
                    w.extend(&**buf);
                }
            },
            |w, fd| w.write_vectored(bufs, fd),
            first_nonempty,
            total_len,
        )
    }

    /// Returns `true` if there is buffered data or FDs.
    pub(super) fn needs_flush(&self) -> bool {
        !self.data_buf.is_empty() || !self.fd_buf.is_empty()
    }

    pub(super) fn flush(&mut self, stream: &impl Stream) -> std::io::Result<()> {
        self.flush_buffer(stream)
    }
}

#[cfg(test)]
mod test {
    use std::io::{Error, ErrorKind, IoSlice, Result};

    use super::super::{PollMode, Stream};
    use super::WriteBuffer;
    use crate::utils::RawFdContainer;

    struct WouldBlockWriter;

    impl Stream for WouldBlockWriter {
        fn poll(&self, _mode: PollMode) -> Result<()> {
            unimplemented!();
        }

        fn read(&self, _buf: &mut [u8], _fd_storage: &mut Vec<RawFdContainer>) -> Result<usize> {
            unimplemented!();
        }

        fn write(&self, _buf: &[u8], _fds: &mut Vec<RawFdContainer>) -> Result<usize> {
            Err(Error::new(ErrorKind::WouldBlock, "would block"))
        }
    }

    // Once upon a time, this paniced because it did bufs[0]
    #[test]
    fn empty_write() {
        let stream = WouldBlockWriter;
        let mut write_buffer = WriteBuffer::new();
        let bufs = &[];
        let _ = write_buffer
            .write_vectored(&stream, bufs, &mut Vec::new())
            .unwrap();
    }

    // Once upon a time, BufWriteFD fell back to only writing the first buffer. This could be
    // mistaken as EOF.
    #[test]
    fn incorrect_eof() {
        let stream = WouldBlockWriter;
        let mut write_buffer = WriteBuffer::with_capacity(1);
        let bufs = &[IoSlice::new(&[]), IoSlice::new(b"fooo")];
        match write_buffer.write_vectored(&stream, bufs, &mut Vec::new()) {
            Ok(0) => panic!("This looks like EOF!?"),
            Ok(_) => {}
            Err(ref e) if e.kind() == ErrorKind::WouldBlock => {}
            Err(e) => panic!("Unexpected error: {:?}", e),
        }
    }
}

//! Read X11 packets from a reader

use std::io::{Error, ErrorKind, Result};
use std::{cmp, fmt};

use super::Stream;
use crate::utils::RawFdContainer;
use x11rb_protocol::packet_reader::PacketReader as ProtoPacketReader;

/// A wrapper around a reader that reads X11 packet.
pub(crate) struct PacketReader {
    /// The read buffer to store incoming bytes in.
    read_buffer: Box<[u8]>,
    /// The inner reader that breaks these bytes into packets.
    inner: ProtoPacketReader,
}

impl fmt::Debug for PacketReader {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PacketReader")
            .field(
                "read_buffer",
                &format_args!("[buffer of size {}]", self.read_buffer.len()),
            )
            .field("inner", &self.inner)
            .finish()
    }
}

impl PacketReader {
    /// Create a new `PacketReader` that reads from the given stream.
    pub(crate) fn new() -> Self {
        Self {
            // Buffer size chosen by checking what libxcb does
            read_buffer: vec![0; 4096].into_boxed_slice(),
            inner: ProtoPacketReader::new(),
        }
    }

    /// Reads as many packets as possible from stream reader without blocking.
    pub(crate) fn try_read_packets(
        &mut self,
        stream: &impl Stream,
        out_packets: &mut Vec<Vec<u8>>,
        fd_storage: &mut Vec<RawFdContainer>,
    ) -> Result<()> {
        let original_length = out_packets.len();
        loop {
            // if the necessary packet size is larger than our buffer, just fill straight
            // into the buffer
            if self.inner.remaining_capacity() >= self.read_buffer.len() {
                crate::trace!(
                    "Trying to read large packet with {} bytes remaining",
                    self.inner.remaining_capacity()
                );
                match stream.read(self.inner.buffer(), fd_storage) {
                    Ok(0) => {
                        crate::error!("Large read returned zero");
                        return Err(Error::new(
                            ErrorKind::UnexpectedEof,
                            "The X11 server closed the connection",
                        ));
                    }
                    Ok(n) => {
                        crate::trace!("Read {} bytes directly into large packet", n);
                        if let Some(packet) = self.inner.advance(n) {
                            out_packets.push(packet);
                        }
                    }
                    Err(ref e) if e.kind() == ErrorKind::WouldBlock => break,
                    Err(e) => return Err(e),
                }
            } else {
                // read into our buffer
                let nread = match stream.read(&mut self.read_buffer, fd_storage) {
                    Ok(0) => {
                        crate::error!("Buffered read returned zero");
                        return Err(Error::new(
                            ErrorKind::UnexpectedEof,
                            "The X11 server closed the connection",
                        ));
                    }
                    Ok(n) => n,
                    Err(ref e) if e.kind() == ErrorKind::WouldBlock => break,
                    Err(e) => return Err(e),
                };
                crate::trace!("Read {} bytes into read buffer", nread);

                // begin reading that data into packets
                let mut src = &self.read_buffer[..nread];
                while !src.is_empty() {
                    let dest = self.inner.buffer();
                    let amt_to_read = cmp::min(src.len(), dest.len());

                    // copy slices over
                    dest[..amt_to_read].copy_from_slice(&src[..amt_to_read]);

                    // reborrow src
                    src = &src[amt_to_read..];

                    // advance by the given amount
                    if let Some(packet) = self.inner.advance(amt_to_read) {
                        out_packets.push(packet);
                    }
                }
            }
        }
        crate::trace!(
            "Read {} complete packet(s)",
            out_packets.len() - original_length
        );

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::PacketReader;
    use crate::rust_connection::{PollMode, Stream};
    use crate::utils::RawFdContainer;
    use std::cell::RefCell;
    use std::cmp;
    use std::io::{Error, ErrorKind, Result};

    // make a Stream that just reads from a Vec<u8>
    struct TestStream {
        data: RefCell<Vec<u8>>,
    }

    impl TestStream {
        fn new(data: Vec<u8>) -> Self {
            Self {
                data: RefCell::new(data),
            }
        }
    }

    impl Stream for TestStream {
        fn read(&self, buf: &mut [u8], _: &mut Vec<RawFdContainer>) -> Result<usize> {
            let mut data = self.data.borrow_mut();
            if data.len() == 0 {
                return Err(Error::from(ErrorKind::WouldBlock));
            }

            let nread = cmp::min(data.len(), buf.len());
            buf[..nread].copy_from_slice(&data[..nread]);
            let _ = data.drain(..nread);
            Ok(nread)
        }

        fn poll(&self, _: PollMode) -> Result<()> {
            Ok(())
        }

        fn write(&self, _: &[u8], _: &mut Vec<RawFdContainer>) -> Result<usize> {
            unreachable!()
        }
    }

    fn test_packet(packet: Vec<u8>) {
        let mut reader = PacketReader::new();
        let original_packet = packet.clone();
        let stream = TestStream::new(packet);

        let mut packets = Vec::new();
        let mut fd_storage = Vec::new();

        reader
            .try_read_packets(&stream, &mut packets, &mut fd_storage)
            .unwrap();

        assert_eq!(packets.len(), 1);
        assert_eq!(packets[0], original_packet);
    }

    #[test]
    fn fixed_size_packet() {
        let packet = vec![0; 32];
        test_packet(packet);
    }

    #[test]
    fn variable_size_packet() {
        let mut len = 120;
        let mut packet = vec![0; len];
        len = (len - 32) / 4;

        // copy len to 4..8
        packet[4..8].copy_from_slice(&(len as u32).to_ne_bytes());
        packet[0] = 1;

        test_packet(packet);
    }

    #[test]
    fn very_large_packet() {
        let mut len = 4800;
        let mut packet = vec![0; len];
        len = (len - 32) / 4;

        // copy len to 4..8
        packet[4..8].copy_from_slice(&(len as u32).to_ne_bytes());
        packet[0] = 1;

        test_packet(packet);
    }
}

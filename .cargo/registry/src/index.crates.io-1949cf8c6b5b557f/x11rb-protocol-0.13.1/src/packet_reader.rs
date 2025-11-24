//! Collects X11 data into "packets" to be parsed by a display.

use core::fmt;
use core::mem::replace;

use alloc::{vec, vec::Vec};

/// Minimal length of an X11 packet.
const MINIMAL_PACKET_LENGTH: usize = 32;

/// A wrapper around a buffer used to read X11 packets.
pub struct PacketReader {
    /// A partially-read packet.
    pending_packet: Vec<u8>,

    /// The point at which the packet is already read.
    already_read: usize,
}

impl fmt::Debug for PacketReader {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("PacketReader")
            .field(&format_args!(
                "{}/{}",
                self.already_read,
                self.pending_packet.len()
            ))
            .finish()
    }
}

impl Default for PacketReader {
    fn default() -> Self {
        Self::new()
    }
}

impl PacketReader {
    /// Create a new, empty `PacketReader`.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use x11rb_protocol::packet_reader::PacketReader;
    /// let reader = PacketReader::new();
    /// ```
    pub fn new() -> Self {
        Self {
            pending_packet: vec![0; MINIMAL_PACKET_LENGTH],
            already_read: 0,
        }
    }

    /// Get the buffer that the reader should fill with data.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use x11rb_protocol::packet_reader::PacketReader;
    /// # use x11rb_protocol::protocol::xproto::{GetInputFocusReply, InputFocus, Window};
    /// let mut reader = PacketReader::new();
    /// let buffer: [u8; 32] = read_in_buffer();
    ///
    /// reader.buffer().copy_from_slice(&buffer);
    ///
    /// # fn read_in_buffer() -> [u8; 32] { [0; 32] }
    /// ```
    pub fn buffer(&mut self) -> &mut [u8] {
        &mut self.pending_packet[self.already_read..]
    }

    /// The remaining capacity that needs to be filled.
    pub fn remaining_capacity(&self) -> usize {
        self.pending_packet.len() - self.already_read
    }

    /// Advance this buffer by the given amount.
    ///
    /// This will return the packet that was read, if enough bytes were read in order
    /// to form a complete packet.
    pub fn advance(&mut self, amount: usize) -> Option<Vec<u8>> {
        self.already_read += amount;
        debug_assert!(self.already_read <= self.pending_packet.len());

        if self.already_read == MINIMAL_PACKET_LENGTH {
            // we've read in the minimal packet, compute the amount of data we need to read
            // to form a complete packet
            let extra_length = extra_length(&self.pending_packet);

            // tell if we need to read more
            if extra_length > 0 {
                let total_length = MINIMAL_PACKET_LENGTH + extra_length;
                self.pending_packet.resize(total_length, 0);
                return None;
            }
        } else if self.already_read != self.pending_packet.len() {
            // we haven't read the full packet yet, return
            return None;
        }

        // we've read in the full packet, return it
        self.already_read = 0;
        Some(replace(
            &mut self.pending_packet,
            vec![0; MINIMAL_PACKET_LENGTH],
        ))
    }
}

/// Compute the length of the data we need to read, beyond the `MINIMAL_PACKET_LENGTH`.
fn extra_length(buffer: &[u8]) -> usize {
    use crate::protocol::xproto::GE_GENERIC_EVENT;
    const REPLY: u8 = 1;

    let response_type = buffer[0];

    if response_type == REPLY || response_type & 0x7f == GE_GENERIC_EVENT {
        let length_field = buffer[4..8].try_into().unwrap();
        let length_field = u32::from_ne_bytes(length_field) as usize;
        4 * length_field
    } else {
        // Fixed size packet: error or event that is not GE_GENERIC_EVENT
        0
    }
}

#[cfg(test)]
mod tests {
    use super::PacketReader;
    use alloc::{vec, vec::Vec};

    fn test_packets(packets: Vec<Vec<u8>>) {
        // Combine all packet data into one big chunk and test that the packet reader splits things
        let mut all_data = packets.iter().flatten().copied().collect::<Vec<u8>>();

        let mut reader = PacketReader::default();
        for (i, packet) in packets.into_iter().enumerate() {
            std::println!("Checking packet {i}");
            loop {
                let buffer = reader.buffer();
                let amount = std::cmp::min(buffer.len(), all_data.len());
                buffer.copy_from_slice(&all_data[..amount]);
                let _ = all_data.drain(..amount);

                if let Some(read_packet) = reader.advance(amount) {
                    assert_eq!(read_packet, packet);
                    break;
                }
            }
        }
    }

    fn make_reply_with_length(len: usize) -> Vec<u8> {
        let mut packet = vec![0; len];
        let len = (len - 32) / 4;

        // write "len" to bytes 4..8 in the packet
        let len_bytes = (len as u32).to_ne_bytes();
        packet[4..8].copy_from_slice(&len_bytes);
        packet[0] = 1;

        packet
    }

    #[test]
    fn fixed_size_packet() {
        // packet with a fixed size
        let packet = vec![0; 32];
        test_packets(vec![packet]);
    }

    #[test]
    fn variable_size_packet() {
        // packet with a variable size
        let packet = make_reply_with_length(1200);
        test_packets(vec![packet]);
    }

    #[test]
    fn test_many_fixed_size_packets() {
        let mut packets = vec![];
        for _ in 0..100 {
            packets.push(vec![0; 32]);
        }
        test_packets(packets);
    }

    #[test]
    fn test_many_variable_size_packets() {
        let mut packets = vec![];
        for i in 0..100 {
            // for maximum variation, increase packet size in a curved parabola
            // defined by -1/25 (x - 50)^2 + 100
            let variation = ((i - 50) * (i - 50)) as f32;
            let variation = -1.0 / 25.0 * variation + 100.0;
            let variation = variation as usize;
            // round to a multiple of 4
            let variation = variation / 4 * 4;

            let mut len = 1200 + variation;
            let mut packet = vec![0; len];
            assert_eq!(0, len % 4);
            len = (len - 32) / 4;

            // write "len" to bytes 4..8 in the packet
            let len_bytes = (len as u32).to_ne_bytes();
            packet[4..8].copy_from_slice(&len_bytes);
            packet[0] = 1;

            packets.push(packet);
        }
        test_packets(packets);
    }

    #[test]
    fn test_many_size_packets_mixed() {
        let mut packets = vec![];
        for i in 0..100 {
            // on odds, do a varsize packet
            let mut len = if i & 1 == 1 {
                // for maximum variation, increase packet size in a curved parabola
                // defined by -1/25 (x - 50)^2 + 100
                let variation = ((i - 50) * (i - 50)) as f32;
                let variation = -1.0 / 25.0 * variation + 100.0;
                let variation = variation as usize;
                // round to a multiple of 4
                let variation = variation / 4 * 4;

                1200 + variation
            } else {
                32
            };
            assert_eq!(0, len % 4);
            let mut packet = vec![0; len];
            len = (len - 32) / 4;

            // write "len" to bytes 4..8 in the packet
            let len_bytes = (len as u32).to_ne_bytes();
            packet[4..8].copy_from_slice(&len_bytes);
            packet[0] = 1;

            packets.push(packet);
        }
        test_packets(packets);
    }

    #[test]
    fn test_debug_fixed_size_packet() {
        // The debug output includes the length of the packet of the packet and how much was
        // already read
        let mut reader = PacketReader::new();
        assert_eq!(std::format!("{:?}", reader), "PacketReader(0/32)");

        let _ = reader.advance(15);
        assert_eq!(std::format!("{:?}", reader), "PacketReader(15/32)");

        let _ = reader.advance(15);
        assert_eq!(std::format!("{:?}", reader), "PacketReader(30/32)");

        let _ = reader.advance(2);
        assert_eq!(std::format!("{:?}", reader), "PacketReader(0/32)");
    }

    #[test]
    fn test_debug_variable_size_packet() {
        let packet = make_reply_with_length(1200);
        let mut reader = PacketReader::new();

        let first_len = 32;
        let second_len = 3;

        reader.buffer()[..first_len].copy_from_slice(&packet[..first_len]);
        let _ = reader.advance(first_len);

        reader.buffer()[..second_len].copy_from_slice(&packet[..second_len]);
        let _ = reader.advance(second_len);

        assert_eq!(std::format!("{:?}", reader), "PacketReader(35/1200)");
    }
}

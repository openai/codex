use alloc::vec::Vec;
use core::mem;
use core::ops::Range;

use super::buffers::{BufferProgress, Coalescer, Delocator, Locator};
use crate::error::InvalidMessage;
use crate::msgs::codec::{Codec, u24};
use crate::msgs::message::InboundPlainMessage;
use crate::{ContentType, ProtocolVersion};

#[derive(Debug)]
pub(crate) struct HandshakeDeframer {
    /// Spans covering individual handshake payloads, in order of receipt.
    spans: Vec<FragmentSpan>,

    /// Discard value, tracking the rightmost extent of the last message
    /// in `spans`.
    outer_discard: usize,
}

impl HandshakeDeframer {
    /// Accepts a message into the deframer.
    ///
    /// `containing_buffer` allows mapping the message payload to its position
    /// in the input buffer, and thereby avoid retaining a borrow on the input
    /// buffer.
    ///
    /// That is required because our processing of handshake messages requires
    /// them to be contiguous (and avoiding that would mean supporting gather-based
    /// parsing in a large number of places, including `core`, `webpki`, and the
    /// `CryptoProvider` interface).  `coalesce()` arranges for that to happen, but
    /// to do so it needs to move the fragments together in the original buffer.
    /// This would not be possible if the messages were borrowing from that buffer.
    ///
    /// `outer_discard` is the rightmost extent of the original message.
    pub(crate) fn input_message(
        &mut self,
        msg: InboundPlainMessage<'_>,
        containing_buffer: &Locator,
        outer_discard: usize,
    ) {
        debug_assert_eq!(msg.typ, ContentType::Handshake);
        debug_assert!(containing_buffer.fully_contains(msg.payload));
        debug_assert!(self.outer_discard <= outer_discard);

        self.outer_discard = outer_discard;

        // if our last span is incomplete, we can blindly add this as a new span --
        // no need to attempt parsing it with `DissectHandshakeIter`.
        //
        // `coalesce()` will later move this new message to be contiguous with
        // `_last_incomplete`, and reparse the result.
        //
        // we cannot merge these processes, because `coalesce` mutates the underlying
        // buffer, and `msg` borrows it.
        if let Some(_last_incomplete) = self
            .spans
            .last()
            .filter(|span| !span.is_complete())
        {
            self.spans.push(FragmentSpan {
                version: msg.version,
                size: None,
                bounds: containing_buffer.locate(msg.payload),
            });
            return;
        }

        // otherwise, we can expect `msg` to contain a handshake header introducing
        // a new message (and perhaps several of them.)
        for span in DissectHandshakeIter::new(msg, containing_buffer) {
            self.spans.push(span);
        }
    }

    /// Returns a `BufferProgress` that skips over unprocessed handshake data.
    pub(crate) fn progress(&self) -> BufferProgress {
        BufferProgress::new(self.outer_discard)
    }

    /// Do we have a message ready? ie, would `iter().next()` return `Some`?
    pub(crate) fn has_message_ready(&self) -> bool {
        match self.spans.first() {
            Some(span) => span.is_complete(),
            None => false,
        }
    }

    /// Do we have any message data, partial or otherwise?
    pub(crate) fn is_active(&self) -> bool {
        !self.spans.is_empty()
    }

    /// We are "aligned" if there is no partial fragment of a handshake
    /// message.
    pub(crate) fn is_aligned(&self) -> bool {
        self.spans
            .iter()
            .all(|span| span.is_complete())
    }

    /// Iterate over the complete messages.
    pub(crate) fn iter<'a, 'b>(&'a mut self, containing_buffer: &'b [u8]) -> HandshakeIter<'a, 'b> {
        HandshakeIter {
            deframer: self,
            containing_buffer: Delocator::new(containing_buffer),
            index: 0,
        }
    }

    /// Coalesce the handshake portions of the given buffer,
    /// if needed.
    ///
    /// This does nothing if there is nothing to do.
    ///
    /// In a normal TLS stream, handshake messages need not be contiguous.
    /// For example, each handshake message could be delivered in its own
    /// outer TLS message.  This would mean the handshake messages are
    /// separated by the outer TLS message headers, and likely also
    /// separated by encryption overhead (any explicit nonce in front,
    /// any padding and authentication tag afterwards).
    ///
    /// For a toy example of one handshake message in two fragments, and:
    ///
    /// - the letter `h` for handshake header octets
    /// - the letter `H` for handshake payload octets
    /// - the letter `x` for octets in the buffer ignored by this code,
    ///
    /// the buffer and `spans` data structure could look like:
    ///
    /// ```text
    /// 0 1 2 3 4 5 6 7 8 9 a b c d e f 0 1 2 3 4 5 6 7 8 9
    /// x x x x x h h h h H H H x x x x x H H H H H H x x x
    ///           '------------'          '----------'
    ///            |                               |
    /// spans = [ { bounds = (5, 12),              |
    ///              size = Some(9), .. },         |
    ///                                 { bounds = (17, 23), .. } ]
    /// ```
    ///
    /// In this case, `requires_coalesce` returns `Some(0)`.  Then
    /// `coalesce_one` moves the second range leftwards:
    ///
    /// ```text
    /// 0 1 2 3 4 5 6 7 8 9 a b c d e f 0 1 2 3 4 5 6 7 8 9
    /// x x x x x h h h h H H H x x x x x H H H H H H x x x
    ///                         '----------'
    ///                          ^        '----------'
    ///                          |         v
    ///                          '--<---<--'
    ///                 copy_within(from = (17, 23),
    ///                             to = (12, 18))
    /// ```
    ///
    /// Leaving the buffer and spans:
    ///
    /// ```text
    /// 0 1 2 3 4 5 6 7 8 9 a b c d e f 0 1 2 3 4 5 6 7 8 9
    /// x x x x x h h h h H H H H H H H H H x x x x x x x x
    ///           '------------------------'
    ///            |
    /// spans = [ { bounds = (5, 18), size = Some(9), .. } ]
    /// ```
    pub(crate) fn coalesce(&mut self, containing_buffer: &mut [u8]) -> Result<(), InvalidMessage> {
        // Strategy: while there is work to do, scan `spans`
        // for a pair where the first is not complete.  move
        // the second down towards the first, then reparse the contents.
        while let Some(i) = self.requires_coalesce() {
            self.coalesce_one(i, Coalescer::new(containing_buffer));
        }

        // check resulting spans pass our imposed length limit
        match self
            .spans
            .iter()
            .any(|span| span.size.unwrap_or_default() > MAX_HANDSHAKE_SIZE)
        {
            true => Err(InvalidMessage::HandshakePayloadTooLarge),
            false => Ok(()),
        }
    }

    /// Within `containing_buffer`, move `span[index+1]` to be contiguous
    /// with `span[index]`.
    fn coalesce_one(&mut self, index: usize, mut containing_buffer: Coalescer<'_>) {
        let second = self.spans.remove(index + 1);
        let mut first = self.spans.remove(index);

        // move the entirety of `second` to be contiguous with `first`
        let len = second.bounds.len();
        let target = Range {
            start: first.bounds.end,
            end: first.bounds.end + len,
        };

        containing_buffer.copy_within(second.bounds, target);
        let delocator = containing_buffer.delocator();

        // now adjust `first` to cover both
        first.bounds.end += len;

        // finally, attempt to re-dissect `first`
        let msg = InboundPlainMessage {
            typ: ContentType::Handshake,
            version: first.version,
            payload: delocator.slice_from_range(&first.bounds),
        };

        for (i, span) in DissectHandshakeIter::new(msg, &delocator.locator()).enumerate() {
            self.spans.insert(index + i, span);
        }
    }

    /// We require coalescing if any span except the last is not complete.
    ///
    /// Returns an index into `spans` for the first non-complete span:
    /// this will never be the last item.
    fn requires_coalesce(&self) -> Option<usize> {
        self.spans
            .split_last()
            .and_then(|(_last, elements)| {
                elements
                    .iter()
                    .enumerate()
                    .find_map(|(i, span)| (!span.is_complete()).then_some(i))
            })
    }
}

impl Default for HandshakeDeframer {
    fn default() -> Self {
        Self {
            // capacity: a typical upper limit on handshake messages in
            // a single flight
            spans: Vec::with_capacity(16),
            outer_discard: 0,
        }
    }
}

struct DissectHandshakeIter<'a, 'b> {
    version: ProtocolVersion,
    payload: &'b [u8],
    containing_buffer: &'a Locator,
}

impl<'a, 'b> DissectHandshakeIter<'a, 'b> {
    fn new(msg: InboundPlainMessage<'b>, containing_buffer: &'a Locator) -> Self {
        Self {
            version: msg.version,
            payload: msg.payload,
            containing_buffer,
        }
    }
}

impl Iterator for DissectHandshakeIter<'_, '_> {
    type Item = FragmentSpan;

    fn next(&mut self) -> Option<Self::Item> {
        if self.payload.is_empty() {
            return None;
        }

        // If there is not enough data to have a header the length is unknown
        if self.payload.len() < HANDSHAKE_HEADER_LEN {
            let buf = mem::take(&mut self.payload);
            let bounds = self.containing_buffer.locate(buf);
            return Some(FragmentSpan {
                version: self.version,
                size: None,
                bounds: bounds.clone(),
            });
        }

        let (header, rest) = mem::take(&mut self.payload).split_at(HANDSHAKE_HEADER_LEN);

        // safety: header[1..] is exactly 3 bytes, so `u24::read_bytes` cannot fail
        let size = u24::read_bytes(&header[1..])
            .unwrap()
            .into();

        let available = if size < rest.len() {
            self.payload = &rest[size..];
            size
        } else {
            rest.len()
        };

        let mut bounds = self.containing_buffer.locate(header);
        bounds.end += available;
        Some(FragmentSpan {
            version: self.version,
            size: Some(size),
            bounds: bounds.clone(),
        })
    }
}

pub(crate) struct HandshakeIter<'a, 'b> {
    deframer: &'a mut HandshakeDeframer,
    containing_buffer: Delocator<'b>,
    index: usize,
}

impl<'b> Iterator for HandshakeIter<'_, 'b> {
    type Item = (InboundPlainMessage<'b>, usize);

    fn next(&mut self) -> Option<Self::Item> {
        let next_span = self.deframer.spans.get(self.index)?;

        if !next_span.is_complete() {
            return None;
        }

        // if this is the last handshake message, then we'll end
        // up with an empty `spans` and can discard the remainder
        // of the input buffer.
        let discard = if self.deframer.spans.len() - 1 == self.index {
            mem::take(&mut self.deframer.outer_discard)
        } else {
            0
        };

        self.index += 1;
        Some((
            InboundPlainMessage {
                typ: ContentType::Handshake,
                version: next_span.version,
                payload: self
                    .containing_buffer
                    .slice_from_range(&next_span.bounds),
            },
            discard,
        ))
    }
}

impl Drop for HandshakeIter<'_, '_> {
    fn drop(&mut self) {
        self.deframer.spans.drain(..self.index);
    }
}

#[derive(Debug)]
struct FragmentSpan {
    /// version taken from containing message.
    version: ProtocolVersion,

    /// size of the handshake message body (excluding header)
    ///
    /// `None` means the size is unknown, because `bounds` is not
    /// large enough to encompass a whole header.
    size: Option<usize>,

    /// bounds of the handshake message, including header
    bounds: Range<usize>,
}

impl FragmentSpan {
    /// A `FragmentSpan` is "complete" if its size is known, and its
    /// bounds exactly encompasses one handshake message.
    fn is_complete(&self) -> bool {
        match self.size {
            Some(sz) => sz + HANDSHAKE_HEADER_LEN == self.bounds.len(),
            None => false,
        }
    }
}

const HANDSHAKE_HEADER_LEN: usize = 1 + 3;

/// TLS allows for handshake messages of up to 16MB.  We
/// restrict that to 64KB to limit potential for denial-of-
/// service.
const MAX_HANDSHAKE_SIZE: usize = 0xffff;

#[cfg(test)]
mod tests {
    use std::vec;

    use super::*;
    use crate::msgs::deframer::DeframerIter;

    fn add_bytes(hs: &mut HandshakeDeframer, slice: &[u8], within: &[u8]) {
        let msg = InboundPlainMessage {
            typ: ContentType::Handshake,
            version: ProtocolVersion::TLSv1_3,
            payload: slice,
        };
        let locator = Locator::new(within);
        let discard = locator.locate(slice).end;
        hs.input_message(msg, &locator, discard);
    }

    #[test]
    fn coalesce() {
        let mut input = vec![0, 0, 0, 0x21, 0, 0, 0, 0, 0x01, 0xff, 0x00, 0x01];
        let mut hs = HandshakeDeframer::default();

        add_bytes(&mut hs, &input[3..4], &input);
        assert_eq!(hs.requires_coalesce(), None);
        add_bytes(&mut hs, &input[4..6], &input);
        assert_eq!(hs.requires_coalesce(), Some(0));
        add_bytes(&mut hs, &input[8..10], &input);
        assert_eq!(hs.requires_coalesce(), Some(0));

        std::println!("before: {hs:?}");
        hs.coalesce(&mut input).unwrap();
        std::println!("after:  {hs:?}");

        let (msg, discard) = hs.iter(&input).next().unwrap();
        std::println!("msg {msg:?} discard {discard:?}");
        assert_eq!(msg.typ, ContentType::Handshake);
        assert_eq!(msg.version, ProtocolVersion::TLSv1_3);
        assert_eq!(msg.payload, &[0x21, 0x00, 0x00, 0x01, 0xff]);

        input.drain(..discard);
        assert_eq!(input, &[0, 1]);
    }

    #[test]
    fn append() {
        let mut input = vec![0, 0, 0, 0x21, 0, 0, 5, 0, 0, 1, 2, 3, 4, 5, 0];
        let mut hs = HandshakeDeframer::default();

        add_bytes(&mut hs, &input[3..7], &input);
        add_bytes(&mut hs, &input[9..14], &input);
        assert_eq!(hs.spans.len(), 2);

        hs.coalesce(&mut input).unwrap();
        assert_eq!(hs.spans.len(), 1);

        let (msg, discard) = std::dbg!(hs.iter(&input).next().unwrap());
        assert_eq!(msg.typ, ContentType::Handshake);
        assert_eq!(msg.version, ProtocolVersion::TLSv1_3);
        assert_eq!(msg.payload, &[0x21, 0x00, 0x00, 0x05, 1, 2, 3, 4, 5]);

        input.drain(..discard);
        assert_eq!(input, &[0]);
    }

    #[test]
    fn coalesce_rejects_excess_size_message() {
        const X: u8 = 0xff;
        let mut input = vec![0x21, 0x01, 0x00, X, 0x00, 0xab, X];
        let mut hs = HandshakeDeframer::default();

        // split header over multiple messages, which motivates doing
        // this check in `coalesce()`
        add_bytes(&mut hs, &input[0..3], &input);
        add_bytes(&mut hs, &input[4..6], &input);

        assert_eq!(
            hs.coalesce(&mut input),
            Err(InvalidMessage::HandshakePayloadTooLarge)
        );
    }

    #[test]
    fn iter_only_returns_full_messages() {
        let input = [0, 0, 0, 0x21, 0, 0, 1, 0xab, 0x21, 0, 0, 1];

        let mut hs = HandshakeDeframer::default();

        add_bytes(&mut hs, &input[3..8], &input);
        add_bytes(&mut hs, &input[8..12], &input);

        let mut iter = hs.iter(&input);
        let (msg, discard) = iter.next().unwrap();
        assert!(iter.next().is_none());

        assert_eq!(msg.typ, ContentType::Handshake);
        assert_eq!(msg.version, ProtocolVersion::TLSv1_3);
        assert_eq!(msg.payload, &[0x21, 0x00, 0x00, 0x01, 0xab]);
        assert_eq!(discard, 0);
    }

    #[test]
    fn handshake_flight() {
        // intended to be a realistic example
        let mut input = include_bytes!("../../testdata/handshake-test.1.bin").to_vec();
        let locator = Locator::new(&input);

        let mut hs = HandshakeDeframer::default();

        let mut iter = DeframerIter::new(&mut input[..]);

        while let Some(message) = iter.next() {
            let plain = message.unwrap().into_plain_message();
            std::println!("message {plain:?}");

            hs.input_message(plain, &locator, iter.bytes_consumed());
        }

        hs.coalesce(&mut input[..]).unwrap();

        let mut iter = hs.iter(&input[..]);
        for _ in 0..4 {
            let (msg, discard) = iter.next().unwrap();
            assert!(matches!(
                msg,
                InboundPlainMessage {
                    typ: ContentType::Handshake,
                    ..
                }
            ));
            assert_eq!(discard, 0);
        }

        let (msg, discard) = iter.next().unwrap();
        assert!(matches!(
            msg,
            InboundPlainMessage {
                typ: ContentType::Handshake,
                ..
            }
        ));
        assert_eq!(discard, 4280);
        drop(iter);

        input.drain(0..discard);
        assert!(input.is_empty());
    }
}

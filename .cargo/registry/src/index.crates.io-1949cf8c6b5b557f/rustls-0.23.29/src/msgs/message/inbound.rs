use core::ops::{Deref, DerefMut, Range};

use crate::enums::{ContentType, ProtocolVersion};
use crate::error::{Error, PeerMisbehaved};
use crate::msgs::fragmenter::MAX_FRAGMENT_LEN;

/// A TLS frame, named TLSPlaintext in the standard.
///
/// This inbound type borrows its encrypted payload from a buffer elsewhere.
/// It is used for joining and is consumed by decryption.
pub struct InboundOpaqueMessage<'a> {
    pub typ: ContentType,
    pub version: ProtocolVersion,
    pub payload: BorrowedPayload<'a>,
}

impl<'a> InboundOpaqueMessage<'a> {
    /// Construct a new `InboundOpaqueMessage` from constituent fields.
    ///
    /// `payload` is borrowed.
    pub fn new(typ: ContentType, version: ProtocolVersion, payload: &'a mut [u8]) -> Self {
        Self {
            typ,
            version,
            payload: BorrowedPayload(payload),
        }
    }

    /// Force conversion into a plaintext message.
    ///
    /// This should only be used for messages that are known to be in plaintext. Otherwise, the
    /// `InboundOpaqueMessage` should be decrypted into a `PlainMessage` using a `MessageDecrypter`.
    pub fn into_plain_message(self) -> InboundPlainMessage<'a> {
        InboundPlainMessage {
            typ: self.typ,
            version: self.version,
            payload: self.payload.into_inner(),
        }
    }

    /// Force conversion into a plaintext message.
    ///
    /// `range` restricts the resulting message: this function panics if it is out of range for
    /// the underlying message payload.
    ///
    /// This should only be used for messages that are known to be in plaintext. Otherwise, the
    /// `InboundOpaqueMessage` should be decrypted into a `PlainMessage` using a `MessageDecrypter`.
    pub fn into_plain_message_range(self, range: Range<usize>) -> InboundPlainMessage<'a> {
        InboundPlainMessage {
            typ: self.typ,
            version: self.version,
            payload: &self.payload.into_inner()[range],
        }
    }

    /// For TLS1.3 (only), checks the length msg.payload is valid and removes the padding.
    ///
    /// Returns an error if the message (pre-unpadding) is too long, or the padding is invalid,
    /// or the message (post-unpadding) is too long.
    pub fn into_tls13_unpadded_message(mut self) -> Result<InboundPlainMessage<'a>, Error> {
        let payload = &mut self.payload;

        if payload.len() > MAX_FRAGMENT_LEN + 1 {
            return Err(Error::PeerSentOversizedRecord);
        }

        self.typ = unpad_tls13_payload(payload);
        if self.typ == ContentType::Unknown(0) {
            return Err(PeerMisbehaved::IllegalTlsInnerPlaintext.into());
        }

        if payload.len() > MAX_FRAGMENT_LEN {
            return Err(Error::PeerSentOversizedRecord);
        }

        self.version = ProtocolVersion::TLSv1_3;
        Ok(self.into_plain_message())
    }
}

pub struct BorrowedPayload<'a>(&'a mut [u8]);

impl Deref for BorrowedPayload<'_> {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        self.0
    }
}

impl DerefMut for BorrowedPayload<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.0
    }
}

impl<'a> BorrowedPayload<'a> {
    pub fn truncate(&mut self, len: usize) {
        if len >= self.len() {
            return;
        }

        self.0 = core::mem::take(&mut self.0)
            .split_at_mut(len)
            .0;
    }

    pub(crate) fn into_inner(self) -> &'a mut [u8] {
        self.0
    }

    pub(crate) fn pop(&mut self) -> Option<u8> {
        if self.is_empty() {
            return None;
        }

        let len = self.len();
        let last = self[len - 1];
        self.truncate(len - 1);
        Some(last)
    }
}

/// A TLS frame, named `TLSPlaintext` in the standard.
///
/// This inbound type borrows its decrypted payload from the original buffer.
/// It results from decryption.
#[derive(Debug)]
pub struct InboundPlainMessage<'a> {
    pub typ: ContentType,
    pub version: ProtocolVersion,
    pub payload: &'a [u8],
}

impl InboundPlainMessage<'_> {
    /// Returns true if the payload is a CCS message.
    ///
    /// We passthrough ChangeCipherSpec messages in the deframer without decrypting them.
    /// Note: this is prior to the record layer, so is unencrypted. See
    /// third paragraph of section 5 in RFC8446.
    pub(crate) fn is_valid_ccs(&self) -> bool {
        self.typ == ContentType::ChangeCipherSpec && self.payload == [0x01]
    }
}

/// Decode a TLS1.3 `TLSInnerPlaintext` encoding.
///
/// `p` is a message payload, immediately post-decryption.  This function
/// removes zero padding bytes, until a non-zero byte is encountered which is
/// the content type, which is returned.  See RFC8446 s5.2.
///
/// ContentType(0) is returned if the message payload is empty or all zeroes.
fn unpad_tls13_payload(p: &mut BorrowedPayload<'_>) -> ContentType {
    loop {
        match p.pop() {
            Some(0) => {}
            Some(content_type) => return ContentType::from(content_type),
            None => return ContentType::Unknown(0),
        }
    }
}

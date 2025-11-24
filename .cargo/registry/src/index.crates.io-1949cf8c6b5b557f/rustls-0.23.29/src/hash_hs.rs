use alloc::boxed::Box;
use alloc::vec::Vec;
use core::mem;

use crate::crypto::hash;
use crate::msgs::codec::Codec;
use crate::msgs::enums::HashAlgorithm;
use crate::msgs::handshake::HandshakeMessagePayload;
use crate::msgs::message::{Message, MessagePayload};

/// Early stage buffering of handshake payloads.
///
/// Before we know the hash algorithm to use to verify the handshake, we just buffer the messages.
/// During the handshake, we may restart the transcript due to a HelloRetryRequest, reverting
/// from the `HandshakeHash` to a `HandshakeHashBuffer` again.
#[derive(Clone)]
pub(crate) struct HandshakeHashBuffer {
    buffer: Vec<u8>,
    client_auth_enabled: bool,
}

impl HandshakeHashBuffer {
    pub(crate) fn new() -> Self {
        Self {
            buffer: Vec::new(),
            client_auth_enabled: false,
        }
    }

    /// We might be doing client auth, so need to keep a full
    /// log of the handshake.
    pub(crate) fn set_client_auth_enabled(&mut self) {
        self.client_auth_enabled = true;
    }

    /// Hash/buffer a handshake message.
    pub(crate) fn add_message(&mut self, m: &Message<'_>) {
        match &m.payload {
            MessagePayload::Handshake { encoded, .. } => self.add_raw(encoded.bytes()),
            MessagePayload::HandshakeFlight(payload) => self.add_raw(payload.bytes()),
            _ => {}
        };
    }

    /// Hash or buffer a byte slice.
    fn add_raw(&mut self, buf: &[u8]) {
        self.buffer.extend_from_slice(buf);
    }

    /// Get the hash value if we were to hash `extra` too.
    pub(crate) fn hash_given(
        &self,
        provider: &'static dyn hash::Hash,
        extra: &[u8],
    ) -> hash::Output {
        let mut ctx = provider.start();
        ctx.update(&self.buffer);
        ctx.update(extra);
        ctx.finish()
    }

    /// We now know what hash function the verify_data will use.
    pub(crate) fn start_hash(self, provider: &'static dyn hash::Hash) -> HandshakeHash {
        let mut ctx = provider.start();
        ctx.update(&self.buffer);
        HandshakeHash {
            provider,
            ctx,
            client_auth: match self.client_auth_enabled {
                true => Some(self.buffer),
                false => None,
            },
        }
    }
}

/// This deals with keeping a running hash of the handshake
/// payloads.  This is computed by buffering initially.  Once
/// we know what hash function we need to use we switch to
/// incremental hashing.
///
/// For client auth, we also need to buffer all the messages.
/// This is disabled in cases where client auth is not possible.
pub(crate) struct HandshakeHash {
    provider: &'static dyn hash::Hash,
    ctx: Box<dyn hash::Context>,

    /// buffer for client-auth.
    client_auth: Option<Vec<u8>>,
}

impl HandshakeHash {
    /// We decided not to do client auth after all, so discard
    /// the transcript.
    pub(crate) fn abandon_client_auth(&mut self) {
        self.client_auth = None;
    }

    /// Hash/buffer a handshake message.
    pub(crate) fn add_message(&mut self, m: &Message<'_>) -> &mut Self {
        match &m.payload {
            MessagePayload::Handshake { encoded, .. } => self.add_raw(encoded.bytes()),
            MessagePayload::HandshakeFlight(payload) => self.add_raw(payload.bytes()),
            _ => self,
        }
    }

    /// Hash/buffer an encoded handshake message.
    pub(crate) fn add(&mut self, bytes: &[u8]) {
        self.add_raw(bytes);
    }

    /// Hash or buffer a byte slice.
    fn add_raw(&mut self, buf: &[u8]) -> &mut Self {
        self.ctx.update(buf);

        if let Some(buffer) = &mut self.client_auth {
            buffer.extend_from_slice(buf);
        }

        self
    }

    /// Get the hash value if we were to hash `extra` too,
    /// using hash function `hash`.
    pub(crate) fn hash_given(&self, extra: &[u8]) -> hash::Output {
        let mut ctx = self.ctx.fork();
        ctx.update(extra);
        ctx.finish()
    }

    pub(crate) fn into_hrr_buffer(self) -> HandshakeHashBuffer {
        let old_hash = self.ctx.finish();
        let old_handshake_hash_msg =
            HandshakeMessagePayload::build_handshake_hash(old_hash.as_ref());

        HandshakeHashBuffer {
            client_auth_enabled: self.client_auth.is_some(),
            buffer: old_handshake_hash_msg.get_encoding(),
        }
    }

    /// Take the current hash value, and encapsulate it in a
    /// 'handshake_hash' handshake message.  Start this hash
    /// again, with that message at the front.
    pub(crate) fn rollup_for_hrr(&mut self) {
        let ctx = &mut self.ctx;

        let old_ctx = mem::replace(ctx, self.provider.start());
        let old_hash = old_ctx.finish();
        let old_handshake_hash_msg =
            HandshakeMessagePayload::build_handshake_hash(old_hash.as_ref());

        self.add_raw(&old_handshake_hash_msg.get_encoding());
    }

    /// Get the current hash value.
    pub(crate) fn current_hash(&self) -> hash::Output {
        self.ctx.fork_finish()
    }

    /// Takes this object's buffer containing all handshake messages
    /// so far.  This method only works once; it resets the buffer
    /// to empty.
    #[cfg(feature = "tls12")]
    pub(crate) fn take_handshake_buf(&mut self) -> Option<Vec<u8>> {
        self.client_auth.take()
    }

    /// The hashing algorithm
    pub(crate) fn algorithm(&self) -> HashAlgorithm {
        self.provider.algorithm()
    }
}

impl Clone for HandshakeHash {
    fn clone(&self) -> Self {
        Self {
            provider: self.provider,
            ctx: self.ctx.fork(),
            client_auth: self.client_auth.clone(),
        }
    }
}

#[cfg(test)]
#[macro_rules_attribute::apply(test_for_each_provider)]
mod tests {
    use super::provider::hash::SHA256;
    use super::*;
    use crate::crypto::hash::Hash;
    use crate::enums::ProtocolVersion;
    use crate::msgs::base::Payload;
    use crate::msgs::handshake::{HandshakeMessagePayload, HandshakePayload};

    #[test]
    fn hashes_correctly() {
        let mut hhb = HandshakeHashBuffer::new();
        hhb.add_raw(b"hello");
        assert_eq!(hhb.buffer.len(), 5);
        let mut hh = hhb.start_hash(&SHA256);
        assert!(hh.client_auth.is_none());
        hh.add_raw(b"world");
        let h = hh.current_hash();
        let h = h.as_ref();
        assert_eq!(h[0], 0x93);
        assert_eq!(h[1], 0x6a);
        assert_eq!(h[2], 0x18);
        assert_eq!(h[3], 0x5c);
    }

    #[test]
    fn hashes_message_types() {
        // handshake protocol encoding of 0x0e 00 00 00
        let server_hello_done_message = Message {
            version: ProtocolVersion::TLSv1_2,
            payload: MessagePayload::handshake(HandshakeMessagePayload(
                HandshakePayload::ServerHelloDone,
            )),
        };

        let app_data_ignored = Message {
            version: ProtocolVersion::TLSv1_3,
            payload: MessagePayload::ApplicationData(Payload::Borrowed(b"hello")),
        };

        let end_of_early_data_flight = Message {
            version: ProtocolVersion::TLSv1_3,
            payload: MessagePayload::HandshakeFlight(Payload::Borrowed(b"\x05\x00\x00\x00")),
        };

        // buffered mode
        let mut hhb = HandshakeHashBuffer::new();
        hhb.add_message(&server_hello_done_message);
        hhb.add_message(&app_data_ignored);
        hhb.add_message(&end_of_early_data_flight);
        assert_eq!(
            hhb.start_hash(&SHA256)
                .current_hash()
                .as_ref(),
            SHA256
                .hash(b"\x0e\x00\x00\x00\x05\x00\x00\x00")
                .as_ref()
        );

        // non-buffered mode
        let mut hh = HandshakeHashBuffer::new().start_hash(&SHA256);
        hh.add_message(&server_hello_done_message);
        hh.add_message(&app_data_ignored);
        hh.add_message(&end_of_early_data_flight);
        assert_eq!(
            hh.current_hash().as_ref(),
            SHA256
                .hash(b"\x0e\x00\x00\x00\x05\x00\x00\x00")
                .as_ref()
        );
    }

    #[cfg(feature = "tls12")]
    #[test]
    fn buffers_correctly() {
        let mut hhb = HandshakeHashBuffer::new();
        hhb.set_client_auth_enabled();
        hhb.add_raw(b"hello");
        assert_eq!(hhb.buffer.len(), 5);
        let mut hh = hhb.start_hash(&SHA256);
        assert_eq!(
            hh.client_auth
                .as_ref()
                .map(|buf| buf.len()),
            Some(5)
        );
        hh.add_raw(b"world");
        assert_eq!(
            hh.client_auth
                .as_ref()
                .map(|buf| buf.len()),
            Some(10)
        );
        let h = hh.current_hash();
        let h = h.as_ref();
        assert_eq!(h[0], 0x93);
        assert_eq!(h[1], 0x6a);
        assert_eq!(h[2], 0x18);
        assert_eq!(h[3], 0x5c);
        let buf = hh.take_handshake_buf();
        assert_eq!(Some(b"helloworld".to_vec()), buf);
    }

    #[test]
    fn abandon() {
        let mut hhb = HandshakeHashBuffer::new();
        hhb.set_client_auth_enabled();
        hhb.add_raw(b"hello");
        assert_eq!(hhb.buffer.len(), 5);
        let mut hh = hhb.start_hash(&SHA256);
        assert_eq!(
            hh.client_auth
                .as_ref()
                .map(|buf| buf.len()),
            Some(5)
        );
        hh.abandon_client_auth();
        assert_eq!(hh.client_auth, None);
        hh.add_raw(b"world");
        assert_eq!(hh.client_auth, None);
        let h = hh.current_hash();
        let h = h.as_ref();
        assert_eq!(h[0], 0x93);
        assert_eq!(h[1], 0x6a);
        assert_eq!(h[2], 0x18);
        assert_eq!(h[3], 0x5c);
    }

    #[test]
    fn clones_correctly() {
        let mut hhb = HandshakeHashBuffer::new();
        hhb.set_client_auth_enabled();
        hhb.add_raw(b"hello");
        assert_eq!(hhb.buffer.len(), 5);

        // Cloning the HHB should result in the same buffer and client auth state.
        let mut hhb_prime = hhb.clone();
        assert_eq!(hhb_prime.buffer, hhb.buffer);
        assert!(hhb_prime.client_auth_enabled);

        // Updating the HHB clone shouldn't affect the original.
        hhb_prime.add_raw(b"world");
        assert_eq!(hhb_prime.buffer.len(), 10);
        assert_ne!(hhb.buffer, hhb_prime.buffer);

        let hh = hhb.start_hash(&SHA256);
        let hh_hash = hh.current_hash();
        let hh_hash = hh_hash.as_ref();

        // Cloning the HH should result in the same current hash.
        let mut hh_prime = hh.clone();
        let hh_prime_hash = hh_prime.current_hash();
        let hh_prime_hash = hh_prime_hash.as_ref();
        assert_eq!(hh_hash, hh_prime_hash);

        // Updating the HH clone shouldn't affect the original.
        hh_prime.add_raw(b"goodbye");
        assert_eq!(hh.current_hash().as_ref(), hh_hash);
        assert_ne!(hh_prime.current_hash().as_ref(), hh_hash);
    }
}

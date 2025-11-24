use alloc::boxed::Box;
use alloc::vec::Vec;
use core::fmt;
use core::fmt::{Debug, Formatter};
use core::sync::atomic::{AtomicUsize, Ordering};

use aws_lc_rs::cipher::{
    AES_256, AES_256_KEY_LEN, AES_CBC_IV_LEN, DecryptionContext, PaddedBlockDecryptingKey,
    PaddedBlockEncryptingKey, UnboundCipherKey,
};
use aws_lc_rs::{hmac, iv};

use super::ring_like::rand::{SecureRandom, SystemRandom};
use super::unspecified_err;
use crate::error::Error;
#[cfg(debug_assertions)]
use crate::log::debug;
use crate::polyfill::try_split_at;
use crate::rand::GetRandomFailed;
use crate::server::ProducesTickets;
use crate::sync::Arc;

/// A concrete, safe ticket creation mechanism.
pub struct Ticketer {}

impl Ticketer {
    /// Make the recommended `Ticketer`.  This produces tickets
    /// with a 12 hour life and randomly generated keys.
    ///
    /// The `Ticketer` uses the [RFC 5077 §4] "Recommended Ticket Construction",
    /// using AES 256 for encryption and HMAC-SHA256 for ciphertext authentication.
    ///
    /// [RFC 5077 §4]: https://www.rfc-editor.org/rfc/rfc5077#section-4
    #[cfg(feature = "std")]
    pub fn new() -> Result<Arc<dyn ProducesTickets>, Error> {
        Ok(Arc::new(crate::ticketer::TicketRotator::new(
            6 * 60 * 60,
            make_ticket_generator,
        )?))
    }
}

fn make_ticket_generator() -> Result<Box<dyn ProducesTickets>, GetRandomFailed> {
    // NOTE(XXX): Unconditionally mapping errors to `GetRandomFailed` here is slightly
    //   misleading in some cases (e.g. failure to construct a padded block cipher encrypting key).
    //   However, we can't change the return type expected from a `TicketSwitcher` `generator`
    //   without breaking semver.
    //   Tracking in https://github.com/rustls/rustls/issues/2074
    Ok(Box::new(
        Rfc5077Ticketer::new().map_err(|_| GetRandomFailed)?,
    ))
}

/// An RFC 5077 "Recommended Ticket Construction" implementation of a [`Ticketer`].
struct Rfc5077Ticketer {
    aes_encrypt_key: PaddedBlockEncryptingKey,
    aes_decrypt_key: PaddedBlockDecryptingKey,
    hmac_key: hmac::Key,
    key_name: [u8; 16],
    lifetime: u32,
    maximum_ciphertext_len: AtomicUsize,
}

impl Rfc5077Ticketer {
    fn new() -> Result<Self, Error> {
        let rand = SystemRandom::new();

        // Generate a random AES 256 key to use for AES CBC encryption.
        let mut aes_key = [0u8; AES_256_KEY_LEN];
        rand.fill(&mut aes_key)
            .map_err(|_| GetRandomFailed)?;

        // Convert the raw AES 256 key bytes into encrypting and decrypting keys using CBC mode and
        // PKCS#7 padding. We don't want to store just the raw key bytes as constructing the
        // cipher keys has some setup overhead. We can't store just the `UnboundCipherKey` since
        // constructing the padded encrypt/decrypt specific types consume the `UnboundCipherKey`.
        let aes_encrypt_key =
            UnboundCipherKey::new(&AES_256, &aes_key[..]).map_err(unspecified_err)?;
        let aes_encrypt_key =
            PaddedBlockEncryptingKey::cbc_pkcs7(aes_encrypt_key).map_err(unspecified_err)?;

        // Convert the raw AES 256 key bytes into a decrypting key using CBC PKCS#7 padding.
        let aes_decrypt_key =
            UnboundCipherKey::new(&AES_256, &aes_key[..]).map_err(unspecified_err)?;
        let aes_decrypt_key =
            PaddedBlockDecryptingKey::cbc_pkcs7(aes_decrypt_key).map_err(unspecified_err)?;

        // Generate a random HMAC SHA256 key to use for HMAC authentication.
        let hmac_key = hmac::Key::generate(hmac::HMAC_SHA256, &rand).map_err(unspecified_err)?;

        // Generate a random key name.
        let mut key_name = [0u8; 16];
        rand.fill(&mut key_name)
            .map_err(|_| GetRandomFailed)?;

        Ok(Self {
            aes_encrypt_key,
            aes_decrypt_key,
            hmac_key,
            key_name,
            lifetime: 60 * 60 * 12,
            maximum_ciphertext_len: AtomicUsize::new(0),
        })
    }
}

impl ProducesTickets for Rfc5077Ticketer {
    fn enabled(&self) -> bool {
        true
    }

    fn lifetime(&self) -> u32 {
        self.lifetime
    }

    /// Encrypt `message` and return the ciphertext.
    fn encrypt(&self, message: &[u8]) -> Option<Vec<u8>> {
        // Encrypt the ticket state - the cipher module handles generating a random IV of
        // appropriate size, returning it in the `DecryptionContext`.
        let mut encrypted_state = Vec::from(message);
        let dec_ctx = self
            .aes_encrypt_key
            .encrypt(&mut encrypted_state)
            .ok()?;
        let iv: &[u8] = (&dec_ctx).try_into().ok()?;

        // Produce the MAC tag over the relevant context & encrypted state.
        // Quoting RFC 5077:
        //   "The Message Authentication Code (MAC) is calculated using HMAC-SHA-256 over
        //    key_name (16 octets) and IV (16 octets), followed by the length of
        //    the encrypted_state field (2 octets) and its contents (variable
        //    length)."
        let mut hmac_data =
            Vec::with_capacity(self.key_name.len() + iv.len() + 2 + encrypted_state.len());
        hmac_data.extend(&self.key_name);
        hmac_data.extend(iv);
        hmac_data.extend(
            u16::try_from(encrypted_state.len())
                .ok()?
                .to_be_bytes(),
        );
        hmac_data.extend(&encrypted_state);
        let tag = hmac::sign(&self.hmac_key, &hmac_data);
        let tag = tag.as_ref();

        // Combine the context, the encrypted state, and the tag to produce the final ciphertext.
        // Ciphertext structure is:
        //   key_name: [u8; 16]
        //   iv: [u8; 16]
        //   encrypted_state: [u8, _]
        //   mac tag: [u8; 32]
        let mut ciphertext =
            Vec::with_capacity(self.key_name.len() + iv.len() + encrypted_state.len() + tag.len());
        ciphertext.extend(self.key_name);
        ciphertext.extend(iv);
        ciphertext.extend(encrypted_state);
        ciphertext.extend(tag);

        self.maximum_ciphertext_len
            .fetch_max(ciphertext.len(), Ordering::SeqCst);

        Some(ciphertext)
    }

    fn decrypt(&self, ciphertext: &[u8]) -> Option<Vec<u8>> {
        if ciphertext.len()
            > self
                .maximum_ciphertext_len
                .load(Ordering::SeqCst)
        {
            #[cfg(debug_assertions)]
            debug!("rejected over-length ticket");
            return None;
        }

        // Split off the key name from the remaining ciphertext.
        let (alleged_key_name, ciphertext) = try_split_at(ciphertext, self.key_name.len())?;

        // Split off the IV from the remaining ciphertext.
        let (iv, ciphertext) = try_split_at(ciphertext, AES_CBC_IV_LEN)?;

        // And finally, split the encrypted state from the tag.
        let tag_len = self
            .hmac_key
            .algorithm()
            .digest_algorithm()
            .output_len();
        let (enc_state, mac) = try_split_at(ciphertext, ciphertext.len() - tag_len)?;

        // Reconstitute the HMAC data to verify the tag.
        let mut hmac_data =
            Vec::with_capacity(alleged_key_name.len() + iv.len() + 2 + enc_state.len());
        hmac_data.extend(alleged_key_name);
        hmac_data.extend(iv);
        hmac_data.extend(
            u16::try_from(enc_state.len())
                .ok()?
                .to_be_bytes(),
        );
        hmac_data.extend(enc_state);
        hmac::verify(&self.hmac_key, &hmac_data, mac).ok()?;

        // Convert the raw IV back into an appropriate decryption context.
        let iv = iv::FixedLength::try_from(iv).ok()?;
        let dec_context = DecryptionContext::Iv128(iv);

        // And finally, decrypt the encrypted state.
        let mut out = Vec::from(enc_state);
        let plaintext = self
            .aes_decrypt_key
            .decrypt(&mut out, dec_context)
            .ok()?;

        Some(plaintext.into())
    }
}

impl Debug for Rfc5077Ticketer {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        // Note: we deliberately omit keys from the debug output.
        f.debug_struct("Rfc5077Ticketer")
            .field("lifetime", &self.lifetime)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use core::time::Duration;

    use pki_types::UnixTime;

    use super::*;

    #[test]
    fn basic_pairwise_test() {
        let t = Ticketer::new().unwrap();
        assert!(t.enabled());
        let cipher = t.encrypt(b"hello world").unwrap();
        let plain = t.decrypt(&cipher).unwrap();
        assert_eq!(plain, b"hello world");
    }

    #[test]
    fn refuses_decrypt_before_encrypt() {
        let t = Ticketer::new().unwrap();
        assert_eq!(t.decrypt(b"hello"), None);
    }

    #[test]
    fn refuses_decrypt_larger_than_largest_encryption() {
        let t = Ticketer::new().unwrap();
        let mut cipher = t.encrypt(b"hello world").unwrap();
        assert_eq!(t.decrypt(&cipher), Some(b"hello world".to_vec()));

        // obviously this would never work anyway, but this
        // and `cannot_decrypt_before_encrypt` exercise the
        // first branch in `decrypt()`
        cipher.push(0);
        assert_eq!(t.decrypt(&cipher), None);
    }

    #[test]
    fn ticketrotator_switching_test() {
        let t = Arc::new(crate::ticketer::TicketRotator::new(1, make_ticket_generator).unwrap());
        let now = UnixTime::now();
        let cipher1 = t.encrypt(b"ticket 1").unwrap();
        assert_eq!(t.decrypt(&cipher1).unwrap(), b"ticket 1");
        {
            // Trigger new ticketer
            t.maybe_roll(UnixTime::since_unix_epoch(Duration::from_secs(
                now.as_secs() + 10,
            )));
        }
        let cipher2 = t.encrypt(b"ticket 2").unwrap();
        assert_eq!(t.decrypt(&cipher1).unwrap(), b"ticket 1");
        assert_eq!(t.decrypt(&cipher2).unwrap(), b"ticket 2");
        {
            // Trigger new ticketer
            t.maybe_roll(UnixTime::since_unix_epoch(Duration::from_secs(
                now.as_secs() + 20,
            )));
        }
        let cipher3 = t.encrypt(b"ticket 3").unwrap();
        assert!(t.decrypt(&cipher1).is_none());
        assert_eq!(t.decrypt(&cipher2).unwrap(), b"ticket 2");
        assert_eq!(t.decrypt(&cipher3).unwrap(), b"ticket 3");
    }

    #[test]
    fn ticketrotator_remains_usable_over_temporary_ticketer_creation_failure() {
        let mut t = crate::ticketer::TicketRotator::new(1, make_ticket_generator).unwrap();
        let now = UnixTime::now();
        let cipher1 = t.encrypt(b"ticket 1").unwrap();
        assert_eq!(t.decrypt(&cipher1).unwrap(), b"ticket 1");
        t.generator = fail_generator;
        {
            // Failed new ticketer; this means we still need to
            // rotate.
            t.maybe_roll(UnixTime::since_unix_epoch(Duration::from_secs(
                now.as_secs() + 10,
            )));
        }

        // check post-failure encryption/decryption still works
        let cipher2 = t.encrypt(b"ticket 2").unwrap();
        assert_eq!(t.decrypt(&cipher1).unwrap(), b"ticket 1");
        assert_eq!(t.decrypt(&cipher2).unwrap(), b"ticket 2");

        // do the rotation for real
        t.generator = make_ticket_generator;
        {
            t.maybe_roll(UnixTime::since_unix_epoch(Duration::from_secs(
                now.as_secs() + 20,
            )));
        }
        let cipher3 = t.encrypt(b"ticket 3").unwrap();
        assert!(t.decrypt(&cipher1).is_some());
        assert_eq!(t.decrypt(&cipher2).unwrap(), b"ticket 2");
        assert_eq!(t.decrypt(&cipher3).unwrap(), b"ticket 3");
    }

    #[test]
    fn ticketswitcher_switching_test() {
        #[expect(deprecated)]
        let t = Arc::new(crate::ticketer::TicketSwitcher::new(1, make_ticket_generator).unwrap());
        let now = UnixTime::now();
        let cipher1 = t.encrypt(b"ticket 1").unwrap();
        assert_eq!(t.decrypt(&cipher1).unwrap(), b"ticket 1");
        {
            // Trigger new ticketer
            t.maybe_roll(UnixTime::since_unix_epoch(Duration::from_secs(
                now.as_secs() + 10,
            )));
        }
        let cipher2 = t.encrypt(b"ticket 2").unwrap();
        assert_eq!(t.decrypt(&cipher1).unwrap(), b"ticket 1");
        assert_eq!(t.decrypt(&cipher2).unwrap(), b"ticket 2");
        {
            // Trigger new ticketer
            t.maybe_roll(UnixTime::since_unix_epoch(Duration::from_secs(
                now.as_secs() + 20,
            )));
        }
        let cipher3 = t.encrypt(b"ticket 3").unwrap();
        assert!(t.decrypt(&cipher1).is_none());
        assert_eq!(t.decrypt(&cipher2).unwrap(), b"ticket 2");
        assert_eq!(t.decrypt(&cipher3).unwrap(), b"ticket 3");
    }

    #[test]
    fn ticketswitcher_recover_test() {
        #[expect(deprecated)]
        let mut t = crate::ticketer::TicketSwitcher::new(1, make_ticket_generator).unwrap();
        let now = UnixTime::now();
        let cipher1 = t.encrypt(b"ticket 1").unwrap();
        assert_eq!(t.decrypt(&cipher1).unwrap(), b"ticket 1");
        t.generator = fail_generator;
        {
            // Failed new ticketer
            t.maybe_roll(UnixTime::since_unix_epoch(Duration::from_secs(
                now.as_secs() + 10,
            )));
        }
        t.generator = make_ticket_generator;
        let cipher2 = t.encrypt(b"ticket 2").unwrap();
        assert_eq!(t.decrypt(&cipher1).unwrap(), b"ticket 1");
        assert_eq!(t.decrypt(&cipher2).unwrap(), b"ticket 2");
        {
            // recover
            t.maybe_roll(UnixTime::since_unix_epoch(Duration::from_secs(
                now.as_secs() + 20,
            )));
        }
        let cipher3 = t.encrypt(b"ticket 3").unwrap();
        assert!(t.decrypt(&cipher1).is_none());
        assert_eq!(t.decrypt(&cipher2).unwrap(), b"ticket 2");
        assert_eq!(t.decrypt(&cipher3).unwrap(), b"ticket 3");
    }

    #[test]
    fn rfc5077ticketer_is_debug_and_producestickets() {
        use alloc::format;

        use super::*;

        let t = make_ticket_generator().unwrap();

        assert_eq!(format!("{t:?}"), "Rfc5077Ticketer { lifetime: 43200 }");
        assert!(t.enabled());
        assert_eq!(t.lifetime(), 43200);
    }

    fn fail_generator() -> Result<Box<dyn ProducesTickets>, GetRandomFailed> {
        Err(GetRandomFailed)
    }
}

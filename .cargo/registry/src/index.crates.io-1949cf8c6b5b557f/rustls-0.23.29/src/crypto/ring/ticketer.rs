use alloc::boxed::Box;
use alloc::vec::Vec;
use core::fmt;
use core::fmt::{Debug, Formatter};
use core::sync::atomic::{AtomicUsize, Ordering};

use subtle::ConstantTimeEq;

use super::ring_like::aead;
use super::ring_like::rand::{SecureRandom, SystemRandom};
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
    /// The encryption mechanism used is Chacha20Poly1305.
    #[cfg(feature = "std")]
    pub fn new() -> Result<Arc<dyn ProducesTickets>, Error> {
        Ok(Arc::new(crate::ticketer::TicketRotator::new(
            6 * 60 * 60,
            make_ticket_generator,
        )?))
    }
}

fn make_ticket_generator() -> Result<Box<dyn ProducesTickets>, GetRandomFailed> {
    Ok(Box::new(AeadTicketer::new()?))
}

/// This is a `ProducesTickets` implementation which uses
/// any *ring* `aead::Algorithm` to encrypt and authentication
/// the ticket payload.  It does not enforce any lifetime
/// constraint.
struct AeadTicketer {
    alg: &'static aead::Algorithm,
    key: aead::LessSafeKey,
    key_name: [u8; 16],
    lifetime: u32,

    /// Tracks the largest ciphertext produced by `encrypt`, and
    /// uses it to early-reject `decrypt` queries that are too long.
    ///
    /// Accepting excessively long ciphertexts means a "Partitioning
    /// Oracle Attack" (see <https://eprint.iacr.org/2020/1491.pdf>)
    /// can be more efficient, though also note that these are thought
    /// to be cryptographically hard if the key is full-entropy (as it
    /// is here).
    maximum_ciphertext_len: AtomicUsize,
}

impl AeadTicketer {
    fn new() -> Result<Self, GetRandomFailed> {
        let mut key = [0u8; 32];
        SystemRandom::new()
            .fill(&mut key)
            .map_err(|_| GetRandomFailed)?;

        let key = aead::UnboundKey::new(TICKETER_AEAD, &key).unwrap();

        let mut key_name = [0u8; 16];
        SystemRandom::new()
            .fill(&mut key_name)
            .map_err(|_| GetRandomFailed)?;

        Ok(Self {
            alg: TICKETER_AEAD,
            key: aead::LessSafeKey::new(key),
            key_name,
            lifetime: 60 * 60 * 12,
            maximum_ciphertext_len: AtomicUsize::new(0),
        })
    }
}

impl ProducesTickets for AeadTicketer {
    fn enabled(&self) -> bool {
        true
    }

    fn lifetime(&self) -> u32 {
        self.lifetime
    }

    /// Encrypt `message` and return the ciphertext.
    fn encrypt(&self, message: &[u8]) -> Option<Vec<u8>> {
        // Random nonce, because a counter is a privacy leak.
        let mut nonce_buf = [0u8; 12];
        SystemRandom::new()
            .fill(&mut nonce_buf)
            .ok()?;
        let nonce = aead::Nonce::assume_unique_for_key(nonce_buf);
        let aad = aead::Aad::from(self.key_name);

        // ciphertext structure is:
        // key_name: [u8; 16]
        // nonce: [u8; 12]
        // message: [u8, _]
        // tag: [u8; 16]

        let mut ciphertext = Vec::with_capacity(
            self.key_name.len() + nonce_buf.len() + message.len() + self.key.algorithm().tag_len(),
        );
        ciphertext.extend(self.key_name);
        ciphertext.extend(nonce_buf);
        ciphertext.extend(message);
        let ciphertext = self
            .key
            .seal_in_place_separate_tag(
                nonce,
                aad,
                &mut ciphertext[self.key_name.len() + nonce_buf.len()..],
            )
            .map(|tag| {
                ciphertext.extend(tag.as_ref());
                ciphertext
            })
            .ok()?;

        self.maximum_ciphertext_len
            .fetch_max(ciphertext.len(), Ordering::SeqCst);
        Some(ciphertext)
    }

    /// Decrypt `ciphertext` and recover the original message.
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

        let (alleged_key_name, ciphertext) = try_split_at(ciphertext, self.key_name.len())?;

        let (nonce, ciphertext) = try_split_at(ciphertext, self.alg.nonce_len())?;

        // checking the key_name is the expected one, *and* then putting it into the
        // additionally authenticated data is duplicative.  this check quickly rejects
        // tickets for a different ticketer (see `TicketSwitcher`), while including it
        // in the AAD ensures it is authenticated independent of that check and that
        // any attempted attack on the integrity such as [^1] must happen for each
        // `key_label`, not over a population of potential keys.  this approach
        // is overall similar to [^2].
        //
        // [^1]: https://eprint.iacr.org/2020/1491.pdf
        // [^2]: "Authenticated Encryption with Key Identification", fig 6
        //       <https://eprint.iacr.org/2022/1680.pdf>
        if ConstantTimeEq::ct_ne(&self.key_name[..], alleged_key_name).into() {
            #[cfg(debug_assertions)]
            debug!("rejected ticket with wrong ticket_name");
            return None;
        }

        // This won't fail since `nonce` has the required length.
        let nonce = aead::Nonce::try_assume_unique_for_key(nonce).ok()?;

        let mut out = Vec::from(ciphertext);

        let plain_len = self
            .key
            .open_in_place(nonce, aead::Aad::from(alleged_key_name), &mut out)
            .ok()?
            .len();
        out.truncate(plain_len);

        Some(out)
    }
}

impl Debug for AeadTicketer {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        // Note: we deliberately omit the key from the debug output.
        f.debug_struct("AeadTicketer")
            .field("alg", &self.alg)
            .field("lifetime", &self.lifetime)
            .finish()
    }
}

static TICKETER_AEAD: &aead::Algorithm = &aead::CHACHA20_POLY1305;

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
    fn aeadticketer_is_debug_and_producestickets() {
        use alloc::format;

        use super::*;

        let t = make_ticket_generator().unwrap();

        let expect = format!("AeadTicketer {{ alg: {TICKETER_AEAD:?}, lifetime: 43200 }}");
        assert_eq!(format!("{t:?}"), expect);
        assert!(t.enabled());
        assert_eq!(t.lifetime(), 43200);
    }

    fn fail_generator() -> Result<Box<dyn ProducesTickets>, GetRandomFailed> {
        Err(GetRandomFailed)
    }
}

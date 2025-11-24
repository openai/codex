// Copyright 2015 Brian Smith.
//
// Permission to use, copy, modify, and/or distribute this software for any
// purpose with or without fee is hereby granted, provided that the above
// copyright notice and this permission notice appear in all copies.
//
// THE SOFTWARE IS PROVIDED "AS IS" AND THE AUTHOR DISCLAIMS ALL WARRANTIES
// WITH REGARD TO THIS SOFTWARE INCLUDING ALL IMPLIED WARRANTIES OF
// MERCHANTABILITY AND FITNESS. IN NO EVENT SHALL THE AUTHOR BE LIABLE FOR ANY
// SPECIAL, DIRECT, INDIRECT, OR CONSEQUENTIAL DAMAGES OR ANY DAMAGES
// WHATSOEVER RESULTING FROM LOSS OF USE, DATA OR PROFITS, WHETHER IN AN ACTION
// OF CONTRACT, NEGLIGENCE OR OTHER TORTIOUS ACTION, ARISING OUT OF OR IN
// CONNECTION WITH THE USE OR PERFORMANCE OF THIS SOFTWARE.

//! PBKDF2 derivation and verification.
//!
//! Use `derive` to derive PBKDF2 outputs. Use `verify` to verify secret
//! against previously-derived outputs.
//!
//! PBKDF2 is specified in [RFC 2898 Section 5.2] with test vectors given in
//! [RFC 6070]. See also [NIST Special Publication 800-132].
//!
//! [RFC 2898 Section 5.2]: https://tools.ietf.org/html/rfc2898#section-5.2
//! [RFC 6070]: https://tools.ietf.org/html/rfc6070
//! [NIST Special Publication 800-132]:
//!    http://nvlpubs.nist.gov/nistpubs/Legacy/SP/nistspecialpublication800-132.pdf
//!
//! # Examples
//!
//! ## Password Database Example
//!
//! ```
//! use ring::{digest, pbkdf2};
//! use std::{collections::HashMap, num::NonZeroU32};
//!
//! static PBKDF2_ALG: pbkdf2::Algorithm = pbkdf2::PBKDF2_HMAC_SHA256;
//! const CREDENTIAL_LEN: usize = digest::SHA256_OUTPUT_LEN;
//! pub type Credential = [u8; CREDENTIAL_LEN];
//!
//! enum Error {
//!     WrongUsernameOrPassword
//! }
//!
//! struct PasswordDatabase {
//!     pbkdf2_iterations: NonZeroU32,
//!     db_salt_component: [u8; 16],
//!
//!     // Normally this would be a persistent database.
//!     storage: HashMap<String, Credential>,
//! }
//!
//! impl PasswordDatabase {
//!     pub fn store_password(&mut self, username: &str, password: &str) {
//!         let salt = self.salt(username);
//!         let mut to_store: Credential = [0u8; CREDENTIAL_LEN];
//!         pbkdf2::derive(PBKDF2_ALG, self.pbkdf2_iterations, &salt,
//!                        password.as_bytes(), &mut to_store);
//!         self.storage.insert(String::from(username), to_store);
//!     }
//!
//!     pub fn verify_password(&self, username: &str, attempted_password: &str)
//!                            -> Result<(), Error> {
//!         match self.storage.get(username) {
//!            Some(actual_password) => {
//!                let salt = self.salt(username);
//!                pbkdf2::verify(PBKDF2_ALG, self.pbkdf2_iterations, &salt,
//!                               attempted_password.as_bytes(),
//!                               actual_password)
//!                     .map_err(|_| Error::WrongUsernameOrPassword)
//!            },
//!
//!            None => Err(Error::WrongUsernameOrPassword)
//!         }
//!     }
//!
//!     // The salt should have a user-specific component so that an attacker
//!     // cannot crack one password for multiple users in the database. It
//!     // should have a database-unique component so that an attacker cannot
//!     // crack the same user's password across databases in the unfortunate
//!     // but common case that the user has used the same password for
//!     // multiple systems.
//!     fn salt(&self, username: &str) -> Vec<u8> {
//!         let mut salt = Vec::with_capacity(self.db_salt_component.len() +
//!                                           username.as_bytes().len());
//!         salt.extend(self.db_salt_component.as_ref());
//!         salt.extend(username.as_bytes());
//!         salt
//!     }
//! }
//!
//! fn main() {
//!     // Normally these parameters would be loaded from a configuration file.
//!     let mut db = PasswordDatabase {
//!         pbkdf2_iterations: NonZeroU32::new(100_000).unwrap(),
//!         db_salt_component: [
//!             // This value was generated from a secure PRNG.
//!             0xd6, 0x26, 0x98, 0xda, 0xf4, 0xdc, 0x50, 0x52,
//!             0x24, 0xf2, 0x27, 0xd1, 0xfe, 0x39, 0x01, 0x8a
//!         ],
//!         storage: HashMap::new(),
//!     };
//!
//!     db.store_password("alice", "@74d7]404j|W}6u");
//!
//!     // An attempt to log in with the wrong password fails.
//!     assert!(db.verify_password("alice", "wrong password").is_err());
//!
//!     // Normally there should be an expoentially-increasing delay between
//!     // attempts to further protect against online attacks.
//!
//!     // An attempt to log in with the right password succeeds.
//!     assert!(db.verify_password("alice", "@74d7]404j|W}6u").is_ok());
//! }

use self::{derive_error::DeriveError, verify_error::VerifyError};
use crate::{
    bb, cpu, digest,
    error::{self, TooMuchOutputRequestedError},
    hmac::{self, InputTooLongError},
};
use core::num::NonZeroU32;

/// A PBKDF2 algorithm.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Algorithm(hmac::Algorithm);

/// PBKDF2 using HMAC-SHA1.
pub static PBKDF2_HMAC_SHA1: Algorithm = Algorithm(hmac::HMAC_SHA1_FOR_LEGACY_USE_ONLY);

/// PBKDF2 using HMAC-SHA256.
pub static PBKDF2_HMAC_SHA256: Algorithm = Algorithm(hmac::HMAC_SHA256);

/// PBKDF2 using HMAC-SHA384.
pub static PBKDF2_HMAC_SHA384: Algorithm = Algorithm(hmac::HMAC_SHA384);

/// PBKDF2 using HMAC-SHA512.
pub static PBKDF2_HMAC_SHA512: Algorithm = Algorithm(hmac::HMAC_SHA512);

/// Fills `out` with the key derived using PBKDF2 with the given inputs.
///
/// Do not use `derive` as part of verifying a secret; use `verify` instead, to
/// minimize the effectiveness of timing attacks.
///
/// `out.len()` must be no larger than the digest length * (2**32 - 1), per the
/// PBKDF2 specification.
///
/// | Parameter   | RFC 2898 Section 5.2 Term
/// |-------------|-------------------------------------------
/// | digest_alg  | PRF (HMAC with the given digest algorithm)
/// | iterations  | c (iteration count)
/// | salt        | S (salt)
/// | secret      | P (password)
/// | out         | dk (derived key)
/// | out.len()   | dkLen (derived key length)
///
/// # Panics
///
/// Panics if `out.len() > u32::MAX * digest_alg.output_len()`, where
/// `digest_alg` is the underlying HMAC/digest algorithm.
///
/// Panics if `salt` is so astronomically gigantic that it isn't a valid input
/// to the underlying digest function.
///
/// Panics if `secret` is so astronomically gigantic that it isn't a valid
/// input to the underlying digest function.
pub fn derive(
    algorithm: Algorithm,
    iterations: NonZeroU32,
    salt: &[u8],
    secret: &[u8],
    out: &mut [u8],
) {
    let cpu = cpu::features();
    try_derive(algorithm, iterations, salt, secret, out, cpu)
        .map_err(error::erase::<DeriveError>)
        .unwrap()
}

fn try_derive(
    algorithm: Algorithm,
    iterations: NonZeroU32,
    salt: &[u8],
    secret: &[u8],
    out: &mut [u8],
    cpu: cpu::Features,
) -> Result<(), DeriveError> {
    let digest_alg = algorithm.0.digest_algorithm();
    let output_len = digest_alg.output_len();

    // This implementation's performance is asymptotically optimal as described
    // in https://jbp.io/2015/08/11/pbkdf2-performance-matters/. However, it
    // hasn't been optimized to the same extent as fastpbkdf2. In particular,
    // this implementation is probably doing a lot of unnecessary copying.

    let secret =
        hmac::Key::try_new(algorithm.0, secret, cpu).map_err(DeriveError::secret_too_long)?;

    // Clear |out|.
    out.fill(0);

    let mut idx: u32 = 0;

    let out_len = out.len();
    for chunk in out.chunks_mut(output_len) {
        idx = idx.checked_add(1).ok_or_else(|| {
            DeriveError::too_much_output_requested(TooMuchOutputRequestedError::new(out_len))
        })?;
        // If the salt is too long, then we'll detect this on the first
        // iteration before we've written any output.
        derive_block(&secret, iterations, salt, idx, chunk, cpu)
            .map_err(DeriveError::salt_too_long)?;
    }
    Ok(())
}

fn derive_block(
    secret: &hmac::Key,
    iterations: NonZeroU32,
    salt: &[u8],
    idx: u32,
    out: &mut [u8],
    cpu: cpu::Features,
) -> Result<(), InputTooLongError> {
    let mut ctx = hmac::Context::with_key(secret);
    ctx.update(salt);
    ctx.update(&u32::to_be_bytes(idx));

    let mut u = ctx.try_sign(cpu)?;

    let mut remaining: u32 = iterations.into();
    loop {
        bb::xor_assign_at_start(&mut out[..], u.as_ref());

        if remaining == 1 {
            break;
        }
        remaining -= 1;

        // This will not fail, because the output of HMAC is never too long to
        // be an input for the same algorithm, but we can't prove that with
        // only locally-available information.
        u = secret.sign(u.as_ref(), cpu)?
    }
    Ok(())
}

cold_exhaustive_error! {
    enum derive_error::DeriveError {
        secret_too_long => SecretTooLong(InputTooLongError),
        salt_too_long => SaltTooLong(InputTooLongError),
        too_much_output_requested => TooMuchOutputRequested(TooMuchOutputRequestedError),
    }
}

cold_exhaustive_error! {
    enum verify_error::VerifyError {
        mismatch => Mismatch(()),
        secret_too_long => SecretTooLong(InputTooLongError),
        salt_too_long => SaltTooLong(InputTooLongError),
        previously_derived_empty => PreviouslyDerivedEmpty(usize),
    }
}

/// Verifies that a previously-derived (e.g., using `derive`) PBKDF2 value
/// matches the PBKDF2 value derived from the other inputs.
///
/// The comparison is done in constant time to prevent timing attacks. The
/// comparison will fail if `previously_derived` is empty (has a length of
/// zero).
///
/// | Parameter                  | RFC 2898 Section 5.2 Term
/// |----------------------------|--------------------------------------------
/// | digest_alg                 | PRF (HMAC with the given digest algorithm).
/// | `iterations`               | c (iteration count)
/// | `salt`                     | S (salt)
/// | `secret`                   | P (password)
/// | `previously_derived`       | dk (derived key)
/// | `previously_derived.len()` | dkLen (derived key length)
pub fn verify(
    algorithm: Algorithm,
    iterations: NonZeroU32,
    salt: &[u8],
    secret: &[u8],
    previously_derived: &[u8],
) -> Result<(), error::Unspecified> {
    let cpu = cpu::features();
    try_verify(algorithm, iterations, salt, secret, previously_derived, cpu)
        .map_err(error::erase::<VerifyError>)
}

fn try_verify(
    algorithm: Algorithm,
    iterations: NonZeroU32,
    salt: &[u8],
    secret: &[u8],
    previously_derived: &[u8],
    cpu: cpu::Features,
) -> Result<(), VerifyError> {
    let digest_alg = algorithm.0.digest_algorithm();

    if previously_derived.is_empty() {
        return Err(VerifyError::previously_derived_empty(0));
    }

    let mut derived_buf = [0u8; digest::MAX_OUTPUT_LEN];

    let output_len = digest_alg.output_len();
    let secret =
        hmac::Key::try_new(algorithm.0, secret, cpu).map_err(VerifyError::secret_too_long)?;
    let mut idx: u32 = 0;

    let mut matches = 1;

    for previously_derived_chunk in previously_derived.chunks(output_len) {
        idx = idx.checked_add(1).ok_or_else(|| {
            // `previously_derived` is so gigantic that PBKDF2 couldn't
            // have been used to compute it.
            VerifyError::mismatch(())
        })?;

        let derived_chunk = &mut derived_buf[..previously_derived_chunk.len()];
        derived_chunk.fill(0);

        derive_block(&secret, iterations, salt, idx, derived_chunk, cpu)
            .map_err(VerifyError::salt_too_long)?;

        // XXX: This isn't fully constant-time-safe. TODO: Fix that.
        #[allow(clippy::bool_to_int_with_if)]
        let current_block_matches =
            if bb::verify_slices_are_equal(derived_chunk, previously_derived_chunk).is_ok() {
                1
            } else {
                0
            };

        matches &= current_block_matches;
    }

    if matches == 0 {
        return Err(VerifyError::mismatch(()));
    }

    Ok(())
}

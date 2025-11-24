// Copyright 2015-2016 Brian Smith.
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

use super::{super::PUBLIC_KEY_PUBLIC_MODULUS_MAX_LEN, mgf1, Padding, RsaEncoding, Verification};
use crate::{bb, bits, digest, error, rand};

/// RSA PSS padding as described in [RFC 3447 Section 8.1].
///
/// See "`RSA_PSS_*` Details\" in `ring::signature`'s module-level
/// documentation for more details.
///
/// [RFC 3447 Section 8.1]: https://tools.ietf.org/html/rfc3447#section-8.1
#[allow(clippy::upper_case_acronyms)] // TODO: Until we implement cargo-semver-checks
#[derive(Debug)]
pub struct PSS {
    digest_alg: &'static digest::Algorithm,
}

impl crate::sealed::Sealed for PSS {}

impl Padding for PSS {
    fn digest_alg(&self) -> &'static digest::Algorithm {
        self.digest_alg
    }
}

impl RsaEncoding for PSS {
    // Implement padding procedure per EMSA-PSS,
    // https://tools.ietf.org/html/rfc3447#section-9.1.
    fn encode(
        &self,
        m_hash: digest::Digest,
        m_out: &mut [u8],
        mod_bits: bits::BitLength,
        rng: &dyn rand::SecureRandom,
    ) -> Result<(), error::Unspecified> {
        let metrics = PSSMetrics::new(self.digest_alg, mod_bits)?;

        // The `m_out` this function fills is the big-endian-encoded value of `m`
        // from the specification, padded to `k` bytes, where `k` is the length
        // in bytes of the public modulus. The spec says "Note that emLen will
        // be one less than k if modBits - 1 is divisible by 8 and equal to k
        // otherwise." In other words we might need to prefix `em` with a
        // leading zero byte to form a correct value of `m`.
        let em = if metrics.top_byte_mask == 0xff {
            m_out[0] = 0;
            &mut m_out[1..]
        } else {
            m_out
        };
        assert_eq!(em.len(), metrics.em_len);

        // Steps 1 and 2 are done by the caller to produce `m_hash`.

        // Step 3 is done by `PSSMetrics::new()` above.

        let (db, digest_terminator) = em.split_at_mut(metrics.db_len);

        let separator_pos = db.len() - 1 - metrics.s_len;

        // Step 4.
        let salt: &[u8] = {
            let salt = &mut db[(separator_pos + 1)..];
            rng.fill(salt)?; // salt
            salt
        };

        // Steps 5 and 6.
        let h = pss_digest(self.digest_alg, m_hash, salt);

        // Step 7.
        db[..separator_pos].fill(0); // ps

        // Step 8.
        db[separator_pos] = 0x01;

        // Steps 9 and 10.
        mgf1(self.digest_alg, h.as_ref(), db);

        // Step 11.
        db[0] &= metrics.top_byte_mask;

        // Step 12.
        digest_terminator[..metrics.h_len].copy_from_slice(h.as_ref());
        digest_terminator[metrics.h_len] = 0xbc;

        Ok(())
    }
}

impl Verification for PSS {
    // RSASSA-PSS-VERIFY from https://tools.ietf.org/html/rfc3447#section-8.1.2
    // where steps 1, 2(a), and 2(b) have been done for us.
    fn verify(
        &self,
        m_hash: digest::Digest,
        m: &mut untrusted::Reader,
        mod_bits: bits::BitLength,
    ) -> Result<(), error::Unspecified> {
        let metrics = PSSMetrics::new(self.digest_alg, mod_bits)?;

        // RSASSA-PSS-VERIFY Step 2(c). The `m` this function is given is the
        // big-endian-encoded value of `m` from the specification, padded to
        // `k` bytes, where `k` is the length in bytes of the public modulus.
        // The spec. says "Note that emLen will be one less than k if
        // modBits - 1 is divisible by 8 and equal to k otherwise," where `k`
        // is the length in octets of the RSA public modulus `n`. In other
        // words, `em` might have an extra leading zero byte that we need to
        // strip before we start the PSS decoding steps which is an artifact of
        // the `Verification` interface.
        if metrics.top_byte_mask == 0xff {
            if m.read_byte()? != 0 {
                return Err(error::Unspecified);
            }
        };
        let em = m;

        // The rest of this function is EMSA-PSS-VERIFY from
        // https://tools.ietf.org/html/rfc3447#section-9.1.2.

        // Steps 1 and 2 are done by the caller to produce `m_hash`.

        // Step 3 is done by `PSSMetrics::new()` above.

        // Step 5, out of order.
        let masked_db = em.read_bytes(metrics.db_len)?;
        let h_hash = em.read_bytes(metrics.h_len)?;

        // Step 4.
        if em.read_byte()? != 0xbc {
            return Err(error::Unspecified);
        }

        // Step 7.
        let mut db = [0u8; PUBLIC_KEY_PUBLIC_MODULUS_MAX_LEN];
        let db = &mut db[..metrics.db_len];

        mgf1(self.digest_alg, h_hash.as_slice_less_safe(), db);

        masked_db.read_all(error::Unspecified, |masked_bytes| {
            // Step 6. Check the top bits of first byte are zero.
            let b = masked_bytes.read_byte()?;
            if b & !metrics.top_byte_mask != 0 {
                return Err(error::Unspecified);
            }
            db[0] ^= b;

            // Step 8.
            let db_rest = &mut db[1..];
            let masked_bytes = masked_bytes.read_bytes(db_rest.len())?;
            bb::xor_assign_at_start(db_rest, masked_bytes.as_slice_less_safe());
            Ok(())
        })?;

        // Step 9.
        db[0] &= metrics.top_byte_mask;

        // Step 10.
        let ps_len = metrics.ps_len;
        if db[0..ps_len].iter().any(|&db| db != 0) {
            return Err(error::Unspecified);
        }
        if db[metrics.ps_len] != 1 {
            return Err(error::Unspecified);
        }

        // Step 11.
        let salt = &db[(db.len() - metrics.s_len)..];

        // Step 12 and 13.
        let h_prime = pss_digest(self.digest_alg, m_hash, salt);

        // Step 14.
        if h_hash.as_slice_less_safe() != h_prime.as_ref() {
            return Err(error::Unspecified);
        }

        Ok(())
    }
}

struct PSSMetrics {
    #[cfg_attr(not(feature = "alloc"), allow(dead_code))]
    em_len: usize,
    db_len: usize,
    ps_len: usize,
    s_len: usize,
    h_len: usize,
    top_byte_mask: u8,
}

impl PSSMetrics {
    fn new(
        digest_alg: &'static digest::Algorithm,
        mod_bits: bits::BitLength,
    ) -> Result<Self, error::Unspecified> {
        let em_bits = mod_bits.try_sub_1()?;
        let em_len = em_bits.as_usize_bytes_rounded_up();
        let leading_zero_bits = (8 * em_len) - em_bits.as_bits();
        debug_assert!(leading_zero_bits < 8);
        let top_byte_mask = 0xffu8 >> leading_zero_bits;

        let h_len = digest_alg.output_len();

        // We require the salt length to be equal to the digest length.
        let s_len = h_len;

        // Step 3 of both `EMSA-PSS-ENCODE` is `EMSA-PSS-VERIFY` requires that
        // we reject inputs where "emLen < hLen + sLen + 2". The definition of
        // `emBits` in RFC 3447 Sections 9.1.1 and 9.1.2 says `emBits` must be
        // "at least 8hLen + 8sLen + 9". Since 9 bits requires two bytes, these
        // two conditions are equivalent. 9 bits are required as the 0x01
        // before the salt requires 1 bit and the 0xbc after the digest
        // requires 8 bits.
        let db_len = em_len.checked_sub(1 + s_len).ok_or(error::Unspecified)?;
        let ps_len = db_len.checked_sub(h_len + 1).ok_or(error::Unspecified)?;

        debug_assert!(em_bits.as_bits() >= (8 * h_len) + (8 * s_len) + 9);

        Ok(Self {
            em_len,
            db_len,
            ps_len,
            s_len,
            h_len,
            top_byte_mask,
        })
    }
}

fn pss_digest(
    digest_alg: &'static digest::Algorithm,
    m_hash: digest::Digest,
    salt: &[u8],
) -> digest::Digest {
    // Fixed prefix.
    const PREFIX_ZEROS: [u8; 8] = [0u8; 8];

    // Encoding step 5 and 6, Verification step 12 and 13.
    let mut ctx = digest::Context::new(digest_alg);
    ctx.update(&PREFIX_ZEROS);
    ctx.update(m_hash.as_ref());
    ctx.update(salt);
    ctx.finish()
}

macro_rules! rsa_pss_padding {
    ( $vis:vis $PADDING_ALGORITHM:ident, $digest_alg:expr, $doc_str:expr ) => {
        #[doc=$doc_str]
        $vis static $PADDING_ALGORITHM: PSS = PSS {
            digest_alg: $digest_alg,
        };
    };
}

rsa_pss_padding!(
    pub RSA_PSS_SHA256,
    &digest::SHA256,
    "RSA PSS padding using SHA-256 for RSA signatures.\n\nSee
                 \"`RSA_PSS_*` Details\" in `ring::signature`'s module-level
                 documentation for more details."
);

rsa_pss_padding!(
    pub RSA_PSS_SHA384,
    &digest::SHA384,
    "RSA PSS padding using SHA-384 for RSA signatures.\n\nSee
                 \"`RSA_PSS_*` Details\" in `ring::signature`'s module-level
                 documentation for more details."
);

rsa_pss_padding!(
    pub RSA_PSS_SHA512,
    &digest::SHA512,
    "RSA PSS padding using SHA-512 for RSA signatures.\n\nSee
                 \"`RSA_PSS_*` Details\" in `ring::signature`'s module-level
                 documentation for more details."
);

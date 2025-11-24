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

use super::{super::PUBLIC_KEY_PUBLIC_MODULUS_MAX_LEN, Padding, RsaEncoding, Verification};
use crate::{bits, digest, error, io::der, rand};

/// PKCS#1 1.5 padding as described in [RFC 3447 Section 8.2].
///
/// See "`RSA_PSS_*` Details\" in `ring::signature`'s module-level
/// documentation for more details.
///
/// [RFC 3447 Section 8.2]: https://tools.ietf.org/html/rfc3447#section-8.2
#[derive(Debug)]
pub struct PKCS1 {
    digest_alg: &'static digest::Algorithm,
    digestinfo_prefix: &'static [u8],
}

impl crate::sealed::Sealed for PKCS1 {}

impl Padding for PKCS1 {
    fn digest_alg(&self) -> &'static digest::Algorithm {
        self.digest_alg
    }
}

impl RsaEncoding for PKCS1 {
    fn encode(
        &self,
        m_hash: digest::Digest,
        m_out: &mut [u8],
        _mod_bits: bits::BitLength,
        _rng: &dyn rand::SecureRandom,
    ) -> Result<(), error::Unspecified> {
        pkcs1_encode(self, m_hash, m_out);
        Ok(())
    }
}

impl Verification for PKCS1 {
    fn verify(
        &self,
        m_hash: digest::Digest,
        m: &mut untrusted::Reader,
        mod_bits: bits::BitLength,
    ) -> Result<(), error::Unspecified> {
        // `mod_bits.as_usize_bytes_rounded_up() <=
        //      PUBLIC_KEY_PUBLIC_MODULUS_MAX_LEN` is ensured by `verify_rsa_()`.
        let mut calculated = [0u8; PUBLIC_KEY_PUBLIC_MODULUS_MAX_LEN];
        let calculated = &mut calculated[..mod_bits.as_usize_bytes_rounded_up()];
        pkcs1_encode(self, m_hash, calculated);
        if m.read_bytes_to_end().as_slice_less_safe() != calculated {
            return Err(error::Unspecified);
        }
        Ok(())
    }
}

// Implement padding procedure per EMSA-PKCS1-v1_5,
// https://tools.ietf.org/html/rfc3447#section-9.2. This is used by both
// verification and signing so it needs to be able to handle moduli of the
// minimum and maximum sizes for both operations.
fn pkcs1_encode(pkcs1: &PKCS1, m_hash: digest::Digest, m_out: &mut [u8]) {
    let em = m_out;

    let digest_len = pkcs1.digestinfo_prefix.len() + pkcs1.digest_alg.output_len();

    // The specification requires at least 8 bytes of padding. Since we
    // disallow keys smaller than 1024 bits, this should always be true.
    assert!(em.len() >= digest_len + 11);
    let pad_len = em.len() - digest_len - 3;
    em[0] = 0;
    em[1] = 1;
    for i in 0..pad_len {
        em[2 + i] = 0xff;
    }
    em[2 + pad_len] = 0;

    let (digest_prefix, digest_dst) = em[3 + pad_len..].split_at_mut(pkcs1.digestinfo_prefix.len());
    digest_prefix.copy_from_slice(pkcs1.digestinfo_prefix);
    digest_dst.copy_from_slice(m_hash.as_ref());
}

macro_rules! rsa_pkcs1_padding {
    ( $vis:vis $PADDING_ALGORITHM:ident, $digest_alg:expr, $digestinfo_prefix:expr,
      $doc_str:expr ) => {
        #[doc=$doc_str]
        $vis static $PADDING_ALGORITHM: PKCS1 = PKCS1 {
            digest_alg: $digest_alg,
            digestinfo_prefix: $digestinfo_prefix,
        };
    };
}

// Intentionally not exposed except internally for signature verification. At a
// minimum, we'd need to create test vectors for signing with it, which we
// don't currently have. But, it's a bad idea to use SHA-1 anyway, so perhaps
// we just won't ever expose it.
rsa_pkcs1_padding!(
    pub(in super::super) RSA_PKCS1_SHA1_FOR_LEGACY_USE_ONLY,
    &digest::SHA1_FOR_LEGACY_USE_ONLY,
    &SHA1_PKCS1_DIGESTINFO_PREFIX,
    "PKCS#1 1.5 padding using SHA-1 for RSA signatures."
);

rsa_pkcs1_padding!(
    pub RSA_PKCS1_SHA256,
    &digest::SHA256,
    &SHA256_PKCS1_DIGESTINFO_PREFIX,
    "PKCS#1 1.5 padding using SHA-256 for RSA signatures."
);

rsa_pkcs1_padding!(
    pub RSA_PKCS1_SHA384,
    &digest::SHA384,
    &SHA384_PKCS1_DIGESTINFO_PREFIX,
    "PKCS#1 1.5 padding using SHA-384 for RSA signatures."
);

rsa_pkcs1_padding!(
    pub RSA_PKCS1_SHA512,
    &digest::SHA512,
    &SHA512_PKCS1_DIGESTINFO_PREFIX,
    "PKCS#1 1.5 padding using SHA-512 for RSA signatures."
);

macro_rules! pkcs1_digestinfo_prefix {
    ( $name:ident, $digest_len:expr, $digest_oid_len:expr,
      [ $( $digest_oid:expr ),* ] ) => {
        static $name: [u8; 2 + 8 + $digest_oid_len] = [
            der::Tag::Sequence.into(), 8 + $digest_oid_len + $digest_len,
                der::Tag::Sequence.into(), 2 + $digest_oid_len + 2,
                    der::Tag::OID.into(), $digest_oid_len, $( $digest_oid ),*,
                    der::Tag::Null.into(), 0,
                der::Tag::OctetString.into(), $digest_len,
        ];
    }
}

pkcs1_digestinfo_prefix!(
    SHA1_PKCS1_DIGESTINFO_PREFIX,
    20,
    5,
    [0x2b, 0x0e, 0x03, 0x02, 0x1a]
);

pkcs1_digestinfo_prefix!(
    SHA256_PKCS1_DIGESTINFO_PREFIX,
    32,
    9,
    [0x60, 0x86, 0x48, 0x01, 0x65, 0x03, 0x04, 0x02, 0x01]
);

pkcs1_digestinfo_prefix!(
    SHA384_PKCS1_DIGESTINFO_PREFIX,
    48,
    9,
    [0x60, 0x86, 0x48, 0x01, 0x65, 0x03, 0x04, 0x02, 0x02]
);

pkcs1_digestinfo_prefix!(
    SHA512_PKCS1_DIGESTINFO_PREFIX,
    64,
    9,
    [0x60, 0x86, 0x48, 0x01, 0x65, 0x03, 0x04, 0x02, 0x03]
);

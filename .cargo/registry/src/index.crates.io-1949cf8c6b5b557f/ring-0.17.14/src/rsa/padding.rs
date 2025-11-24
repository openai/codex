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

use crate::{bb, bits, digest, error, rand};

mod pkcs1;
mod pss;

pub use self::{
    pkcs1::{RSA_PKCS1_SHA256, RSA_PKCS1_SHA384, RSA_PKCS1_SHA512},
    pss::{RSA_PSS_SHA256, RSA_PSS_SHA384, RSA_PSS_SHA512},
};
pub(super) use pkcs1::RSA_PKCS1_SHA1_FOR_LEGACY_USE_ONLY;

/// Common features of both RSA padding encoding and RSA padding verification.
pub trait Padding: 'static + Sync + crate::sealed::Sealed + core::fmt::Debug {
    // The digest algorithm used for digesting the message (and maybe for
    // other things).
    fn digest_alg(&self) -> &'static digest::Algorithm;
}

pub(super) fn encode(
    encoding: &dyn RsaEncoding,
    m_hash: digest::Digest,
    m_out: &mut [u8],
    mod_bits: bits::BitLength,
    rng: &dyn rand::SecureRandom,
) -> Result<(), error::Unspecified> {
    #[allow(deprecated)]
    encoding.encode(m_hash, m_out, mod_bits, rng)
}

/// An RSA signature encoding as described in [RFC 3447 Section 8].
///
/// [RFC 3447 Section 8]: https://tools.ietf.org/html/rfc3447#section-8
#[cfg(feature = "alloc")]
pub trait RsaEncoding: Padding {
    #[deprecated(note = "internal API that will be removed")]
    #[doc(hidden)]
    fn encode(
        &self,
        m_hash: digest::Digest,
        m_out: &mut [u8],
        mod_bits: bits::BitLength,
        rng: &dyn rand::SecureRandom,
    ) -> Result<(), error::Unspecified>;
}

/// Verification of an RSA signature encoding as described in
/// [RFC 3447 Section 8].
///
/// [RFC 3447 Section 8]: https://tools.ietf.org/html/rfc3447#section-8
pub trait Verification: Padding {
    fn verify(
        &self,
        m_hash: digest::Digest,
        m: &mut untrusted::Reader,
        mod_bits: bits::BitLength,
    ) -> Result<(), error::Unspecified>;
}

// Masks `out` with the output of the mask-generating function MGF1 as
// described in https://tools.ietf.org/html/rfc3447#appendix-B.2.1.
fn mgf1(digest_alg: &'static digest::Algorithm, seed: &[u8], out: &mut [u8]) {
    let digest_len = digest_alg.output_len();

    // Maximum counter value is the value of (mask_len / digest_len) rounded up.
    for (i, out) in out.chunks_mut(digest_len).enumerate() {
        let mut ctx = digest::Context::new(digest_alg);
        ctx.update(seed);
        // The counter will always fit in a `u32` because we reject absurdly
        // long inputs very early.
        ctx.update(&u32::to_be_bytes(i.try_into().unwrap()));
        let digest = ctx.finish();

        // The last chunk may legitimately be shorter than `digest`, but
        // `digest` will never be shorter than `out`.
        bb::xor_assign_at_start(out, digest.as_ref());
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::testutil as test;
    use crate::{digest, error};
    use alloc::vec;

    #[test]
    fn test_pss_padding_verify() {
        test::run(
            test_vector_file!("rsa_pss_padding_tests.txt"),
            |section, test_case| {
                assert_eq!(section, "");

                let digest_name = test_case.consume_string("Digest");
                let alg = match digest_name.as_ref() {
                    "SHA256" => &RSA_PSS_SHA256,
                    "SHA384" => &RSA_PSS_SHA384,
                    "SHA512" => &RSA_PSS_SHA512,
                    _ => panic!("Unsupported digest: {}", digest_name),
                };

                let msg = test_case.consume_bytes("Msg");
                let msg = untrusted::Input::from(&msg);
                let m_hash = digest::digest(alg.digest_alg(), msg.as_slice_less_safe());

                let encoded = test_case.consume_bytes("EM");
                let encoded = untrusted::Input::from(&encoded);

                // Salt is recomputed in verification algorithm.
                let _ = test_case.consume_bytes("Salt");

                let bit_len = test_case.consume_usize_bits("Len");
                let is_valid = test_case.consume_string("Result") == "P";

                let actual_result =
                    encoded.read_all(error::Unspecified, |m| alg.verify(m_hash, m, bit_len));
                assert_eq!(actual_result.is_ok(), is_valid);

                Ok(())
            },
        );
    }

    // Tests PSS encoding for various public modulus lengths.
    #[cfg(feature = "alloc")]
    #[test]
    fn test_pss_padding_encode() {
        test::run(
            test_vector_file!("rsa_pss_padding_tests.txt"),
            |section, test_case| {
                assert_eq!(section, "");

                let digest_name = test_case.consume_string("Digest");
                let alg = match digest_name.as_ref() {
                    "SHA256" => &RSA_PSS_SHA256,
                    "SHA384" => &RSA_PSS_SHA384,
                    "SHA512" => &RSA_PSS_SHA512,
                    _ => panic!("Unsupported digest: {}", digest_name),
                };

                let msg = test_case.consume_bytes("Msg");
                let salt = test_case.consume_bytes("Salt");
                let encoded = test_case.consume_bytes("EM");
                let bit_len = test_case.consume_usize_bits("Len");
                let expected_result = test_case.consume_string("Result");

                // Only test the valid outputs
                if expected_result != "P" {
                    return Ok(());
                }

                let rng = test::rand::FixedSliceRandom { bytes: &salt };

                let mut m_out = vec![0u8; bit_len.as_usize_bytes_rounded_up()];
                let digest = digest::digest(alg.digest_alg(), &msg);
                #[allow(deprecated)]
                alg.encode(digest, &mut m_out, bit_len, &rng).unwrap();
                assert_eq!(m_out, encoded);

                Ok(())
            },
        );
    }
}

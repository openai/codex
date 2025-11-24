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

//! Verification of RSA signatures.

use super::{
    parse_public_key, public_key, PublicExponent, RsaParameters, PUBLIC_KEY_PUBLIC_MODULUS_MAX_LEN,
};
use crate::{
    bits::{self, FromByteLen as _},
    cpu, digest,
    error::{self, InputTooLongError},
    sealed, signature,
};

impl signature::VerificationAlgorithm for RsaParameters {
    fn verify(
        &self,
        public_key: untrusted::Input,
        msg: untrusted::Input,
        signature: untrusted::Input,
    ) -> Result<(), error::Unspecified> {
        let (n, e) = parse_public_key(public_key)?;
        verify_rsa_(
            self,
            (
                n.big_endian_without_leading_zero_as_input(),
                e.big_endian_without_leading_zero_as_input(),
            ),
            msg,
            signature,
            cpu::features(),
        )
    }
}

impl sealed::Sealed for RsaParameters {}

macro_rules! rsa_params {
    ( $VERIFY_ALGORITHM:ident, $min_bits:expr, $PADDING_ALGORITHM:expr,
      $doc_str:expr ) => {
        #[doc=$doc_str]
        ///
        /// Only available in `alloc` mode.
        pub static $VERIFY_ALGORITHM: RsaParameters = RsaParameters {
            padding_alg: $PADDING_ALGORITHM,
            min_bits: bits::BitLength::from_bits($min_bits),
        };
    };
}

rsa_params!(
    RSA_PKCS1_1024_8192_SHA1_FOR_LEGACY_USE_ONLY,
    1024,
    &super::padding::RSA_PKCS1_SHA1_FOR_LEGACY_USE_ONLY,
    "Verification of signatures using RSA keys of 1024-8192 bits,
             PKCS#1.5 padding, and SHA-1.\n\nSee \"`RSA_PKCS1_*` Details\" in
             `ring::signature`'s module-level documentation for more details."
);
rsa_params!(
    RSA_PKCS1_2048_8192_SHA1_FOR_LEGACY_USE_ONLY,
    2048,
    &super::padding::RSA_PKCS1_SHA1_FOR_LEGACY_USE_ONLY,
    "Verification of signatures using RSA keys of 2048-8192 bits,
             PKCS#1.5 padding, and SHA-1.\n\nSee \"`RSA_PKCS1_*` Details\" in
             `ring::signature`'s module-level documentation for more details."
);
rsa_params!(
    RSA_PKCS1_1024_8192_SHA256_FOR_LEGACY_USE_ONLY,
    1024,
    &super::padding::RSA_PKCS1_SHA256,
    "Verification of signatures using RSA keys of 1024-8192 bits,
             PKCS#1.5 padding, and SHA-256.\n\nSee \"`RSA_PKCS1_*` Details\" in
             `ring::signature`'s module-level documentation for more details."
);
rsa_params!(
    RSA_PKCS1_2048_8192_SHA256,
    2048,
    &super::padding::RSA_PKCS1_SHA256,
    "Verification of signatures using RSA keys of 2048-8192 bits,
             PKCS#1.5 padding, and SHA-256.\n\nSee \"`RSA_PKCS1_*` Details\" in
             `ring::signature`'s module-level documentation for more details."
);
rsa_params!(
    RSA_PKCS1_2048_8192_SHA384,
    2048,
    &super::padding::RSA_PKCS1_SHA384,
    "Verification of signatures using RSA keys of 2048-8192 bits,
             PKCS#1.5 padding, and SHA-384.\n\nSee \"`RSA_PKCS1_*` Details\" in
             `ring::signature`'s module-level documentation for more details."
);
rsa_params!(
    RSA_PKCS1_2048_8192_SHA512,
    2048,
    &super::padding::RSA_PKCS1_SHA512,
    "Verification of signatures using RSA keys of 2048-8192 bits,
             PKCS#1.5 padding, and SHA-512.\n\nSee \"`RSA_PKCS1_*` Details\" in
             `ring::signature`'s module-level documentation for more details."
);
rsa_params!(
    RSA_PKCS1_1024_8192_SHA512_FOR_LEGACY_USE_ONLY,
    1024,
    &super::padding::RSA_PKCS1_SHA512,
    "Verification of signatures using RSA keys of 1024-8192 bits,
             PKCS#1.5 padding, and SHA-512.\n\nSee \"`RSA_PKCS1_*` Details\" in
             `ring::signature`'s module-level documentation for more details."
);
rsa_params!(
    RSA_PKCS1_3072_8192_SHA384,
    3072,
    &super::padding::RSA_PKCS1_SHA384,
    "Verification of signatures using RSA keys of 3072-8192 bits,
             PKCS#1.5 padding, and SHA-384.\n\nSee \"`RSA_PKCS1_*` Details\" in
             `ring::signature`'s module-level documentation for more details."
);

rsa_params!(
    RSA_PSS_2048_8192_SHA256,
    2048,
    &super::padding::RSA_PSS_SHA256,
    "Verification of signatures using RSA keys of 2048-8192 bits,
             PSS padding, and SHA-256.\n\nSee \"`RSA_PSS_*` Details\" in
             `ring::signature`'s module-level documentation for more details."
);
rsa_params!(
    RSA_PSS_2048_8192_SHA384,
    2048,
    &super::padding::RSA_PSS_SHA384,
    "Verification of signatures using RSA keys of 2048-8192 bits,
             PSS padding, and SHA-384.\n\nSee \"`RSA_PSS_*` Details\" in
             `ring::signature`'s module-level documentation for more details."
);
rsa_params!(
    RSA_PSS_2048_8192_SHA512,
    2048,
    &super::padding::RSA_PSS_SHA512,
    "Verification of signatures using RSA keys of 2048-8192 bits,
             PSS padding, and SHA-512.\n\nSee \"`RSA_PSS_*` Details\" in
             `ring::signature`'s module-level documentation for more details."
);

pub use super::PublicKeyComponents as RsaPublicKeyComponents;

impl<B> super::PublicKeyComponents<B>
where
    B: AsRef<[u8]>,
{
    /// Verifies that `signature` is a valid signature of `message` using `self`
    /// as the public key. `params` determine what algorithm parameters
    /// (padding, digest algorithm, key length range, etc.) are used in the
    /// verification.
    ///
    /// When the public key is in DER-encoded PKCS#1 ASN.1 format, it is
    /// recommended to use `ring::signature::verify()` with
    /// `ring::signature::RSA_PKCS1_*`, because `ring::signature::verify()`
    /// will handle the parsing in that case. Otherwise, this function can be used
    /// to pass in the raw bytes for the public key components as
    /// `untrusted::Input` arguments.
    //
    // There are a small number of tests that test this directly, but the
    // test coverage for this function mostly depends on the test coverage for the
    // `signature::VerificationAlgorithm` implementation for `RsaParameters`. If we
    // change that, test coverage for `verify_rsa()` will need to be reconsidered.
    // (The NIST test vectors were originally in a form that was optimized for
    // testing `verify_rsa` directly, but the testing work for RSA PKCS#1
    // verification was done during the implementation of
    // `signature::VerificationAlgorithm`, before `verify_rsa` was factored out).
    pub fn verify(
        &self,
        params: &RsaParameters,
        message: &[u8],
        signature: &[u8],
    ) -> Result<(), error::Unspecified> {
        verify_rsa_(
            params,
            (
                untrusted::Input::from(self.n.as_ref()),
                untrusted::Input::from(self.e.as_ref()),
            ),
            untrusted::Input::from(message),
            untrusted::Input::from(signature),
            cpu::features(),
        )
    }
}

pub(crate) fn verify_rsa_(
    params: &RsaParameters,
    (n, e): (untrusted::Input, untrusted::Input),
    msg: untrusted::Input,
    signature: untrusted::Input,
    cpu_features: cpu::Features,
) -> Result<(), error::Unspecified> {
    let max_bits: bits::BitLength =
        bits::BitLength::from_byte_len(PUBLIC_KEY_PUBLIC_MODULUS_MAX_LEN)
            .map_err(error::erase::<InputTooLongError>)?;

    // XXX: FIPS 186-4 seems to indicate that the minimum
    // exponent value is 2**16 + 1, but it isn't clear if this is just for
    // signing or also for verification. We support exponents of 3 and larger
    // for compatibility with other commonly-used crypto libraries.
    let key = public_key::Inner::from_modulus_and_exponent(
        n,
        e,
        params.min_bits,
        max_bits,
        PublicExponent::_3,
        cpu_features,
    )?;

    // RFC 8017 Section 5.2.2: RSAVP1.
    let mut decoded = [0u8; PUBLIC_KEY_PUBLIC_MODULUS_MAX_LEN];
    let decoded = key.exponentiate(signature, &mut decoded, cpu_features)?;

    // Verify the padded message is correct.
    let m_hash = digest::digest(params.padding_alg.digest_alg(), msg.as_slice_less_safe());
    untrusted::Input::from(decoded).read_all(error::Unspecified, |m| {
        params.padding_alg.verify(m_hash, m, key.n().len_bits())
    })
}

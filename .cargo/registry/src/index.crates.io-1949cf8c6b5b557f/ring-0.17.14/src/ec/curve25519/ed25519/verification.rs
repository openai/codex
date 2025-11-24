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

//! EdDSA Signatures.

use super::{super::ops::*, eddsa_digest};
use crate::{cpu, error, sealed, signature};

/// Parameters for EdDSA signing and verification.
pub struct EdDSAParameters;

impl core::fmt::Debug for EdDSAParameters {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> Result<(), core::fmt::Error> {
        write!(f, "ring::signature::ED25519")
    }
}

/// Verification of [Ed25519] signatures.
///
/// Ed25519 uses SHA-512 as the digest algorithm.
///
/// [Ed25519]: https://ed25519.cr.yp.to/
pub static ED25519: EdDSAParameters = EdDSAParameters {};

impl signature::VerificationAlgorithm for EdDSAParameters {
    fn verify(
        &self,
        public_key: untrusted::Input,
        msg: untrusted::Input,
        signature: untrusted::Input,
    ) -> Result<(), error::Unspecified> {
        let cpu_features = cpu::features();

        let public_key: &[u8; ELEM_LEN] = public_key.as_slice_less_safe().try_into()?;
        let (signature_r, signature_s) = signature.read_all(error::Unspecified, |input| {
            let signature_r: &[u8; ELEM_LEN] = input
                .read_bytes(ELEM_LEN)?
                .as_slice_less_safe()
                .try_into()?;
            let signature_s: &[u8; SCALAR_LEN] = input
                .read_bytes(SCALAR_LEN)?
                .as_slice_less_safe()
                .try_into()?;
            Ok((signature_r, signature_s))
        })?;

        let signature_s = Scalar::from_bytes_checked(*signature_s)?;

        let mut a = ExtPoint::from_encoded_point_vartime(public_key)?;
        a.invert_vartime();

        let h_digest = eddsa_digest(signature_r, public_key, msg.as_slice_less_safe());
        let h = Scalar::from_sha512_digest_reduced(h_digest);

        let mut r = Point::new_at_infinity();
        unsafe { x25519_ge_double_scalarmult_vartime(&mut r, &h, &a, &signature_s) };
        let r_check = r.into_encoded_point(cpu_features);
        if *signature_r != r_check {
            return Err(error::Unspecified);
        }
        Ok(())
    }
}

impl sealed::Sealed for EdDSAParameters {}

prefixed_extern! {
    fn x25519_ge_double_scalarmult_vartime(
        r: &mut Point,
        a_coeff: &Scalar,
        a: &ExtPoint,
        b_coeff: &Scalar,
    );
}

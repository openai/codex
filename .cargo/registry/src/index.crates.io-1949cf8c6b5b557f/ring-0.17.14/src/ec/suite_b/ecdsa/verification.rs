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

//! ECDSA Signatures using the P-256 and P-384 curves.

use super::digest_scalar::digest_scalar;
use crate::{
    arithmetic::montgomery::*,
    cpu, digest,
    ec::suite_b::{ops::*, public_key::*, verify_jacobian_point_is_on_the_curve},
    error,
    io::der,
    limb, sealed, signature,
};

/// An ECDSA verification algorithm.
pub struct EcdsaVerificationAlgorithm {
    ops: &'static PublicScalarOps,
    digest_alg: &'static digest::Algorithm,
    split_rs:
        for<'a> fn(
            ops: &'static ScalarOps,
            input: &mut untrusted::Reader<'a>,
        )
            -> Result<(untrusted::Input<'a>, untrusted::Input<'a>), error::Unspecified>,
    id: AlgorithmID,
}

#[derive(Debug)]
enum AlgorithmID {
    ECDSA_P256_SHA256_ASN1,
    ECDSA_P256_SHA256_FIXED,
    ECDSA_P256_SHA384_ASN1,
    ECDSA_P384_SHA256_ASN1,
    ECDSA_P384_SHA384_ASN1,
    ECDSA_P384_SHA384_FIXED,
}

derive_debug_via_id!(EcdsaVerificationAlgorithm);

impl signature::VerificationAlgorithm for EcdsaVerificationAlgorithm {
    fn verify(
        &self,
        public_key: untrusted::Input,
        msg: untrusted::Input,
        signature: untrusted::Input,
    ) -> Result<(), error::Unspecified> {
        let cpu = cpu::features();
        let e = {
            // NSA Guide Step 2: "Use the selected hash function to compute H =
            // Hash(M)."
            let h = digest::digest(self.digest_alg, msg.as_slice_less_safe());

            // NSA Guide Step 3: "Convert the bit string H to an integer e as
            // described in Appendix B.2."
            let n = &self.ops.scalar_ops.scalar_modulus(cpu);
            digest_scalar(n, h)
        };

        self.verify_digest(public_key, e, signature)
    }
}

impl EcdsaVerificationAlgorithm {
    /// This is intentionally not public.
    fn verify_digest(
        &self,
        public_key: untrusted::Input,
        e: Scalar,
        signature: untrusted::Input,
    ) -> Result<(), error::Unspecified> {
        let cpu = cpu::features();

        // NSA Suite B Implementer's Guide to ECDSA Section 3.4.2.

        let public_key_ops = self.ops.public_key_ops;
        let scalar_ops = self.ops.scalar_ops;
        let q = &public_key_ops.common.elem_modulus(cpu);
        let n = &scalar_ops.scalar_modulus(cpu);

        // NSA Guide Prerequisites:
        //
        //    Prior to accepting a verified digital signature as valid the
        //    verifier shall have:
        //
        //    1. assurance of the signatory’s claimed identity,
        //    2. an authentic copy of the domain parameters, (q, FR, a, b, SEED,
        //       G, n, h),
        //    3. assurance of the validity of the public key, and
        //    4. assurance that the claimed signatory actually possessed the
        //       private key that was used to generate the digital signature at
        //       the time that the signature was generated.
        //
        // Prerequisites #1 and #4 are outside the scope of what this function
        // can do. Prerequisite #2 is handled implicitly as the domain
        // parameters are hard-coded into the source. Prerequisite #3 is
        // handled by `parse_uncompressed_point`.
        let peer_pub_key = parse_uncompressed_point(public_key_ops, q, public_key)?;

        let (r, s) = signature.read_all(error::Unspecified, |input| {
            (self.split_rs)(scalar_ops, input)
        })?;

        // NSA Guide Step 1: "If r and s are not both integers in the interval
        // [1, n − 1], output INVALID."
        let r = scalar_parse_big_endian_variable(n, limb::AllowZero::No, r)?;
        let s = scalar_parse_big_endian_variable(n, limb::AllowZero::No, s)?;

        // NSA Guide Step 4: "Compute w = s**−1 mod n, using the routine in
        // Appendix B.1."
        let w = self.ops.scalar_inv_to_mont_vartime(&s, cpu);

        // NSA Guide Step 5: "Compute u1 = (e * w) mod n, and compute
        // u2 = (r * w) mod n."
        let u1 = scalar_ops.scalar_product(&e, &w, cpu);
        let u2 = scalar_ops.scalar_product(&r, &w, cpu);

        // NSA Guide Step 6: "Compute the elliptic curve point
        // R = (xR, yR) = u1*G + u2*Q, using EC scalar multiplication and EC
        // addition. If R is equal to the point at infinity, output INVALID."
        let product = (self.ops.twin_mul)(&u1, &u2, &peer_pub_key, cpu);

        // Verify that the point we computed is on the curve; see
        // `verify_affine_point_is_on_the_curve_scaled` for details on why. It
        // would be more secure to do the check on the affine coordinates if we
        // were going to convert to affine form (again, see
        // `verify_affine_point_is_on_the_curve_scaled` for details on why).
        // But, we're going to avoid converting to affine for performance
        // reasons, so we do the verification using the Jacobian coordinates.
        let z2 = verify_jacobian_point_is_on_the_curve(q, &product)?;

        // NSA Guide Step 7: "Compute v = xR mod n."
        // NSA Guide Step 8: "Compare v and r0. If v = r0, output VALID;
        // otherwise, output INVALID."
        //
        // Instead, we use Greg Maxwell's trick to avoid the inversion mod `q`
        // that would be necessary to compute the affine X coordinate.
        let x = q.point_x(&product);
        fn sig_r_equals_x(q: &Modulus<Q>, r: &Elem<Unencoded>, x: &Elem<R>, z2: &Elem<R>) -> bool {
            let r_jacobian = q.elem_product(z2, r);
            let x = q.elem_unencoded(x);
            q.elems_are_equal(&r_jacobian, &x).leak()
        }
        let mut r = self.ops.scalar_as_elem(&r);
        if sig_r_equals_x(q, &r, &x, &z2) {
            return Ok(());
        }
        if q.elem_less_than_vartime(&r, &self.ops.q_minus_n) {
            let n = Elem::from(self.ops.n());
            q.add_assign(&mut r, &n);
            if sig_r_equals_x(q, &r, &x, &z2) {
                return Ok(());
            }
        }

        Err(error::Unspecified)
    }
}

impl sealed::Sealed for EcdsaVerificationAlgorithm {}

fn split_rs_fixed<'a>(
    ops: &'static ScalarOps,
    input: &mut untrusted::Reader<'a>,
) -> Result<(untrusted::Input<'a>, untrusted::Input<'a>), error::Unspecified> {
    let scalar_len = ops.scalar_bytes_len();
    let r = input.read_bytes(scalar_len)?;
    let s = input.read_bytes(scalar_len)?;
    Ok((r, s))
}

fn split_rs_asn1<'a>(
    _ops: &'static ScalarOps,
    input: &mut untrusted::Reader<'a>,
) -> Result<(untrusted::Input<'a>, untrusted::Input<'a>), error::Unspecified> {
    der::nested(input, der::Tag::Sequence, error::Unspecified, |input| {
        let r = der::positive_integer(input)?.big_endian_without_leading_zero_as_input();
        let s = der::positive_integer(input)?.big_endian_without_leading_zero_as_input();
        Ok((r, s))
    })
}

/// Verification of fixed-length (PKCS#11 style) ECDSA signatures using the
/// P-256 curve and SHA-256.
///
/// See "`ECDSA_*_FIXED` Details" in `ring::signature`'s module-level
/// documentation for more details.
pub static ECDSA_P256_SHA256_FIXED: EcdsaVerificationAlgorithm = EcdsaVerificationAlgorithm {
    ops: &p256::PUBLIC_SCALAR_OPS,
    digest_alg: &digest::SHA256,
    split_rs: split_rs_fixed,
    id: AlgorithmID::ECDSA_P256_SHA256_FIXED,
};

/// Verification of fixed-length (PKCS#11 style) ECDSA signatures using the
/// P-384 curve and SHA-384.
///
/// See "`ECDSA_*_FIXED` Details" in `ring::signature`'s module-level
/// documentation for more details.
pub static ECDSA_P384_SHA384_FIXED: EcdsaVerificationAlgorithm = EcdsaVerificationAlgorithm {
    ops: &p384::PUBLIC_SCALAR_OPS,
    digest_alg: &digest::SHA384,
    split_rs: split_rs_fixed,
    id: AlgorithmID::ECDSA_P384_SHA384_FIXED,
};

/// Verification of ASN.1 DER-encoded ECDSA signatures using the P-256 curve
/// and SHA-256.
///
/// See "`ECDSA_*_ASN1` Details" in `ring::signature`'s module-level
/// documentation for more details.
pub static ECDSA_P256_SHA256_ASN1: EcdsaVerificationAlgorithm = EcdsaVerificationAlgorithm {
    ops: &p256::PUBLIC_SCALAR_OPS,
    digest_alg: &digest::SHA256,
    split_rs: split_rs_asn1,
    id: AlgorithmID::ECDSA_P256_SHA256_ASN1,
};

/// *Not recommended*. Verification of ASN.1 DER-encoded ECDSA signatures using
/// the P-256 curve and SHA-384.
///
/// In most situations, P-256 should be used only with SHA-256 and P-384
/// should be used only with SHA-384. However, in some cases, particularly TLS
/// on the web, it is necessary to support P-256 with SHA-384 for compatibility
/// with widely-deployed implementations that do not follow these guidelines.
///
/// See "`ECDSA_*_ASN1` Details" in `ring::signature`'s module-level
/// documentation for more details.
pub static ECDSA_P256_SHA384_ASN1: EcdsaVerificationAlgorithm = EcdsaVerificationAlgorithm {
    ops: &p256::PUBLIC_SCALAR_OPS,
    digest_alg: &digest::SHA384,
    split_rs: split_rs_asn1,
    id: AlgorithmID::ECDSA_P256_SHA384_ASN1,
};

/// *Not recommended*. Verification of ASN.1 DER-encoded ECDSA signatures using
/// the P-384 curve and SHA-256.
///
/// In most situations, P-256 should be used only with SHA-256 and P-384
/// should be used only with SHA-384. However, in some cases, particularly TLS
/// on the web, it is necessary to support P-256 with SHA-384 for compatibility
/// with widely-deployed implementations that do not follow these guidelines.
///
/// See "`ECDSA_*_ASN1` Details" in `ring::signature`'s module-level
/// documentation for more details.
pub static ECDSA_P384_SHA256_ASN1: EcdsaVerificationAlgorithm = EcdsaVerificationAlgorithm {
    ops: &p384::PUBLIC_SCALAR_OPS,
    digest_alg: &digest::SHA256,
    split_rs: split_rs_asn1,
    id: AlgorithmID::ECDSA_P384_SHA256_ASN1,
};

/// Verification of ASN.1 DER-encoded ECDSA signatures using the P-384 curve
/// and SHA-384.
///
/// See "`ECDSA_*_ASN1` Details" in `ring::signature`'s module-level
/// documentation for more details.
pub static ECDSA_P384_SHA384_ASN1: EcdsaVerificationAlgorithm = EcdsaVerificationAlgorithm {
    ops: &p384::PUBLIC_SCALAR_OPS,
    digest_alg: &digest::SHA384,
    split_rs: split_rs_asn1,
    id: AlgorithmID::ECDSA_P384_SHA384_ASN1,
};

#[cfg(test)]
mod tests {
    extern crate alloc;
    use super::*;
    use crate::testutil as test;
    use alloc::{vec, vec::Vec};

    #[test]
    fn test_digest_based_test_vectors() {
        let cpu = cpu::features();
        test::run(
            test_vector_file!("../../../../crypto/fipsmodule/ecdsa/ecdsa_verify_tests.txt"),
            |section, test_case| {
                assert_eq!(section, "");

                let curve_name = test_case.consume_string("Curve");

                let public_key = {
                    let mut public_key = vec![0x04];
                    public_key.extend(&test_case.consume_bytes("X"));
                    public_key.extend(&test_case.consume_bytes("Y"));
                    public_key
                };

                let digest = test_case.consume_bytes("Digest");

                let sig = {
                    let mut sig = Vec::new();
                    sig.extend(&test_case.consume_bytes("R"));
                    sig.extend(&test_case.consume_bytes("S"));
                    sig
                };

                let invalid = test_case.consume_optional_string("Invalid");

                let alg = match curve_name.as_str() {
                    "P-256" => &ECDSA_P256_SHA256_FIXED,
                    "P-384" => &ECDSA_P384_SHA384_FIXED,
                    _ => {
                        panic!("Unsupported curve: {}", curve_name);
                    }
                };
                let n = &alg.ops.scalar_ops.scalar_modulus(cpu);

                let digest = super::super::digest_scalar::digest_bytes_scalar(n, &digest[..]);
                let actual_result = alg.verify_digest(
                    untrusted::Input::from(&public_key[..]),
                    digest,
                    untrusted::Input::from(&sig[..]),
                );
                assert_eq!(actual_result.is_ok(), invalid.is_none());

                Ok(())
            },
        );
    }
}

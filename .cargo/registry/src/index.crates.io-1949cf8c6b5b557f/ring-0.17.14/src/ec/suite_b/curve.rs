// Copyright 2015-2017 Brian Smith.
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

use crate::{cpu, ec, error, rand};

/// A key agreement algorithm.
macro_rules! suite_b_curve {
    ( $NAME:ident, $bits:expr, $private_key_ops:expr, $id:expr,
      $check_private_key_bytes:ident, $generate_private_key:ident,
      $public_from_private:ident) => {
        /// Public keys are encoding in uncompressed form using the
        /// Octet-String-to-Elliptic-Curve-Point algorithm in
        /// [SEC 1: Elliptic Curve Cryptography, Version 2.0]. Public keys are
        /// validated during key agreement according to
        /// [NIST Special Publication 800-56A, revision 2] and Appendix B.3 of
        /// the NSA's [Suite B Implementer's Guide to NIST SP 800-56A].
        ///
        /// [SEC 1: Elliptic Curve Cryptography, Version 2.0]:
        ///     http://www.secg.org/sec1-v2.pdf
        /// [NIST Special Publication 800-56A, revision 2]:
        ///     http://nvlpubs.nist.gov/nistpubs/SpecialPublications/NIST.SP.800-56Ar2.pdf
        /// [Suite B Implementer's Guide to NIST SP 800-56A]:
        ///     https://github.com/briansmith/ring/blob/main/doc/ecdh.pdf
        pub static $NAME: ec::Curve = ec::Curve {
            public_key_len: 1 + (2 * (($bits + 7) / 8)),
            elem_scalar_seed_len: ($bits + 7) / 8,
            id: $id,
            check_private_key_bytes: $check_private_key_bytes,
            generate_private_key: $generate_private_key,
            public_from_private: $public_from_private,
        };

        fn $check_private_key_bytes(
            bytes: &[u8],
            cpu: cpu::Features,
        ) -> Result<(), error::Unspecified> {
            debug_assert_eq!(bytes.len(), $bits / 8);
            ec::suite_b::private_key::check_scalar_big_endian_bytes($private_key_ops, bytes, cpu)
        }

        fn $generate_private_key(
            rng: &dyn rand::SecureRandom,
            out: &mut [u8],
            cpu: cpu::Features,
        ) -> Result<(), error::Unspecified> {
            ec::suite_b::private_key::generate_private_scalar_bytes($private_key_ops, rng, out, cpu)
        }

        fn $public_from_private(
            public_out: &mut [u8],
            private_key: &ec::Seed,
            cpu: cpu::Features,
        ) -> Result<(), error::Unspecified> {
            ec::suite_b::private_key::public_from_private(
                $private_key_ops,
                public_out,
                private_key,
                cpu,
            )
        }
    };
}

suite_b_curve!(
    P256,
    256,
    &ec::suite_b::ops::p256::PRIVATE_KEY_OPS,
    ec::CurveID::P256,
    p256_check_private_key_bytes,
    p256_generate_private_key,
    p256_public_from_private
);

suite_b_curve!(
    P384,
    384,
    &ec::suite_b::ops::p384::PRIVATE_KEY_OPS,
    ec::CurveID::P384,
    p384_check_private_key_bytes,
    p384_generate_private_key,
    p384_public_from_private
);

// Copyright 2016 Brian Smith.
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

//! Elliptic curve operations on P-256 & P-384.

use self::ops::*;
use crate::{arithmetic::montgomery::*, cpu, ec, error, io::der, pkcs8};

// NIST SP 800-56A Step 3: "If q is an odd prime p, verify that
// yQ**2 = xQ**3 + axQ + b in GF(p), where the arithmetic is performed modulo
// p."
//
// That is, verify that (x, y) is on the curve, which is true iif:
//
//     y**2 == x**3 + a*x + b (mod q)
//
// Or, equivalently, but more efficiently:
//
//     y**2 == (x**2 + a)*x + b  (mod q)
//
fn verify_affine_point_is_on_the_curve(
    q: &Modulus<Q>,
    (x, y): (&Elem<R>, &Elem<R>),
) -> Result<(), error::Unspecified> {
    verify_affine_point_is_on_the_curve_scaled(q, (x, y), &Elem::from(q.a()), &Elem::from(q.b()))
}

// Use `verify_affine_point_is_on_the_curve` instead of this function whenever
// the affine coordinates are available or will become available. This function
// should only be used then the affine coordinates are never calculated. See
// the notes for `verify_affine_point_is_on_the_curve_scaled`.
//
// The value `z**2` is returned on success because it is useful for ECDSA
// verification.
//
// This function also verifies that the point is not at infinity.
fn verify_jacobian_point_is_on_the_curve(
    q: &Modulus<Q>,
    p: &Point,
) -> Result<Elem<R>, error::Unspecified> {
    let z = q.point_z(p);

    // Verify that the point is not at infinity.
    q.elem_verify_is_not_zero(&z)?;

    let x = q.point_x(p);
    let y = q.point_y(p);

    // We are given Jacobian coordinates (x, y, z). So, we have:
    //
    //    (x/z**2, y/z**3) == (x', y'),
    //
    // where (x', y') are the affine coordinates. The curve equation is:
    //
    //     y'**2  ==  x'**3 + a*x' + b  ==  (x'**2 + a)*x' + b
    //
    // Substituting our Jacobian coordinates, we get:
    //
    //    /   y  \**2       /  /   x  \**2       \   /   x  \
    //    | ---- |      ==  |  | ---- |    +  a  | * | ---- |  +  b
    //    \ z**3 /          \  \ z**2 /          /   \ z**2 /
    //
    // Simplify:
    //
    //            y**2      / x**2       \     x
    //            ----  ==  | ----  +  a | * ----  +  b
    //            z**6      \ z**4       /   z**2
    //
    // Multiply both sides by z**6:
    //
    //     z**6             / x**2       \   z**6
    //     ---- * y**2  ==  | ----  +  a | * ---- * x  +  (z**6) * b
    //     z**6             \ z**4       /   z**2
    //
    // Simplify:
    //
    //                      / x**2       \
    //            y**2  ==  | ----  +  a | * z**4 * x  +  (z**6) * b
    //                      \ z**4       /
    //
    // Distribute z**4:
    //
    //                      / z**4                     \
    //            y**2  ==  | ---- * x**2  +  z**4 * a | * x  +  (z**6) * b
    //                      \ z**4                     /
    //
    // Simplify:
    //
    //            y**2  ==  (x**2  +  z**4 * a) * x  +  (z**6) * b
    //
    let z2 = q.elem_squared(&z);
    let z4 = q.elem_squared(&z2);
    let z4_a = q.elem_product(&z4, &Elem::from(q.a()));
    let z6 = q.elem_product(&z4, &z2);
    let z6_b = q.elem_product(&z6, &Elem::from(q.b()));
    verify_affine_point_is_on_the_curve_scaled(q, (&x, &y), &z4_a, &z6_b)?;
    Ok(z2)
}

// Handles the common logic of point-is-on-the-curve checks for both affine and
// Jacobian cases.
//
// When doing the check that the point is on the curve after a computation,
// to avoid fault attacks or mitigate potential bugs, it is better for security
// to use `verify_affine_point_is_on_the_curve` on the affine coordinates,
// because it provides some protection against faults that occur in the
// computation of the inverse of `z`. See the paper and presentation "Fault
// Attacks on Projective-to-Affine Coordinates Conversion" by Diana Maimuţ,
// Cédric Murdica, David Naccache, Mehdi Tibouchi. That presentation concluded
// simply "Check the validity of the result after conversion to affine
// coordinates." (It seems like a good idea to verify that
// z_inv * z == 1 mod q too).
//
// In the case of affine coordinates (x, y), `a_scaled` and `b_scaled` are
// `a` and `b`, respectively. In the case of Jacobian coordinates (x, y, z),
// the computation and comparison is the same, except `a_scaled` and `b_scaled`
// are (z**4 * a) and (z**6 * b), respectively. Thus, performance is another
// reason to prefer doing the check on the affine coordinates, as Jacobian
// computation requires 3 extra multiplications and 2 extra squarings.
//
// An example of a fault attack that isn't mitigated by a point-on-the-curve
// check after multiplication is given in "Sign Change Fault Attacks On
// Elliptic Curve Cryptosystems" by Johannes Blömer, Martin Otto, and
// Jean-Pierre Seifert.
fn verify_affine_point_is_on_the_curve_scaled(
    q: &Modulus<Q>,
    (x, y): (&Elem<R>, &Elem<R>),
    a_scaled: &Elem<R>,
    b_scaled: &Elem<R>,
) -> Result<(), error::Unspecified> {
    let lhs = q.elem_squared(y);

    let mut rhs = q.elem_squared(x);
    q.add_assign(&mut rhs, a_scaled);
    q.elem_mul(&mut rhs, x);
    q.add_assign(&mut rhs, b_scaled);

    if !q.elems_are_equal(&lhs, &rhs).leak() {
        return Err(error::Unspecified);
    }

    Ok(())
}

pub(crate) fn key_pair_from_pkcs8(
    curve: &'static ec::Curve,
    template: &pkcs8::Template,
    input: untrusted::Input,
    cpu_features: cpu::Features,
) -> Result<ec::KeyPair, error::KeyRejected> {
    let (ec_private_key, _) = pkcs8::unwrap_key(template, pkcs8::Version::V1Only, input)?;
    let (private_key, public_key) =
        ec_private_key.read_all(error::KeyRejected::invalid_encoding(), |input| {
            // https://tools.ietf.org/html/rfc5915#section-3
            der::nested(
                input,
                der::Tag::Sequence,
                error::KeyRejected::invalid_encoding(),
                |input| key_pair_from_pkcs8_(template, input),
            )
        })?;
    key_pair_from_bytes(curve, private_key, public_key, cpu_features)
}

fn key_pair_from_pkcs8_<'a>(
    template: &pkcs8::Template,
    input: &mut untrusted::Reader<'a>,
) -> Result<(untrusted::Input<'a>, untrusted::Input<'a>), error::KeyRejected> {
    let version = der::small_nonnegative_integer(input)
        .map_err(|error::Unspecified| error::KeyRejected::invalid_encoding())?;
    if version != 1 {
        return Err(error::KeyRejected::version_not_supported());
    }

    let private_key = der::expect_tag_and_get_value(input, der::Tag::OctetString)
        .map_err(|error::Unspecified| error::KeyRejected::invalid_encoding())?;

    // [0] parameters (optional).
    if input.peek(u8::from(der::Tag::ContextSpecificConstructed0)) {
        let actual_alg_id =
            der::expect_tag_and_get_value(input, der::Tag::ContextSpecificConstructed0)
                .map_err(|error::Unspecified| error::KeyRejected::invalid_encoding())?;
        if actual_alg_id.as_slice_less_safe() != template.curve_oid().as_slice_less_safe() {
            return Err(error::KeyRejected::wrong_algorithm());
        }
    }

    // [1] publicKey. The RFC says it is optional, but we require it
    // to be present.
    let public_key = der::nested(
        input,
        der::Tag::ContextSpecificConstructed1,
        error::Unspecified,
        der::bit_string_with_no_unused_bits,
    )
    .map_err(|error::Unspecified| error::KeyRejected::invalid_encoding())?;

    Ok((private_key, public_key))
}

pub(crate) fn key_pair_from_bytes(
    curve: &'static ec::Curve,
    private_key_bytes: untrusted::Input,
    public_key_bytes: untrusted::Input,
    cpu_features: cpu::Features,
) -> Result<ec::KeyPair, error::KeyRejected> {
    let seed = ec::Seed::from_bytes(curve, private_key_bytes, cpu_features)
        .map_err(|error::Unspecified| error::KeyRejected::invalid_component())?;

    let r = ec::KeyPair::derive(seed, cpu_features)
        .map_err(|error::Unspecified| error::KeyRejected::unexpected_error())?;
    if public_key_bytes.as_slice_less_safe() != r.public_key().as_ref() {
        return Err(error::KeyRejected::inconsistent_components());
    }

    Ok(r)
}

pub mod curve;

pub mod ecdh;

pub mod ecdsa;

mod ops;

mod private_key;
mod public_key;

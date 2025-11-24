// Copyright 2017 Brian Smith.
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

#![allow(missing_docs)]
#![cfg(feature = "alloc")]

use ring::{
    error,
    io::der,
    rand, rsa,
    signature::{self, KeyPair},
};
#[allow(deprecated)]
use ring::{test, test_file};

#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
use wasm_bindgen_test::{wasm_bindgen_test as test, wasm_bindgen_test_configure};

#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
wasm_bindgen_test_configure!(run_in_browser);

#[test]
fn rsa_from_pkcs8_test() {
    test::run(
        test_file!("rsa_from_pkcs8_tests.txt"),
        |section, test_case| {
            assert_eq!(section, "");

            let input = test_case.consume_bytes("Input");
            let error = test_case.consume_optional_string("Error");

            match (rsa::KeyPair::from_pkcs8(&input), error) {
                (Ok(_), None) => {}
                (Err(e), None) => panic!("Failed with error \"{}\", but expected to succeed", e),
                (Ok(_), Some(e)) => panic!("Succeeded, but expected error \"{}\"", e),
                (Err(actual), Some(expected)) => assert_eq!(format!("{}", actual), expected),
            };

            Ok(())
        },
    );
}

#[cfg(feature = "alloc")]
#[test]
fn test_signature_rsa_pkcs1_sign() {
    let rng = rand::SystemRandom::new();
    test::run(
        test_file!("rsa_pkcs1_sign_tests.txt"),
        |section, test_case| {
            assert_eq!(section, "");

            let digest_name = test_case.consume_string("Digest");
            let alg = match digest_name.as_ref() {
                "SHA256" => &signature::RSA_PKCS1_SHA256,
                "SHA384" => &signature::RSA_PKCS1_SHA384,
                "SHA512" => &signature::RSA_PKCS1_SHA512,
                _ => panic!("Unsupported digest: {}", digest_name),
            };

            let private_key = test_case.consume_bytes("Key");
            let msg = test_case.consume_bytes("Msg");
            let expected = test_case.consume_bytes("Sig");
            let result = test_case.consume_string("Result");

            let key_pair = rsa::KeyPair::from_der(&private_key);
            if result == "Fail-Invalid-Key" {
                assert!(key_pair.is_err());
                return Ok(());
            }
            let key_pair = key_pair.unwrap();

            // XXX: This test is too slow on Android ARM Travis CI builds.
            // TODO: re-enable these tests on Android ARM.
            let mut actual = vec![0u8; key_pair.public().modulus_len()];
            key_pair
                .sign(alg, &rng, &msg, actual.as_mut_slice())
                .unwrap();
            assert_eq!(actual.as_slice() == &expected[..], result == "Pass");
            Ok(())
        },
    );
}

#[cfg(feature = "alloc")]
#[test]
fn test_signature_rsa_pss_sign() {
    test::run(
        test_file!("rsa_pss_sign_tests.txt"),
        |section, test_case| {
            assert_eq!(section, "");

            let digest_name = test_case.consume_string("Digest");
            let alg = match digest_name.as_ref() {
                "SHA256" => &signature::RSA_PSS_SHA256,
                "SHA384" => &signature::RSA_PSS_SHA384,
                "SHA512" => &signature::RSA_PSS_SHA512,
                _ => panic!("Unsupported digest: {}", digest_name),
            };

            let result = test_case.consume_string("Result");
            let private_key = test_case.consume_bytes("Key");
            let key_pair = rsa::KeyPair::from_der(&private_key);
            if key_pair.is_err() && result == "Fail-Invalid-Key" {
                return Ok(());
            }
            let key_pair = key_pair.unwrap();
            let msg = test_case.consume_bytes("Msg");
            let salt = test_case.consume_bytes("Salt");
            let expected = test_case.consume_bytes("Sig");

            #[allow(deprecated)]
            let rng = test::rand::FixedSliceRandom { bytes: &salt };

            let mut actual = vec![0u8; key_pair.public().modulus_len()];
            key_pair.sign(alg, &rng, &msg, actual.as_mut_slice())?;
            assert_eq!(actual.as_slice() == &expected[..], result == "Pass");
            Ok(())
        },
    );
}

// `KeyPair::sign` requires that the output buffer is the same length as
// the public key modulus. Test what happens when it isn't the same length.
#[test]
fn test_signature_rsa_pkcs1_sign_output_buffer_len() {
    // Sign the message "hello, world", using PKCS#1 v1.5 padding and the
    // SHA256 digest algorithm.
    const MESSAGE: &[u8] = b"hello, world";
    let rng = rand::SystemRandom::new();

    const PRIVATE_KEY_DER: &[u8] =
        include_bytes!("../src/rsa/signature_rsa_example_private_key.der");
    let key_pair = rsa::KeyPair::from_der(PRIVATE_KEY_DER).unwrap();

    // When the output buffer is not exactly the right length, `sign()` returns
    // an error (and does not panic or invoke UB). if `sign` doesn't check that
    // the length is correct at the beginning then there are various possible
    // failure points when the output buffer is too small.
    for len in 0..key_pair.public().modulus_len() + 1 {
        let mut signature = vec![0; len];
        assert_eq!(
            len == key_pair.public().modulus_len(),
            key_pair
                .sign(&signature::RSA_PKCS1_SHA256, &rng, MESSAGE, &mut signature)
                .is_ok()
        );
    }
}

#[cfg(feature = "alloc")]
#[test]
fn test_signature_rsa_pkcs1_verify() {
    let sha1_params = &[
        (
            &signature::RSA_PKCS1_1024_8192_SHA1_FOR_LEGACY_USE_ONLY,
            1024,
        ),
        (
            &signature::RSA_PKCS1_2048_8192_SHA1_FOR_LEGACY_USE_ONLY,
            2048,
        ),
    ];
    let sha256_params = &[
        (
            &signature::RSA_PKCS1_1024_8192_SHA256_FOR_LEGACY_USE_ONLY,
            1024,
        ),
        (&signature::RSA_PKCS1_2048_8192_SHA256, 2048),
    ];
    let sha384_params = &[
        (&signature::RSA_PKCS1_2048_8192_SHA384, 2048),
        (&signature::RSA_PKCS1_3072_8192_SHA384, 3072),
    ];
    let sha512_params = &[
        (
            &signature::RSA_PKCS1_1024_8192_SHA512_FOR_LEGACY_USE_ONLY,
            1024,
        ),
        (&signature::RSA_PKCS1_2048_8192_SHA512, 2048),
    ];
    test::run(
        test_file!("rsa_pkcs1_verify_tests.txt"),
        |section, test_case| {
            assert_eq!(section, "");

            let digest_name = test_case.consume_string("Digest");
            let params: &[_] = match digest_name.as_ref() {
                "SHA1" => sha1_params,
                "SHA256" => sha256_params,
                "SHA384" => sha384_params,
                "SHA512" => sha512_params,
                _ => panic!("Unsupported digest: {}", digest_name),
            };

            let public_key = test_case.consume_bytes("Key");

            // Sanity check that we correctly DER-encoded the originally-
            // provided separate (n, e) components. When we add test vectors
            // for improperly-encoded signatures, we'll have to revisit this.
            let key_bits = untrusted::Input::from(&public_key)
                .read_all(error::Unspecified, |input| {
                    der::nested(input, der::Tag::Sequence, error::Unspecified, |input| {
                        let n_bytes =
                            der::positive_integer(input)?.big_endian_without_leading_zero();
                        let _e = der::positive_integer(input)?;

                        // Because `n_bytes` has the leading zeros stripped and is big-endian, there
                        // must be less than 8 leading zero bits.
                        let n_leading_zeros = usize::try_from(n_bytes[0].leading_zeros()).unwrap();
                        assert!(n_leading_zeros < 8);
                        Ok((n_bytes.len() * 8) - n_leading_zeros)
                    })
                })
                .expect("invalid DER");

            let msg = test_case.consume_bytes("Msg");
            let sig = test_case.consume_bytes("Sig");
            let is_valid = test_case.consume_string("Result") == "P";
            for &(alg, min_bits) in params {
                let width_ok = key_bits >= min_bits;
                let actual_result =
                    signature::UnparsedPublicKey::new(alg, &public_key).verify(&msg, &sig);
                assert_eq!(actual_result.is_ok(), is_valid && width_ok);
            }

            Ok(())
        },
    );
}

#[cfg(feature = "alloc")]
#[test]
fn test_signature_rsa_pss_verify() {
    test::run(
        test_file!("rsa_pss_verify_tests.txt"),
        |section, test_case| {
            assert_eq!(section, "");

            let digest_name = test_case.consume_string("Digest");
            let alg = match digest_name.as_ref() {
                "SHA256" => &signature::RSA_PSS_2048_8192_SHA256,
                "SHA384" => &signature::RSA_PSS_2048_8192_SHA384,
                "SHA512" => &signature::RSA_PSS_2048_8192_SHA512,
                _ => panic!("Unsupported digest: {}", digest_name),
            };

            let public_key = test_case.consume_bytes("Key");

            // Sanity check that we correctly DER-encoded the originally-
            // provided separate (n, e) components. When we add test vectors
            // for improperly-encoded signatures, we'll have to revisit this.
            assert!(untrusted::Input::from(&public_key)
                .read_all(error::Unspecified, |input| der::nested(
                    input,
                    der::Tag::Sequence,
                    error::Unspecified,
                    |input| {
                        let _ = der::positive_integer(input)?;
                        let _ = der::positive_integer(input)?;
                        Ok(())
                    }
                ))
                .is_ok());

            let msg = test_case.consume_bytes("Msg");
            let sig = test_case.consume_bytes("Sig");
            let is_valid = test_case.consume_string("Result") == "P";

            let actual_result =
                signature::UnparsedPublicKey::new(alg, &public_key).verify(&msg, &sig);
            assert_eq!(actual_result.is_ok(), is_valid);

            Ok(())
        },
    );
}

// Test for `primitive::verify()`. Read public key parts from a file
// and use them to verify a signature.
#[cfg(feature = "alloc")]
#[test]
fn test_signature_rsa_primitive_verification() {
    test::run(
        test_file!("rsa_primitive_verify_tests.txt"),
        |section, test_case| {
            assert_eq!(section, "");
            let n = test_case.consume_bytes("n");
            let e = test_case.consume_bytes("e");
            let msg = test_case.consume_bytes("Msg");
            let sig = test_case.consume_bytes("Sig");
            let expected = test_case.consume_string("Result");
            let public_key = signature::RsaPublicKeyComponents { n: &n, e: &e };
            let result = public_key.verify(&signature::RSA_PKCS1_2048_8192_SHA256, &msg, &sig);
            assert_eq!(result.is_ok(), expected == "Pass");
            Ok(())
        },
    )
}

#[cfg(feature = "alloc")]
#[test]
fn rsa_test_keypair_coverage() {
    const PRIVATE_KEY: &[u8] = include_bytes!("rsa_test_private_key_2048.p8");

    let key_pair = rsa::KeyPair::from_pkcs8(PRIVATE_KEY).unwrap();

    // Test that `signature::KeyPair::PublicKey` is `rsa::PublicKey`; if it
    // were a separate type then it would need to be tested separately.
    let _: &rsa::PublicKey = key_pair.public_key();

    test_public_key_coverage(key_pair.public());
    // Test clones.
    test_public_key_coverage(&key_pair.public().clone());

    // Test `Debug`
    assert_eq!(
        format!("RsaKeyPair {{ public: {:?} }}", key_pair.public_key()),
        format!("{:?}", key_pair)
    );
}

fn test_public_key_coverage(key: &rsa::PublicKey) {
    // Test `AsRef<[u8]>`
    const PUBLIC_KEY: &[u8] = include_bytes!("rsa_test_public_key_2048.der");
    assert_eq!(key.as_ref(), PUBLIC_KEY);

    // Test `Debug`.
    const PUBLIC_KEY_DEBUG: &str = include_str!("rsa_test_public_key_2048_debug.txt");
    assert_eq!(PUBLIC_KEY_DEBUG, format!("{:?}", key));

    let components = rsa::PublicKeyComponents::<Vec<_>>::from(key);
    const PUBLIC_KEY_MODULUS_BE_BYTES: &[u8] = include_bytes!("rsa_test_public_modulus.bin");
    assert_eq!(PUBLIC_KEY_MODULUS_BE_BYTES, &components.n);
    const _65537: &[u8] = &[0x01, 0x00, 0x01];
    assert_eq!(_65537, &components.e);
}

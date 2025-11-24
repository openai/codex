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

#![allow(missing_docs)]

use ring::{
    rand,
    signature::{self, KeyPair},
};
#[allow(deprecated)]
use ring::{test, test_file};

// ECDSA *signing* tests are in src/ec/ecdsa/signing.rs.

#[test]
fn ecdsa_from_pkcs8_test() {
    let rng = rand::SystemRandom::new();

    test::run(
        test_file!("ecdsa_from_pkcs8_tests.txt"),
        |section, test_case| {
            assert_eq!(section, "");

            let curve_name = test_case.consume_string("Curve");
            let ((this_fixed, this_asn1), (other_fixed, other_asn1)) = match curve_name.as_str() {
                "P-256" => (
                    (
                        &signature::ECDSA_P256_SHA256_FIXED_SIGNING,
                        &signature::ECDSA_P256_SHA256_ASN1_SIGNING,
                    ),
                    (
                        &signature::ECDSA_P384_SHA384_FIXED_SIGNING,
                        &signature::ECDSA_P384_SHA384_ASN1_SIGNING,
                    ),
                ),
                "P-384" => (
                    (
                        &signature::ECDSA_P384_SHA384_FIXED_SIGNING,
                        &signature::ECDSA_P384_SHA384_ASN1_SIGNING,
                    ),
                    (
                        &signature::ECDSA_P256_SHA256_FIXED_SIGNING,
                        &signature::ECDSA_P256_SHA256_ASN1_SIGNING,
                    ),
                ),
                _ => unreachable!(),
            };

            let input = test_case.consume_bytes("Input");

            let error = test_case.consume_optional_string("Error");

            match (
                signature::EcdsaKeyPair::from_pkcs8(this_fixed, &input, &rng),
                error.clone(),
            ) {
                (Ok(_), None) => (),
                (Err(e), None) => panic!("Failed with error \"{}\", but expected to succeed", e),
                (Ok(_), Some(e)) => panic!("Succeeded, but expected error \"{}\"", e),
                (Err(actual), Some(expected)) => assert_eq!(format!("{}", actual), expected),
            };

            match (
                signature::EcdsaKeyPair::from_pkcs8(this_asn1, &input, &rng),
                error,
            ) {
                (Ok(_), None) => (),
                (Err(e), None) => panic!("Failed with error \"{}\", but expected to succeed", e),
                (Ok(_), Some(e)) => panic!("Succeeded, but expected error \"{}\"", e),
                (Err(actual), Some(expected)) => assert_eq!(format!("{}", actual), expected),
            };

            assert!(signature::EcdsaKeyPair::from_pkcs8(other_fixed, &input, &rng).is_err());
            assert!(signature::EcdsaKeyPair::from_pkcs8(other_asn1, &input, &rng).is_err());

            Ok(())
        },
    );
}

// Verify that, at least, we generate PKCS#8 documents that we can read.
#[test]
fn ecdsa_generate_pkcs8_test() {
    let rng = rand::SystemRandom::new();

    for alg in &[
        &signature::ECDSA_P256_SHA256_ASN1_SIGNING,
        &signature::ECDSA_P256_SHA256_FIXED_SIGNING,
        &signature::ECDSA_P384_SHA384_ASN1_SIGNING,
        &signature::ECDSA_P384_SHA384_FIXED_SIGNING,
    ] {
        let pkcs8 = signature::EcdsaKeyPair::generate_pkcs8(alg, &rng).unwrap();
        println!();
        for b in pkcs8.as_ref() {
            print!("{:02x}", *b);
        }
        println!();
        println!();

        #[cfg(feature = "alloc")]
        let _ = signature::EcdsaKeyPair::from_pkcs8(alg, pkcs8.as_ref(), &rng).unwrap();
    }
}

#[test]
fn signature_ecdsa_verify_asn1_test() {
    test::run(
        test_file!("ecdsa_verify_asn1_tests.txt"),
        |section, test_case| {
            assert_eq!(section, "");

            let curve_name = test_case.consume_string("Curve");
            let digest_name = test_case.consume_string("Digest");
            let msg = test_case.consume_bytes("Msg");
            let public_key = test_case.consume_bytes("Q");
            let sig = test_case.consume_bytes("Sig");
            let is_valid = test_case.consume_string("Result") == "P (0 )";

            let alg = match (curve_name.as_str(), digest_name.as_str()) {
                ("P-256", "SHA256") => &signature::ECDSA_P256_SHA256_ASN1,
                ("P-256", "SHA384") => &signature::ECDSA_P256_SHA384_ASN1,
                ("P-384", "SHA256") => &signature::ECDSA_P384_SHA256_ASN1,
                ("P-384", "SHA384") => &signature::ECDSA_P384_SHA384_ASN1,
                _ => {
                    panic!("Unsupported curve+digest: {}+{}", curve_name, digest_name);
                }
            };

            let actual_result =
                signature::UnparsedPublicKey::new(alg, &public_key).verify(&msg, &sig);
            assert_eq!(actual_result.is_ok(), is_valid);

            Ok(())
        },
    );
}

#[test]
fn signature_ecdsa_verify_fixed_test() {
    test::run(
        test_file!("ecdsa_verify_fixed_tests.txt"),
        |section, test_case| {
            assert_eq!(section, "");

            let curve_name = test_case.consume_string("Curve");
            let digest_name = test_case.consume_string("Digest");

            let msg = test_case.consume_bytes("Msg");
            let public_key = test_case.consume_bytes("Q");
            let sig = test_case.consume_bytes("Sig");
            let expected_result = test_case.consume_string("Result");

            let alg = match (curve_name.as_str(), digest_name.as_str()) {
                ("P-256", "SHA256") => &signature::ECDSA_P256_SHA256_FIXED,
                ("P-384", "SHA384") => &signature::ECDSA_P384_SHA384_FIXED,
                _ => {
                    panic!("Unsupported curve+digest: {}+{}", curve_name, digest_name);
                }
            };

            let is_valid = expected_result == "P (0 )";

            let actual_result =
                signature::UnparsedPublicKey::new(alg, &public_key).verify(&msg, &sig);
            assert_eq!(actual_result.is_ok(), is_valid);

            Ok(())
        },
    );
}

#[test]
fn ecdsa_test_public_key_coverage() {
    const PRIVATE_KEY: &[u8] = include_bytes!("ecdsa_test_private_key_p256.p8");
    const PUBLIC_KEY: &[u8] = include_bytes!("ecdsa_test_public_key_p256.der");
    const PUBLIC_KEY_DEBUG: &str = include_str!("ecdsa_test_public_key_p256_debug.txt");

    let rng = rand::SystemRandom::new();
    let key_pair = signature::EcdsaKeyPair::from_pkcs8(
        &signature::ECDSA_P256_SHA256_FIXED_SIGNING,
        PRIVATE_KEY,
        &rng,
    )
    .unwrap();

    // Test `AsRef<[u8]>`
    assert_eq!(key_pair.public_key().as_ref(), PUBLIC_KEY);

    // Test `Clone`.
    #[allow(clippy::clone_on_copy, clippy::redundant_clone)]
    let _: <signature::EcdsaKeyPair as KeyPair>::PublicKey = key_pair.public_key().clone();

    // Test `Copy`.
    let _: <signature::EcdsaKeyPair as KeyPair>::PublicKey = *key_pair.public_key();

    // Test `Debug`.
    assert_eq!(PUBLIC_KEY_DEBUG, format!("{:?}", key_pair.public_key()));
    assert_eq!(
        format!("EcdsaKeyPair {{ public_key: {:?} }}", key_pair.public_key()),
        format!("{:?}", key_pair)
    );
}

// This test is not a known-answer test, though it re-uses the known-answer
// test vectors. Because the nonce is randomized, the signature will be
// different each time. Because of that, here we simply verify that the
// signature verifies correctly. The known-answer tests themselves are in
// ecsda/signing.rs.
#[test]
fn signature_ecdsa_sign_fixed_sign_and_verify_test() {
    let rng = rand::SystemRandom::new();

    test::run(
        test_file!("../src/ec/suite_b/ecdsa/ecdsa_sign_fixed_tests.txt"),
        |section, test_case| {
            assert_eq!(section, "");

            let curve_name = test_case.consume_string("Curve");
            let digest_name = test_case.consume_string("Digest");

            let msg = test_case.consume_bytes("Msg");
            let d = test_case.consume_bytes("d");
            let q = test_case.consume_bytes("Q");

            // Ignored since the actual signature will use a randomized nonce.
            let _k = test_case.consume_bytes("k");
            let _expected_result = test_case.consume_bytes("Sig");

            let (signing_alg, verification_alg) = match (curve_name.as_str(), digest_name.as_str())
            {
                ("P-256", "SHA256") => (
                    &signature::ECDSA_P256_SHA256_FIXED_SIGNING,
                    &signature::ECDSA_P256_SHA256_FIXED,
                ),
                ("P-384", "SHA384") => (
                    &signature::ECDSA_P384_SHA384_FIXED_SIGNING,
                    &signature::ECDSA_P384_SHA384_FIXED,
                ),
                _ => {
                    panic!("Unsupported curve+digest: {}+{}", curve_name, digest_name);
                }
            };

            let private_key =
                signature::EcdsaKeyPair::from_private_key_and_public_key(signing_alg, &d, &q, &rng)
                    .unwrap();

            let signature = private_key.sign(&rng, &msg).unwrap();

            let public_key = signature::UnparsedPublicKey::new(verification_alg, q);
            assert_eq!(public_key.verify(&msg, signature.as_ref()), Ok(()));

            Ok(())
        },
    );
}

// This test is not a known-answer test, though it re-uses the known-answer
// test vectors. Because the nonce is randomized, the signature will be
// different each time. Because of that, here we simply verify that the
// signature verifies correctly. The known-answer tests themselves are in
// ecsda/signing.rs.
#[test]
fn signature_ecdsa_sign_asn1_test() {
    let rng = rand::SystemRandom::new();

    test::run(
        test_file!("../src/ec/suite_b/ecdsa/ecdsa_sign_asn1_tests.txt"),
        |section, test_case| {
            assert_eq!(section, "");

            let curve_name = test_case.consume_string("Curve");
            let digest_name = test_case.consume_string("Digest");

            let msg = test_case.consume_bytes("Msg");
            let d = test_case.consume_bytes("d");
            let q = test_case.consume_bytes("Q");

            // Ignored since the actual signature will use a randomized nonce.
            let _k = test_case.consume_bytes("k");
            let _expected_result = test_case.consume_bytes("Sig");

            let (signing_alg, verification_alg) = match (curve_name.as_str(), digest_name.as_str())
            {
                ("P-256", "SHA256") => (
                    &signature::ECDSA_P256_SHA256_ASN1_SIGNING,
                    &signature::ECDSA_P256_SHA256_ASN1,
                ),
                ("P-384", "SHA384") => (
                    &signature::ECDSA_P384_SHA384_ASN1_SIGNING,
                    &signature::ECDSA_P384_SHA384_ASN1,
                ),
                _ => {
                    panic!("Unsupported curve+digest: {}+{}", curve_name, digest_name);
                }
            };

            let private_key =
                signature::EcdsaKeyPair::from_private_key_and_public_key(signing_alg, &d, &q, &rng)
                    .unwrap();

            let signature = private_key.sign(&rng, &msg).unwrap();

            let public_key = signature::UnparsedPublicKey::new(verification_alg, q);
            assert_eq!(public_key.verify(&msg, signature.as_ref()), Ok(()));

            Ok(())
        },
    );
}

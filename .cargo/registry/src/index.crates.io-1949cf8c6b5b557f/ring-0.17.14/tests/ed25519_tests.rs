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

#![allow(missing_docs)]

use ring::{
    error, rand,
    signature::{self, Ed25519KeyPair, KeyPair},
};
#[allow(deprecated)]
use ring::{test, test_file};

#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
use wasm_bindgen_test::{wasm_bindgen_test as test, wasm_bindgen_test_configure};

#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
wasm_bindgen_test_configure!(run_in_browser);

/// Test vectors from BoringSSL.
#[test]
fn test_signature_ed25519() {
    test::run(test_file!("ed25519_tests.txt"), |section, test_case| {
        assert_eq!(section, "");
        let seed = test_case.consume_bytes("SEED");
        assert_eq!(32, seed.len());

        let public_key = test_case.consume_bytes("PUB");
        assert_eq!(32, public_key.len());

        let msg = test_case.consume_bytes("MESSAGE");

        let expected_sig = test_case.consume_bytes("SIG");

        {
            let key_pair = Ed25519KeyPair::from_seed_and_public_key(&seed, &public_key).unwrap();
            let actual_sig = key_pair.sign(&msg);
            assert_eq!(&expected_sig[..], actual_sig.as_ref());
        }

        // Test PKCS#8 generation, parsing, and private-to-public calculations.
        #[allow(deprecated)]
        let rng = test::rand::FixedSliceRandom { bytes: &seed };
        let pkcs8 = Ed25519KeyPair::generate_pkcs8(&rng).unwrap();
        let key_pair = Ed25519KeyPair::from_pkcs8(pkcs8.as_ref()).unwrap();
        assert_eq!(public_key, key_pair.public_key().as_ref());

        // Test Signature generation.
        let actual_sig = key_pair.sign(&msg);
        assert_eq!(&expected_sig[..], actual_sig.as_ref());

        // Test Signature verification.
        test_signature_verification(&public_key, &msg, &expected_sig, Ok(()));

        let mut tampered_sig = expected_sig;
        tampered_sig[0] ^= 1;

        test_signature_verification(&public_key, &msg, &tampered_sig, Err(error::Unspecified));

        Ok(())
    });
}

/// Test vectors from BoringSSL.
#[test]
fn test_signature_ed25519_verify() {
    test::run(
        test_file!("ed25519_verify_tests.txt"),
        |section, test_case| {
            assert_eq!(section, "");

            let public_key = test_case.consume_bytes("PUB");
            let msg = test_case.consume_bytes("MESSAGE");
            let sig = test_case.consume_bytes("SIG");
            let expected_result = match test_case.consume_string("Result").as_str() {
                "P" => Ok(()),
                "F" => Err(error::Unspecified),
                s => panic!("{:?} is not a valid result", s),
            };
            test_signature_verification(&public_key, &msg, &sig, expected_result);
            Ok(())
        },
    );
}

fn test_signature_verification(
    public_key: &[u8],
    msg: &[u8],
    sig: &[u8],
    expected_result: Result<(), error::Unspecified>,
) {
    assert_eq!(
        expected_result,
        signature::UnparsedPublicKey::new(&signature::ED25519, public_key).verify(msg, sig)
    );
}

#[test]
fn test_ed25519_from_seed_and_public_key_misuse() {
    const PRIVATE_KEY: &[u8] = include_bytes!("ed25519_test_private_key.bin");
    const PUBLIC_KEY: &[u8] = include_bytes!("ed25519_test_public_key.bin");

    assert!(Ed25519KeyPair::from_seed_and_public_key(PRIVATE_KEY, PUBLIC_KEY).is_ok());

    // Truncated private key.
    assert!(Ed25519KeyPair::from_seed_and_public_key(&PRIVATE_KEY[..31], PUBLIC_KEY).is_err());

    // Truncated public key.
    assert!(Ed25519KeyPair::from_seed_and_public_key(PRIVATE_KEY, &PUBLIC_KEY[..31]).is_err());

    // Swapped public and private key.
    assert!(Ed25519KeyPair::from_seed_and_public_key(PUBLIC_KEY, PRIVATE_KEY).is_err());
}

enum FromPkcs8Variant {
    Checked,
    MaybeUnchecked,
}

#[test]
fn test_ed25519_from_pkcs8_unchecked() {
    test_ed25519_from_pkcs8_(
        FromPkcs8Variant::MaybeUnchecked,
        Ed25519KeyPair::from_pkcs8_maybe_unchecked,
    )
}

#[test]
fn test_ed25519_from_pkcs8() {
    test_ed25519_from_pkcs8_(FromPkcs8Variant::Checked, Ed25519KeyPair::from_pkcs8)
}

fn test_ed25519_from_pkcs8_(
    variant: FromPkcs8Variant,
    f: impl Fn(&[u8]) -> Result<Ed25519KeyPair, error::KeyRejected>,
) {
    // Just test that we can parse the input.
    test::run(
        test_file!("ed25519_from_pkcs8_tests.txt"),
        |section, test_case| {
            assert_eq!(section, "");
            let input = test_case.consume_bytes("Input");
            let expected_error = {
                let expected_checked = test_case.consume_string("Result-Checked");
                let expected_maybe_unchecked = test_case.consume_string("Result-Maybe-Unchecked");
                let expected_result = match variant {
                    FromPkcs8Variant::Checked => expected_checked,
                    FromPkcs8Variant::MaybeUnchecked => expected_maybe_unchecked,
                };
                if expected_result == "OK" {
                    None
                } else {
                    Some(expected_result)
                }
            };
            let expected_public = {
                let expected_if_no_error = test_case.consume_optional_bytes("Public");
                if expected_error.is_none() {
                    Some(expected_if_no_error.unwrap())
                } else {
                    None
                }
            };

            match f(&input) {
                Ok(keypair) => {
                    assert_eq!(expected_error, None);
                    assert_eq!(
                        expected_public.as_deref(),
                        Some(keypair.public_key().as_ref())
                    );
                }
                Err(actual_error) => {
                    assert_eq!(expected_error, Some(format!("{}", actual_error)));
                    assert_eq!(expected_public, None);
                }
            }

            Ok(())
        },
    );
}

#[test]
fn ed25519_test_generate_pkcs8() {
    let rng = rand::SystemRandom::new();
    let generated = Ed25519KeyPair::generate_pkcs8(&rng).unwrap();
    let generated = generated.as_ref();

    let _ronudtripped = Ed25519KeyPair::from_pkcs8(generated).unwrap();

    // Regression test: Verify we're generating the correct encoding, as
    // `Ed25519KeyPair::from_pkcs8` also accepts our old wrong encoding.
    assert_eq!(generated.len(), 19 + 32 + 32);
    assert_eq!(&generated[..2], &[0x30, 0x51]);
}

#[test]
fn ed25519_test_public_key_coverage() {
    const PRIVATE_KEY: &[u8] = include_bytes!("ed25519_test_private_key.p8");
    const PUBLIC_KEY: &[u8] = include_bytes!("ed25519_test_public_key.der");
    const PUBLIC_KEY_DEBUG: &str =
        "PublicKey(\"5809e9fef6dcec58f0f2e3b0d67e9880a11957e083ace85835c3b6c8fbaf6b7d\")";

    let key_pair = Ed25519KeyPair::from_pkcs8(PRIVATE_KEY).unwrap();

    // Test `AsRef<[u8]>`
    assert_eq!(key_pair.public_key().as_ref(), PUBLIC_KEY);

    // Test `Clone`.
    #[allow(clippy::clone_on_copy)]
    let _: <Ed25519KeyPair as KeyPair>::PublicKey = key_pair.public_key().clone();

    // Test `Copy`.
    let _: <Ed25519KeyPair as KeyPair>::PublicKey = *key_pair.public_key();

    // Test `Debug`.
    assert_eq!(PUBLIC_KEY_DEBUG, format!("{:?}", key_pair.public_key()));
    assert_eq!(
        format!(
            "Ed25519KeyPair {{ public_key: {:?} }}",
            key_pair.public_key()
        ),
        format!("{:?}", key_pair)
    );
}

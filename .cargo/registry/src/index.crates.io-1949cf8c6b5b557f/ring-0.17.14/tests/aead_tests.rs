// Copyright 2015-2021 Brian Smith.
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

#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
use wasm_bindgen_test::{wasm_bindgen_test as test, wasm_bindgen_test_configure};

#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
wasm_bindgen_test_configure!(run_in_browser);

use core::ops::RangeFrom;
use ring::{aead, error};
#[allow(deprecated)]
use ring::{test, test_file};

/// Generate the known answer test functions for the given algorithm and test
/// case input file, where each test is implemented by a test in `$test`.
///
/// All of these tests can be run in parallel.
macro_rules! test_known_answer {
    ( $alg:ident, $test_file:expr, [ $( $test:ident ),+, ] ) => {
        $(
            #[test]
            fn $test() {
                test_aead(
                    &aead::$alg,
                    super::super::$test,
                    test_file!($test_file));
            }
        )+
    }
}

/// Generate the tests for a given algorithm.
///
/// All of these tests can be run in parallel.
macro_rules! test_aead {
    { $( { $alg:ident, $test_file:expr } ),+, } => {
        mod aead_test { // Make `cargo test aead` include these files.
            $(
                #[allow(non_snake_case)]
                mod $alg { // Provide a separate namespace for each algorithm's test.
                    use super::super::*;

                    #[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
                    use wasm_bindgen_test::wasm_bindgen_test as test;

                    test_known_answer!(
                        $alg,
                        $test_file,
                        [
                            less_safe_key_open_in_place,
                            less_safe_key_open_within,
                            less_safe_key_seal_in_place_append_tag,
                            less_safe_key_seal_in_place_separate_tag,
                            opening_key_open_in_place,
                            opening_key_open_within,
                            sealing_key_seal_in_place_append_tag,
                            sealing_key_seal_in_place_separate_tag,
                            test_open_in_place_seperate_tag,
                        ]);

                    #[test]
                    fn key_sizes() {
                        super::super::key_sizes(&aead::$alg);
                    }
                }
            )+
        }
    }
}

test_aead! {
    { AES_128_GCM, "aead_aes_128_gcm_tests.txt" },
    { AES_256_GCM, "aead_aes_256_gcm_tests.txt" },
    { CHACHA20_POLY1305, "aead_chacha20_poly1305_tests.txt" },
}

struct KnownAnswerTestCase<'a> {
    key: &'a [u8],
    nonce: [u8; aead::NONCE_LEN],
    plaintext: &'a [u8],
    aad: aead::Aad<&'a [u8]>,
    ciphertext: &'a [u8],
    tag: &'a [u8],
}

fn test_aead(
    aead_alg: &'static aead::Algorithm,
    f: impl Fn(&'static aead::Algorithm, KnownAnswerTestCase) -> Result<(), error::Unspecified>,
    test_file: test::File,
) {
    test::run(test_file, |section, test_case| {
        assert_eq!(section, "");
        let key = test_case.consume_bytes("KEY");
        let nonce = test_case.consume_bytes("NONCE");
        let plaintext = test_case.consume_bytes("IN");
        let aad = test_case.consume_bytes("AD");
        let ct = test_case.consume_bytes("CT");
        let tag = test_case.consume_bytes("TAG");
        let error = test_case.consume_optional_string("FAILS");

        match error.as_deref() {
            Some("WRONG_NONCE_LENGTH") => {
                assert!(matches!(
                    aead::Nonce::try_assume_unique_for_key(&nonce),
                    Err(error::Unspecified)
                ));
                return Ok(());
            }
            Some(unexpected) => {
                unreachable!("unexpected error in test data: {}", unexpected);
            }
            None => {}
        };

        let test_case = KnownAnswerTestCase {
            key: &key,
            nonce: nonce.as_slice().try_into().unwrap(),
            plaintext: &plaintext,
            aad: aead::Aad::from(&aad),
            ciphertext: &ct,
            tag: &tag,
        };

        f(aead_alg, test_case)
    })
}

fn test_seal_append_tag<Seal>(
    tc: &KnownAnswerTestCase,
    seal: Seal,
) -> Result<(), error::Unspecified>
where
    Seal: FnOnce(aead::Nonce, &mut Vec<u8>) -> Result<(), error::Unspecified>,
{
    let mut in_out = Vec::from(tc.plaintext);
    seal(aead::Nonce::assume_unique_for_key(tc.nonce), &mut in_out)?;

    let mut expected_ciphertext_and_tag = Vec::from(tc.ciphertext);
    expected_ciphertext_and_tag.extend_from_slice(tc.tag);

    assert_eq!(in_out, expected_ciphertext_and_tag);

    Ok(())
}

fn test_seal_separate_tag<Seal>(
    tc: &KnownAnswerTestCase,
    seal: Seal,
) -> Result<(), error::Unspecified>
where
    Seal: Fn(aead::Nonce, &mut [u8]) -> Result<aead::Tag, error::Unspecified>,
{
    let mut in_out = Vec::from(tc.plaintext);
    let actual_tag = seal(aead::Nonce::assume_unique_for_key(tc.nonce), &mut in_out)?;
    assert_eq!(actual_tag.as_ref(), tc.tag);
    assert_eq!(in_out, tc.ciphertext);

    Ok(())
}

fn test_open_in_place<OpenInPlace>(
    tc: &KnownAnswerTestCase<'_>,
    open_in_place: OpenInPlace,
) -> Result<(), error::Unspecified>
where
    OpenInPlace:
        for<'a> FnOnce(aead::Nonce, &'a mut [u8]) -> Result<&'a mut [u8], error::Unspecified>,
{
    let nonce = aead::Nonce::assume_unique_for_key(tc.nonce);

    let mut in_out = Vec::from(tc.ciphertext);
    in_out.extend_from_slice(tc.tag);

    let actual_plaintext = open_in_place(nonce, &mut in_out)?;

    assert_eq!(actual_plaintext, tc.plaintext);
    assert_eq!(&in_out[..tc.plaintext.len()], tc.plaintext);
    Ok(())
}

fn test_open_in_place_seperate_tag(
    alg: &'static aead::Algorithm,
    tc: KnownAnswerTestCase,
) -> Result<(), error::Unspecified> {
    let key = make_less_safe_key(alg, tc.key);

    let mut in_out = Vec::from(tc.ciphertext);
    let tag = tc.tag.try_into().unwrap();

    // Test the simplest behavior.
    {
        let nonce = aead::Nonce::assume_unique_for_key(tc.nonce);
        let actual_plaintext =
            key.open_in_place_separate_tag(nonce, tc.aad, tag, &mut in_out, 0..)?;

        assert_eq!(actual_plaintext, tc.plaintext);
        assert_eq!(&in_out[..tc.plaintext.len()], tc.plaintext);
    }

    // Test that ciphertext range shifting works as expected.
    {
        let range = in_out.len()..;
        in_out.extend_from_slice(tc.ciphertext);

        let nonce = aead::Nonce::assume_unique_for_key(tc.nonce);
        let actual_plaintext =
            key.open_in_place_separate_tag(nonce, tc.aad, tag, &mut in_out, range)?;

        assert_eq!(actual_plaintext, tc.plaintext);
        assert_eq!(&in_out[..tc.plaintext.len()], tc.plaintext);
    }

    Ok(())
}

fn test_open_within<OpenWithin>(
    tc: &KnownAnswerTestCase<'_>,
    open_within: OpenWithin,
) -> Result<(), error::Unspecified>
where
    OpenWithin: for<'a> Fn(
        aead::Nonce,
        &'a mut [u8],
        RangeFrom<usize>,
    ) -> Result<&'a mut [u8], error::Unspecified>,
{
    // In release builds, test all prefix lengths from 0 to 4096 bytes.
    // Debug builds are too slow for this, so for those builds, only
    // test a smaller subset.

    // TLS record headers are 5 bytes long.
    // TLS explicit nonces for AES-GCM are 8 bytes long.
    static MINIMAL_IN_PREFIX_LENS: [usize; 36] = [
        // No input prefix to overwrite; i.e. the opening is exactly
        // "in place."
        0,
        1,
        2,
        // Proposed TLS 1.3 header (no explicit nonce).
        5,
        8,
        // Probably the most common use of a non-zero `in_prefix_len`
        // would be to write a decrypted TLS record over the top of the
        // TLS header and nonce.
        5 /* record header */ + 8, /* explicit nonce */
        // The stitched AES-GCM x86-64 code works on 6-block (96 byte)
        // units. Some of the ChaCha20 code is even weirder.
        15,  // The maximum partial AES block.
        16,  // One AES block.
        17,  // One byte more than a full AES block.
        31,  // 2 AES blocks or 1 ChaCha20 block, minus 1.
        32,  // Two AES blocks, one ChaCha20 block.
        33,  // 2 AES blocks or 1 ChaCha20 block, plus 1.
        47,  // Three AES blocks - 1.
        48,  // Three AES blocks.
        49,  // Three AES blocks + 1.
        63,  // Four AES blocks or two ChaCha20 blocks, minus 1.
        64,  // Four AES blocks or two ChaCha20 blocks.
        65,  // Four AES blocks or two ChaCha20 blocks, plus 1.
        79,  // Five AES blocks, minus 1.
        80,  // Five AES blocks.
        81,  // Five AES blocks, plus 1.
        95,  // Six AES blocks or three ChaCha20 blocks, minus 1.
        96,  // Six AES blocks or three ChaCha20 blocks.
        97,  // Six AES blocks or three ChaCha20 blocks, plus 1.
        111, // Seven AES blocks, minus 1.
        112, // Seven AES blocks.
        113, // Seven AES blocks, plus 1.
        127, // Eight AES blocks or four ChaCha20 blocks, minus 1.
        128, // Eight AES blocks or four ChaCha20 blocks.
        129, // Eight AES blocks or four ChaCha20 blocks, plus 1.
        143, // Nine AES blocks, minus 1.
        144, // Nine AES blocks.
        145, // Nine AES blocks, plus 1.
        255, // 16 AES blocks or 8 ChaCha20 blocks, minus 1.
        256, // 16 AES blocks or 8 ChaCha20 blocks.
        257, // 16 AES blocks or 8 ChaCha20 blocks, plus 1.
    ];

    let mut more_comprehensive_in_prefix_lengths = [0; 4096];
    let in_prefix_lengths = if cfg!(debug_assertions) {
        &MINIMAL_IN_PREFIX_LENS[..]
    } else {
        #[allow(clippy::needless_range_loop)]
        for b in 0..more_comprehensive_in_prefix_lengths.len() {
            more_comprehensive_in_prefix_lengths[b] = b;
        }
        &more_comprehensive_in_prefix_lengths[..]
    };
    let mut in_out = vec![123u8; 4096];

    for &in_prefix_len in in_prefix_lengths.iter() {
        in_out.truncate(0);
        in_out.resize(in_prefix_len, 123);
        in_out.extend_from_slice(tc.ciphertext);
        in_out.extend_from_slice(tc.tag);

        let actual_plaintext = open_within(
            aead::Nonce::assume_unique_for_key(tc.nonce),
            &mut in_out,
            in_prefix_len..,
        )?;
        assert_eq!(actual_plaintext, tc.plaintext);
        assert_eq!(&in_out[..tc.plaintext.len()], tc.plaintext);
    }

    Ok(())
}

fn sealing_key_seal_in_place_append_tag(
    alg: &'static aead::Algorithm,
    tc: KnownAnswerTestCase,
) -> Result<(), error::Unspecified> {
    test_seal_append_tag(&tc, |nonce, in_out| {
        let mut key: aead::SealingKey<OneNonceSequence> = make_key(alg, tc.key, nonce);
        key.seal_in_place_append_tag(tc.aad, in_out)
    })
}

fn sealing_key_seal_in_place_separate_tag(
    alg: &'static aead::Algorithm,
    tc: KnownAnswerTestCase,
) -> Result<(), error::Unspecified> {
    test_seal_separate_tag(&tc, |nonce, in_out| {
        let mut key: aead::SealingKey<_> = make_key(alg, tc.key, nonce);
        key.seal_in_place_separate_tag(tc.aad, in_out)
    })
}

fn opening_key_open_in_place(
    alg: &'static aead::Algorithm,
    tc: KnownAnswerTestCase,
) -> Result<(), error::Unspecified> {
    test_open_in_place(&tc, |nonce, in_out| {
        let mut key: aead::OpeningKey<_> = make_key(alg, tc.key, nonce);
        key.open_in_place(tc.aad, in_out)
    })
}

fn opening_key_open_within(
    alg: &'static aead::Algorithm,
    tc: KnownAnswerTestCase,
) -> Result<(), error::Unspecified> {
    test_open_within(&tc, |nonce, in_out, ciphertext_and_tag| {
        let mut key: aead::OpeningKey<OneNonceSequence> = make_key(alg, tc.key, nonce);
        key.open_within(tc.aad, in_out, ciphertext_and_tag)
    })
}

fn less_safe_key_seal_in_place_append_tag(
    alg: &'static aead::Algorithm,
    tc: KnownAnswerTestCase,
) -> Result<(), error::Unspecified> {
    test_seal_append_tag(&tc, |nonce, in_out| {
        let key = make_less_safe_key(alg, tc.key);
        key.seal_in_place_append_tag(nonce, tc.aad, in_out)
    })
}

fn less_safe_key_open_in_place(
    alg: &'static aead::Algorithm,
    tc: KnownAnswerTestCase,
) -> Result<(), error::Unspecified> {
    test_open_in_place(&tc, |nonce, in_out| {
        let key = make_less_safe_key(alg, tc.key);
        key.open_in_place(nonce, tc.aad, in_out)
    })
}

fn less_safe_key_seal_in_place_separate_tag(
    alg: &'static aead::Algorithm,
    tc: KnownAnswerTestCase,
) -> Result<(), error::Unspecified> {
    test_seal_separate_tag(&tc, |nonce, in_out| {
        let key = make_less_safe_key(alg, tc.key);
        key.seal_in_place_separate_tag(nonce, tc.aad, in_out)
    })
}

fn less_safe_key_open_within(
    alg: &'static aead::Algorithm,
    tc: KnownAnswerTestCase,
) -> Result<(), error::Unspecified> {
    test_open_within(&tc, |nonce, in_out, ciphertext_and_tag| {
        let key = make_less_safe_key(alg, tc.key);
        key.open_within(nonce, tc.aad, in_out, ciphertext_and_tag)
    })
}

#[allow(clippy::range_plus_one)]
fn key_sizes(aead_alg: &'static aead::Algorithm) {
    let key_len = aead_alg.key_len();
    let key_data = vec![0u8; key_len * 2];

    // Key is the right size.
    assert!(aead::UnboundKey::new(aead_alg, &key_data[..key_len]).is_ok());

    // Key is one byte too small.
    assert!(aead::UnboundKey::new(aead_alg, &key_data[..(key_len - 1)]).is_err());

    // Key is one byte too large.
    assert!(aead::UnboundKey::new(aead_alg, &key_data[..(key_len + 1)]).is_err());

    // Key is half the required size.
    assert!(aead::UnboundKey::new(aead_alg, &key_data[..(key_len / 2)]).is_err());

    // Key is twice the required size.
    assert!(aead::UnboundKey::new(aead_alg, &key_data[..(key_len * 2)]).is_err());

    // Key is empty.
    assert!(aead::UnboundKey::new(aead_alg, &[]).is_err());

    // Key is one byte.
    assert!(aead::UnboundKey::new(aead_alg, &[0]).is_err());
}

// Test that we reject non-standard nonce sizes.
#[allow(clippy::range_plus_one)]
#[test]
fn test_aead_nonce_sizes() {
    let nonce_len = aead::NONCE_LEN;
    let nonce = vec![0u8; nonce_len * 2];

    assert!(aead::Nonce::try_assume_unique_for_key(&nonce[..nonce_len]).is_ok());
    assert!(aead::Nonce::try_assume_unique_for_key(&nonce[..(nonce_len - 1)]).is_err());
    assert!(aead::Nonce::try_assume_unique_for_key(&nonce[..(nonce_len + 1)]).is_err());
    assert!(aead::Nonce::try_assume_unique_for_key(&nonce[..(nonce_len / 2)]).is_err());
    assert!(aead::Nonce::try_assume_unique_for_key(&nonce[..(nonce_len * 2)]).is_err());
    assert!(aead::Nonce::try_assume_unique_for_key(&[]).is_err());
    assert!(aead::Nonce::try_assume_unique_for_key(&nonce[..1]).is_err());
    assert!(aead::Nonce::try_assume_unique_for_key(&nonce[..16]).is_err()); // 128 bits.
}

#[allow(clippy::range_plus_one)]
#[test]
fn aead_chacha20_poly1305_openssh() {
    // TODO: test_aead_key_sizes(...);

    test::run(
        test_file!("aead_chacha20_poly1305_openssh_tests.txt"),
        |section, test_case| {
            assert_eq!(section, "");

            // XXX: `polyfill::convert` isn't available here.
            let key_bytes = {
                let as_vec = test_case.consume_bytes("KEY");
                let mut as_array = [0u8; aead::chacha20_poly1305_openssh::KEY_LEN];
                as_array.copy_from_slice(&as_vec);
                as_array
            };

            let sequence_num: u32 = test_case
                .consume_usize("SEQUENCE_NUMBER")
                .try_into()
                .unwrap();
            let plaintext = test_case.consume_bytes("IN");
            let ct = test_case.consume_bytes("CT");
            let expected_tag = test_case.consume_bytes("TAG");

            // TODO: Add some tests for when things fail.
            //let error = test_case.consume_optional_string("FAILS");

            let mut tag = [0u8; aead::chacha20_poly1305_openssh::TAG_LEN];
            let mut s_in_out = plaintext.clone();
            let s_key = aead::chacha20_poly1305_openssh::SealingKey::new(&key_bytes);
            s_key.seal_in_place(sequence_num, &mut s_in_out[..], &mut tag);
            assert_eq!(&ct, &s_in_out);
            assert_eq!(&expected_tag, &tag);
            let o_key = aead::chacha20_poly1305_openssh::OpeningKey::new(&key_bytes);

            {
                let o_result = o_key.open_in_place(sequence_num, &mut s_in_out[..], &tag);
                assert_eq!(o_result, Ok(&plaintext[4..]));
            }
            assert_eq!(&s_in_out[..4], &ct[..4]);
            assert_eq!(&s_in_out[4..], &plaintext[4..]);

            Ok(())
        },
    );
}

#[test]
fn aead_test_aad_traits() {
    test::compile_time_assert_send::<aead::Aad<&'_ [u8]>>();
    test::compile_time_assert_sync::<aead::Aad<&'_ [u8]>>();
    test::compile_time_assert_copy::<aead::Aad<&'_ [u8]>>();
    test::compile_time_assert_eq::<aead::Aad<Vec<u8>>>(); // `!Copy`

    let aad_123 = aead::Aad::from(vec![1, 2, 3]); // `!Copy`
    assert_eq!(aad_123, aad_123.clone()); // Cover `Clone` and `PartialEq`
    assert_eq!(
        format!("{:?}", aead::Aad::from(&[1, 2, 3])),
        "Aad([1, 2, 3])"
    );
}

#[test]
fn test_nonce_traits() {
    test::compile_time_assert_send::<aead::Nonce>();
    test::compile_time_assert_sync::<aead::Nonce>();
}

#[test]
fn test_tag_traits() {
    test::compile_time_assert_send::<aead::Tag>();
    test::compile_time_assert_sync::<aead::Tag>();

    test::compile_time_assert_copy::<aead::Tag>();
    test::compile_time_assert_clone::<aead::Tag>();

    let tag = aead::Tag::from([4u8; 16]);
    let _tag_2 = tag; // Cover `Copy`
    assert_eq!(tag.as_ref(), tag.clone().as_ref()); // Cover `Clone`
}

fn test_aead_key_traits<T: Send + Sync>() {}

#[test]
fn test_aead_key_traits_all() {
    test_aead_key_traits::<aead::OpeningKey<OneNonceSequence>>();
    test_aead_key_traits::<aead::SealingKey<OneNonceSequence>>();
    test_aead_key_traits::<aead::LessSafeKey>();
}

#[test]
fn test_aead_key_debug() {
    let key_bytes = [0; 32];
    let nonce = [0; aead::NONCE_LEN];

    let key = aead::UnboundKey::new(&aead::AES_256_GCM, &key_bytes).unwrap();
    assert_eq!(
        "UnboundKey { algorithm: AES_256_GCM }",
        format!("{:?}", key)
    );

    let sealing_key: aead::SealingKey<OneNonceSequence> = make_key(
        &aead::AES_256_GCM,
        &key_bytes,
        aead::Nonce::try_assume_unique_for_key(&nonce).unwrap(),
    );
    assert_eq!(
        "SealingKey { algorithm: AES_256_GCM }",
        format!("{:?}", sealing_key)
    );

    let opening_key: aead::OpeningKey<OneNonceSequence> = make_key(
        &aead::AES_256_GCM,
        &key_bytes,
        aead::Nonce::try_assume_unique_for_key(&nonce).unwrap(),
    );
    assert_eq!(
        "OpeningKey { algorithm: AES_256_GCM }",
        format!("{:?}", opening_key)
    );

    let key: aead::LessSafeKey = make_less_safe_key(&aead::AES_256_GCM, &key_bytes);
    assert_eq!(
        "LessSafeKey { algorithm: AES_256_GCM }",
        format!("{:?}", key)
    );
}

fn test_aead_lesssafekey_clone_for_algorithm(algorithm: &'static aead::Algorithm) {
    let test_bytes: Vec<u8> = (0..32).collect();
    let key_bytes = &test_bytes[..algorithm.key_len()];
    let nonce_bytes = &test_bytes[..algorithm.nonce_len()];

    let key1: aead::LessSafeKey =
        aead::LessSafeKey::new(aead::UnboundKey::new(algorithm, key_bytes).unwrap());
    let key2 = key1.clone();

    // LessSafeKey doesn't support AsRef or PartialEq, so instead just check that both keys produce
    // the same encrypted output.
    let mut buf1: Vec<u8> = (0..100).collect();
    let mut buf2 = buf1.clone();
    let tag1 = key1
        .seal_in_place_separate_tag(
            aead::Nonce::try_assume_unique_for_key(nonce_bytes).unwrap(),
            aead::Aad::empty(),
            &mut buf1,
        )
        .unwrap();
    let tag2 = key2
        .seal_in_place_separate_tag(
            aead::Nonce::try_assume_unique_for_key(nonce_bytes).unwrap(),
            aead::Aad::empty(),
            &mut buf2,
        )
        .unwrap();
    assert_eq!(tag1.as_ref(), tag2.as_ref());
    assert_eq!(buf1, buf2);
}

#[test]
fn test_aead_lesssafekey_clone_aes_128_gcm() {
    test_aead_lesssafekey_clone_for_algorithm(&aead::AES_128_GCM);
}

#[test]
fn test_aead_lesssafekey_clone_aes_256_gcm() {
    test_aead_lesssafekey_clone_for_algorithm(&aead::AES_256_GCM);
}

#[test]
fn test_aead_lesssafekey_clone_chacha20_poly1305() {
    test_aead_lesssafekey_clone_for_algorithm(&aead::CHACHA20_POLY1305);
}

fn make_key<K: aead::BoundKey<OneNonceSequence>>(
    algorithm: &'static aead::Algorithm,
    key: &[u8],
    nonce: aead::Nonce,
) -> K {
    let key = aead::UnboundKey::new(algorithm, key).unwrap();
    let nonce_sequence = OneNonceSequence::new(nonce);
    K::new(key, nonce_sequence)
}

fn make_less_safe_key(algorithm: &'static aead::Algorithm, key: &[u8]) -> aead::LessSafeKey {
    let key = aead::UnboundKey::new(algorithm, key).unwrap();
    aead::LessSafeKey::new(key)
}

struct OneNonceSequence(Option<aead::Nonce>);

impl OneNonceSequence {
    /// Constructs the sequence allowing `advance()` to be called
    /// `allowed_invocations` times.
    fn new(nonce: aead::Nonce) -> Self {
        Self(Some(nonce))
    }
}

impl aead::NonceSequence for OneNonceSequence {
    fn advance(&mut self) -> Result<aead::Nonce, error::Unspecified> {
        self.0.take().ok_or(error::Unspecified)
    }
}

#![allow(clippy::non_ascii_literal)]

use simdutf8::basic::from_utf8 as basic_from_utf8;
use simdutf8::basic::from_utf8_mut as basic_from_utf8_mut;
use simdutf8::compat::from_utf8 as compat_from_utf8;
use simdutf8::compat::from_utf8_mut as compat_from_utf8_mut;

#[cfg(not(feature = "std"))]
extern crate std;

#[cfg(not(feature = "std"))]
use std::{borrow::ToOwned, format};

pub trait BStrExt {
    fn repeat_x(&self, count: usize) -> Vec<u8>;
}

/// b"a".repeat() is not implemented for Rust 1.38.0 (MSRV)
impl<T> BStrExt for T
where
    T: AsRef<[u8]>,
{
    fn repeat_x(&self, count: usize) -> Vec<u8> {
        use std::io::Write;

        let x = self.as_ref();
        let mut res = Vec::with_capacity(x.len() * count);
        for _ in 0..count {
            #[allow(clippy::unwrap_used)]
            res.write_all(x).unwrap();
        }
        res
    }
}

fn test_valid(input: &[u8]) {
    // std lib sanity check
    assert!(std::str::from_utf8(input).is_ok());

    assert!(basic_from_utf8(input).is_ok());
    assert!(compat_from_utf8(input).is_ok());

    let mut mut_input = input.to_owned();
    assert!(basic_from_utf8_mut(mut_input.as_mut_slice()).is_ok());
    assert!(compat_from_utf8_mut(mut_input.as_mut_slice()).is_ok());

    #[cfg(feature = "public_imp")]
    test_valid_public_imp(input);
}

// unused for cases where public_imp is set but no SIMD functions generated...
#[cfg(feature = "public_imp")]
#[allow(dead_code)]
fn test_streaming<T: simdutf8::basic::imp::Utf8Validator>(input: &[u8], ok: bool) {
    unsafe {
        let mut validator = T::new();
        validator.update(input);
        assert_eq!(validator.finalize().is_ok(), ok);
    }
    for i in [64, 128, 256, 1024, 65536, 1, 2, 3, 36, 99].iter() {
        test_streaming_blocks::<T>(input, *i, ok)
    }
}

// unused for cases where public_imp is set but no SIMD functions generated...
#[cfg(feature = "public_imp")]
#[allow(dead_code)]
fn test_streaming_blocks<T: simdutf8::basic::imp::Utf8Validator>(
    input: &[u8],
    block_size: usize,
    ok: bool,
) {
    unsafe {
        let mut validator = T::new();
        for chunk in input.chunks(block_size) {
            validator.update(chunk);
        }
        assert_eq!(validator.finalize().is_ok(), ok);
    }
}

// unused for cases where public_imp is set but no SIMD functions generated...
#[cfg(feature = "public_imp")]
#[allow(dead_code)]
fn test_chunked_streaming<T: simdutf8::basic::imp::ChunkedUtf8Validator>(input: &[u8], ok: bool) {
    for i in [64, 128, 256, 1024, 65536].iter() {
        test_chunked_streaming_with_chunk_size::<T>(input, *i, ok)
    }
}

// unused for cases where public_imp is set but no SIMD functions generated...
#[cfg(feature = "public_imp")]
#[allow(dead_code)]
fn test_chunked_streaming_with_chunk_size<T: simdutf8::basic::imp::ChunkedUtf8Validator>(
    input: &[u8],
    chunk_size: usize,
    ok: bool,
) {
    unsafe {
        let mut validator = T::new();
        let mut chunks = input.chunks_exact(chunk_size);
        for chunk in &mut chunks {
            validator.update_from_chunks(chunk);
        }
        assert_eq!(validator.finalize(Some(chunks.remainder())).is_ok(), ok);
    }
}

#[cfg(feature = "public_imp")]
#[allow(unused_variables)]
fn test_valid_public_imp(input: &[u8]) {
    if cfg!(any(target_arch = "x86", target_arch = "x86_64")) {
        #[cfg(target_feature = "avx2")]
        unsafe {
            assert!(simdutf8::basic::imp::x86::avx2::validate_utf8(input).is_ok());
            assert!(simdutf8::compat::imp::x86::avx2::validate_utf8(input).is_ok());

            test_streaming::<simdutf8::basic::imp::x86::avx2::Utf8ValidatorImp>(input, true);
            test_chunked_streaming::<simdutf8::basic::imp::x86::avx2::ChunkedUtf8ValidatorImp>(
                input, true,
            );
        }

        #[cfg(target_feature = "sse4.2")]
        unsafe {
            assert!(simdutf8::basic::imp::x86::sse42::validate_utf8(input).is_ok());
            assert!(simdutf8::compat::imp::x86::sse42::validate_utf8(input).is_ok());

            test_streaming::<simdutf8::basic::imp::x86::sse42::Utf8ValidatorImp>(input, true);
            test_chunked_streaming::<simdutf8::basic::imp::x86::sse42::ChunkedUtf8ValidatorImp>(
                input, true,
            );
        }
    }
    #[cfg(all(
        feature = "aarch64_neon",
        target_arch = "aarch64",
        target_feature = "neon"
    ))]
    unsafe {
        assert!(simdutf8::basic::imp::aarch64::neon::validate_utf8(input).is_ok());
        assert!(simdutf8::compat::imp::aarch64::neon::validate_utf8(input).is_ok());

        test_streaming::<simdutf8::basic::imp::aarch64::neon::Utf8ValidatorImp>(input, true);
        test_chunked_streaming::<simdutf8::basic::imp::aarch64::neon::ChunkedUtf8ValidatorImp>(
            input, true,
        );
    }
    #[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
    unsafe {
        assert!(simdutf8::basic::imp::wasm32::simd128::validate_utf8(input).is_ok());
        assert!(simdutf8::compat::imp::wasm32::simd128::validate_utf8(input).is_ok());

        test_streaming::<simdutf8::basic::imp::wasm32::simd128::Utf8ValidatorImp>(input, true);
        test_chunked_streaming::<simdutf8::basic::imp::wasm32::simd128::ChunkedUtf8ValidatorImp>(
            input, true,
        );
    }
}

fn test_invalid(input: &[u8], valid_up_to: usize, error_len: Option<usize>) {
    // std lib sanity check
    let err = std::str::from_utf8(input).unwrap_err();
    assert_eq!(err.valid_up_to(), valid_up_to);
    assert_eq!(err.error_len(), error_len);

    assert!(basic_from_utf8(input).is_err());
    let err = compat_from_utf8(input).unwrap_err();
    assert_eq!(err.valid_up_to(), valid_up_to);
    assert_eq!(err.error_len(), error_len);

    #[cfg(feature = "public_imp")]
    test_invalid_public_imp(input, valid_up_to, error_len);
}

#[cfg(feature = "public_imp")]
#[allow(unused_variables)]
fn test_invalid_public_imp(input: &[u8], valid_up_to: usize, error_len: Option<usize>) {
    if cfg!(any(target_arch = "x86", target_arch = "x86_64")) {
        #[cfg(target_feature = "avx2")]
        unsafe {
            assert!(simdutf8::basic::imp::x86::avx2::validate_utf8(input).is_err());
            let err = simdutf8::compat::imp::x86::avx2::validate_utf8(input).unwrap_err();
            assert_eq!(err.valid_up_to(), valid_up_to);
            assert_eq!(err.error_len(), error_len);

            test_streaming::<simdutf8::basic::imp::x86::avx2::Utf8ValidatorImp>(input, false);
            test_chunked_streaming::<simdutf8::basic::imp::x86::avx2::ChunkedUtf8ValidatorImp>(
                input, false,
            );
        }
        #[cfg(target_feature = "sse4.2")]
        unsafe {
            assert!(simdutf8::basic::imp::x86::sse42::validate_utf8(input).is_err());
            let err = simdutf8::compat::imp::x86::sse42::validate_utf8(input).unwrap_err();
            assert_eq!(err.valid_up_to(), valid_up_to);
            assert_eq!(err.error_len(), error_len);

            test_streaming::<simdutf8::basic::imp::x86::sse42::Utf8ValidatorImp>(input, false);
            test_chunked_streaming::<simdutf8::basic::imp::x86::sse42::ChunkedUtf8ValidatorImp>(
                input, false,
            );
        }
    }
    #[cfg(all(
        feature = "aarch64_neon",
        target_arch = "aarch64",
        target_feature = "neon"
    ))]
    unsafe {
        assert!(simdutf8::basic::imp::aarch64::neon::validate_utf8(input).is_err());
        let err = simdutf8::compat::imp::aarch64::neon::validate_utf8(input).unwrap_err();
        assert_eq!(err.valid_up_to(), valid_up_to);
        assert_eq!(err.error_len(), error_len);

        test_streaming::<simdutf8::basic::imp::aarch64::neon::Utf8ValidatorImp>(input, false);
        test_chunked_streaming::<simdutf8::basic::imp::aarch64::neon::ChunkedUtf8ValidatorImp>(
            input, false,
        );
    }
    #[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
    unsafe {
        assert!(simdutf8::basic::imp::wasm32::simd128::validate_utf8(input).is_err());
        let err = simdutf8::compat::imp::wasm32::simd128::validate_utf8(input).unwrap_err();
        assert_eq!(err.valid_up_to(), valid_up_to);
        assert_eq!(err.error_len(), error_len);

        test_streaming::<simdutf8::basic::imp::wasm32::simd128::Utf8ValidatorImp>(input, false);
        test_chunked_streaming::<simdutf8::basic::imp::wasm32::simd128::ChunkedUtf8ValidatorImp>(
            input, false,
        );
    }
}

fn test_invalid_after_specific_prefix(
    input: &[u8],
    valid_up_to: usize,
    error_len: Option<usize>,
    with_suffix_error_len: Option<usize>,
    repeat: usize,
    prefix_bytes: &[u8],
) {
    {
        let mut prefixed_input = prefix_bytes.repeat_x(repeat);
        let prefix_len = prefixed_input.len();
        prefixed_input.extend_from_slice(input);
        test_invalid(prefixed_input.as_ref(), valid_up_to + prefix_len, error_len)
    }

    if repeat != 0 {
        let mut prefixed_input = prefix_bytes.repeat_x(repeat);
        let prefix_len = prefixed_input.len();
        prefixed_input.extend_from_slice(input);
        prefixed_input.extend_from_slice(prefix_bytes.repeat_x(repeat).as_slice());
        test_invalid(
            prefixed_input.as_ref(),
            valid_up_to + prefix_len,
            with_suffix_error_len,
        )
    }
}

fn test_invalid_after_prefix(
    input: &[u8],
    valid_up_to: usize,
    error_len: Option<usize>,
    with_suffix_error_len: Option<usize>,
    repeat: usize,
) {
    for prefix in [
        "a",
        "ö",
        "😊",
        "a".repeat(64).as_str(),
        ("a".repeat(64) + "ö".repeat(32).as_str()).as_str(),
    ]
    .iter()
    {
        test_invalid_after_specific_prefix(
            input,
            valid_up_to,
            error_len,
            with_suffix_error_len,
            repeat,
            prefix.as_bytes(),
        );
    }
}

fn test_invalid_after_prefixes(
    input: &[u8],
    valid_up_to: usize,
    error_len: Option<usize>,
    with_suffix_error_len: Option<usize>,
) {
    for repeat in [
        0, 1, 2, 7, 8, 9, 15, 16, 16, 31, 32, 33, 63, 64, 65, 127, 128, 129,
    ]
    .iter()
    {
        test_invalid_after_prefix(
            input,
            valid_up_to,
            error_len,
            with_suffix_error_len,
            *repeat,
        );
    }
}

#[test]
fn simple_valid() {
    test_valid(b"");

    test_valid(b"\0");

    test_valid(b"a".repeat_x(64).as_ref());

    test_valid(b"a".repeat_x(128).as_ref());

    test_valid(b"The quick brown fox jumps over the lazy dog");

    // umlauts
    test_valid("öäüÖÄÜß".as_bytes());

    // emojis
    test_valid("❤️✨🥺🔥😂😊✔️👍🥰".as_bytes());

    // Chinese
    test_valid("断用山昨屈内銀代意検瓶調像。情旗最投任留財夜隆年表高学送意功者。辺図掲記込真通第民国聞平。海帰傷芸記築世防橋整済歳権君注。選紙例並情夕破勢景移情誇進場豊読。景関有権米武野範随惑旬特覧刊野。相毎加共情面教地作減関絡。暖料児違歩致本感閉浦出楽赤何。時選権週邑針格事提一案質名投百定。止感右聞食三年外積文載者別。".as_bytes());

    // Japanese
    test_valid("意ざど禁23費サヒ車園オスミト規更ワエ異67事続トソキ音合岡治こ訪京ぴ日9稿がト明安イ抗的ウクロコ売一エコヨホ必噴塗ッ。索墓ー足議需レ応予ニ質県トぴン学市機だほせフ車捕コニ自校がこで極3力イい増娘汁表製ク。委セヤホネ作誌ミマクソ続新ほし月中報制どてびフ字78完りっせが村惹ヨサコ訳器りそ参受草ムタ大移ッけでつ番足ほこン質北ぽのよう応一ア輝労イ手人う再茨夕へしう。".as_bytes());

    // Korean
    test_valid("3인은 대법원장이 지명하는 자를 임명한다, 대통령은 제3항과 제4항의 사유를 지체없이 공포하여야 한다, 제한하는 경우에도 자유와 권리의 본질적인 내용을 침해할 수 없다, 국가는 전통문화의 계승·발전과 민족문화의 창달에 노력하여야 한다.".as_bytes());
}

#[test]
fn simple_invalid() {
    test_invalid_after_prefixes(b"\xFF", 0, Some(1), Some(1));

    // incomplete umlaut
    test_invalid_after_prefixes(b"\xC3", 0, None, Some(1));

    // incomplete emoji
    test_invalid_after_prefixes(b"\xF0", 0, None, Some(1));
    test_invalid_after_prefixes(b"\xF0\x9F", 0, None, Some(2));
    test_invalid_after_prefixes(b"\xF0\x9F\x98", 0, None, Some(3));
}

#[test]
fn incomplete_on_32nd_byte() {
    let mut invalid = b"a".repeat_x(31);
    invalid.push(b'\xF0');
    test_invalid(&invalid, 31, None)
}

#[test]
fn incomplete_on_64th_byte() {
    let mut invalid = b"a".repeat_x(63);
    invalid.push(b'\xF0');
    test_invalid(&invalid, 63, None)
}

#[test]
fn incomplete_on_64th_byte_65_bytes_total() {
    let mut invalid = b"a".repeat_x(63);
    invalid.push(b'\xF0');
    invalid.push(b'a');
    test_invalid(&invalid, 63, Some(1))
}

#[test]
fn error_display_basic() {
    assert_eq!(
        format!("{}", basic_from_utf8(b"\xF0").unwrap_err()),
        "invalid utf-8 sequence"
    );
    assert_eq!(
        format!("{}", basic_from_utf8(b"a\xF0a").unwrap_err()),
        "invalid utf-8 sequence"
    );
}

#[test]
fn error_display_compat() {
    assert_eq!(
        format!("{}", compat_from_utf8(b"\xF0").unwrap_err()),
        "incomplete utf-8 byte sequence from index 0"
    );
    assert_eq!(
        format!("{}", compat_from_utf8(b"a\xF0a").unwrap_err()),
        "invalid utf-8 sequence of 1 bytes from index 1"
    );
    assert_eq!(
        format!("{}", compat_from_utf8(b"a\xF0\x9Fa").unwrap_err()),
        "invalid utf-8 sequence of 2 bytes from index 1"
    );
    assert_eq!(
        format!("{}", compat_from_utf8(b"a\xF0\x9F\x98a").unwrap_err()),
        "invalid utf-8 sequence of 3 bytes from index 1"
    );
}

#[test]
fn error_debug_basic() {
    assert_eq!(
        format!("{:?}", basic_from_utf8(b"\xF0").unwrap_err()),
        "Utf8Error"
    );
}

#[test]
fn error_debug_compat() {
    assert_eq!(
        format!("{:?}", compat_from_utf8(b"\xF0").unwrap_err()),
        "Utf8Error { valid_up_to: 0, error_len: None }"
    );
    assert_eq!(
        format!("{:?}", compat_from_utf8(b"a\xF0a").unwrap_err()),
        "Utf8Error { valid_up_to: 1, error_len: Some(1) }"
    );
}

#[test]
fn error_derives_basic() {
    let err = basic_from_utf8(b"\xF0").unwrap_err();
    #[allow(clippy::clone_on_copy)] // used for coverage
    let err2 = err.clone();
    assert_eq!(err, err2);
    assert!(!(err != err2));
}

#[test]
fn error_derives_compat() {
    let err = compat_from_utf8(b"\xF0").unwrap_err();
    #[allow(clippy::clone_on_copy)] // used for coverage
    let err2 = err.clone();
    assert_eq!(err, err2);
    assert!(!(err != err2));
}

#[test]
#[should_panic]
#[cfg(all(feature = "public_imp", target_feature = "avx2"))]
fn test_avx2_chunked_panic() {
    test_chunked_streaming_with_chunk_size::<
        simdutf8::basic::imp::x86::avx2::ChunkedUtf8ValidatorImp,
    >(b"abcd", 1, true);
}

#[test]
#[should_panic]
#[cfg(all(feature = "public_imp", target_feature = "sse4.2"))]
fn test_sse42_chunked_panic() {
    test_chunked_streaming_with_chunk_size::<
        simdutf8::basic::imp::x86::sse42::ChunkedUtf8ValidatorImp,
    >(b"abcd", 1, true);
}

#[test]
#[should_panic]
#[cfg(all(
    feature = "public_imp",
    target_arch = "aarch64",
    feature = "aarch64_neon"
))]
fn test_neon_chunked_panic() {
    test_chunked_streaming_with_chunk_size::<
        simdutf8::basic::imp::aarch64::neon::ChunkedUtf8ValidatorImp,
    >(b"abcd", 1, true);
}

// the test runner will ignore this test probably due to limitations of panic handling/threading
// of that target--keeping this here so that when it can be tested properly, it will
// FIXME: remove this comment once this works properly.
#[test]
#[should_panic]
#[cfg(all(
    feature = "public_imp",
    target_arch = "wasm32",
    target_feature = "simd128"
))]
fn test_simd128_chunked_panic() {
    test_chunked_streaming_with_chunk_size::<
        simdutf8::basic::imp::wasm32::simd128::ChunkedUtf8ValidatorImp,
    >(b"abcd", 1, true);
}

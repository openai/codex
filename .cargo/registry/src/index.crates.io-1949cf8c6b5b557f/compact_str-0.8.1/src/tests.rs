use alloc::borrow::Cow;
use alloc::boxed::Box;
use alloc::string::{
    String,
    ToString,
};
use alloc::vec::Vec;
use core::str::FromStr;
use core::{
    num,
    slice,
};

use proptest::collection::SizeRange;
use proptest::prelude::*;
use test_case::test_case;
use test_strategy::proptest;

use crate::{
    format_compact,
    CompactString,
    ToCompactString,
};

#[cfg(target_pointer_width = "64")]
const MAX_SIZE: usize = 24;
#[cfg(target_pointer_width = "32")]
const MAX_SIZE: usize = 12;

const SIXTEEN_MB: usize = 16 * 1024 * 1024;

/// generates random unicode strings, upto 80 chars long
pub fn rand_unicode() -> impl Strategy<Value = String> {
    proptest::collection::vec(proptest::char::any(), 0..80).prop_map(|v| v.into_iter().collect())
}

/// generates a random collection of bytes, upto 80 bytes long
pub fn rand_bytes() -> impl Strategy<Value = Vec<u8>> {
    proptest::collection::vec(any::<u8>(), 0..80)
}

/// generates a random collection of `u16`s, upto 80 elements long
pub fn rand_u16s() -> impl Strategy<Value = Vec<u16>> {
    proptest::collection::vec(any::<u16>(), 0..80)
}

/// [`proptest::strategy::Strategy`] that generates [`String`]s with up to `len` bytes
pub fn rand_unicode_with_range(range: impl Into<SizeRange>) -> impl Strategy<Value = String> {
    proptest::collection::vec(proptest::char::any(), range).prop_map(|v| v.into_iter().collect())
}

/// generates groups upto 40 strings long of random unicode strings, upto 80 chars long
fn rand_unicode_collection() -> impl Strategy<Value = Vec<String>> {
    proptest::collection::vec(rand_unicode(), 0..40)
}

/// Asserts a [`CompactString`] is allocated properly
fn assert_allocated_properly(compact: &CompactString) {
    if compact.len() <= MAX_SIZE {
        assert!(!compact.is_heap_allocated())
    } else {
        assert!(compact.is_heap_allocated())
    }
}

#[proptest]
#[cfg_attr(miri, ignore)]
fn proptest_strings_roundtrip(#[strategy(rand_unicode())] word: String) {
    let compact = CompactString::new(&word);
    prop_assert_eq!(&word, &compact);
}

#[proptest]
#[cfg_attr(miri, ignore)]
fn proptest_strings_allocated_properly(#[strategy(rand_unicode())] word: String) {
    let compact = CompactString::new(word);
    assert_allocated_properly(&compact);
}

#[proptest]
#[cfg_attr(miri, ignore)]
fn proptest_char_iterator_roundtrips(#[strategy(rand_unicode())] word: String) {
    let compact: CompactString = word.clone().chars().collect();
    prop_assert_eq!(&word, &compact)
}

#[proptest]
#[cfg_attr(miri, ignore)]
fn proptest_string_iterator_roundtrips(
    #[strategy(rand_unicode_collection())] collection: Vec<String>,
) {
    let compact: CompactString = collection.clone().into_iter().collect();
    let word: String = collection.into_iter().collect();
    prop_assert_eq!(&word, &compact);
}

#[proptest]
#[cfg_attr(miri, ignore)]
fn proptest_from_bytes_roundtrips(#[strategy(rand_unicode())] word: String) {
    let bytes = word.into_bytes();
    let compact = CompactString::from_utf8(&bytes).unwrap();
    let word = String::from_utf8(bytes).unwrap();

    prop_assert_eq!(compact, word);
}

#[proptest]
#[cfg_attr(miri, ignore)]
fn proptest_from_bytes_only_valid_utf8(#[strategy(rand_bytes())] bytes: Vec<u8>) {
    let compact_result = CompactString::from_utf8(&bytes);
    let word_result = String::from_utf8(bytes);

    match (compact_result, word_result) {
        (Ok(c), Ok(s)) => prop_assert_eq!(c, s),
        (Err(c_err), Err(s_err)) => prop_assert_eq!(c_err, s_err.utf8_error()),
        _ => panic!("CompactString and core::str read UTF-8 differently?"),
    }
}

#[proptest]
#[cfg_attr(miri, ignore)]
fn proptest_from_lossy_cow_roundtrips(#[strategy(rand_bytes())] bytes: Vec<u8>) {
    let cow = String::from_utf8_lossy(&bytes[..]);
    let compact = CompactString::from(cow.clone());
    prop_assert_eq!(cow, compact);
}

#[proptest]
#[cfg_attr(miri, ignore)]
fn proptest_reserve_and_write_bytes(#[strategy(rand_unicode())] word: String) {
    let mut compact = CompactString::default();
    prop_assert!(compact.is_empty());

    // reserve enough space to write our bytes
    compact.reserve(word.len());

    // SAFETY: We're writing a String which we know is UTF-8
    let slice = unsafe { compact.as_mut_bytes() };
    slice[..word.len()].copy_from_slice(word.as_bytes());

    // SAFETY: We know this is the length of our string, since `compact` started with 0 bytes
    // and we just wrote `word.len()` bytes
    unsafe { compact.set_len(word.len()) }

    prop_assert_eq!(&word, &compact);
}

#[proptest]
#[cfg_attr(miri, ignore)]
fn proptest_reserve_and_write_bytes_allocated_properly(#[strategy(rand_unicode())] word: String) {
    let mut compact = CompactString::default();
    prop_assert!(compact.is_empty());

    // reserve enough space to write our bytes
    compact.reserve(word.len());

    // SAFETY: We're writing a String which we know is UTF-8
    let slice = unsafe { compact.as_mut_bytes() };
    slice[..word.len()].copy_from_slice(word.as_bytes());

    // SAFETY: We know this is the length of our string, since `compact` started with 0 bytes
    // and we just wrote `word.len()` bytes
    unsafe { compact.set_len(word.len()) }

    prop_assert_eq!(compact.len(), word.len());

    // The string should be heap allocated if `word` was > MAX_SIZE
    //
    // NOTE: The reserve and write API's don't currently support the Packed representation
    prop_assert_eq!(compact.is_heap_allocated(), word.len() > MAX_SIZE);
}

#[proptest]
#[cfg_attr(miri, ignore)]
fn proptest_arbitrary_compact_string_converts_to_string(#[strategy(rand_unicode())] word: String) {
    let compact = CompactString::new(&word);
    let result = String::from(compact);

    prop_assert_eq!(result.len(), word.len());
    prop_assert_eq!(result, word);
}

#[proptest]
#[cfg_attr(miri, ignore)]
fn proptest_extend_chars_allocated_properly(
    #[strategy(rand_unicode())] start: String,
    #[strategy(rand_unicode())] extend: String,
) {
    let mut compact = CompactString::new(&start);
    compact.extend(extend.chars());

    let mut control = start.clone();
    #[allow(clippy::string_extend_chars)]
    control.extend(extend.chars());

    prop_assert_eq!(&compact, &control);
    assert_allocated_properly(&compact);
}

#[proptest]
#[cfg_attr(miri, ignore)]
fn proptest_truncate(#[strategy(rand_unicode())] mut control: String, val: u8) {
    let initial_len = control.len();
    let mut compact = CompactString::new(&control);

    // turn the arbitrary number `val` into character indices
    let new_len = control
        .char_indices()
        .cycle()
        .nth(val as usize)
        .unwrap_or_default()
        .0;

    // then truncate both strings string
    control.truncate(new_len);
    compact.truncate(new_len);

    // assert they're equal
    prop_assert_eq!(&control, &compact);
    prop_assert_eq!(control.len(), compact.len());

    // If we started as heap allocated, we should stay heap allocated. This prevents us from
    // needing to deallocate the buffer on the heap
    if initial_len > MAX_SIZE {
        prop_assert!(compact.is_heap_allocated());
    } else {
        prop_assert!(!compact.is_heap_allocated());
    }
}

#[proptest]
#[cfg_attr(miri, ignore)]
fn proptest_from_utf16_roundtrips(#[strategy(rand_unicode())] control: String) {
    let utf16_buf: Vec<u16> = control.encode_utf16().collect();
    let compact = CompactString::from_utf16(utf16_buf).unwrap();

    assert_eq!(compact, control);
}

#[proptest]
#[cfg_attr(miri, ignore)]
fn proptest_from_utf16_random(#[strategy(rand_u16s())] buf: Vec<u16>) {
    let compact = CompactString::from_utf16(&buf);
    let std_str = String::from_utf16(&buf);

    match (compact, std_str) {
        (Ok(c), Ok(s)) => assert_eq!(c, s),
        (Err(_), Err(_)) => (),
        (c_res, s_res) => panic!(
            "CompactString and String decode UTF-16 differently? {:?} {:?}",
            c_res, s_res
        ),
    }
}

#[proptest]
#[cfg_attr(miri, ignore)]
fn proptest_from_utf16_lossy_roundtrips(#[strategy(rand_unicode())] control: String) {
    let utf16_buf: Vec<u16> = control.encode_utf16().collect();
    let compact = CompactString::from_utf16_lossy(utf16_buf);

    assert_eq!(compact, control);
}

#[proptest]
#[cfg_attr(miri, ignore)]
fn proptest_from_utf16_lossy_random(#[strategy(rand_u16s())] buf: Vec<u16>) {
    let control = String::from_utf16_lossy(&buf);
    let compact = CompactString::from_utf16_lossy(&buf);
    assert_eq!(compact, control);
}

#[proptest]
#[cfg_attr(miri, ignore)]
fn proptest_remove(#[strategy(rand_unicode_with_range(1..80))] mut control: String, val: u8) {
    let initial_len = control.len();
    let mut compact = CompactString::new(&control);

    let idx = control
        .char_indices()
        .cycle()
        .nth(val as usize)
        .unwrap_or_default()
        .0;

    let control_char = control.remove(idx);
    let compact_char = compact.remove(idx);

    prop_assert_eq!(control_char, compact_char);
    prop_assert_eq!(control_char, compact_char);
    prop_assert_eq!(control.len(), compact.len());

    // If we started as heap allocated, we should stay heap allocated. This prevents us from
    // needing to deallocate the buffer on the heap
    if initial_len > MAX_SIZE {
        prop_assert!(compact.is_heap_allocated());
    } else {
        prop_assert!(!compact.is_heap_allocated());
    }
}

#[proptest]
#[cfg_attr(miri, ignore)]
fn proptest_from_utf8_unchecked(#[strategy(rand_bytes())] bytes: Vec<u8>) {
    let compact = unsafe { CompactString::from_utf8_unchecked(&bytes) };
    let std_str = unsafe { String::from_utf8_unchecked(bytes.clone()) };

    // we might not make valid strings, but we should be able to read the underlying bytes
    assert_eq!(compact.as_bytes(), std_str.as_bytes());
    assert_eq!(compact.as_bytes(), bytes);

    // make sure the length is correct
    assert_eq!(compact.len(), bytes.len());

    // check if we were valid UTF-8, if so, assert the data written into the CompactString is
    // correct
    let data_is_valid = core::str::from_utf8(&bytes);
    let compact_is_valid = core::str::from_utf8(compact.as_bytes());
    let std_str_is_valid = core::str::from_utf8(std_str.as_bytes());

    match (data_is_valid, compact_is_valid, std_str_is_valid) {
        (Ok(d), Ok(c), Ok(s)) => {
            // if we get &str's back, make sure they're all equal
            assert_eq!(d, c);
            assert_eq!(c, s);
        }
        (Err(d), Err(c), Err(s)) => {
            // if we get errors back, the errors should be the same
            assert_eq!(d, c);
            assert_eq!(c, s);
        }
        _ => panic!("data, CompactString, and String disagreed?"),
    }
}

#[proptest]
#[cfg_attr(miri, ignore)]
fn proptest_to_ascii_uppercase(#[strategy(rand_unicode())] control: String) {
    let compact = CompactString::new(&control);

    let control = control.to_ascii_uppercase();
    let compact = compact.to_ascii_uppercase();

    prop_assert_eq!(control, compact);
}

#[proptest]
#[cfg_attr(miri, ignore)]
fn proptest_to_ascii_lowercase(#[strategy(rand_unicode())] control: String) {
    let compact = CompactString::new(&control);

    let control = control.to_ascii_lowercase();
    let compact = compact.to_ascii_lowercase();

    prop_assert_eq!(control, compact);
}

#[proptest]
#[cfg_attr(miri, ignore)]
fn proptest_to_uppercase(#[strategy(rand_unicode())] control: String) {
    let compact = CompactString::new(&control);

    let control = control.to_uppercase();
    let compact = compact.to_uppercase();

    prop_assert_eq!(control, compact);
}

#[proptest]
#[cfg_attr(miri, ignore)]
fn proptest_to_lowercase(#[strategy(rand_unicode())] control: String) {
    let compact = CompactString::new(&control);

    let control = control.to_lowercase();
    let compact = compact.to_lowercase();

    prop_assert_eq!(control, compact);
}

#[test]
fn test_const_creation() {
    const EMPTY: CompactString = CompactString::const_new("");
    const SHORT: CompactString = CompactString::const_new("rust");

    const EMPTY_STATIC_STR: CompactString = CompactString::const_new("");
    const SHORT_STATIC_STR: CompactString = CompactString::const_new("rust");

    #[cfg(target_pointer_width = "64")]
    const PACKED: CompactString = CompactString::const_new("i am 24 characters long!");
    #[cfg(target_pointer_width = "32")]
    const PACKED: CompactString = CompactString::const_new("i am 12 char");

    const PACKED_STATIC_STR0: CompactString = CompactString::const_new("i am 24 characters long!");
    const PACKED_STATIC_STR1: CompactString = CompactString::const_new("i am 12 char");

    assert_eq!(EMPTY, CompactString::new(""));
    assert_eq!(SHORT, CompactString::new("rust"));

    assert_eq!(EMPTY_STATIC_STR, CompactString::new(""));
    assert_eq!(SHORT_STATIC_STR, CompactString::new("rust"));

    #[cfg(target_pointer_width = "64")]
    assert_eq!(PACKED, CompactString::new("i am 24 characters long!"));
    #[cfg(target_pointer_width = "32")]
    assert_eq!(PACKED, CompactString::new("i am 12 char"));

    assert_eq!(
        PACKED_STATIC_STR0,
        CompactString::new("i am 24 characters long!")
    );
    assert_eq!(PACKED_STATIC_STR1, CompactString::new("i am 12 char"));
}

#[test]
fn test_short_ascii() {
    // always inlined on all archs
    let strs = vec!["nyc", "statue", "liberty", "img_1234.png"];

    for s in strs {
        let compact = CompactString::new(s);
        assert_eq!(compact, s);
        assert_eq!(s, compact);
        assert!(!compact.is_heap_allocated());
    }
}

#[test]
fn test_short_unicode() {
    let strs = vec![
        ("🦀", false),
        ("🌧☀️", false),
        // str is 12 bytes long, and leading character is non-ASCII
        ("咬𓅈ꁈ:_", false),
    ];

    for (s, is_heap) in strs {
        let compact = CompactString::new(s);
        assert_eq!(compact, s);
        assert_eq!(s, compact);
        assert_eq!(compact.is_heap_allocated(), is_heap);
    }
}

#[test]
fn test_medium_ascii() {
    let strs = vec![
        "rustconf 2021",
        "new york city",
        "nyc pizza is good",
        "test the 24 char limit!!",
    ];

    for s in strs {
        let compact = CompactString::new(s);
        assert_eq!(compact, s);
        assert_eq!(s, compact);

        #[cfg(target_pointer_width = "64")]
        let is_heap = false;
        #[cfg(target_pointer_width = "32")]
        let is_heap = true;
        assert_eq!(compact.is_heap_allocated(), is_heap);
    }
}

#[test]
fn test_medium_unicode() {
    let strs = vec![
        ("☕️👀😁🎉", false),
        // str is 24 bytes long, and leading character is non-ASCII
        ("🦀😀😃😄😁🦀", false),
    ];

    #[allow(unused_variables)]
    for (s, is_heap) in strs {
        let compact = CompactString::new(s);
        assert_eq!(compact, s);
        assert_eq!(s, compact);

        #[cfg(target_pointer_width = "32")]
        let is_heap = true;

        assert_eq!(compact.is_heap_allocated(), is_heap);
    }
}

#[test]
fn test_from_str_trait() {
    let s = "hello_world";

    // Until the never type `!` is stabilized, we have to unwrap here
    let c = CompactString::from_str(s).unwrap();

    assert_eq!(s, c);
}

#[test]
#[cfg_attr(target_pointer_width = "32", ignore)]
fn test_from_char_iter() {
    let s = "\u{0} 0 \u{0}a𐀀𐀀 𐀀a𐀀";
    let compact: CompactString = s.chars().collect();

    assert!(!compact.is_heap_allocated());
    assert_eq!(s, compact);
}

#[test]
#[cfg_attr(target_pointer_width = "32", ignore)]
fn test_extend_packed_from_empty() {
    let s = "  0\u{80}A\u{0}𐀀 𐀀¡a𐀀0";

    let mut compact = CompactString::new(s);
    assert!(!compact.is_heap_allocated());

    // extend from an empty iterator
    compact.extend("".chars());

    // we should still be heap allocated
    assert!(!compact.is_heap_allocated());
}

#[test_case(CompactString::from(""); "inline")]
#[test_case(CompactString::const_new(""); "static_str")]
fn test_pop_empty(mut compact: CompactString) {
    let num_pops = 256;
    (0..num_pops).for_each(|_| {
        let ch = compact.pop();
        assert!(ch.is_none());
    });
    assert!(compact.is_empty());
    assert_eq!(compact, "");
}

#[test]
fn test_extend_from_empty_strs() {
    let strs = vec![
        "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "",
        "", "",
    ];
    let compact: CompactString = strs.clone().into_iter().collect();

    assert_eq!(compact, "");
    assert!(compact.is_empty());
    assert!(!compact.is_heap_allocated());
}

#[test]
fn test_compact_str_is_send_and_sync() {
    fn is_send_and_sync<T: Send + Sync>() {}
    is_send_and_sync::<CompactString>();
}

#[test_case(CompactString::default(); "inline")]
#[test_case(CompactString::const_new(""); "static_str")]
fn test_fmt_write(mut compact: CompactString) {
    use core::fmt::Write;

    write!(compact, "test").unwrap();
    assert_eq!(compact, "test");

    writeln!(compact, "{}", 1234).unwrap();
    assert_eq!(compact, "test1234\n");

    #[allow(clippy::write_literal)]
    write!(compact, "{:>8} {} {:<8}", "some", "more", "words").unwrap();
    assert_eq!(compact, "test1234\n    some more words   ");
}

#[allow(clippy::unnecessary_to_owned, clippy::op_ref)]
#[test]
fn test_plus_operator() {
    // + &CompactString
    assert_eq!(CompactString::from("a") + &CompactString::from("b"), "ab");
    // + &str
    assert_eq!(CompactString::from("a") + "b", "ab");
    // + &String
    assert_eq!(CompactString::from("a") + &String::from("b"), "ab");
    // + &Box<str>
    let box_str = String::from("b").into_boxed_str();
    assert_eq!(CompactString::from("a") + &box_str, "ab");
    // + &Cow<'a, str>
    let cow = Cow::from("b");
    assert_eq!(CompactString::from("a") + &cow, "ab");

    // Implementing `Add<T> for String` can break adding &String or other types to String, so we
    // explicitly don't do this. See https://github.com/rust-lang/rust/issues/77143 for more details.
    // Below we assert adding types to String still compiles

    // String + &CompactString
    assert_eq!(String::from("a") + &CompactString::from("b"), "ab");
    // String + &String
    assert_eq!(String::from("a") + &("b".to_string()), "ab");
    // String + &str
    assert_eq!(String::from("a") + "b", "ab");
}

#[allow(clippy::unnecessary_to_owned, clippy::op_ref)]
#[test]
fn test_plus_operator_static_str() {
    // + &CompactString
    assert_eq!(
        CompactString::const_new("a") + &CompactString::const_new("b"),
        "ab"
    );
    // + &str
    assert_eq!(CompactString::const_new("a") + "b", "ab");
    // + &String
    assert_eq!(CompactString::const_new("a") + &String::from("b"), "ab");
    // + &Box<str>
    let box_str = String::from("b").into_boxed_str();
    assert_eq!(CompactString::const_new("a") + &box_str, "ab");
    // + &Cow<'a, str>
    let cow = Cow::from("b");
    assert_eq!(CompactString::const_new("a") + &cow, "ab");

    // Implementing `Add<T> for String` can break adding &String or other types to String, so we
    // explicitly don't do this. See https://github.com/rust-lang/rust/issues/77143 for more details.
    // Below we assert adding types to String still compiles

    // String + &CompactString
    assert_eq!(String::from("a") + &CompactString::const_new("b"), "ab");
    // String + &String
    assert_eq!(String::from("a") + &("b".to_string()), "ab");
    // String + &str
    assert_eq!(String::from("a") + "b", "ab");
}

#[test]
fn test_plus_equals_operator() {
    let mut m = CompactString::from("a");
    m += "b";
    assert_eq!(m, "ab");
}

#[test]
fn test_plus_equals_operator_static_str() {
    let mut m = CompactString::const_new("a");
    m += "b";
    assert_eq!(m, "ab");
}

// Allow these lints because we're explicitly testing impls for owned types and
// reference types.
#[allow(clippy::cmp_owned)]
#[allow(clippy::op_ref)]
#[test]
fn test_eq_operator() {
    let x = CompactString::const_new("foo");
    let y = x.clone();

    macro_rules! test_impl {
        ($a:expr, $b:expr) => {
            let _ = $a == $b;
            let _ = &$a == $b;
            let _ = &$a == &$b;

            let _ = $b == $a;
            let _ = &$b == $a;
            let _ = &$b == &$a;
        };
    }

    test_impl!("a", x);
    test_impl!(String::from("a"), x);
    test_impl!(Cow::Borrowed("a"), x);
    test_impl!(y, x);
}

#[test]
fn test_u8_to_compact_string() {
    let vals = [u8::MIN, 1, 42, u8::MAX - 2, u8::MAX - 1, u8::MAX];

    for x in &vals {
        let c = x.to_compact_string();
        let s = x.to_string();

        assert_eq!(c, s);
        assert!(!c.is_heap_allocated());
    }
}

#[test]
fn test_i8_to_compact_string() {
    let vals = [
        i8::MIN,
        i8::MIN + 1,
        i8::MIN + 2,
        -1,
        0,
        1,
        42,
        i8::MAX - 2,
        i8::MAX - 1,
        i8::MAX,
    ];

    for x in &vals {
        let c = x.to_compact_string();
        let s = x.to_string();

        assert_eq!(c, s);
        assert!(!c.is_heap_allocated());
    }
}

#[test]
fn test_u16_to_compact_string() {
    let vals = [u16::MIN, 1, 42, 999, u16::MAX - 2, u16::MAX - 1, u16::MAX];

    for x in &vals {
        let c = x.to_compact_string();
        let s = x.to_string();

        assert_eq!(c, s);
        assert!(!c.is_heap_allocated());
    }
}

#[test]
fn test_i16_to_compact_string() {
    let vals = [
        i16::MIN,
        i16::MIN + 1,
        i16::MIN + 2,
        -42,
        -1,
        0,
        1,
        42,
        999,
        i16::MAX - 2,
        i16::MAX - 1,
        i16::MAX,
    ];

    for x in &vals {
        let c = x.to_compact_string();
        let s = x.to_string();

        assert_eq!(c, s);
        assert!(!c.is_heap_allocated());
    }
}

#[test]
fn test_u32_to_compact_string() {
    let vals = [
        u32::MIN,
        1,
        42,
        999,
        123456789,
        u32::MAX - 2,
        u32::MAX - 1,
        u32::MAX,
    ];

    for x in &vals {
        let c = x.to_compact_string();
        let s = x.to_string();

        assert_eq!(c, s);
        assert!(!c.is_heap_allocated());
    }
}

#[test]
fn test_i32_to_compact_string() {
    let vals = [
        i32::MIN,
        i32::MIN + 2,
        i32::MIN + 1,
        -12345678,
        -42,
        -1,
        0,
        1,
        999,
        123456789,
        i32::MAX - 2,
        i32::MAX - 1,
        i32::MAX,
    ];

    for x in &vals {
        let c = x.to_compact_string();
        let s = x.to_string();

        assert_eq!(c, s);
        assert!(!c.is_heap_allocated());
    }
}

#[test]
fn test_u64_to_compact_string() {
    let vals = [
        u64::MIN,
        1,
        999,
        123456789,
        98765432123456,
        u64::MAX - 2,
        u64::MAX - 1,
        u64::MAX,
    ];

    for x in &vals {
        let c = x.to_compact_string();
        let s = x.to_string();

        assert_eq!(c, s);

        // u64 can be up-to 20 characters long, which can't be inlined on 32-bit arches
        #[cfg(target_pointer_width = "64")]
        assert!(!c.is_heap_allocated());
    }
}

#[test]
fn test_i64_to_compact_string() {
    let vals = [
        i64::MIN,
        i64::MIN + 1,
        i64::MIN + 2,
        -22222222,
        -42,
        0,
        1,
        999,
        123456789,
        i64::MAX - 2,
        i64::MAX - 1,
        i64::MAX,
    ];

    for x in &vals {
        let c = x.to_compact_string();
        let s = x.to_string();

        assert_eq!(c, s);

        // i64 can be up-to 20 characters long, which can't be inlined on 32-bit arches
        #[cfg(target_pointer_width = "64")]
        assert!(!c.is_heap_allocated());
    }
}

#[test]
fn test_u128_to_compact_string() {
    let vals = [
        u128::MIN,
        1,
        999,
        123456789,
        u128::MAX - 2,
        u128::MAX - 1,
        u128::MAX,
    ];

    for x in &vals {
        let c = x.to_compact_string();
        let s = x.to_string();

        assert_eq!(c, s);
    }
}

#[test]
fn test_i128_to_compact_string() {
    let vals = [
        i128::MIN,
        i128::MIN + 1,
        i128::MIN + 2,
        -22222222,
        -42,
        0,
        1,
        999,
        123456789,
        i128::MAX - 2,
        i128::MAX - 1,
        i128::MAX,
    ];

    for x in &vals {
        let c = x.to_compact_string();
        let s = x.to_string();

        assert_eq!(c, s);
    }
}

#[test]
fn test_bool_to_compact_string() {
    let c = true.to_compact_string();
    let s = true.to_string();

    assert_eq!("true", c);
    assert_eq!(c, s);
    assert!(!c.is_heap_allocated());

    let c = false.to_compact_string();
    let s = false.to_string();

    assert_eq!("false", c);
    assert_eq!(c, s);
    assert!(!c.is_heap_allocated());
}

macro_rules! assert_int_MAX_to_compact_string {
    ($int: ty) => {
        assert_eq!(&*<$int>::MAX.to_string(), &*<$int>::MAX.to_compact_string());
    };
}

#[test]
fn test_to_compact_string() {
    // Test specialisation for bool, char and String
    assert_eq!(&*true.to_string(), "true".to_compact_string());
    assert_eq!(&*false.to_string(), "false".to_compact_string());

    assert_eq!("1", '1'.to_compact_string());
    assert_eq!("2333", "2333".to_string().to_compact_string());
    assert_eq!("2333", "2333".to_compact_string().to_compact_string());

    // Test specialisation for int and nonzero_int using itoa
    assert_eq!("234", 234.to_compact_string());
    assert_eq!(
        "234",
        num::NonZeroU64::new(234).unwrap().to_compact_string()
    );

    assert_int_MAX_to_compact_string!(u8);
    assert_int_MAX_to_compact_string!(i8);

    assert_int_MAX_to_compact_string!(u16);
    assert_int_MAX_to_compact_string!(i16);

    assert_int_MAX_to_compact_string!(u32);
    assert_int_MAX_to_compact_string!(i32);

    assert_int_MAX_to_compact_string!(u64);
    assert_int_MAX_to_compact_string!(i64);

    assert_int_MAX_to_compact_string!(usize);
    assert_int_MAX_to_compact_string!(isize);

    // Test specialisation for f32 and f64 using ryu
    // TODO: Fix bug in powerpc64, which is a little endian system
    #[cfg(not(all(target_arch = "powerpc64", target_pointer_width = "64")))]
    {
        assert_eq!(
            (&*3.2_f32.to_string(), &*288888.290028_f64.to_string()),
            (
                &*3.2_f32.to_compact_string(),
                &*288888.290028_f64.to_compact_string()
            )
        );

        assert_eq!("inf", f32::INFINITY.to_compact_string());
        assert_eq!("-inf", f32::NEG_INFINITY.to_compact_string());

        assert_eq!("inf", f64::INFINITY.to_compact_string());
        assert_eq!("-inf", f64::NEG_INFINITY.to_compact_string());

        assert_eq!("NaN", f32::NAN.to_compact_string());
        assert_eq!("NaN", f64::NAN.to_compact_string());
    }

    // Test generic Display implementation
    assert_eq!("234", "234".to_compact_string());
    assert_eq!("12345", format_compact!("{}", "12345"));
    assert_eq!("112345", format_compact!("1{}", "12345"));
    assert_eq!("1123452", format_compact!("1{}{}", "12345", 2));
    assert_eq!("11234522", format_compact!("1{}{}{}", "12345", 2, '2'));
    assert_eq!(
        "112345221000",
        format_compact!("1{}{}{}{}", "12345", 2, '2', 1000)
    );

    // Test string longer than repr::MAX_SIZE
    assert_eq!(
        "01234567890123456789999999",
        format_compact!("0{}67890123456789{}", "12345", 999999)
    );
}

#[test]
fn test_into_string_large_string_with_excess_capacity() {
    let mut string = String::with_capacity(128);
    string.push_str("abcdefghijklmnopqrstuvwxyz");
    let str_addr = string.as_ptr();
    let str_len = string.len();
    let str_cap = string.capacity();

    let compact = CompactString::from(string);
    let new_string = String::from(compact);
    let new_str_addr = new_string.as_ptr();
    let new_str_len = new_string.len();
    let new_str_cap = new_string.capacity();

    assert_eq!(str_addr, new_str_addr);
    assert_eq!(str_len, new_str_len);
    assert_eq!(str_cap, new_str_cap);
}

#[test]
fn test_into_string_where_32_bit_capacity_is_on_heap() {
    let buf = vec![b'a'; SIXTEEN_MB - 1];
    // SAFETY: `buf` is filled with ASCII `a`s.
    // This primarily speeds up miri, as we don't need to check every byte
    // in the input buffer
    let string = unsafe { String::from_utf8_unchecked(buf) };

    let str_addr = string.as_ptr();
    let str_len = string.len();
    let str_cap = string.capacity();

    let compact = CompactString::from(string);
    let new_string = String::from(compact);
    let new_str_addr = new_string.as_ptr();
    let new_str_len = new_string.len();
    let new_str_cap = new_string.capacity();

    assert_eq!(str_len, new_str_len);

    if cfg!(target_pointer_width = "64") {
        assert_eq!(str_addr, new_str_addr);
        assert_eq!(str_cap, new_str_cap);
    } else {
        assert_eq!(&new_string.as_bytes()[0..10], b"aaaaaaaaaa");
        assert_eq!(str_len, new_str_cap);
    }
}

#[test]
fn test_into_string_small_string_with_excess_capacity() {
    let mut string = String::with_capacity(128);
    string.push_str("abcdef");
    let str_len = string.len();

    let compact = CompactString::from(string);
    // we should inline this string, which would truncate capacity
    //
    // note: String truncates capacity on Clone, so truncating here seems reasonable
    assert!(!compact.is_heap_allocated());
    assert_eq!(compact.len(), str_len);
    assert_eq!(compact.capacity(), MAX_SIZE);
}

#[test]
fn test_from_string_buffer_small_string_with_excess_capacity() {
    let mut string = String::with_capacity(128);
    string.push_str("abcedfg");

    let str_ptr = string.as_ptr();
    let str_len = string.len();
    let str_cap = string.capacity();

    // using from_string_buffer should always re-use the underlying buffer
    let compact = CompactString::from_string_buffer(string);
    assert!(compact.is_heap_allocated());

    let cpt_ptr = compact.as_ptr();
    let cpt_len = compact.len();
    let cpt_cap = compact.capacity();

    assert_eq!(str_ptr, cpt_ptr);
    assert_eq!(str_len, cpt_len);
    assert_eq!(str_cap, cpt_cap);
}

#[test]
fn test_into_string_small_string_with_no_excess_capacity() {
    let string = String::from("abcdef");
    let str_len = string.len();

    let compact = CompactString::from(string);

    // we should eagerly inline the string
    assert!(!compact.is_heap_allocated());
    assert_eq!(compact.len(), str_len);
    assert_eq!(compact.capacity(), MAX_SIZE);
}

#[test]
fn test_from_string_buffer_small_string_with_no_excess_capacity() {
    let string = String::from("abcdefg");

    let str_ptr = string.as_ptr();
    let str_len = string.len();
    let str_cap = string.capacity();

    // using from_string_buffer should always re-use the underlying buffer
    let compact = CompactString::from_string_buffer(string);
    assert!(compact.is_heap_allocated());

    let cpt_ptr = compact.as_ptr();
    let cpt_len = compact.len();
    let cpt_cap = compact.capacity();

    assert_eq!(str_ptr, cpt_ptr);
    assert_eq!(str_len, cpt_len);
    assert_eq!(str_cap, cpt_cap);
}

#[test]
fn test_roundtrip_from_string_empty_string() {
    let string = String::new();

    let str_ptr = string.as_ptr();
    let str_len = string.len();
    let str_cap = string.capacity();

    let compact = CompactString::from(string);
    // we should always inline empty strings
    assert!(!compact.is_heap_allocated());

    let new_string = String::from(compact);

    let new_str_ptr = new_string.as_ptr();
    let new_str_len = new_string.len();
    let new_str_cap = new_string.capacity();

    assert_eq!(str_ptr, new_str_ptr);
    assert_eq!(str_len, new_str_len);
    assert_eq!(str_cap, new_str_cap);
}

#[test]
fn test_roundtrip_from_string_buffer_empty_string() {
    let string = String::new();

    let str_ptr = string.as_ptr();
    let str_len = string.len();
    let str_cap = string.capacity();

    let compact = CompactString::from_string_buffer(string);
    // we should always inline empty strings
    assert!(!compact.is_heap_allocated());

    let new_string = String::from(compact);

    let new_str_ptr = new_string.as_ptr();
    let new_str_len = new_string.len();
    let new_str_cap = new_string.capacity();

    assert_eq!(str_ptr, new_str_ptr);
    assert_eq!(str_len, new_str_len);
    assert_eq!(str_cap, new_str_cap);
}

#[test]
fn test_into_string_small_str() {
    let data = "abcdef";
    let str_addr = data.as_ptr();
    let str_len = data.len();

    let compact = CompactString::from(data);
    let new_string = String::from(compact);
    let new_str_addr = new_string.as_ptr();
    let new_str_len = new_string.len();
    let new_str_cap = new_string.capacity();

    assert_ne!(str_addr, new_str_addr);
    assert_eq!(str_len, new_str_len);
    assert_eq!(str_len, new_str_cap);
}

#[test]
fn test_into_string_small_static_str() {
    let data = "abcdef";
    let str_addr = data.as_ptr();
    let str_len = data.len();

    let compact = CompactString::const_new(data);
    let new_string = String::from(compact);
    let new_str_addr = new_string.as_ptr();
    let new_str_len = new_string.len();
    let new_str_cap = new_string.capacity();

    assert_ne!(str_addr, new_str_addr);
    assert_eq!(str_len, new_str_len);
    assert_eq!(str_len, new_str_cap);
}

#[test]
fn test_into_string_long_str() {
    let data = "this is a long string that will be on the heap";
    let str_addr = data.as_ptr();
    let str_len = data.len();

    let compact = CompactString::from(data);
    let new_string = String::from(compact);
    let new_str_addr = new_string.as_ptr();
    let new_str_len = new_string.len();
    let new_str_cap = new_string.capacity();

    assert_ne!(str_addr, new_str_addr);
    assert_eq!(str_len, new_str_len);
    assert_eq!(str_len, new_str_cap);
}

#[test]
fn test_into_string_long_static_str() {
    let data = "this is a long string that will be on the heap";
    let str_addr = data.as_ptr();
    let str_len = data.len();

    let compact = CompactString::const_new(data);
    let new_string = String::from(compact);
    let new_str_addr = new_string.as_ptr();
    let new_str_len = new_string.len();
    let new_str_cap = new_string.capacity();

    assert_ne!(str_addr, new_str_addr);
    assert_eq!(str_len, new_str_len);
    assert_eq!(str_len, new_str_cap);
}

#[test]
fn test_into_string_empty_str() {
    let data = "";
    let str_len = data.len();

    let compact = CompactString::from(data);
    let new_string = String::from(compact);
    let new_str_addr = new_string.as_ptr();
    let new_str_len = new_string.len();
    let new_str_cap = new_string.capacity();

    assert_eq!(String::new().as_ptr(), new_str_addr);
    assert_eq!(str_len, new_str_len);
    assert_eq!(str_len, new_str_cap);
}

#[test]
fn test_into_string_empty_static_str() {
    let data = "";
    let str_len = data.len();

    let compact = CompactString::const_new(data);
    let new_string = String::from(compact);
    let new_str_addr = new_string.as_ptr();
    let new_str_len = new_string.len();
    let new_str_cap = new_string.capacity();

    assert_eq!(String::new().as_ptr(), new_str_addr);
    assert_eq!(str_len, new_str_len);
    assert_eq!(str_len, new_str_cap);
}

#[test]
fn test_truncate_noops_if_new_len_greater_than_current() {
    let mut short = CompactString::from("short");
    let short_cap = short.capacity();
    short.truncate(100);

    assert_eq!(short.len(), 5);
    assert_eq!(short.capacity(), short_cap);

    let mut long = CompactString::from("i am a long string that will be allocated on the heap");
    let long_cap = long.capacity();
    long.truncate(500);

    assert_eq!(long.len(), 53);
    assert_eq!(long.capacity(), long_cap);
}

#[test]
fn test_truncate_noops_if_new_len_greater_than_current_static_str() {
    let mut short = CompactString::const_new("short");
    short.truncate(100);

    assert_eq!(short.len(), 5);
    assert_eq!(short.capacity(), MAX_SIZE);

    let mut long =
        CompactString::const_new("i am a long string that will be allocated on the heap");
    long.truncate(500);

    assert_eq!(long.len(), 53);
    assert_eq!(long.capacity(), 53);
}

#[test]
#[should_panic(expected = "new_len must lie on char boundary")]
fn test_truncate_panics_on_non_char_boundary() {
    let mut emojis = CompactString::from("😀😀😀😀");
    assert!('😀'.len_utf8() > 1);
    emojis.truncate(1);
}

#[test_case(CompactString::from; "inline")]
#[test_case(CompactString::const_new; "static_str")]
fn test_insert(to_compact: fn(&'static str) -> CompactString) {
    // insert into empty string
    let mut one_byte = to_compact("");
    one_byte.insert(0, '.');
    assert_eq!(one_byte, ".");

    let mut two_bytes = to_compact("");
    two_bytes.insert(0, 'Ü');
    assert_eq!(two_bytes, "Ü");

    let mut three_bytes = to_compact("");
    three_bytes.insert(0, '€');
    assert_eq!(three_bytes, "€");

    let mut four_bytes = to_compact("");
    four_bytes.insert(0, '😀');
    assert_eq!(four_bytes, "😀");

    // insert at the front of string
    let mut one_byte = to_compact("😀");
    one_byte.insert(0, '.');
    assert_eq!(one_byte, ".😀");

    let mut two_bytes = to_compact("😀");
    two_bytes.insert(0, 'Ü');
    assert_eq!(two_bytes, "Ü😀");

    let mut three_bytes = to_compact("😀");
    three_bytes.insert(0, '€');
    assert_eq!(three_bytes, "€😀");

    let mut four_bytes = to_compact("😀");
    four_bytes.insert(0, '😀');
    assert_eq!(four_bytes, "😀😀");

    // insert at the end of string
    let mut one_byte = to_compact("😀");
    one_byte.insert(4, '.');
    assert_eq!(one_byte, "😀.");

    let mut two_bytes = to_compact("😀");
    two_bytes.insert(4, 'Ü');
    assert_eq!(two_bytes, "😀Ü");

    let mut three_bytes = to_compact("😀");
    three_bytes.insert(4, '€');
    assert_eq!(three_bytes, "😀€");

    let mut four_bytes = to_compact("😀");
    four_bytes.insert(4, '😀');
    assert_eq!(four_bytes, "😀😀");

    // insert in the middle of string
    let mut one_byte = to_compact("😀😀");
    one_byte.insert(4, '.');
    assert_eq!(one_byte, "😀.😀");

    let mut two_bytes = to_compact("😀😀");
    two_bytes.insert(4, 'Ü');
    assert_eq!(two_bytes, "😀Ü😀");

    let mut three_bytes = to_compact("😀😀");
    three_bytes.insert(4, '€');
    assert_eq!(three_bytes, "😀€😀");

    let mut four_bytes = to_compact("😀😀");
    four_bytes.insert(4, '😀');
    assert_eq!(four_bytes, "😀😀😀");

    // edge case: new length is 24 bytes
    let mut s = to_compact("\u{ffff}\u{ffff}\u{ffff}\u{ffff}\u{ffff}\u{ffff}\u{ffff}");
    s.insert(21, '\u{ffff}');
    assert_eq!(
        s,
        "\u{ffff}\u{ffff}\u{ffff}\u{ffff}\u{ffff}\u{ffff}\u{ffff}\u{ffff}",
    );
}

#[test]
#[cfg_attr(not(panic = "unwind"), ignore = "test requires unwinding support")]
fn test_retain() {
    let mut s = CompactString::from("α_β_γ");

    s.retain(|_| true);
    assert_eq!(s, "α_β_γ");

    s.retain(|c| c != '_');
    assert_eq!(s, "αβγ");

    s.retain(|c| c != 'β');
    assert_eq!(s, "αγ");

    s.retain(|c| c == 'α');
    assert_eq!(s, "α");

    s.retain(|_| false);
    assert_eq!(s, "");

    let mut s = CompactString::from("0è0");
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut count = 0;
        s.retain(|_| {
            count += 1;
            match count {
                1 => false,
                2 => true,
                _ => panic!(),
            }
        });
    }));
    assert!(std::str::from_utf8(s.as_bytes()).is_ok());
}

#[test]
fn test_remove() {
    let mut control = String::from("🦄🦀hello🎶world🇺🇸");
    let mut compact = CompactString::from(&control);

    assert_eq!(control.remove(0), compact.remove(0));
    assert_eq!(control, compact);
    assert_eq!(compact, "🦀hello🎶world🇺🇸");

    let music_idx = control
        .char_indices()
        .find(|(_idx, c)| *c == '🎶')
        .map(|(idx, _c)| idx)
        .unwrap();
    assert_eq!(control.remove(music_idx), compact.remove(music_idx));
    assert_eq!(control, compact);
    assert_eq!(compact, "🦀helloworld🇺🇸");
}

#[test]
#[should_panic(expected = "cannot remove a char from the end of a string")]
fn test_remove_empty_string() {
    let mut compact = CompactString::new("");
    compact.remove(0);
}

#[test]
#[should_panic(expected = "cannot remove a char from the end of a string")]
fn test_remove_empty_string_static() {
    let mut compact = CompactString::const_new("");
    compact.remove(0);
}

#[test]
#[should_panic(expected = "cannot remove a char from the end of a string")]
fn test_remove_str_len() {
    let mut compact = CompactString::new("hello world");
    compact.remove(compact.len());
}

#[test]
fn test_with_capacity_16711422() {
    // Fuzzing with AFL on a 32-bit ARM arch found this bug!
    //
    // We have our own heap implementation called BoxString, which optionally stores the capacity
    // on the heap, which is really only relevant for 32-bit architectures. The discriminant it used
    // to determine if capacity was on the heap, was when the last `usize` number of bytes were all
    // equal to our internal HEAP_MASK, which at the time was `255`. At the time this worked and was
    // correct.
    //
    // When we released support to make the size of CompactString == Option<CompactString>, we
    // changed the HEAP_MASK to `254`, which unintentionally made our discriminant for determining
    // if our capacity was on the heap, all `254`s, yet our "max inline capacity value" was still
    // based on the discriminant being all `255`s.
    //
    // When creating a BoxString with capacity 16711422, we'd correctly decide we could store the
    // capacity inline, but this would create a capacity with an underlying value of
    // [254, 254, 254, HEAP_MASK]. Once the HEAP_MASK changed to 254, this capacity was now the same
    // as the discriminant to determine if the capacity was on the heap, so we'd incorrectly
    // identify the capacity as being on the heap, when it was really inline.

    assert_eq!(16711422_u32.to_le_bytes(), [254, 254, 254, 0]);

    let compact = CompactString::with_capacity(16711422);
    let std_str = String::with_capacity(16711422);

    assert!(compact.is_heap_allocated());
    assert_eq!(compact.capacity(), std_str.capacity());
    assert_eq!(compact, "");
    assert_eq!(compact, std_str);
}

#[test]
fn test_from_utf16() {
    let control = String::from("🦄 hello world! 🎮 ");
    let utf16_buf: Vec<u16> = control.encode_utf16().collect();
    let compact = CompactString::from_utf16(utf16_buf).unwrap();

    assert_eq!(compact, control);

    cfg_if::cfg_if! {
        if #[cfg(target_pointer_width = "64")] {
            assert!(!compact.is_heap_allocated());
        } else if #[cfg(target_pointer_width = "32")] {
            assert!(compact.is_heap_allocated());
        } else {
            compile_error!("unsupported pointer width!");
        }
    }
}

#[test]
fn test_reserve_shrink_roundtrip() {
    const TEXT: &str = "Hello.";

    let mut s = CompactString::new(TEXT);
    assert!(!s.is_heap_allocated());
    assert_eq!(s.capacity(), MAX_SIZE);
    assert_eq!(s, TEXT);

    s.reserve(128);
    assert!(s.is_heap_allocated());
    assert!(s.capacity() >= 128 + TEXT.len());
    assert_eq!(s, TEXT);

    s.shrink_to(64);
    assert!(s.is_heap_allocated());
    assert!(s.capacity() >= 64);
    assert_eq!(s, TEXT);

    s.shrink_to_fit();
    assert!(!s.is_heap_allocated());
    assert_eq!(s.capacity(), MAX_SIZE);
    assert_eq!(s, TEXT);

    s.reserve(SIXTEEN_MB);
    assert!(s.is_heap_allocated());
    assert!(s.capacity() >= SIXTEEN_MB + TEXT.len());
    assert_eq!(s, TEXT);

    s.shrink_to(64);
    assert!(s.is_heap_allocated());
    assert!(s.capacity() >= 64);
    assert_eq!(s, TEXT);

    s.reserve(SIXTEEN_MB);
    assert!(s.is_heap_allocated());
    assert!(s.capacity() >= SIXTEEN_MB + TEXT.len());
    assert_eq!(s, TEXT);

    s.shrink_to_fit();
    assert!(!s.is_heap_allocated());
    assert_eq!(s.capacity(), MAX_SIZE);
    assert_eq!(s, TEXT);
}

#[test]
fn test_reserve_shrink_roundtrip_static() {
    // longer than 24 bytes, so the string does not get inlined
    const TEXT: &str = "Hello, world! How are you today?";

    let mut s = CompactString::const_new(TEXT);
    assert!(!s.is_heap_allocated());
    assert_eq!(s.capacity(), TEXT.len());
    assert_eq!(s, TEXT);

    s.reserve(128);
    assert!(s.is_heap_allocated());
    assert!(s.capacity() >= 128 + TEXT.len());
    assert_eq!(s, TEXT);

    s.shrink_to(64);
    assert!(s.is_heap_allocated());
    assert!(s.capacity() >= 64);
    assert_eq!(s, TEXT);

    s.shrink_to_fit();
    assert!(s.is_heap_allocated());
    assert_eq!(s.capacity(), s.len());
    assert_eq!(s, TEXT);

    s.reserve(SIXTEEN_MB);
    assert!(s.is_heap_allocated());
    assert!(s.capacity() >= SIXTEEN_MB + TEXT.len());
    assert_eq!(s, TEXT);

    s.shrink_to(64);
    assert!(s.is_heap_allocated());
    assert!(s.capacity() >= 64);
    assert_eq!(s, TEXT);

    s.reserve(SIXTEEN_MB);
    assert!(s.is_heap_allocated());
    assert!(s.capacity() >= SIXTEEN_MB + TEXT.len());
    assert_eq!(s, TEXT);

    s.shrink_to_fit();
    assert!(s.is_heap_allocated());
    assert_eq!(s.capacity(), s.len());
    assert_eq!(s, TEXT);
}

#[test]
fn test_reserve_shrink_roundtrip_static_inline() {
    // shorter than 12 bytes, so the string gets inlined
    const TEXT: &str = "Hello.";

    let mut s = CompactString::const_new(TEXT);
    assert!(!s.is_heap_allocated());
    assert_eq!(s.capacity(), MAX_SIZE);
    assert_eq!(s, TEXT);

    s.reserve(128);
    assert!(s.is_heap_allocated());
    assert!(s.capacity() >= 128 + TEXT.len());
    assert_eq!(s, TEXT);

    s.shrink_to(64);
    assert!(s.is_heap_allocated());
    assert!(s.capacity() >= 64);
    assert_eq!(s, TEXT);

    s.shrink_to_fit();
    assert!(!s.is_heap_allocated());
    assert_eq!(s.capacity(), MAX_SIZE);
    assert_eq!(s, TEXT);

    s.reserve(SIXTEEN_MB);
    assert!(s.is_heap_allocated());
    assert!(s.capacity() >= SIXTEEN_MB + TEXT.len());
    assert_eq!(s, TEXT);

    s.shrink_to(64);
    assert!(s.is_heap_allocated());
    assert!(s.capacity() >= 64);
    assert_eq!(s, TEXT);

    s.reserve(SIXTEEN_MB);
    assert!(s.is_heap_allocated());
    assert!(s.capacity() >= SIXTEEN_MB + TEXT.len());
    assert_eq!(s, TEXT);

    s.shrink_to_fit();
    assert!(!s.is_heap_allocated());
    assert_eq!(s.capacity(), MAX_SIZE);
    assert_eq!(s, TEXT);
}

#[test]
fn test_from_utf8_unchecked_sanity() {
    let text = "hello 🌎, you are nice";
    let compact = unsafe { CompactString::from_utf8_unchecked(text) };

    assert_eq!(compact, text);
}

#[test]
fn test_from_utf8_unchecked_long() {
    let bytes = [255; 2048];
    let compact = unsafe { CompactString::from_utf8_unchecked(bytes) };

    assert_eq!(compact.len(), 2048);
    assert_eq!(compact.as_bytes(), bytes);
}

#[test]
fn test_from_utf8_unchecked_short() {
    let bytes = [255; 10];
    let compact = unsafe { CompactString::from_utf8_unchecked(bytes) };

    assert_eq!(compact.len(), 10);
    assert_eq!(compact.as_bytes(), bytes);
}

#[test]
fn test_from_utf8_unchecked_empty() {
    let bytes = [255; 0];
    let compact = unsafe { CompactString::from_utf8_unchecked(bytes) };

    assert_eq!(compact.len(), 0);
    assert_eq!(compact.as_bytes(), bytes);
}

#[proptest]
#[cfg_attr(miri, ignore)]
fn proptest_from_utf8_lossy(#[strategy(rand_bytes())] bytes: Vec<u8>) {
    let compact = CompactString::from_utf8_lossy(&bytes);
    let control = String::from_utf8_lossy(&bytes);

    assert_eq!(compact, control);
    assert_eq!(compact.len(), control.len());
}

#[proptest]
#[cfg_attr(miri, ignore)]
fn proptest_from_utf16(#[strategy(rand_u16s())] buf: Vec<u16>) {
    type FromUtf16Func = fn(&[u8]) -> Result<CompactString, crate::Utf16Error>;
    type FromU16Endian = fn(u16) -> u16;
    type FromU16EndianBytes = fn([u8; 2]) -> u16;

    const FUNCS: &[(FromUtf16Func, FromU16Endian, FromU16EndianBytes)] = &[
        (
            |v| CompactString::from_utf16le(v),
            u16::from_le,
            u16::from_le_bytes,
        ),
        (
            |v| CompactString::from_utf16be(v),
            u16::from_be,
            u16::from_be_bytes,
        ),
    ];

    for (new_compact_string, from_int, from_bytes) in FUNCS {
        let buf = &*buf;
        let bytes: &[u8] = unsafe { slice::from_raw_parts(buf.as_ptr().cast(), buf.len() * 2) };

        let compact = new_compact_string(bytes);
        let control = String::from_utf16(&buf.iter().copied().map(from_int).collect::<Vec<u16>>());
        assert_eq!(compact.is_ok(), control.is_ok());

        if let (Ok(compact), Ok(control)) = (compact, control) {
            assert_eq!(compact.len(), control.len());
            assert_eq!(compact, control);
        }

        if bytes.len() >= 2 {
            // Test if `CompactString::from_utf16x()` works with misaligned slices.

            let bytes: &[u8] = &bytes[1..bytes.len() - 1];
            let buf: Vec<u16> = bytes
                .chunks_exact(2)
                .map(|v| from_bytes([v[0], v[1]]))
                .collect();

            let compact = new_compact_string(bytes);
            let control = String::from_utf16(&buf);
            assert_eq!(compact.is_ok(), control.is_ok());

            if let (Ok(compact), Ok(control)) = (compact, control) {
                assert_eq!(compact.len(), control.len());
                assert_eq!(compact, control);
            }
        }
    }
}

#[proptest]
#[cfg_attr(miri, ignore)]
fn proptest_repeat(n: u16, s: String) {
    let compact = CompactString::new(&s).repeat(n as usize);
    let control = s.repeat(n as usize);

    assert_eq!(compact, control);
}

#[test]
fn test_from_utf16x() {
    let dancing_men = b"\x3d\xd8\x6f\xdc\x0d\x20\x42\x26\x0f\xfe";
    assert_eq!(CompactString::from_utf16le(dancing_men).unwrap(), "👯‍♂️");

    let dancing_men = b"0\x3d\xd8\x6f\xdc\x0d\x20\x42\x26\x0f\xfe";
    assert!(CompactString::from_utf16le(dancing_men).is_err());
    assert_eq!(
        CompactString::from_utf16le(&dancing_men[1..]).unwrap(),
        "👯‍♂️",
    );

    let dancing_women = b"\xd8\x3d\xdc\x6f\x20\x0d\x26\x40\xfe\x0f";
    assert_eq!(CompactString::from_utf16be(dancing_women).unwrap(), "👯‍♀️");

    let dancing_women = b"0\xd8\x3d\xdc\x6f\x20\x0d\x26\x40\xfe\x0f";
    assert!(CompactString::from_utf16be(dancing_women).is_err());
    assert_eq!(
        CompactString::from_utf16be(&dancing_women[1..]).unwrap(),
        "👯‍♀️",
    );
}

#[test]
fn test_from_utf16x_lossy() {
    let dancing_men = b"\x3d\xd8\x6f\xfc\x0d\x20\x42\x26\x0f\xfe";
    assert_eq!(
        CompactString::from_utf16le_lossy(dancing_men),
        "�\u{fc6f}\u{200d}♂️",
    );

    let dancing_men = b"0\x3d\xd8\x6f\xfc\x0d\x20\x42\x26\x0f\xfe";
    assert_eq!(
        CompactString::from_utf16le_lossy(&dancing_men[1..]),
        "�\u{fc6f}\u{200d}♂️",
    );

    let dancing_women = b"\xd8\x3d\xdc\x6f\x20\x0d\x26\x40\xde\x0f";
    assert_eq!(
        CompactString::from_utf16be_lossy(dancing_women),
        "👯\u{200d}♀�",
    );

    let dancing_women = b"0\xd8\x3d\xdc\x6f\x20\x0d\x26\x40\xde\x0f";
    assert_eq!(
        CompactString::from_utf16be_lossy(&dancing_women[1..]),
        "👯\u{200d}♀�",
    );
}

#[test]
fn test_collect() {
    const VALUES: &[&str] = &["foo", "bar", "baz"];

    assert_eq!(
        VALUES
            .iter()
            .copied()
            .map(Cow::Borrowed)
            .collect::<CompactString>(),
        "foobarbaz",
    );
    assert_eq!(
        VALUES
            .iter()
            .copied()
            .map(|s| Cow::Owned(s.into()))
            .collect::<CompactString>(),
        "foobarbaz",
    );
    assert_eq!(
        VALUES
            .iter()
            .copied()
            .map(Box::<str>::from)
            .collect::<CompactString>(),
        "foobarbaz",
    );
    assert_eq!(
        VALUES
            .iter()
            .copied()
            .map(CompactString::from)
            .collect::<String>(),
        "foobarbaz",
    );
    assert_eq!(
        VALUES
            .iter()
            .copied()
            .map(CompactString::from)
            .collect::<Cow<'_, str>>(),
        "foobarbaz",
    );
    assert_eq!(
        VALUES
            .iter()
            .copied()
            .flat_map(|s| s.chars())
            .collect::<Cow<'_, str>>(),
        "foobarbaz",
    );
}

#[test]
fn test_into_cow() {
    let og = "aaa";
    let compact = CompactString::new(og);
    let cow: alloc::borrow::Cow<'_, str> = compact.into();

    assert_eq!(og, cow);
}

#[test]
fn test_into_arc() {
    let short = "short";
    let long = "i am a long string that will be allocated on the heap";
    let arc = alloc::sync::Arc::<str>::from(CompactString::new(short));
    assert_eq!(short, &*arc);
    let arc = alloc::sync::Arc::<str>::from(CompactString::new(long));
    assert_eq!(long, &*arc);
}

#[test]
fn test_into_rc() {
    let short = "short";
    let long = "i am a long string that will be allocated on the heap";
    let rc = alloc::rc::Rc::<str>::from(CompactString::new(short));
    assert_eq!(short, &*rc);
    let rc = alloc::rc::Rc::<str>::from(CompactString::new(long));
    assert_eq!(long, &*rc);
}

#[test]
fn test_into_error() {
    let short = "short";
    let long = "i am a long string that will be allocated on the heap";
    let short_error_ss =
        Box::<dyn std::error::Error + Send + Sync>::from(CompactString::new(short));
    assert_eq!(short, format!("{short_error_ss}"));
    assert_eq!(format!("{short:?}"), format!("{short_error_ss:?}"));
    let long_error_ss = Box::<dyn std::error::Error + Send + Sync>::from(CompactString::new(long));
    assert_eq!(long, format!("{long_error_ss}"));
    assert_eq!(format!("{long:?}"), format!("{long_error_ss:?}"));

    let short_error = Box::<dyn std::error::Error>::from(CompactString::new(short));
    assert_eq!(short, format!("{short_error}"));
    assert_eq!(format!("{short:?}"), format!("{short_error:?}"));
    let long_error = Box::<dyn std::error::Error>::from(CompactString::new(long));
    assert_eq!(long, format!("{long_error}"));
    assert_eq!(format!("{long:?}"), format!("{long_error:?}"));
}

#[test]
fn test_into_box_str() {
    let short = "short";
    let long = "i am a long string that will be allocated on the heap";
    let s = Box::<str>::from(CompactString::new(short));
    assert_eq!(short, &*s);
    let l = Box::<str>::from(CompactString::new(long));
    assert_eq!(long, &*l);
}

#[test]
fn test_into_os_string() {
    let short = "short";
    let long = "i am a long string that will be allocated on the heap";
    let s = std::ffi::OsString::from(CompactString::new(short));
    assert_eq!(s.as_os_str().to_str().unwrap(), short);
    let l = std::ffi::OsString::from(CompactString::new(long));
    assert_eq!(l.as_os_str().to_str().unwrap(), long);
}

#[test]
fn test_into_path_buf() {
    let short = "short";
    let long = "i am a long string that will be allocated on the heap";
    let s = std::path::PathBuf::from(CompactString::new(short));
    assert_eq!(s.as_os_str().to_str().unwrap(), short);
    let l = std::path::PathBuf::from(CompactString::new(long));
    assert_eq!(l.as_os_str().to_str().unwrap(), long);
}

#[test]
fn test_as_ref_path() {
    let short = "short";
    let long = "i am a long string that will be allocated on the heap";
    let s = CompactString::new(short);
    assert_eq!(
        AsRef::<std::path::Path>::as_ref(&s).to_str().unwrap(),
        short
    );
    let l = CompactString::new(long);
    assert_eq!(AsRef::<std::path::Path>::as_ref(&l).to_str().unwrap(), long);
}

#[test]
fn test_into_vec_u8() {
    let short = "short";
    let long = "i am a long string that will be allocated on the heap";
    let s = Vec::<u8>::from(CompactString::new(short));
    assert_eq!(&s, short.as_bytes());
    let l = Vec::<u8>::from(CompactString::new(long));
    assert_eq!(&l, long.as_bytes());
}

#[test]
fn test_from_string_buffer_inlines_on_push() {
    let mut compact = CompactString::from_string_buffer("hello".to_string());
    assert!(compact.is_heap_allocated());

    compact.push_str(" world");
    // when growing the CompactString we should inline it
    assert!(!compact.is_heap_allocated());
}

#[test]
fn test_from_string_buffer_inlines_on_clone() {
    let a = CompactString::from_string_buffer("hello".to_string());
    assert!(a.is_heap_allocated());

    let b = a.clone();
    // when cloning the CompactString we should inline it
    assert!(!b.is_heap_allocated());
}

// With debug assertions enabled the invocation will panic if you try to allocate more memory than
// the system even has.
#[cfg(not(debug_assertions))]
#[cfg(target_pointer_width = "64")]
#[test]
#[should_panic = "Cannot allocate memory to hold CompactString"]
fn test_alloc_excessively_long_string() {
    // 2**56 - 2 bytes, the maximum number `Capacity` can hold
    CompactString::with_capacity((1 << 56) - 2);
}

// This feature was enabled by <https://github.com/rust-lang/rust/pull/94075> which was first
// released in Rust 1.65.
#[rustversion::since(1.65)]
#[test]
fn multiple_niches_test() {
    #[allow(unused)]
    enum Value {
        String(CompactString),
        Bool(bool),
        Signed(isize),
        Unsigned(usize),
        Null,
    }
    assert_eq!(
        core::mem::size_of::<Value>(),
        core::mem::size_of::<String>()
    );
}

#[test]
fn test_is_empty() {
    const ZEROS: &[&str] = &[
        "\0",                                                 // 1
        "\0\0\0\0\0\0\0\0\0\0\0\0",                           // 12
        "\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0",   // 24
        "\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0", // 25
    ];

    assert!(CompactString::new("").is_empty());
    assert!(CompactString::const_new("").is_empty());

    for (len, s) in ZEROS.iter().copied().enumerate() {
        let mut a = CompactString::new(s);
        let mut b = CompactString::new(s);

        for _ in (1..=len).rev() {
            a.truncate(len);
            b.truncate(len);
            assert!(!a.is_empty());
            assert!(!b.is_empty());
        }
    }
}

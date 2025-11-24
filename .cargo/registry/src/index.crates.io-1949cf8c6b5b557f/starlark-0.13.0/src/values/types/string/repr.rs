/*
 * Copyright 2018 The Starlark in Rust Authors.
 * Copyright (c) Facebook, Inc. and its affiliates.
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     https://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

//! Implementation of `repr()`.

use std::mem;

use crate::hint::unlikely;
use crate::values::types::string::simd::SwitchHaveSimd;
use crate::values::types::string::simd::Vector;

/// Check if any byte in the buffer is non-ASCII or need escape.
#[inline(always)]
#[allow(dead_code)]
unsafe fn chunk_non_ascii_or_need_escape<V: Vector>(chunk: V) -> bool {
    #[allow(clippy::many_single_char_names)]
    unsafe fn or4<V: Vector>(a: V, b: V, c: V, d: V) -> V {
        let ab = V::or(a, b);
        let cd = V::or(c, d);
        V::or(ab, cd)
    }

    // Note `cmplt` is signed comparison.
    let any_control_or_non_ascii = chunk.cmplt(V::splat(32));
    let any_7f = chunk.cmpeq(V::splat(0x7f));
    let any_double_quote = chunk.cmpeq(V::splat(b'"'));
    let any_backslash = chunk.cmpeq(V::splat(b'\\'));

    let need_escape = or4(
        any_control_or_non_ascii,
        any_7f,
        any_double_quote,
        any_backslash,
    );
    need_escape.movemask() != 0
}

#[inline(always)]
fn push_escape(to_escape: char, buffer: &mut String) {
    // Starlark behavior of `repr` is underspecified,
    // so use mix of Starlark spec and PEP-3138.

    use std::fmt::Write;

    match to_escape {
        '\n' => buffer.push_str("\\n"),
        '\r' => buffer.push_str("\\r"),
        '\t' => buffer.push_str("\\t"),
        '\\' => buffer.push_str("\\\\"),
        '"' => buffer.push_str("\\\""),
        // `write!` is slow, but these branches are rare.
        c if (c as u32) < 0x100 => write!(buffer, "\\x{:02x}", c as u32).unwrap(),
        c if (c as u32) < 0x10000 => write!(buffer, "\\u{:04x}", c as u32).unwrap(),
        c => write!(buffer, "\\U{:08x}", c as u32).unwrap(),
    }
}

#[inline(always)]
fn need_escape(c: char) -> bool {
    match c {
        c if (c as u32) < 0x20 => true,
        '"' => true,
        '\\' => true,
        // Note 0x7f needs to be escaped.
        c if (c as u32) < 0x7f => false,
        // Now all 8bit characters are covered:
        c if (c as u32) <= 0xff => true,
        // Rust does not expose `is_printable`.
        // PEP-3138 goes long way defining precisely the Unicode groups which need escaping.
        // We could pick more character groups here,
        // but `is_alphanumeric` is practically enough for now.
        c => !c.is_alphanumeric(),
    }
}

pub(crate) fn string_repr(str: &str, buffer: &mut String) {
    // this method is surprisingly hot
    // so we first try and do a fast pass that only works for ASCII-only

    // Simple but definitely correct version
    fn loop_unicode(val: &str, buffer: &mut String) {
        for x in val.chars() {
            if need_escape(x) {
                push_escape(x, buffer);
            } else {
                buffer.push(x);
            }
        }
    }

    // Process the ASCII prefix, bailing out to loop_unicode if we fail
    fn loop_ascii(val: &str, buffer: &mut String) {
        for (done, x) in val.as_bytes().iter().enumerate() {
            let x = *x;
            // Note 0x7f is ASCII, but it is rarely used, so handle it common case
            // to do fewer branches in common case.
            if unlikely(x >= 0x7f) {
                // bail out into a unicode-aware version
                loop_unicode(&val[done..], buffer);
                return;
            }

            if unlikely(need_escape(x as char)) {
                push_escape(x as char, buffer);
            } else {
                // safe because we know the following values are all lower-ascii bytes
                let byte_buffer = unsafe { buffer.as_mut_vec() };
                byte_buffer.push(x);
            }
        }
    }

    #[inline(always)]
    #[allow(dead_code)] // FIXME: Investigate if this is really needed, fails on Mac.
    unsafe fn loop_ascii_simd<V: Vector>(val: &str, buffer: &mut String) {
        // `buffer` must have enough capacity to contain `val` if it does not need escaping
        // followed by trailing double quote.
        debug_assert!(buffer.capacity() - buffer.len() >= val.len() + 1);

        if val.len() < mem::size_of::<V>() {
            // Not enough length even for single SIMD iteration.
            return loop_ascii(val, buffer);
        }

        /// Push the tail of the vec to the buffer overwriting previously written buffer content
        /// (with identical content due to how this function is used).
        /// ```text
        /// buffer:   [       buffer.len         |  buffer rem capacity   ]
        /// vector:              [  overwriting  |  tail_len  ]
        /// ```
        #[inline(always)]
        unsafe fn push_vec_tail<V: Vector>(buffer: &mut String, vector: V, tail_len: usize) {
            debug_assert!(tail_len > 0);
            debug_assert!(tail_len <= mem::size_of::<V>());

            let new_buffer_len = buffer.len() + tail_len;

            // Write won't undershoot.
            debug_assert!(new_buffer_len >= mem::size_of::<V>());
            // Write won't overshoot.
            debug_assert!(new_buffer_len <= buffer.capacity());

            // Single store instruction.
            vector.store_unaligned(
                buffer
                    .as_bytes_mut()
                    .as_mut_ptr()
                    .add(new_buffer_len)
                    .sub(mem::size_of::<V>()),
            );
            buffer.as_mut_vec().set_len(new_buffer_len);
        }

        // Current offset in `val`.
        let mut val_offset = 0;

        // First process full chunks. Should take at least one iteration.
        debug_assert!(val.len() >= mem::size_of::<V>());
        while val_offset + mem::size_of::<V>() <= val.len() {
            let chunk = V::load_unaligned(val.as_ptr().add(val_offset) as *const _);

            if chunk_non_ascii_or_need_escape(chunk) {
                // NOTE(nga): two possible optimizations here:
                // * instead of `need_escape` flag, fetch position
                //   of the first character which need to be escaped, and write good prefix.
                // * if this chunk is ASCII, escape it and then return back to this loop.
                return loop_ascii(&val[val_offset..], buffer);
            }

            push_vec_tail(buffer, chunk, mem::size_of::<V>());

            val_offset += mem::size_of::<V>();
        }

        debug_assert!(val_offset >= mem::size_of::<V>());
        debug_assert!(val_offset + mem::size_of::<V>() > val.len());
        debug_assert!(val_offset % mem::size_of::<V>() == 0);

        // The remaining chunk is shorter than vector (or empty).
        if val_offset < val.len() {
            let chunk_len = val.len() - val_offset;
            debug_assert!(chunk_len > 0);
            debug_assert!(chunk_len < mem::size_of::<V>());

            debug_assert!(val.len() >= mem::size_of::<V>());
            // Last `chunk_len` bytes of `chunk` is new data,
            // and before `0..=chunk_len` of vec is data we have already written as is to `buffer`.
            let chunk = V::load_unaligned(val.as_ptr().add(val.len()).sub(mem::size_of::<V>()));

            if chunk_non_ascii_or_need_escape(chunk) {
                return loop_ascii(&val[val_offset..], buffer);
            }

            push_vec_tail(buffer, chunk, chunk_len);
        }
    }

    struct Switch<'a> {
        s: &'a str,
        buffer: &'a mut String,
    }

    impl<'a> SwitchHaveSimd<()> for Switch<'a> {
        fn no_simd(self) {
            loop_ascii(self.s, self.buffer)
        }

        fn simd<V: Vector>(self) {
            unsafe { loop_ascii_simd::<V>(self.s, self.buffer) }
        }
    }

    buffer.reserve(2 + str.len());
    buffer.push('"');
    Switch { s: str, buffer }.switch();
    buffer.push('"');
}

#[cfg(test)]
mod tests {

    use crate::assert;
    use crate::values::types::string::repr::string_repr;

    #[test]
    fn test_to_repr() {
        assert::all_true(
            r#"
"\"\\t\\n'\\\"\"" == repr("\t\n'\"")
"\"Hello, 世界\"" == repr("Hello, 世界")
"#,
        );
    }

    #[test]
    fn test_string_repr() {
        fn test(expected: &str, input: &str) {
            let mut repr = String::new();
            string_repr(input, &mut repr);
            assert_eq!(expected, &repr);
        }
        test(r#""\x12""#, "\x12");
        test(r#""\x7f""#, "\x7f");
        test(r#""\n""#, "\n");
        // Do not escape single quotes because repr in Starlark uses double quotes.
        test(r#""'""#, "'");
        test(r#""\"""#, "\"");
        test(r#""\\""#, "\\");
        // Non-printable whitespace.
        test(r#""\u200b""#, "\u{200b}");
        test(r#""Hello, 世界""#, "Hello, 世界");
        // Largest unicode number.
        test(r#""\U0010ffff""#, "\u{10ffff}");
    }

    #[test]
    fn test_to_repr_long_smoke() {
        assert::all_true(
            r#"
'"0123456789abcdef"' == repr("0123456789abcdef")
'"0123456789\\nbcdef"' == repr("0123456789\nbcdef")
'"Мы, оглядываясь, видим лишь руины"' == repr("Мы, оглядываясь, видим лишь руины")
"#,
        )
    }

    fn string_repr_for_test(s: &str) -> String {
        let mut r = String::new();
        string_repr(s, &mut r);
        r
    }

    #[test]
    fn to_repr_sse() {
        for i in 0..0x80 {
            let s = String::from_utf8((0..33).map(|_| i as u8).collect()).unwrap();
            // Trigger debug assertions.
            string_repr_for_test(&s);
        }
    }

    #[test]
    fn to_repr_no_escape_all_lengths() {
        for len in 0..100 {
            let s = String::from_utf8((0..len).map(|i| b'0' + (i % 10)).collect()).unwrap();
            assert_eq!(format!("\"{}\"", s), string_repr_for_test(&s));
        }
    }

    #[test]
    fn to_repr_tail_escape_all_lengths() {
        for len in 0..100 {
            let s = String::from_utf8((0..len).map(|i| b'0' + (i % 10)).collect()).unwrap();
            assert_eq!(
                format!("\"{}\\n\"", s),
                string_repr_for_test(&format!("{}\n", s))
            );
        }
    }

    #[test]
    fn to_repr_middle_escape_all_lengths() {
        for len in 0..100 {
            let s = String::from_utf8((0..len).map(|i| b'0' + (i % 10)).collect()).unwrap();
            assert_eq!(
                format!("\"{}\\n{}\"", s, s),
                string_repr_for_test(&format!("{}\n{}", s, s))
            );
        }
    }

    // Test SSE version of `need_escape` works correctly.
    #[cfg(target_feature = "sse2")]
    #[test]
    fn test_chunk_non_ascii_or_need_escape() {
        use std::arch::x86_64::*;
        use std::mem;

        use crate::values::string::repr::chunk_non_ascii_or_need_escape;
        use crate::values::types::string::simd::Vector;

        unsafe fn load(s: &str) -> __m128i {
            assert_eq!(s.len(), mem::size_of::<__m128i>());
            <__m128i as Vector>::load_unaligned(s.as_ptr())
        }

        unsafe {
            assert!(!chunk_non_ascii_or_need_escape(load("0123456789abcdef")));
            assert!(!chunk_non_ascii_or_need_escape(load("0123456789abcde ")));
            assert!(chunk_non_ascii_or_need_escape(load("0123456789ab\x19def")));
            assert!(chunk_non_ascii_or_need_escape(load("0123456789abcde\n")));
            assert!(chunk_non_ascii_or_need_escape(load("0123456789ab\x7fdef")));
            assert!(chunk_non_ascii_or_need_escape(load(
                "0123\x0456789ab\x02def"
            )));
            assert!(!chunk_non_ascii_or_need_escape(load("'123456789abcdef")));
            assert!(chunk_non_ascii_or_need_escape(load("0\"23456789abcdef")));
            assert!(chunk_non_ascii_or_need_escape(load("0123456 Я bcdef")));
        }
    }
}

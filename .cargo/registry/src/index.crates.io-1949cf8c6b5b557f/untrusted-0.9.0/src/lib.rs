// Copyright 2015-2021 Brian Smith.
//
// Permission to use, copy, modify, and/or distribute this software for any
// purpose with or without fee is hereby granted, provided that the above
// copyright notice and this permission notice appear in all copies.
//
// THE SOFTWARE IS PROVIDED "AS IS" AND THE AUTHORS DISCLAIM ALL WARRANTIES
// WITH REGARD TO THIS SOFTWARE INCLUDING ALL IMPLIED WARRANTIES OF
// MERCHANTABILITY AND FITNESS. IN NO EVENT SHALL THE AUTHORS BE LIABLE FOR
// ANY SPECIAL, DIRECT, INDIRECT, OR CONSEQUENTIAL DAMAGES OR ANY DAMAGES
// WHATSOEVER RESULTING FROM LOSS OF USE, DATA OR PROFITS, WHETHER IN AN
// ACTION OF CONTRACT, NEGLIGENCE OR OTHER TORTIOUS ACTION, ARISING OUT OF
// OR IN CONNECTION WITH THE USE OR PERFORMANCE OF THIS SOFTWARE.

//! untrusted.rs: Safe, fast, zero-panic, zero-crashing, zero-allocation
//! parsing of untrusted inputs in Rust.
//!
//! <code>git clone https://github.com/briansmith/untrusted</code>
//!
//! untrusted.rs goes beyond Rust's normal safety guarantees by  also
//! guaranteeing that parsing will be panic-free, as long as
//! `untrusted::Input::as_slice_less_safe()` is not used. It avoids copying
//! data and heap allocation and strives to prevent common pitfalls such as
//! accidentally parsing input bytes multiple times. In order to meet these
//! goals, untrusted.rs is limited in functionality such that it works best for
//! input languages with a small fixed amount of lookahead such as ASN.1, TLS,
//! TCP/IP, and many other networking, IPC, and related protocols. Languages
//! that require more lookahead and/or backtracking require some significant
//! contortions to parse using this framework. It would not be realistic to use
//! it for parsing programming language code, for example.
//!
//! The overall pattern for using untrusted.rs is:
//!
//! 1. Write a recursive-descent-style parser for the input language, where the
//!    input data is given as a `&mut untrusted::Reader` parameter to each
//!    function. Each function should have a return type of `Result<V, E>` for
//!    some value type `V` and some error type `E`, either or both of which may
//!    be `()`. Functions for parsing the lowest-level language constructs
//!    should be defined. Those lowest-level functions will parse their inputs
//!    using `::read_byte()`, `Reader::peek()`, and similar functions.
//!    Higher-level language constructs are then parsed by calling the
//!    lower-level functions in sequence.
//!
//! 2. Wrap the top-most functions of your recursive-descent parser in
//!    functions that take their input data as an `untrusted::Input`. The
//!    wrapper functions should call the `Input`'s `read_all` (or a variant
//!    thereof) method. The wrapper functions are the only ones that should be
//!    exposed outside the parser's module.
//!
//! 3. After receiving the input data to parse, wrap it in an `untrusted::Input`
//!    using `untrusted::Input::from()` as early as possible. Pass the
//!    `untrusted::Input` to the wrapper functions when they need to be parsed.
//!
//! In general parsers built using `untrusted::Reader` do not need to explicitly
//! check for end-of-input unless they are parsing optional constructs, because
//! `Reader::read_byte()` will return `Err(EndOfInput)` on end-of-input.
//! Similarly, parsers using `untrusted::Reader` generally don't need to check
//! for extra junk at the end of the input as long as the parser's API uses the
//! pattern described above, as `read_all` and its variants automatically check
//! for trailing junk. `Reader::skip_to_end()` must be used when any remaining
//! unread input should be ignored without triggering an error.
//!
//! untrusted.rs works best when all processing of the input data is done
//! through the `untrusted::Input` and `untrusted::Reader` types. In
//! particular, avoid trying to parse input data using functions that take
//! byte slices. However, when you need to access a part of the input data as
//! a slice to use a function that isn't written using untrusted.rs,
//! `Input::as_slice_less_safe()` can be used.
//!
//! It is recommend to use `use untrusted;` and then `untrusted::Input`,
//! `untrusted::Reader`, etc., instead of using `use untrusted::*`. Qualifying
//! the names with `untrusted` helps remind the reader of the code that it is
//! dealing with *untrusted* input.
//!
//! # Examples
//!
//! [*ring*](https://github.com/briansmith/ring)'s parser for the subset of
//! ASN.1 DER it needs to understand,
//! [`ring::der`](https://github.com/briansmith/ring/blob/main/src/io/der.rs),
//! is built on top of untrusted.rs. *ring* also uses untrusted.rs to parse ECC
//! public keys, RSA PKCS#1 1.5 padding, and for all other parsing it does.
//!
//! All of [webpki](https://github.com/briansmith/webpki)'s parsing of X.509
//! certificates (also ASN.1 DER) is done using untrusted.rs.

#![doc(html_root_url = "https://briansmith.org/rustdoc/")]
#![no_std]

mod input;
mod no_panic;
mod reader;

pub use {
    input::Input,
    reader::{EndOfInput, Reader},
};

/// Calls `read` with the given input as a `Reader`, ensuring that `read`
/// consumed the entire input. When `input` is `None`, `read` will be
/// called with `None`.
pub fn read_all_optional<'a, F, R, E>(
    input: Option<Input<'a>>,
    incomplete_read: E,
    read: F,
) -> Result<R, E>
where
    F: FnOnce(Option<&mut Reader<'a>>) -> Result<R, E>,
{
    match input {
        Some(input) => {
            let mut input = Reader::new(input);
            let result = read(Some(&mut input))?;
            if input.at_end() {
                Ok(result)
            } else {
                Err(incomplete_read)
            }
        }
        None => read(None),
    }
}

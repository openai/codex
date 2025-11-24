// Copyright 2014 The Servo Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//!
//! A library for interning things that are `AsRef<str>`.
//!
//! Some strings may be interned at compile time using the `string-cache-codegen` crate, or the
//! `EmptyStaticAtomSet` may be used that has no compile-time interned strings. An `Atom` is an
//! interned string for a given set (either `EmptyStaticAtomSet` or a generated `StaticAtomSet`).
//!
//! Generated `Atom`s will have assocated macros to intern static strings at compile-time.
//!
//! # Examples
//!
//! Here are two examples, one with compile-time `Atom`s, and one without.
//!
//! ## With compile-time atoms
//!
//! In `Cargo.toml`:
//! ```toml
//! [dependencies]
//! string_cache = "0.8"
//!
//! [dev-dependencies]
//! string_cache_codegen = "0.5"
//! ```
//!
//! In `build.rs`:
//!
//! ```ignore
//! extern crate string_cache_codegen;
//!
//! use std::env;
//! use std::path::Path;
//!
//! fn main() {
//!     string_cache_codegen::AtomType::new("foo::FooAtom", "foo_atom!")
//!         .atoms(&["foo", "bar"])
//!         .write_to_file(&Path::new(&env::var("OUT_DIR").unwrap()).join("foo_atom.rs"))
//!         .unwrap()
//! }
//! ```
//!
//! In `lib.rs`:
//!
//! ```ignore
//! extern crate string_cache;
//!
//! mod foo {
//!     include!(concat!(env!("OUT_DIR"), "/foo_atom.rs"));
//! }
//!
//! fn use_the_atom(t: &str) {
//!     match *t {
//!         foo_atom!("foo") => println!("Found foo!"),
//!         foo_atom!("bar") => println!("Found bar!"),
//!         // foo_atom!("baz") => println!("Found baz!"), - would be a compile time error
//!         _ => {
//!             println!("String not interned");
//!             // We can intern strings at runtime as well
//!             foo::FooAtom::from(t)
//!         }
//!     }
//! }
//! ```
//!
//! ## No compile-time atoms
//!
//! ```
//! # extern crate string_cache;
//! use string_cache::DefaultAtom;
//!
//! # fn main() {
//! let mut interned_stuff = Vec::new();
//! let text = "here is a sentence of text that will be tokenised and
//!             interned and some repeated tokens is of text and";
//! for word in text.split_whitespace() {
//!     let seen_before = interned_stuff.iter()
//!         // We can use impl PartialEq<T> where T is anything string-like
//!         // to compare to interned strings to either other interned strings,
//!         // or actual strings  Comparing two interned strings is very fast
//!         // (normally a single cpu operation).
//!         .filter(|interned_word| interned_word == &word)
//!         .count();
//!     if seen_before > 0 {
//!         println!(r#"Seen the word "{}" {} times"#, word, seen_before);
//!     } else {
//!         println!(r#"Not seen the word "{}" before"#, word);
//!     }
//!     // We use the impl From<(Cow<'a, str>, or &'a str, or String)> for
//!     // Atom<Static> to intern a new string.
//!     interned_stuff.push(DefaultAtom::from(word));
//! }
//! # }
//! ```
//!

#![cfg_attr(test, deny(warnings))]

// Types, such as Atom, that impl Hash must follow the hash invariant: if two objects match
// with PartialEq, they must also have the same Hash. Clippy warns on types that derive one while
// manually impl-ing the other, because it seems easy for the two to drift apart, causing the
// invariant to be violated.
//
// But Atom is a newtype over NonZeroU64, and probably always will be, since cheap comparisons and
// copying are this library's purpose. So we know what the PartialEq comparison is going to do.
//
// The `get_hash` function, seen in `atom.rs`, consults that number, plus the global string interner
// tables. The only way for the resulting hash for two Atoms with the same inner 64-bit number to
// differ would be if the table entry changed between invocations, and that would be really bad.
#![allow(clippy::derive_hash_xor_eq)]

mod atom;
mod dynamic_set;
mod static_sets;
mod trivial_impls;

pub use atom::Atom;
pub use static_sets::{EmptyStaticAtomSet, PhfStrSet, StaticAtomSet};

/// Use this if you don’t care about static atoms.
pub type DefaultAtom = Atom<EmptyStaticAtomSet>;

// Some minor tests of internal layout here.
// See ../integration-tests for much more.

/// Guard against accidental changes to the sizes of things.
#[test]
fn assert_sizes() {
    use std::mem::size_of;
    assert_eq!(size_of::<DefaultAtom>(), 8);
    assert_eq!(size_of::<Option<DefaultAtom>>(), size_of::<DefaultAtom>(),);
}

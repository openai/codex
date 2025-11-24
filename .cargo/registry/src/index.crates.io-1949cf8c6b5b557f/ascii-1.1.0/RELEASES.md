Version 1.1.0 (2022-09-18)
==========================
* Add alloc feature.
  This enables `AsciiString` and methods that take or return `Box<[AsciiStr]>` in `!#[no_std]`-mode.
* Add `AsciiStr::into_ascii_string()`, `AsciiString::into_boxed_ascii_str()` and `AsciiString::insert_str()`.
* Implement `From<Box<AsciiStr>>` and `From<AsciiChar>` for `AsciiString`.
* Implement `From<AsciiString>` for `Box<AsciiStr>`, `Rc<AsciiStr>`, `Arc<AsciiStr>` and `Vec<AsciiChar>`.
* Make `AsciiString::new()`, `AsciiStr::len()` and `AsciiStr::is_empty()` `const fn`. 
* Require Rust 1.44.1.

Version 1.0.0 (2019-08-26)
==========================

Breaking changes:

* Change `AsciiChar.is_whitespace()` to also return true for '\0xb' (vertical tab) and '\0xc' (form feed).
* Remove quickcheck feature.
* Remove `AsciiStr::new()`.
* Rename `AsciiChar::from()` and `AsciiChar::from_unchecked()` to `from_ascii()` and `from_ascii_unchecked()`.
* Rename several `AsciiChar.is_xxx()` methods to `is_ascii_xxx()` (for comsistency with std).
* Rename `AsciiChar::Null` to `Nul` (for consistency with eg. `CStr::from_bytes_with_nul()`).
* Rename `AsciiStr.trim_left()` and `AsciiStr.trim_right()` to `trim_start()` and `trim_end()`.
* Remove impls of the deprecated `std::ascii::AsciiExt` trait.
* Change iterators `Chars`, `CharsMut` and `CharsRef` from type aliases to newtypes.
* Return `impl Trait` from `AsciiStr.lines()` and `AsciiStr.split()`, and remove iterator types `Lines` and `Split`.
* Add `slice_ascii_str()`, `get_ascii()` and `unwrap_ascii()` to the `AsAsciiStr` trait.
* Add `slice_mut_ascii_str()` and `unwrap_ascii_mut()` to the `AsMutAsciiStr` trait.
* Require Rust 1.33.0 for 1.0.\*, and allow later semver-compatible 1.y.0 releases to increase it.

Additions:

* Add `const fn` `AsciiChar::new()` which panicks on invalid values.
* Make most `AsciiChar` methods `const fn`.
* Add multiple `AsciiChar::is_[ascii_]xxx()` methods.
* Implement `AsRef<AsciiStr>` for `AsciiChar`.
* Make `AsciiString`'s `Extend` and `FromIterator` impl generic over all `AsRef<AsciiStr>`.
* Implement inclusive range indexing for `AsciiStr` (and thereby `AsciiString`).
* Mark `AsciiStr` and `AsciiString` `#[repr(transparent)]` (to `[AsciiChar]` and `Vec<AsciiChar>` respectively).

Version 0.9.3 (2019-08-26)
==========================

Soundness fix:

**Remove** [unsound](https://github.com/tomprogrammer/rust-ascii/issues/64) impls of `From<&mut AsciiStr>` for `&mut [u8]` and `&mut str`.
This is a breaking change, but theese impls can lead to undefined behavior in safe code.

If you use this impl and know that non-ASCII values are never inserted into the `[u8]` or `str`,
you can pin ascii to 0.9.2.

Other changes:

* Make quickcheck `Arbitrary` impl sometimes produce `AsciiChar::DEL`.
* Implement `Clone`, `Copy` and `Eq` for `ToAsciiCharError`.
* Implement `ToAsciiChar` for `u16`, `u32` and `i8`.

Version 0.9.2 (2019-07-07)
==========================
* Implement the `IntoAsciiString` trait for `std::ffi::CStr` and `std::ffi::CString` types,
  and implemented the `AsAsciiStr` trait for `std::ffi::CStr` type.
* Implement the `IntoAsciiString` for `std::borrow::Cow`, where the inner types themselves
  implement `IntoAsciiString`.
* Implement conversions between `AsciiString` and `Cow<'a, AsciiStr>`.
* Implement the `std::ops::AddAssign` trait for `AsciiString`.
* Implement `BorrowMut<AsciiStr>`, `AsRef<[AsciiChar]>`, `AsRef<str>`, `AsMut<[AsciiChar]>` for `AsciiString`.
* Implement `PartialEq<[u8]>` and `PartialEq<[AsciiChar]>` for `AsciiStr`.
* Add `AsciiStr::first()`, `AsciiStr::last()` and `AsciiStr::split()` methods.
* Implement `DoubleEndedIterator` for `AsciiStr::lines()`.
* Implement `AsRef<AsciiStr>` and `AsMut<AsciiStr` for `[AsciiChar]`.
* Implement `Default` for `AsciiChar`.

Version 0.9.1 (2018-09-12)
==========================
* Implement the `serde::Serialize` and `serde::Deserialize` traits for `AsciiChar`, `AsciiStr`, and `AsciiString`.
  The implementation is enabled by the `serde` feature.
* **Bugfix**: `AsciiStr::lines()` now behaves similar to `str::lines()`.
  ([#51](https://github.com/tomprogrammer/rust-ascii/issues/51))

Version 0.9.0 (2018-04-05)
==========================
* Update the optional `quickcheck` feature to version 0.6.

Version 0.8.7 (2018-04-04)
==========================
* Implement `AsAsciiStr` and `AsMutAsciiStr` for references to to types that implement them.
* Make all methods of deprecated `AsciiExt` except `is_ascii()` available as inherent methods in std-mode.
* Compile without warnings on Rust 1.26.0
* Acknowledge that the crate doesn't compile on Rust < 1.8.0 (cannot be fixed without breaking changes).

Version 0.8.6 (2017-07-02)
==========================
* Implement `AsRef<u8> for AsciiString`.

Version 0.8.4 (2017-04-18)
==========================
* Fix the tests when running without std.

Version 0.8.3 (2017-04-18)
==========================
* Bugfix: `<AsciiStr as AsciiExt>::to_ascii_lowercase` did erroneously convert to uppercase.

Version 0.8.2 (2017-04-17)
==========================
* Implement `IntoAsciiString` for `&'a str` and `&'a [u8]`.
* Implement the `quickcheck::Arbitrary` trait for `AsciiChar` and `AsciiString`.
  The implementation is enabled by the `quickcheck` feature.

Version 0.8.1 (2017-02-11)
==========================
* Add `Chars`, `CharsMut` and `Lines` iterators.
* Implement `std::fmt::Write` for `AsciiString`.

Version 0.8.0 (2017-01-02)
==========================

Breaking changes:

* Return `FromAsciiError` instead of the input when `AsciiString::from_ascii()` or `into_ascii_string()` fails.
* Replace the `no_std` feature with the additive `std` feature, which is part of the default features. (Issue #29)
* `AsciiChar::is_*()` and `::as_{byte,char}()` take `self` by value instead of by reference.

Additions:

* Make `AsciiChar` comparable with `char` and `u8`.
* Add `AsciiChar::as_printable_char()` and the free functions `caret_encode()` and `caret_decode()`.
* Implement some methods from `AsciiExt` and `Error` (which are not in libcore) directly in `core` mode:
  * `Ascii{Char,Str}::eq_ignore_ascii_case()`
  * `AsciiChar::to_ascii_{upper,lower}case()`
  * `AsciiStr::make_ascii_{upper,lower}case()`
  * `{ToAsciiChar,AsAsciiStr}Error::description()`

Version 0.7.1 (2016-08-15)
==========================
* Fix the implementation of `AsciiExt::to_ascii_lowercase()` for `AsciiChar` converting to uppercase. (introduced in 0.7.0)

Version 0.7.0 (2016-06-25)
==========================
* Rename `Ascii` to `AsciiChar` and convert it into an enum.
  (with a variant for every ASCII character)
* Replace `OwnedAsciiCast` with `IntoAsciiString`.
* Replace `AsciiCast` with `As[Mut]AsciiStr` and `IntoAsciiChar`.
* Add *from[_ascii]_unchecked* methods.
* Replace *from_bytes* with *from_ascii* in method names.
* Return `std::error::Error`-implementing types instead of `()` and `None` when
  conversion to `AsciiStr` or `AsciiChar` fails.
* Implement `AsciiExt` without the `unstable` Cargo feature flag, which is removed.
* Require Rust 1.9 or later.
* Add `#[no_std]` support in a Cargo feature.
* Implement `From<{&,&mut,Box<}AsciiStr>` for `[Ascii]`, `[u8]` and `str`
* Implement `From<{&,&mut,Box<}[Ascii]>`, `As{Ref,Mut}<[Ascii]>` and Default for `AsciiStr`
* Implement `From<Vec<Ascii>>` for `AsciiString`.
* Implement `AsMut<AsciiStr>` for `AsciiString`.
* Stop some `Ascii::is_xxx()` methods from panicking.
* Add `Ascii::is_whitespace()`.
* Add `AsciiString::as_mut_slice()`.
* Add raw pointer methods on `AsciiString`:
  * `from_raw_parts`
  * `as_ptr`
  * `as_mut_ptr`

Version 0.6.0 (2015-12-30)
==========================
* Add `Ascii::from_byte()`
* Add `AsciiStr::trim[_{left,right}]()`

Version 0.5.4 (2015-07-29)
==========================
Implement `IndexMut` for AsciiStr and AsciiString.

Version 0.5.1 (2015-06-13)
==========================
* Add `Ascii::from()`.
* Implement `Index` for `AsciiStr` and `AsciiString`.
* Implement `Default`,`FromIterator`,`Extend` and `Add` for `AsciiString`
* Added inherent methods on `AsciiString`:
  * `with_capacity`
  * `push_str`
  * `capacity`
  * `reserve`
  * `reserve_exact`
  * `shrink_to_fit`
  * `push`
  * `truncate`
  * `pop`
  * `remove`
  * `insert`
  * `len`
  * `is_empty`
  * `clear`

Version 0.5.0 (2015-05-05)
==========================
First release compatible with Rust 1.0.0.

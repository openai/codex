**WARNING:** This CHANGELOG is no longer updated. The activity for this project
is sparse enough that you should refer to the commit log instead.


1.3.4
=====
This patch release squashes deprecation warnings for the `try!` macro, in
accordance with byteorder's minimum supported Rust version (currently at Rust
1.12.0).


1.3.3
=====
This patch release adds `ByteOrder::write_i8_into()` as a simple, safe interface
for ordinarily unsafe or tedious code.


1.3.2
=====
This patch release adds `ReadBytesExt::read_i8_into()` as a simple, safe interface
for ordinarily unsafe or tedious code.


1.3.1
=====
This minor release performs mostly small internal changes. Going forward, these
are not going to be incorporated into the changelog.


1.3.0
=====
This new minor release now enables `i128` support automatically on Rust
compilers that support 128-bit integers. The `i128` feature is now a no-op, but
continues to exist for backward compatibility purposes. The crate continues to
maintain compatibility with Rust 1.12.0.

This release also deprecates the `ByteOrder` trait methods
`read_f32_into_unchecked` and `read_f64_into_unchecked` in favor of
`read_f32_into` and `read_f64_into`. This was an oversight from the 1.2 release
where the corresponding methods on `ReadBytesExt` were deprecated.

`quickcheck` and `rand` were bumped to `0.8` and `0.6`, respectively.

A few small documentation related bugs have been fixed.


1.2.7
=====
This patch release excludes some CI files from the crate release and updates
the license field to use `OR` instead of `/`.


1.2.6
=====
This patch release fixes some test compilation errors introduced by an
over-eager release of 1.2.5.


1.2.5
=====
This patch release fixes some typos in the docs, adds doc tests to methods on
`WriteByteExt` and bumps the quickcheck dependency to `0.7`.


1.2.4
=====
This patch release adds support for 48-bit integers by adding the following
methods to the `ByteOrder` trait: `read_u48`, `read_i48`, `write_u48` and
`write_i48`. Corresponding methods have been added to the `ReadBytesExt` and
`WriteBytesExt` traits as well.


1.2.3
=====
This patch release removes the use of `feature(i128_type)` from byteorder,
since it has been stabilized. We leave byteorder's `i128` feature in place
in order to continue supporting compilation on older versions of Rust.


1.2.2
=====
This patch release only consists of internal improvements and refactorings.
Notably, this removes all uses of `transmute` and instead uses pointer casts.


1.2.1
=====
This patch release removes more unnecessary uses of `unsafe` that
were overlooked in the prior `1.2.0` release. In particular, the
`ReadBytesExt::read_{f32,f64}_into_checked` methods have been deprecated and
replaced by more appropriately named `read_{f32,f64}_into` methods.


1.2.0
=====
The most prominent change in this release of `byteorder` is the removal of
unnecessary signaling NaN masking, and in turn, the `unsafe` annotations
associated with methods that didn't do masking. See
[#103](https://github.com/BurntSushi/byteorder/issues/103)
for more details.

* [BUG #102](https://github.com/BurntSushi/byteorder/issues/102):
  Fix big endian tests.
* [BUG #103](https://github.com/BurntSushi/byteorder/issues/103):
  Remove sNaN masking.


1.1.0
=====
This release of `byteorder` features a number of fixes and improvements, mostly
as a result of the
[Litz Blitz evaluation](https://public.etherpad-mozilla.org/p/rust-crate-eval-byteorder).

Feature enhancements:

* [FEATURE #63](https://github.com/BurntSushi/byteorder/issues/63):
  Add methods for reading/writing slices of numbers for a specific
  endianness.
* [FEATURE #65](https://github.com/BurntSushi/byteorder/issues/65):
  Add support for `u128`/`i128` types. (Behind the nightly only `i128`
  feature.)
* [FEATURE #72](https://github.com/BurntSushi/byteorder/issues/72):
  Add "panics" and "errors" sections for each relevant public API item.
* [FEATURE #74](https://github.com/BurntSushi/byteorder/issues/74):
  Add CI badges to Cargo.toml.
* [FEATURE #75](https://github.com/BurntSushi/byteorder/issues/75):
  Add more examples to public API items.
* Add 24-bit read/write methods.
* Add `BE` and `LE` type aliases for `BigEndian` and `LittleEndian`,
  respectively.

Bug fixes:

* [BUG #68](https://github.com/BurntSushi/byteorder/issues/68):
  Panic in {BigEndian,LittleEndian}::default.
* [BUG #69](https://github.com/BurntSushi/byteorder/issues/69):
  Seal the `ByteOrder` trait to prevent out-of-crate implementations.
* [BUG #71](https://github.com/BurntSushi/byteorder/issues/71):
  Guarantee that the results of `read_f32`/`read_f64` are always defined.
* [BUG #73](https://github.com/BurntSushi/byteorder/issues/73):
  Add crates.io categories.
* [BUG #77](https://github.com/BurntSushi/byteorder/issues/77):
  Add `html_root` doc attribute.

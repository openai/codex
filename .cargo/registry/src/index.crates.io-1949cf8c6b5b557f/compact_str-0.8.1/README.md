<div align="center">
  <h1><code>compact_str</code></h1>
  <p><strong>A memory efficient string type that can store up to 24* bytes on the stack.</strong></p>

  <a href="https://crates.io/crates/compact_str">
    <img alt="version on crates.io" src="https://img.shields.io/crates/v/compact_str"/>
  </a>
  <img alt="Minimum supported Rust Version: 1.60" src="https://img.shields.io/badge/MSRV-1.60-blueviolet">
  <a href="LICENSE">
    <img alt="mit license" src="https://img.shields.io/crates/l/compact_str"/>
  </a>

   <br />

  <a href="https://github.com/ParkMyCar/compact_str/actions/workflows/ci.yml">
    <img alt="Continuous Integration Status" src="https://github.com/ParkMyCar/compact_str/actions/workflows/ci.yml/badge.svg?branch=main&event=push"/>
  </a>
  <a href="https://github.com/ParkMyCar/compact_str/actions/workflows/cross_platform.yml">
    <img alt="Cross Platform Status" src="https://github.com/ParkMyCar/compact_str/actions/workflows/cross_platform.yml/badge.svg?branch=main&event=push"/>
  </a>
  <a href="https://github.com/ParkMyCar/compact_str/actions/workflows/msrv.yml">
    <img alt="Minimum Supported Rust Version Status" src="https://github.com/ParkMyCar/compact_str/actions/workflows/msrv.yml/badge.svg?branch=main&event=push"/>
  </a>
  <a href="https://github.com/ParkMyCar/compact_str/actions/workflows/clippy.yml">
    <img alt="Clippy Status" src="https://github.com/ParkMyCar/compact_str/actions/workflows/clippy.yml/badge.svg?branch=main&event=push"/>
  </a>

  <p  align=right><sub>* 12 bytes for 32-bit architectures</sub></p>
</div>

<br />

### About
A `CompactString` is a more memory efficient string type, that can store smaller strings on the stack, and transparently stores longer strings on the heap (aka a small string optimization).
It can mostly be used as a drop in replacement for `String` and are particularly useful in parsing, deserializing, or any other application where you may
have smaller strings.

### Properties
A `CompactString` specifically has the following properties:
  * `size_of::<CompactString>() == size_of::<String>()`
  * Stores up to 24 bytes on the stack
    * 12 bytes if running on a 32 bit architecture
  * Strings longer than 24 bytes are stored on the heap
  * `Clone` is `O(n)`
  * `From<String>` or `From<Box<str>>` re-uses underlying buffer
    * Eagerly inlines small strings
  * `O(1)` creation from `&'static str` with `CompactString::const_new`
  * Heap based string grows at a rate of 1.5x
    * The std library `String` grows at a rate of 2x
  * Space optimized for `Option<_>`
    * `size_of::<CompactString>() == size_of::<Option<CompactString>>()`
  * Uses [branchless instructions](https://en.algorithmica.org/hpc/pipelining/branchless/) for string accesses
  * Supports `no_std` environments

### Traits
This crate exposes two traits, `ToCompactString` and `CompactStringExt`.

#### `ToCompactString`
Provides the `to_compact_string(&self)` method for converting types into a `CompactString`. This trait is automatically implemented for all types that are `std::fmt::Display`, with specialized higher performance impls for:
* `u8`, `u16`, `u32`, `u64`, `usize`, `u128`
* `i8`, `i16`, `i32`, `i64`, `isize`, `i128`
* `f32`, `f64`
* `bool`, `char`
* `NonZeroU*`, `NonZeroI*`
* `String`, `CompactString`

#### `CompactStringExt`
Provides two methods `join_compact(seperator: impl AsRef<str>)` and `concat_compact()`. This trait is automatically implemented for all types that can be converted into an iterator and yield types that `impl AsRef<str>`. This allows you to join Vec's, slices, and any other collection to form `CompactString`s.

### Macros
This crate exposes one macro `format_compact!` that can be used to create `CompactString`s from arguments, like you can `String`s with the `std::format!` macro.

### Features
`compact_str` has the following optional features:
* `serde`, which implements [`Deserialize`](https://docs.rs/serde/1/serde/trait.Deserialize.html) and [`Serialize`](https://docs.rs/serde/1/serde/trait.Serialize.html) from the popular [`serde`](https://docs.rs/serde/1/serde/) crate, for `CompactString`
* `bytes`, which provides two methods `from_utf8_buf<B: Buf>(buf: &mut B)` and `from_utf8_buf_unchecked<B: Buf>(buf: &mut B)`, which allows for the creation of a `CompactString` from a [`bytes::Buf`](https://docs.rs/bytes/1/bytes/trait.Buf.html)
* `markup`, which implements [`Render`](https://docs.rs/markup/0.13/markup/trait.Render.html) trait, so `CompactString`s can be used in templates as HTML escaped strings
* `diesel`, which allows using CompactStrings in [`diesel`](https://diesel.rs/) text columns
* `sqlx-mysql` / `sqlx-postgres` / `sqlx-sqlite`, which allows using CompactStrings in [`sqlx`](https://github.com/launchbadge/sqlx) text columns
* `arbitrary`, which implements the [`arbitrary::Arbitrary`](https://docs.rs/arbitrary/1/arbitrary/trait.Arbitrary.html) trait for fuzzing
* `proptest`, which implements the [`proptest::arbitrary::Arbitrary`](https://docs.rs/proptest/1/proptest/arbitrary/trait.Arbitrary.html) trait for fuzzing
* `quickcheck`, which implements the [`quickcheck::Arbitrary`](https://docs.rs/quickcheck/1/quickcheck/trait.Arbitrary.html) trait for fuzzing
* `rkyv`, which implements [`rkyv::Archive`](https://docs.rs/rkyv/0.7/rkyv/trait.Archive.html), [`rkyv::Serialize`](https://docs.rs/rkyv/0.7/rkyv/trait.Serialize.html) and [`rkyv::Deserialize`](https://docs.rs/rkyv/0.7/rkyv/trait.Deserialize.html) for fast zero-copy serialization, interchangable with serialized Strings
* `smallvec`, provides the `into_bytes()` method which enables you to convert a `CompactString` into a byte vector, using [`smallvec::SmallVec`](https://docs.rs/smallvec/latest/smallvec/struct.SmallVec.html)

### How it works
Note: this explanation assumes a 64-bit architecture, for 32-bit architectures generally divide any number by 2.

Normally strings are stored on the heap since they're dynamically sized. In Rust a `String` consists of three fields, each of which are the size of a `usize`.
e.g. its layout is something like the following:

`String: [ ptr<8> | len<8> | cap<8> ]`
1. `ptr` is a pointer to a location on the heap that stores the string
2. `len` is the length of the string
3. `cap` is the total capacity of the buffer being pointed to

This results in 24 bytes being stored on the stack, 8 bytes for each field. Then the actual string is stored on the heap, usually with additional memory allocated to prevent re-allocating if the string is mutated.

The idea of `CompactString` is instead of storing metadata on the stack, just store the string itself. This way for smaller strings we save a bit of memory, and we
don't have to heap allocate so it's more performant. A `CompactString` is limited to 24 bytes (aka `size_of::<String>()`) so it won't ever use more memory than a
`String` would.

The memory layout of a `CompactString` looks something like:

`CompactString: [ buffer<23> | len<1> ]`

#### Memory Layout
Internally a `CompactString` has two variants:

1. **Inline**, a string <= 24 bytes long
2. **Heap** allocated, a string > 24 bytes long

We define a discriminant (aka track which variant we are) *within* the last byte, specifically:

1. `0b11111110` - All 1s with a trailing 0, indicates **heap** allocated
2. `0b11XXXXXX` - Two leading 1s, indicates **inline**, with the trailing 6 bits used to store the length

and the overall memory layout of a `CompactString` is:

1. `heap:   { ptr: NonNull<u8>, len: usize, cap: Capacity }`
2. `inline: { buffer: [u8; 24] }`

<sub>Both variants are 24 bytes long</sub>

For **heap** allocated strings we use a custom `HeapBuffer` which normally stores the capacity of the string on the stack, but also optionally allows us to store it on the heap. Since we use the last byte to track our discriminant, we only have 7 bytes to store the capacity, or 3 bytes on a 32-bit architecture. 7 bytes allows us to store a value up to `2^56`, aka 64 petabytes, while 3 bytes only allows us to store a value up to `2^24`, aka 16 megabytes.

For 64-bit architectures we always inline the capacity, because we can safely assume our strings will never be larger than 64 petabytes, but on 32-bit architectures, when creating or growing a `CompactString`, if the text is larger than 16MB then we move the capacity onto the heap.

We handle the capacity in this way for two reasons:
1. Users shouldn't have to pay for what they don't use. Meaning, in the _majority_ of cases the capacity of the buffer could easily fit into 7 or 3 bytes, so the user shouldn't have to pay the memory cost of storing the capacity on the heap, if they don't need to.
2. Allows us to convert `From<String>` in `O(1)` time, by taking the parts of a `String` (e.g. `ptr`, `len`, and `cap`) and using those to create a `CompactString`, without having to do any heap allocations. This is important when using `CompactString` in large codebases where you might have `CompactString` working alongside of `String`.

For **inline** strings we only have a 24 byte buffer on the stack. This might make you wonder how can we store a 24 byte long string, inline? Don't we also need to store the length somewhere?

To do this, we utilize the fact that the last byte of our string could only ever have a value in the range `[0, 192)`. We know this because all strings in Rust are valid [UTF-8](https://en.wikipedia.org/wiki/UTF-8), and the only valid byte pattern for the last byte of a UTF-8 character (and thus the possible last byte of a string) is `0b0XXXXXXX` aka `[0, 128)` or `0b10XXXXXX` aka `[128, 192)`. This leaves all values in `[192, 255]` as unused in our last byte. Therefore, we can use values in the range of `[192, 215]` to represent a length in the range of `[0, 23]`, and if our last byte has a value `< 192`, we know that's a UTF-8 character, and can interpret the length of our string as `24`.

Specifically, the last byte on the stack for a `CompactString` has the following uses:
* `[0, 191]` - Is the last byte of a UTF-8 char, the `CompactString` is stored on the stack and implicitly has a length of `24`
* `[192, 215]` - Denotes a length in the range of `[0, 23]`, this `CompactString` is stored on the stack.
* `216` - Denotes this `CompactString` is stored on the heap
* `217` - Denotes this `CompactString` stores a `&'static str`.
* `[218, 255]` - Unused, denotes e.g. the `None` variant for `Option<CompactString>`

### Testing
Strings and unicode can be quite messy, even further, we're working with things at the bit level. `compact_str` has an _extensive_ test suite comprised of unit testing, property testing, and fuzz testing, to ensure our invariants are upheld. We test across all major OSes (Windows, macOS, and Linux), architectures (64-bit and 32-bit), and endian-ness (big endian and little endian).

Fuzz testing is run with `libFuzzer`, `AFL++`, *and* `honggfuzz`, with `AFL++` running on both `x86_64` and `ARMv7` architectures. We test with [`miri`](https://github.com/rust-lang/miri) to catch cases of undefined behavior, and run all tests on every Rust compiler since `v1.60` to ensure support for our minimum supported Rust version (MSRV).

### `unsafe` code
`CompactString` uses a bit of unsafe code because we manually define what variant we are, so unlike an enum, the compiler can't guarantee what value is actually stored.
We also have some manually implemented heap data structures, i.e. `HeapBuffer`, and mess with bytes at a bit level, to make the most out of our resources.
That being said, uses of unsafe code in this library are constrained to only where *absolutely* necessary, and always documented with
`// SAFETY: <reason>`.

### Similar Crates
Storing strings on the stack is not a new idea, in fact there are a few other crates in the Rust ecosystem that do similar things, an incomplete list:
1. [`smol_str`](https://crates.io/crates/smol_str) - Can inline 22 bytes, `Clone` is `O(1)`, doesn't adjust for 32-bit archs
2. [`smartstring`](https://crates.io/crates/smartstring) - Can inline 23 bytes, `Clone` is `O(n)`, is mutable
3. [`kstring`](https://crates.io/crates/kstring) - Can inline 15 or 22 bytes dependent on crate features, `Clone` is `O(1)`, can also store `&'static str`s
4. [`flexstr`](https://crates.io/crates/flexstr) - Can inline 22 bytes, `Clone` is `O(1)`, can also store `&'static str`s

<br />
Thanks for readingme!

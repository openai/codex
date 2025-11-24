[![CI](https://github.com/rusticstuff/simdutf8/actions/workflows/ci.yml/badge.svg)](https://github.com/rusticstuff/simdutf8/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/simdutf8.svg)](https://crates.io/crates/simdutf8)
[![docs.rs](https://docs.rs/simdutf8/badge.svg)](https://docs.rs/simdutf8)

# simdutf8 – High-speed UTF-8 validation

Blazingly fast API-compatible UTF-8 validation for Rust using SIMD extensions, based on the implementation from
[simdjson](https://github.com/simdjson/simdjson). Originally ported to Rust by the developers of [simd-json.rs](https://simd-json.rs), but now heavily improved.

## Status
This library has been thoroughly tested with sample data as well as fuzzing and there are no known bugs.

## Features
* `basic` API for the fastest validation, optimized for valid UTF-8
* `compat` API as a fully compatible replacement for `std::str::from_utf8()`
* Supports AVX 2 and SSE 4.2 implementations on x86 and x86-64
* 🆕 ARM64 (aarch64) SIMD is supported since Rust 1.61
* 🆕 WASM (wasm32) SIMD is supported
* x86-64: Up to 23 times faster than the std library on valid non-ASCII, up to four times faster on ASCII
* aarch64: Up to eleven times faster than the std library on valid non-ASCII, up to four times faster on ASCII (Apple Silicon)
* Faster than the original simdjson implementation
* Selects the fastest implementation at runtime based on CPU support (on x86)
* Falls back to the excellent std implementation if SIMD extensions are not supported
* Written in pure Rust
* No dependencies
* No-std support

## Quick start
Add the dependency to your Cargo.toml file:
```toml
[dependencies]
simdutf8 = "0.1.5"
```

Use `simdutf8::basic::from_utf8()` as a drop-in replacement for `std::str::from_utf8()`.

```rust
use simdutf8::basic::from_utf8;

println!("{}", from_utf8(b"I \xE2\x9D\xA4\xEF\xB8\x8F UTF-8!").unwrap());
```

If you need detailed information on validation failures, use `simdutf8::compat::from_utf8()`
instead.

```rust
use simdutf8::compat::from_utf8;

let err = from_utf8(b"I \xE2\x9D\xA4\xEF\xB8 UTF-8!").unwrap_err();
assert_eq!(err.valid_up_to(), 5);
assert_eq!(err.error_len(), Some(2));
```

## APIs

### Basic flavor
Use the `basic` API flavor for maximum speed. It is fastest on valid UTF-8, but only checks
for errors after processing the whole byte sequence and does not provide detailed information if the data
is not valid UTF-8. `simdutf8::basic::Utf8Error` is a zero-sized error struct.

### Compat flavor
The `compat` flavor is fully API-compatible with `std::str::from_utf8()`. In particular, `simdutf8::compat::from_utf8()`
returns a `simdutf8::compat::Utf8Error`, which has `valid_up_to()` and `error_len()` methods. The first is useful for
verification of streamed data. The second is useful e.g. for replacing invalid byte sequences with a replacement character.

It also fails early: errors are checked on the fly as the string is processed and once
an invalid UTF-8 sequence is encountered, it returns without processing the rest of the data.
This comes at a slight performance penalty compared to the `basic` API even if the input is valid UTF-8.

## Implementation selection

### X86
The fastest implementation is selected at runtime using the `std::is_x86_feature_detected!` macro, unless the CPU
targeted by the compiler supports the fastest available implementation.
So if you compile with `RUSTFLAGS="-C target-cpu=native"` on a recent x86-64 machine, the AVX 2 implementation is selected at
compile-time and runtime selection is disabled.

For no-std support (compiled with `--no-default-features`) the implementation is always selected at compile time based on
the targeted CPU. Use `RUSTFLAGS="-C target-feature=+avx2"` for the AVX 2 implementation or `RUSTFLAGS="-C target-feature=+sse4.2"`
for the SSE 4.2 implementation.

### ARM64
The SIMD implementation is used automatically since Rust 1.61.

### WASM32
For wasm32 support, the implementation is selected at compile time based on the presence of the `simd128` target feature.
Use `RUSTFLAGS="-C target-feature=+simd128"` to enable the WASM SIMD implementation.  WASM, at
the time of this writing, doesn't have a way to detect SIMD through WASM itself.  Although this capability
is available in various WASM host environments (e.g., [wasm-feature-detect] in the web browser), there is no portable
way from within the library to detect this.

[wasm-feature-detect]: https://github.com/GoogleChromeLabs/wasm-feature-detect

#### Building/Targeting WASM
See [this document](./wasm32-development.md) for more details.

### Access to low-level functionality

If you want to be able to call a SIMD implementation directly, use the `public_imp` feature flag. The validation implementations are then accessible in the `simdutf8::{basic, compat}::imp` hierarchy. Traits
facilitating streaming validation are available there as well.

## Optimisation flags
Do not use [`opt-level = "z"`](https://doc.rust-lang.org/cargo/reference/profiles.html), which prevents inlining and makes
the code quite slow.

## Minimum Supported Rust Version (MSRV)
This crate's minimum supported Rust version is 1.38.0.

## Benchmarks
The benchmarks have been done with [criterion](https://bheisler.github.io/criterion.rs/book/index.html), the tables
are created with [critcmp](https://github.com/BurntSushi/critcmp). Source code and data are in the
[bench directory](https://github.com/rusticstuff/simdutf8/tree/main/bench).

The naming schema is id-charset/size. _0-empty_ is the empty byte slice, _x-error/66536_ is a 64KiB slice where the very
first character is invalid UTF-8. Library versions are simdutf8 v0.1.2 and simdjson v0.9.2. When comparing
with simdjson simdutf8 is compiled with `#inline(never)`.

Configurations:
* X86-64: PC with an AMD Ryzen 7 PRO 3700 CPU (Zen2) on Linux with Rust 1.52.0
* Aarch64: Macbook Air with an Apple M1 CPU (Apple Silicon) on macOS with Rust rustc 1.54.0-nightly (881c1ac40 2021-05-08).

### simdutf8 basic vs std library on x86-64 (AMD Zen2)
![image](https://user-images.githubusercontent.com/3736990/117568104-1c00f900-b0bf-11eb-938f-4c253d192480.png)
Simdutf8 is up to 23 times faster than the std library on valid non-ASCII, up to four times on pure ASCII.

### simdutf8 basic vs std library on aarch64 (Apple Silicon)
![image](https://user-images.githubusercontent.com/3736990/117568160-42bf2f80-b0bf-11eb-86a4-9aeee4cee87d.png)
Simdutf8 is up to to eleven times faster than the std library on valid non-ASCII, up to four times faster on
pure ASCII.

### simdutf8 basic vs simdjson on x86-64
![image](https://user-images.githubusercontent.com/3736990/117568231-80bc5380-b0bf-11eb-8e90-1dcc6d966ebd.png)
Simdutf8 is faster than simdjson on almost all inputs.

### simdutf8 basic vs simdutf8 compat UTF-8 on x86-64
![image](https://user-images.githubusercontent.com/3736990/117568270-af3a2e80-b0bf-11eb-8ec4-e5a0a4ad7210.png)
There is a small performance penalty to continuously checking the error status while processing data, but detecting
errors early provides a huge benefit for the _x-error/66536_ benchmark.

## Technical details
For inputs shorter than 64 bytes validation is delegated to `core::str::from_utf8()` except for the direct-access
functions in `simdutf8::{basic, compat}::imp`.

The SIMD implementation is mostly similar to the one in simdjson except that it is has additional optimizations
for the pure ASCII case. Also it uses prefetch with AVX 2 on x86 which leads to slightly better performance with
some Intel CPUs on synthetic benchmarks.

For the compat API, we need to check the error status vector on each 64-byte block instead of just aggregating it. If an
error is found, the last bytes of the previous block are checked for a cross-block continuation and then
`std::str::from_utf8()` is run to find the exact location of the error.

Care is taken that all functions are properly inlined up to the public interface.

## Thanks
* to the authors of simdjson for coming up with the high-performance SIMD implementation and in particular to Daniel Lemire
  for his feedback. It was very helpful.
* to the authors of the simdjson Rust port who did most of the heavy lifting of porting the C++ code to Rust.


## License
This code is dual-licensed under the [Apache License 2.0](https://www.apache.org/licenses/LICENSE-2.0.html) and the [MIT License](https://opensource.org/licenses/MIT).

It is based on code distributed with simd-json.rs, the Rust port of simdjson, which is dual-licensed under
the MIT license and Apache 2.0 license as well.

simdjson itself is distributed under the Apache License 2.0.

## References
John Keiser, Daniel Lemire, [Validating UTF-8 In Less Than One Instruction Per Byte](https://arxiv.org/abs/2010.03090), Software: Practice and Experience 51 (5), 2021

#![warn(unused_extern_crates)]
#![deny(
    clippy::all,
    clippy::unwrap_used,
    clippy::unnecessary_unwrap,
    clippy::pedantic,
    clippy::nursery
)]
#![allow(clippy::redundant_pub_crate)] // check is broken
#![allow(clippy::redundant_else)] // can make code more readable
#![allow(clippy::explicit_iter_loop)] // can make code more readable
#![allow(clippy::semicolon_if_nothing_returned)] // see https://github.com/rust-lang/rust-clippy/issues/7768
#![allow(clippy::missing_const_for_fn)] // not necessary most of the times
#![deny(missing_docs)]
#![cfg_attr(not(feature = "std"), no_std)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![cfg_attr(
    all(target_arch = "aarch64", feature = "aarch64_neon_prefetch"),
    feature(stdarch_aarch64_prefetch)
)]

//! Blazingly fast API-compatible UTF-8 validation for Rust using SIMD extensions, based on the implementation from
//! [simdjson](https://github.com/simdjson/simdjson). Originally ported to Rust by the developers of [simd-json.rs](https://simd-json.rs), but now heavily improved.
//!
//! ## Quick start
//! Add the dependency to your Cargo.toml file:
//! ```toml
//! [dependencies]
//! simdutf8 = "0.1.5"
//! ```
//!
//! Use [`basic::from_utf8()`] as a drop-in replacement for `std::str::from_utf8()`.
//!
//! ```rust
//! use simdutf8::basic::from_utf8;
//!
//! println!("{}", from_utf8(b"I \xE2\x9D\xA4\xEF\xB8\x8F UTF-8!").unwrap());
//! ```
//!
//! If you need detailed information on validation failures, use [`compat::from_utf8()`]
//! instead.
//!
//! ```rust
//! use simdutf8::compat::from_utf8;
//!
//! let err = from_utf8(b"I \xE2\x9D\xA4\xEF\xB8 UTF-8!").unwrap_err();
//! assert_eq!(err.valid_up_to(), 5);
//! assert_eq!(err.error_len(), Some(2));
//! ```
//!
//! ## APIs
//!
//! ### Basic flavor
//! Use the `basic` API flavor for maximum speed. It is fastest on valid UTF-8, but only checks
//! for errors after processing the whole byte sequence and does not provide detailed information if the data
//! is not valid UTF-8. [`basic::Utf8Error`] is a zero-sized error struct.
//!
//! ### Compat flavor
//! The `compat` flavor is fully API-compatible with `std::str::from_utf8()`. In particular, [`compat::from_utf8()`]
//! returns a [`compat::Utf8Error`], which has [`valid_up_to()`](compat::Utf8Error#method.valid_up_to) and
//! [`error_len()`](compat::Utf8Error#method.error_len) methods. The first is useful for verification of streamed data. The
//! second is useful e.g. for replacing invalid byte sequences with a replacement character.
//!
//! It also fails early: errors are checked on the fly as the string is processed and once
//! an invalid UTF-8 sequence is encountered, it returns without processing the rest of the data.
//! This comes at a slight performance penalty compared to the [`basic`] API even if the input is valid UTF-8.
//!
//! ## Implementation selection
//!
//! ### X86
//! The fastest implementation is selected at runtime using the `std::is_x86_feature_detected!` macro, unless the CPU
//! targeted by the compiler supports the fastest available implementation.
//! So if you compile with `RUSTFLAGS="-C target-cpu=native"` on a recent x86-64 machine, the AVX 2 implementation is selected at
//! compile-time and runtime selection is disabled.
//!
//! For no-std support (compiled with `--no-default-features`) the implementation is always selected at compile time based on
//! the targeted CPU. Use `RUSTFLAGS="-C target-feature=+avx2"` for the AVX 2 implementation or `RUSTFLAGS="-C target-feature=+sse4.2"`
//! for the SSE 4.2 implementation.
//!
//! ### ARM64
//! The SIMD implementation is used automatically since Rust 1.61.
//!
//! ### WASM32
//! For wasm32 support, the implementation is selected at compile time based on the presence of the `simd128` target feature.
//! Use `RUSTFLAGS="-C target-feature=+simd128"` to enable the WASM SIMD implementation.  WASM, at
//! the time of this writing, doesn't have a way to detect SIMD through WASM itself.  Although this capability
//! is available in various WASM host environments (e.g., [wasm-feature-detect] in the web browser), there is no portable
//! way from within the library to detect this.
//!
//! [wasm-feature-detect]: https://github.com/GoogleChromeLabs/wasm-feature-detect
//!
//! ### Access to low-level functionality
//! If you want to be able to call a SIMD implementation directly, use the `public_imp` feature flag. The validation
//! implementations are then accessible via [`basic::imp`] and [`compat::imp`]. Traits facilitating streaming validation are available
//! there as well.
//!
//! ## Optimisation flags
//! Do not use [`opt-level = "z"`](https://doc.rust-lang.org/cargo/reference/profiles.html), which prevents inlining and makes
//! the code quite slow.
//!
//! ## Minimum Supported Rust Version (MSRV)
//! This crate's minimum supported Rust version is 1.38.0.
//!
//! ## Algorithm
//!
//! See Validating UTF-8 In Less Than One Instruction Per Byte, Software: Practice and Experience 51 (5), 2021
//! <https://arxiv.org/abs/2010.03090>

pub mod basic;
pub mod compat;
mod implementation;

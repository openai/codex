// Copyright 2015-2016 Brian Smith.
//
// Permission to use, copy, modify, and/or distribute this software for any
// purpose with or without fee is hereby granted, provided that the above
// copyright notice and this permission notice appear in all copies.
//
// THE SOFTWARE IS PROVIDED "AS IS" AND THE AUTHOR DISCLAIMS ALL WARRANTIES
// WITH REGARD TO THIS SOFTWARE INCLUDING ALL IMPLIED WARRANTIES OF
// MERCHANTABILITY AND FITNESS. IN NO EVENT SHALL THE AUTHOR BE LIABLE FOR ANY
// SPECIAL, DIRECT, INDIRECT, OR CONSEQUENTIAL DAMAGES OR ANY DAMAGES
// WHATSOEVER RESULTING FROM LOSS OF USE, DATA OR PROFITS, WHETHER IN AN ACTION
// OF CONTRACT, NEGLIGENCE OR OTHER TORTIOUS ACTION, ARISING OUT OF OR IN
// CONNECTION WITH THE USE OR PERFORMANCE OF THIS SOFTWARE.

//! # Feature Flags
//!
//! <table>
//! <tr><th>Feature
//!     <th>Description
//! <tr><td><code>alloc (default)</code>
//!     <td>Enable features that require use of the heap, RSA in particular.
//! <tr><td><code>less-safe-getrandom-custom-or-rdrand</code>
//!     <td>Treat user-provided ("custom") and RDRAND-based <code>getrandom</code>
//!         implementations as secure random number generators (see
//!         <code>SecureRandom</code>). This feature only works with
//!         <code>os = "none"</code> targets. See
//!         <a href="https://docs.rs/getrandom/0.2.10/getrandom/macro.register_custom_getrandom.html">
//!             <code>register_custom_getrandom</code>
//!         </a> and <a href="https://docs.rs/getrandom/0.2.10/getrandom/#rdrand-on-x86">
//!             RDRAND on x86
//!         </a> for additional details.
//! <tr><td><code>less-safe-getrandom-espidf</code>
//!     <td>Treat getrandom as a secure random number generator (see
//!         <code>SecureRandom</code>) on the esp-idf target. While the esp-idf
//!         target does have hardware RNG, it is beyond the scope of ring to
//!         ensure its configuration. This feature allows ring to build
//!         on esp-idf despite the likelihood that RNG is not secure.
//!         This feature only works with <code>os = espidf</code> targets.
//!         See <a href="https://docs.espressif.com/projects/esp-idf/en/stable/esp32/api-reference/system/random.html">
//! <tr><td><code>std</code>
//!     <td>Enable features that use libstd, in particular
//!         <code>std::error::Error</code> integration. Implies `alloc`.
//! <tr><td><code>wasm32_unknown_unknown_js</code>
//!     <td>When this feature is enabled, for the wasm32-unknown-unknown target,
//!         Web APIs will be used to implement features like `ring::rand` that
//!         require an operating environment of some kind. This has no effect
//!         for any other target. This enables the `getrandom` crate's `js`
//!         feature.
//! </table>

// When running mk/package.sh, don't actually build any code.
#![allow(
    clippy::collapsible_if,
    clippy::identity_op,
    clippy::len_without_is_empty,
    clippy::let_unit_value,
    clippy::new_without_default,
    clippy::neg_cmp_op_on_partial_ord,
    clippy::too_many_arguments,
    clippy::type_complexity,
    non_camel_case_types,
    non_snake_case,
    unsafe_code
)]
#![deny(variant_size_differences)]
#![forbid(
    unused_results,
    unsafe_op_in_unsafe_fn,
    clippy::char_lit_as_u8,
    clippy::fn_to_numeric_cast,
    clippy::fn_to_numeric_cast_with_truncation,
    clippy::ptr_as_ptr
)]
#![warn(
    clippy::unnecessary_cast,
    clippy::cast_lossless,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss
)]
#![cfg_attr(
    not(any(
        all(target_arch = "aarch64", target_endian = "little"),
        all(target_arch = "arm", target_endian = "little"),
        target_arch = "x86",
        target_arch = "x86_64",
        feature = "alloc"
    )),
    allow(dead_code, unused_imports, unused_macros)
)]
#![no_std]

#[cfg(feature = "alloc")]
extern crate alloc;

#[macro_use]
mod debug;

#[macro_use]
mod prefixed;

#[doc(hidden)]
#[macro_use]
mod testutil;

#[macro_use]
mod bssl;

#[macro_use]
mod polyfill;

pub mod aead;

pub mod agreement;
mod arithmetic;
mod bits;

pub(crate) mod bb;
pub(crate) mod c;

#[doc(hidden)]
#[deprecated(
    note = "Will be removed. Internal module not intended for external use, with no promises regarding side channels."
)]
pub mod deprecated_constant_time;

#[doc(hidden)]
#[allow(deprecated)]
#[deprecated(
    note = "Will be removed. Internal module not intended for external use, with no promises regarding side channels."
)]
pub use deprecated_constant_time as constant_time;

pub mod io;

mod cpu;
pub mod digest;
mod ec;
pub mod error;
pub mod hkdf;
pub mod hmac;
mod limb;
pub mod pbkdf2;
pub mod pkcs8;
pub mod rand;

#[cfg(feature = "alloc")]
pub mod rsa;

pub mod signature;

#[cfg(test)]
mod tests;

mod sealed {
    /// Traits that are designed to only be implemented internally in *ring*.
    //
    // Usage:
    // ```
    // use crate::sealed;
    //
    // pub trait MyType: sealed::Sealed {
    //     // [...]
    // }
    //
    // impl sealed::Sealed for MyType {}
    // ```
    pub trait Sealed {}
}

#[deprecated(note = "internal API that will be removed")]
pub mod deprecated_test;

#[allow(deprecated)]
#[deprecated(note = "internal API that will be removed")]
pub use deprecated_test as test;

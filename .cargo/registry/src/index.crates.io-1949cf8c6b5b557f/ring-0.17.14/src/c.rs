// Copyright 2016-2019 Brian Smith.
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

//! C types.
//!
//! Avoid using the `libc` crate to get C types since `libc` doesn't support
//! all the targets we need to support. It turns out that the few types we need
//! are all uniformly defined on the platforms we care about. This will
//! probably change if/when we support 16-bit platforms or platforms where
//! `usize` and `uintptr_t` are different sizes.
//!
//! TODO(MSRV, feature(c_size_t)): Use `core::{ffi::c_size_t}`.
//! TODO(MSRV-1.79): Use `NonZero<c_size_t>`.

// Keep in sync with the checks in base.h that verify these assumptions.

#![allow(dead_code)]

use core::num::NonZeroUsize;

pub(crate) type size_t = usize;
pub(crate) type NonZero_size_t = NonZeroUsize;

// Copyright 2018 Developers of the Rand project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! The ChaCha random number generator.

#![doc(
    html_logo_url = "https://www.rust-lang.org/logos/rust-logo-128x128-blk.png",
    html_favicon_url = "https://www.rust-lang.org/favicon.ico",
    html_root_url = "https://rust-random.github.io/rand/"
)]
#![deny(missing_docs)]
#![deny(missing_debug_implementations)]
#![doc(test(attr(allow(unused_variables), deny(warnings))))]
#![cfg_attr(not(feature = "std"), no_std)]

pub use rand_core;

mod chacha;
mod guts;

pub use crate::chacha::{
    ChaCha12Core, ChaCha12Rng, ChaCha20Core, ChaCha20Rng, ChaCha8Core, ChaCha8Rng,
};

/// ChaCha with 20 rounds
pub type ChaChaRng = ChaCha20Rng;
/// ChaCha with 20 rounds, low-level interface
pub type ChaChaCore = ChaCha20Core;

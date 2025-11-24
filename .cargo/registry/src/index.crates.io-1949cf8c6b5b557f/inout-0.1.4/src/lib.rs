//! Collection of custom reference types for code generic over in-place and
//! buffer-to-buffer modes of operation.

#![no_std]
#![doc(
    html_logo_url = "https://raw.githubusercontent.com/RustCrypto/media/8f1a9894/logo.svg",
    html_favicon_url = "https://raw.githubusercontent.com/RustCrypto/media/8f1a9894/logo.svg",
    html_root_url = "https://docs.rs/inout/0.1.4"
)]
#![allow(clippy::needless_lifetimes)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![warn(missing_docs, rust_2018_idioms)]

#[cfg(feature = "std")]
extern crate std;

#[cfg(feature = "block-padding")]
#[cfg_attr(docsrs, doc(cfg(feature = "block-padding")))]
pub use block_padding;

mod errors;
mod inout;
mod inout_buf;
mod reserved;

pub use crate::{errors::*, inout::*, inout_buf::*, reserved::*};

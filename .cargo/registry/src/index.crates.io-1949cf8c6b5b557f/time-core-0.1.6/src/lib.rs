//! Core items for `time`.
//!
//! This crate is an implementation detail of `time` and should not be relied upon directly.

#![no_std]
#![doc(html_favicon_url = "https://avatars0.githubusercontent.com/u/55999857")]
#![doc(html_logo_url = "https://avatars0.githubusercontent.com/u/55999857")]
#![doc(test(attr(deny(warnings))))]

pub mod convert;
mod hint;
pub mod util;

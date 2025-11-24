/*!
Learn more about Rust for Windows here: <https://github.com/microsoft/windows-rs>
*/

#![no_std]
#![doc(html_no_source)]
#![allow(non_snake_case, non_upper_case_globals, non_camel_case_types, clippy::all)]
#![cfg_attr(not(feature = "docs"), doc(hidden))]

extern crate self as windows_sys;
pub mod core;

include!("Windows/mod.rs");

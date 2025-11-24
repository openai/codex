//! `powerfmt` is a library that provides utilities for formatting values. Specifically, it makes it
//! significantly easier to support filling to a minimum width with alignment, avoid heap
//! allocation, and avoid repetitive calculations.

#![cfg_attr(not(feature = "std"), no_std)]
#![cfg_attr(__powerfmt_docs, feature(doc_auto_cfg, rustc_attrs))]
#![cfg_attr(__powerfmt_docs, allow(internal_features))]

#[cfg(feature = "alloc")]
extern crate alloc;

pub mod buf;
pub mod ext;
pub mod smart_display;
mod smart_display_impls;

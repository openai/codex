#![allow(clippy::module_inception)]

#[allow(dead_code, clippy::redundant_static_lifetimes, clippy::unreadable_literal)]
#[rustfmt::skip]
mod trie;

pub(crate) use self::trie::*;

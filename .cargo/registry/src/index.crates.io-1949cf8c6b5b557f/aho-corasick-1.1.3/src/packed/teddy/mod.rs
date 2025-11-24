// Regrettable, but Teddy stuff just isn't used on all targets. And for some
// targets, like aarch64, only "slim" Teddy is used and so "fat" Teddy gets a
// bunch of dead-code warnings. Just not worth trying to squash them. Blech.
#![allow(dead_code)]

pub(crate) use self::builder::{Builder, Searcher};

mod builder;
mod generic;

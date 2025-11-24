#![cfg(not(miri))]
#![recursion_limit = "1024"]
#![feature(rustc_private)]
#![allow(
    clippy::elidable_lifetime_names,
    clippy::manual_assert,
    clippy::match_like_matches_macro,
    clippy::needless_lifetimes,
    clippy::uninlined_format_args
)]

use crate::common::visit::{AsIfPrinted, FlattenParens};
use quote::ToTokens as _;
use std::fs;
use std::panic;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use syn::visit_mut::VisitMut as _;

#[macro_use]
mod macros;

mod common;
mod repo;

#[test]
fn test_unparenthesize() {
    repo::rayon_init();
    repo::clone_rust();

    let failed = AtomicUsize::new(0);

    repo::for_each_rust_file(|path| test(path, &failed));

    let failed = failed.into_inner();
    if failed > 0 {
        panic!("{} failures", failed);
    }
}

fn test(path: &Path, failed: &AtomicUsize) {
    let content = fs::read_to_string(path).unwrap();

    match panic::catch_unwind(|| -> syn::Result<()> {
        let mut before = syn::parse_file(&content)?;
        FlattenParens::discard_attrs().visit_file_mut(&mut before);
        let printed = before.to_token_stream();
        let mut after = syn::parse2::<syn::File>(printed.clone())?;
        FlattenParens::discard_attrs().visit_file_mut(&mut after);
        // Normalize features that we expect Syn not to print.
        AsIfPrinted.visit_file_mut(&mut before);
        if before != after {
            errorf!("=== {}\n", path.display());
            if failed.fetch_add(1, Ordering::Relaxed) == 0 {
                errorf!("BEFORE:\n{:#?}\nAFTER:\n{:#?}\n", before, after);
            }
        }
        Ok(())
    }) {
        Err(_) => {
            errorf!("=== {}: syn panic\n", path.display());
            failed.fetch_add(1, Ordering::Relaxed);
        }
        Ok(Err(msg)) => {
            errorf!("=== {}: syn failed to parse\n{:?}\n", path.display(), msg);
            failed.fetch_add(1, Ordering::Relaxed);
        }
        Ok(Ok(())) => {}
    }
}

/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under both the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree and the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree.
 */

mod clone;
mod copy;
mod dupe;
mod util;

/// Derive the `Dupe` trait.
#[proc_macro_derive(Dupe)]
pub fn derive_dupe(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    dupe::derive_dupe(input)
}

/// Derive the `Dupe` trait, but without requiring all type arguments to implement `Dupe`.
#[proc_macro_derive(Dupe_)]
pub fn derive_dupe_(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    dupe::derive_dupe_(input)
}

/// Derive the [`Clone` trait](Clone), but without requiring all type arguments to implement [`Clone`].
#[proc_macro_derive(Clone_)]
pub fn derive_clone_(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    clone::derive_clone_(input)
}

/// Derive the [`Copy` trait](Copy), but without requiring all type arguments to implement [`Copy`].
#[proc_macro_derive(Copy_)]
pub fn derive_copy_(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    copy::derive_copy_(input)
}

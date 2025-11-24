/*
 * Copyright 2019 The Starlark in Rust Authors.
 * Copyright (c) Facebook, Inc. and its affiliates.
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     https://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

pub(crate) mod param_spec;
pub(crate) mod parse;
mod render;
pub(crate) mod simple_param;
mod typ;
mod util;

use proc_macro::TokenStream;
use syn::parse_macro_input;
use syn::ItemFn;

pub(crate) fn starlark_module(attr: TokenStream, input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as ItemFn);

    fn starlark_module_impl(attr: TokenStream, input: ItemFn) -> syn::Result<TokenStream> {
        assert!(attr.is_empty());
        let x = parse::parse(input)?;
        Ok(render::render(x)?.into())
    }

    match starlark_module_impl(attr, input) {
        Ok(x) => x,
        Err(e) => e.to_compile_error().into(),
    }
}

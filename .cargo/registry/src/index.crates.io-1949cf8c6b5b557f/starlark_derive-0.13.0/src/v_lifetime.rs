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

use crate::util::GenericsUtil;

/// Find at most one lifetime parameter, which must be named `'v`.
pub(crate) fn find_v_lifetime(generics: &syn::Generics) -> syn::Result<Option<&syn::Lifetime>> {
    let generics = GenericsUtil::new(generics);
    let Some(lifetime) = generics.assert_at_most_one_lifetime_param()? else {
        return Ok(None);
    };
    if !lifetime.bounds.is_empty() {
        return Err(syn::Error::new_spanned(
            lifetime,
            "Lifetime parameter cannot have bounds",
        ));
    }
    if lifetime.lifetime.ident != "v" {
        return Err(syn::Error::new_spanned(
            lifetime,
            "Lifetime parameter must be named 'v'",
        ));
    }
    Ok(Some(&lifetime.lifetime))
}

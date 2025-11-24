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

use syn::*;

/// Is a type matching a given name
pub(crate) fn is_type_name(x: &Type, name: &str) -> bool {
    if let Type::Path(TypePath {
        path: Path { segments, .. },
        ..
    }) = x
    {
        if let Some(seg1) = segments.last() {
            return seg1.ident == name;
        }
    }
    false
}

/// If the type is `Option`, return the type inside it.
pub(crate) fn unpack_option(x: &Type) -> Option<&Type> {
    if let Type::Path(TypePath {
        path: Path { segments, .. },
        ..
    }) = x
    {
        if let Some(seg1) = segments.last() {
            if seg1.ident == "Option" {
                if let PathArguments::AngleBracketed(AngleBracketedGenericArguments {
                    args, ..
                }) = &seg1.arguments
                {
                    if let Some(GenericArgument::Type(x)) = args.first() {
                        return Some(x);
                    }
                }
            }
        }
    }
    None
}

pub(crate) fn ident_string(x: &Ident) -> String {
    let x = x.to_string();
    match x.strip_prefix("r#") {
        Some(x) => x.to_owned(),
        None => x,
    }
}

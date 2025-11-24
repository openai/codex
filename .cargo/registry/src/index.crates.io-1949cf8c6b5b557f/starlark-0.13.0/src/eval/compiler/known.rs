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

//! Things that operate on known values where we know we can do better.

use std::borrow::Cow;

use starlark_syntax::syntax::ast::ExprP;

use crate::codemap::Spanned;
use crate::eval::compiler::scope::payload::CstExpr;

/// Convert a list into a tuple. In many cases (iteration, `in`) these types
/// behave the same, but a list has identity and mutability, so much better to
/// switch to tuple where it makes no difference. A tuple of constants
/// will go on the FrozenHeap, while a list of constants will be continually
/// reallocated.
pub(crate) fn list_to_tuple(x: &CstExpr) -> Cow<CstExpr> {
    match x {
        Spanned {
            node: ExprP::List(x),
            span,
        } => Cow::Owned(Spanned {
            // TODO(nga): do not clone.
            node: ExprP::Tuple(x.clone()),
            span: *span,
        }),
        _ => Cow::Borrowed(x),
    }
}

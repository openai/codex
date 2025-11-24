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

use starlark_syntax::codemap::Spanned;

use crate::typing::Ty;

/// Function call arguments.
pub struct TyCallArgs<'a> {
    pub(crate) pos: Vec<Spanned<Ty>>,
    pub(crate) named: Vec<Spanned<(&'a str, Ty)>>,
    /// In starlark, `*args` always come after all positional and named arguments.
    pub(crate) args: Option<Spanned<Ty>>,
    pub(crate) kwargs: Option<Spanned<Ty>>,
}

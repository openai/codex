/*
 * Copyright 2018 The Starlark in Rust Authors.
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

use std::fmt::Display;

use crate::codemap::CodeMap;
use crate::codemap::Span;
use crate::eval_exception::EvalException;
use crate::syntax::Dialect;

pub(crate) struct ParserState<'a> {
    pub(crate) dialect: &'a Dialect,
    pub(crate) codemap: &'a CodeMap,
    /// Recoverable errors.
    pub(crate) errors: &'a mut Vec<EvalException>,
}

impl<'a> ParserState<'a> {
    /// Add recoverable error.
    pub(crate) fn error(&mut self, span: Span, error: impl Display) {
        self.errors
            .push(EvalException::parser_error(error, span, self.codemap));
    }
}

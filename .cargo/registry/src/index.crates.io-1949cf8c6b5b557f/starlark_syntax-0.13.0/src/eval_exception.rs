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

use std::fmt::Display;

use crate::call_stack::CallStack;
use crate::codemap::CodeMap;
use crate::codemap::Span;
use crate::diagnostic::WithDiagnostic;
use crate::internal_error;

/// Error with location.
#[derive(Debug, derive_more::Display)]
pub struct EvalException(
    /// Error is guaranteed to have a diagnostic
    crate::Error,
);

impl EvalException {
    #[cold]
    pub fn into_error(self) -> crate::Error {
        self.0
    }

    #[cold]
    pub fn into_internal_error(self) -> Self {
        EvalException(self.0.into_internal_error())
    }

    #[cold]
    pub fn new(mut error: crate::Error, span: Span, codemap: &CodeMap) -> EvalException {
        error.set_span(span, codemap);
        EvalException(error)
    }

    /// `EvalException` is meant to provide type-safe guard against missing span.
    /// Sometimes we need to construct `EvalException`, but span is not available,
    /// so this function can be used. Avoid this function if possible.
    #[cold]
    pub fn new_unknown_span(error: crate::Error) -> EvalException {
        EvalException(error)
    }

    #[cold]
    pub fn new_with_callstack(
        mut error: crate::Error,
        span: Span,
        codemap: &CodeMap,
        call_stack: impl FnOnce() -> CallStack,
    ) -> EvalException {
        error.set_span(span, codemap);
        error.set_call_stack(call_stack);
        EvalException(error)
    }

    #[cold]
    pub fn new_anyhow(error: anyhow::Error, span: Span, codemap: &CodeMap) -> EvalException {
        EvalException(crate::Error::new_spanned(
            crate::ErrorKind::Other(error),
            span,
            codemap,
        ))
    }

    #[cold]
    pub fn internal_error(error: impl Display, span: Span, codemap: &CodeMap) -> EvalException {
        Self::new(internal_error!("{}", error), span, codemap)
    }

    #[cold]
    pub(crate) fn parser_error(
        error: impl Display,
        span: Span,
        codemap: &CodeMap,
    ) -> EvalException {
        EvalException(crate::Error::new_spanned(
            crate::ErrorKind::Parser(anyhow::anyhow!("{error}")),
            span,
            codemap,
        ))
    }

    pub fn _testing_loc(err: &crate::Error) -> crate::codemap::ResolvedFileSpan {
        match err.span() {
            Some(d) => d.resolve(),
            None => panic!("Expected error with diagnostic, got {:#?}", err),
        }
    }
}

impl<T: Into<crate::Error>> From<WithDiagnostic<T>> for EvalException {
    fn from(e: WithDiagnostic<T>) -> Self {
        Self(e.into())
    }
}

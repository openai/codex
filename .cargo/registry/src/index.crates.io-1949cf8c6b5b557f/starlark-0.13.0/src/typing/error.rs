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

use starlark_syntax::diagnostic::WithDiagnostic;
use starlark_syntax::eval_exception::EvalException;

use crate::codemap::CodeMap;
use crate::codemap::Span;

/// Internal error, bug in the typechecker.
#[derive(Debug)]
pub struct InternalError(EvalException);

impl InternalError {
    #[cold]
    pub(crate) fn msg(message: impl Display, span: Span, codemap: &CodeMap) -> InternalError {
        InternalError(EvalException::new(
            crate::Error::new_kind(crate::ErrorKind::Internal(anyhow::Error::msg(
                message.to_string(),
            ))),
            span,
            codemap,
        ))
    }

    #[cold]
    pub(crate) fn from_diagnostic(d: WithDiagnostic<impl Display>) -> InternalError {
        let internal = d.map(|m| {
            crate::Error::new_kind(crate::ErrorKind::Internal(anyhow::Error::msg(
                m.to_string(),
            )))
        });
        InternalError(internal.into())
    }

    #[cold]
    pub(crate) fn from_eval_exception(e: EvalException) -> InternalError {
        InternalError(e.into_internal_error())
    }

    #[cold]
    pub(crate) fn from_error(e: crate::Error, span: Span, codemap: &CodeMap) -> InternalError {
        let e = e.into_internal_error();
        InternalError(EvalException::new(e, span, codemap))
    }

    #[cold]
    pub(crate) fn into_error(self) -> crate::Error {
        self.0.into_error()
    }

    #[cold]
    pub(crate) fn into_eval_exception(self) -> EvalException {
        self.0
    }
}

/// Errors used in typechecker API. Error has a span.
pub struct TypingError(EvalException);

impl TypingError {
    // TODO(nga): some errors we create, we ignore later. For example, when typechecking a union,
    //   if either variant is good, we ignore the other variant errors.
    //   So we pay for expensive error creation we ignore. Make this function cheap.
    #[cold]
    pub(crate) fn msg(message: impl Display, span: Span, codemap: &CodeMap) -> TypingError {
        TypingError(EvalException::new_anyhow(
            anyhow::Error::msg(message.to_string()),
            span,
            codemap,
        ))
    }

    #[cold]
    pub(crate) fn new(error: crate::Error, span: Span, codemap: &CodeMap) -> TypingError {
        TypingError(EvalException::new(error, span, codemap))
    }

    #[cold]
    pub(crate) fn new_anyhow(error: anyhow::Error, span: Span, codemap: &CodeMap) -> TypingError {
        TypingError(EvalException::new_anyhow(error, span, codemap))
    }

    #[cold]
    pub(crate) fn from_eval_exception(e: EvalException) -> TypingError {
        TypingError(e)
    }

    #[cold]
    pub(crate) fn into_error(self) -> crate::Error {
        self.0.into_error()
    }

    #[cold]
    pub(crate) fn into_eval_exception(self) -> EvalException {
        self.0
    }
}

/// Like [`TypingError`], but without a message or span.
pub struct TypingNoContextError;

/// Either a typing error or an internal error.
/// * Typing error means, types are not compatible.
/// * Internal error means, bug in the typechecker.
pub enum TypingOrInternalError {
    Typing(TypingError),
    Internal(InternalError),
}

impl From<TypingError> for TypingOrInternalError {
    fn from(e: TypingError) -> Self {
        TypingOrInternalError::Typing(e)
    }
}

impl From<InternalError> for TypingOrInternalError {
    fn from(e: InternalError) -> Self {
        TypingOrInternalError::Internal(e)
    }
}

pub enum TypingNoContextOrInternalError {
    Typing,
    Internal(InternalError),
}

impl From<TypingNoContextError> for TypingNoContextOrInternalError {
    fn from(_: TypingNoContextError) -> Self {
        TypingNoContextOrInternalError::Typing
    }
}

impl From<InternalError> for TypingNoContextOrInternalError {
    fn from(e: InternalError) -> Self {
        TypingNoContextOrInternalError::Internal(e)
    }
}

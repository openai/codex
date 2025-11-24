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

use std::fmt;
use std::mem;

use crate::call_stack::CallStack;
use crate::codemap::CodeMap;
use crate::codemap::FileSpan;
use crate::codemap::Span;
use crate::diagnostic::diagnostic_display;
use crate::diagnostic::WithDiagnostic;

/// An error produced by starlark.
///
/// This error is composed of an error kind, together with some diagnostic information indicating
/// where it occurred.
///
/// In order to prevent accidental conversions to `anyhow::Error`, this type intentionally does not
/// implement `std::error::Error`. That should probably change in the future.
pub struct Error(pub(crate) WithDiagnostic<ErrorKind>);

const _: () = assert!(mem::size_of::<Error>() == mem::size_of::<usize>());

impl Error {
    /// Create a new error
    #[cold]
    pub fn new_kind(kind: ErrorKind) -> Self {
        Self(WithDiagnostic::new_empty(kind))
    }

    /// Create a new error with a span
    #[cold]
    pub fn new_spanned(kind: ErrorKind, span: Span, codemap: &CodeMap) -> Self {
        Self(WithDiagnostic::new_spanned(kind, span, codemap))
    }

    /// Create a new error with no diagnostic and of kind [`ErrorKind::Other`]
    #[cold]
    pub fn new_other(e: impl Into<anyhow::Error>) -> Self {
        Self(WithDiagnostic::new_empty(ErrorKind::Other(e.into())))
    }

    /// Create a new error with no diagnostic and of kind [`ErrorKind::Native`]
    #[cold]
    pub fn new_native(e: impl Into<anyhow::Error>) -> Self {
        Self(WithDiagnostic::new_empty(ErrorKind::Native(e.into())))
    }

    /// Create a new error with no diagnostic and of kind [`ErrorKind::Value`]
    #[cold]
    pub fn new_value(e: impl Into<anyhow::Error>) -> Self {
        Self(WithDiagnostic::new_empty(ErrorKind::Value(e.into())))
    }

    /// The kind of this error
    pub fn kind(&self) -> &ErrorKind {
        self.0.inner()
    }

    /// Convert the error into the underlying kind
    pub fn into_kind(self) -> ErrorKind {
        self.0.into_inner()
    }

    pub fn has_diagnostic(&self) -> bool {
        self.0.span().is_some() || !self.0.call_stack().is_empty()
    }

    /// Convert this error into an `anyhow::Error`
    #[cold]
    pub fn into_anyhow(self) -> anyhow::Error {
        struct Wrapped(Error);

        impl fmt::Display for Wrapped {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                fmt::Display::fmt(&self.0, f)
            }
        }

        impl fmt::Debug for Wrapped {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                fmt::Debug::fmt(&self.0, f)
            }
        }

        impl std::error::Error for Wrapped {
            fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
                self.0.kind().source()
            }
        }

        anyhow::Error::new(Wrapped(self))
    }

    /// Returns a value that can be used to format this error without including the diagnostic
    /// information
    ///
    /// This is the same as [`kind`](crate::Error::kind), just a bit more explicit.
    pub fn without_diagnostic<'a>(&'a self) -> impl fmt::Debug + fmt::Display + 'a {
        self.0.inner()
    }

    pub fn span(&self) -> Option<&FileSpan> {
        self.0.span()
    }

    pub fn call_stack(&self) -> &CallStack {
        self.0.call_stack()
    }

    /// Set the span, unless it's already been set.
    pub fn set_span(&mut self, span: Span, codemap: &CodeMap) {
        self.0.set_span(span, codemap);
    }

    /// Set the `call_stack` field, unless it's already been set.
    pub fn set_call_stack(&mut self, call_stack: impl FnOnce() -> CallStack) {
        self.0.set_call_stack(call_stack);
    }

    /// Print an error to the stderr stream. If the error has diagnostic information it will use
    /// color-codes when printing.
    ///
    /// Note that this function doesn't print any context information if the error is a diagnostic,
    /// so you might prefer to use `eprintln!("{:#}"), err)` if you suspect there is useful context
    /// (although you won't get pretty colors).
    pub fn eprint(&self) {
        if self.has_diagnostic() {
            let mut stderr = String::new();
            diagnostic_display(&self.0, true, &mut stderr, true).unwrap();
            eprint!("{}", stderr);
        } else {
            eprintln!("{:#}", self)
        }
    }

    /// Change error kind to internal error.
    pub fn into_internal_error(self) -> Error {
        if let ErrorKind::Internal(_) = self.kind() {
            self
        } else {
            Error(self.0.map(ErrorKind::into_internal_error))
        }
    }
}

fn fmt_impl(this: &Error, is_debug: bool, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    if this.has_diagnostic() {
        // Not showing the context trace without `{:#}` or `{:?}` is the same thing that anyhow does
        let with_context = (f.alternate() || is_debug) && this.kind().source().is_some();
        diagnostic_display(&this.0, false, f, with_context)
    } else {
        fmt::Display::fmt(&this.without_diagnostic(), f)
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt_impl(self, false, f)
    }
}

impl fmt::Debug for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt_impl(self, true, f)
    }
}

/// The different kinds of errors that can be produced by starlark
#[non_exhaustive]
pub enum ErrorKind {
    /// An explicit `fail` invocation
    Fail(anyhow::Error),
    /// Starlark call stack overflow.
    StackOverflow(anyhow::Error),
    /// An error approximately associated with a value.
    ///
    /// Includes unsupported operations, missing attributes, things of that sort.
    Value(anyhow::Error),
    /// Errors relating to the way a function is called (wrong number of args, etc.)
    Function(anyhow::Error),
    /// Out of scope variables and similar
    Scope(anyhow::Error),
    /// Syntax error.
    Parser(anyhow::Error),
    /// Freeze errors. Should have no metadata attached
    Freeze(anyhow::Error),
    /// Indicates a logic bug in starlark
    Internal(anyhow::Error),
    /// Error from user provided native function
    /// (but not from native functions provided by starlark crate).
    /// When a native function declares `anyhow::Result<_>`
    /// return type, it is automatically converted to this variant.
    Native(anyhow::Error),
    /// Fallback option
    ///
    /// For errors produced by starlark which have not yet been assigned their own kind
    Other(anyhow::Error),
}

impl ErrorKind {
    /// The source of the error, akin to `[std::error::Error::source]`
    pub fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Fail(_) => None,
            Self::StackOverflow(_) => None,
            Self::Value(_) => None,
            Self::Function(_) => None,
            Self::Scope(_) => None,
            Self::Freeze(_) => None,
            Self::Parser(_) => None,
            Self::Internal(_) => None,
            Self::Native(e) => e.source(),
            Self::Other(e) => e.source(),
        }
    }

    /// Change type to `Internal`.
    pub(crate) fn into_internal_error(self) -> ErrorKind {
        match self {
            ErrorKind::Internal(e)
            | ErrorKind::Fail(e)
            | ErrorKind::Value(e)
            | ErrorKind::Function(e)
            | ErrorKind::Scope(e)
            | ErrorKind::Freeze(e)
            | ErrorKind::Parser(e)
            | ErrorKind::StackOverflow(e)
            | ErrorKind::Native(e)
            | ErrorKind::Other(e) => ErrorKind::Internal(e),
        }
    }
}

impl fmt::Debug for ErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Fail(s) => write!(f, "fail:{}", s),
            Self::Value(e) => fmt::Debug::fmt(e, f),
            Self::StackOverflow(e) => fmt::Debug::fmt(e, f),
            Self::Function(e) => fmt::Debug::fmt(e, f),
            Self::Scope(e) => fmt::Debug::fmt(e, f),
            Self::Freeze(e) => fmt::Debug::fmt(e, f),
            Self::Parser(e) => fmt::Debug::fmt(e, f),
            Self::Internal(e) => write!(f, "Internal error: {}", e),
            Self::Native(e) => fmt::Debug::fmt(e, f),
            Self::Other(e) => fmt::Debug::fmt(e, f),
        }
    }
}

impl fmt::Display for ErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Fail(s) => write!(f, "fail:{}", s),
            Self::StackOverflow(e) => fmt::Display::fmt(e, f),
            Self::Value(e) => fmt::Display::fmt(e, f),
            Self::Function(e) => fmt::Display::fmt(e, f),
            Self::Scope(e) => fmt::Display::fmt(e, f),
            Self::Freeze(e) => fmt::Display::fmt(e, f),
            Self::Parser(e) => fmt::Display::fmt(e, f),
            Self::Internal(e) => write!(f, "Internal error: {}", e),
            Self::Native(e) => fmt::Display::fmt(e, f),
            Self::Other(e) => fmt::Display::fmt(e, f),
        }
    }
}

impl From<anyhow::Error> for Error {
    #[cold]
    fn from(e: anyhow::Error) -> Self {
        Self(WithDiagnostic::new_empty(ErrorKind::Other(e)))
    }
}

pub trait StarlarkResultExt<T> {
    fn into_anyhow_result(self) -> anyhow::Result<T>;
}

impl<T> StarlarkResultExt<T> for crate::Result<T> {
    #[inline]
    fn into_anyhow_result(self) -> anyhow::Result<T> {
        self.map_err(Error::into_anyhow)
    }
}

#[doc(hidden)]
#[cold]
pub fn internal_error_impl(args: fmt::Arguments<'_>) -> Error {
    Error::new_kind(ErrorKind::Internal(anyhow::anyhow!("{}", args)))
}

#[doc(hidden)]
#[cold]
pub fn other_error_impl(args: fmt::Arguments<'_>) -> Error {
    Error::new_kind(ErrorKind::Other(anyhow::anyhow!("{}", args)))
}

#[doc(hidden)]
#[cold]
pub fn value_error_impl(args: fmt::Arguments<'_>) -> Error {
    Error::new_kind(ErrorKind::Value(anyhow::anyhow!("{}", args)))
}

#[doc(hidden)]
#[cold]
pub fn function_error_impl(args: fmt::Arguments<'_>) -> Error {
    Error::new_kind(ErrorKind::Function(anyhow::anyhow!("{}", args)))
}

/// Internal error of starlark.
#[macro_export]
macro_rules! internal_error {
    ($format:literal) => {
        internal_error!($format,)
    };
    ($format:literal, $($args:tt)*) => {
        $crate::error::internal_error_impl(format_args!($format, $($args)*))
    };
}

#[macro_export]
macro_rules! other_error {
    ($format:literal) => {
        other_error!($format,)
    };
    ($format:literal, $($args:tt)*) => {
        $crate::error::other_error_impl(format_args!($format, $($args)*))
    };
}

#[macro_export]
macro_rules! value_error {
    ($format:literal) => {
        value_error!($format,)
    };
    ($format:literal, $($args:tt)*) => {
        $crate::error::value_error_impl(format_args!($format, $($args)*))
    };
}

#[macro_export]
macro_rules! function_error {
    ($format:literal) => {
        function_error!($format,)
    };
    ($format:literal, $($args:tt)*) => {
        $crate::error::function_error_impl(format_args!($format, $($args)*))
    };
}

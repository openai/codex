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

use std::error::Error as StdError;
use std::fmt;

use crate::call_stack::CallStack;
use crate::codemap::CodeMap;
use crate::codemap::FileSpan;
use crate::codemap::Span;
use crate::span_display::span_display;

/// A value of type `T`, together with some diagnostic information.
///
/// Most code in starlark should be using `starlark::Error` as the error type. However, some code
/// may want to have strongly typed errors, while still being able to have diagnostics.
/// `WithDiagnostic<MyErrorType>` is the tool for that. `WithDiagnostic` is always one word in size,
/// and so can be used as an error type in performance sensitive code.
///
/// `WithDiagnostic` is `pub`, but only within the starlark crates, it's not a part of the API.
///
/// Returning a `WithDiagnostic` value guarantees that a diagnostic is actually present, ie the
/// diagnostic is not optional.
pub struct WithDiagnostic<T>(Box<WithDiagnosticInner<T>>);

struct WithDiagnosticInner<T> {
    t: T,
    diagnostic: Diagnostic,
}

impl<T> WithDiagnostic<T> {
    pub fn new_spanned(t: T, span: Span, codemap: &CodeMap) -> Self {
        Self(Box::new(WithDiagnosticInner {
            t,
            diagnostic: Diagnostic {
                span: Some(codemap.file_span(span)),
                call_stack: CallStack::default(),
            },
        }))
    }

    /// The contract of this type is normally that it actually contains diagnostic information.
    /// However, `starlark::Error` doesn't guarantee that, but it'd be convenient to use this type
    /// for it anyway. So we make an exception. Don't use this function for anything else.
    pub(crate) fn new_empty(t: T) -> Self {
        Self(Box::new(WithDiagnosticInner {
            t,
            diagnostic: Diagnostic::default(),
        }))
    }

    pub fn inner(&self) -> &T {
        &self.0.t
    }

    pub fn into_inner(self) -> T {
        self.0.t
    }

    pub fn map<U>(self, f: impl FnOnce(T) -> U) -> WithDiagnostic<U> {
        WithDiagnostic(Box::new(WithDiagnosticInner {
            t: f(self.0.t),
            diagnostic: self.0.diagnostic,
        }))
    }

    pub fn span(&self) -> Option<&FileSpan> {
        self.0.diagnostic.span.as_ref()
    }

    pub fn call_stack(&self) -> &CallStack {
        &self.0.diagnostic.call_stack
    }

    /// Set the span, unless it's already been set.
    pub fn set_span(&mut self, span: Span, codemap: &CodeMap) {
        if self.0.diagnostic.span.is_none() {
            self.0.diagnostic.span = Some(codemap.file_span(span));
        }
    }

    /// Set the `call_stack` field, unless it's already been set.
    pub fn set_call_stack(&mut self, call_stack: impl FnOnce() -> CallStack) {
        if self.0.diagnostic.call_stack.is_empty() {
            self.0.diagnostic.call_stack = call_stack();
        }
    }
}

impl<T: StdError> fmt::Display for WithDiagnostic<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Not showing the context trace without `{:#}` or `{:?}` is the same thing that anyhow does
        let with_context = f.alternate() && self.0.t.source().is_some();
        diagnostic_display(self, false, f, with_context)
    }
}

impl<T: StdError> fmt::Debug for WithDiagnostic<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        diagnostic_display(self, false, f, /* with_context */ true)
    }
}

impl<T: Into<crate::Error>> From<WithDiagnostic<T>> for crate::Error {
    fn from(e: WithDiagnostic<T>) -> Self {
        let diagnostic = e.0.diagnostic;
        let mut e: crate::Error = e.0.t.into();
        e.0.0.diagnostic = diagnostic;
        e
    }
}

/// A description of where in starlark execution the error happened.
#[derive(Debug, Default)]
struct Diagnostic {
    /// Location where the error originated.
    span: Option<FileSpan>,

    /// Call stack where the error originated.
    call_stack: CallStack,
}

impl Diagnostic {
    /// Gets annotated snippets for a [`Diagnostic`].
    fn get_display_list<'a>(
        &'a self,
        annotation_label: &'a str,
        color: bool,
    ) -> impl fmt::Display + 'a {
        span_display(
            self.span.as_ref().map(|s| s.as_ref()),
            annotation_label,
            color,
        )
    }
}

/////////////////////////////////////////////////////////////////////
// DISPLAY RELATED UTILITIES
// Since formatting these types is difficult, we reuse the Rust compiler
// variants by doing a conversion using annotate-snippets
// (https://github.com/rust-lang/annotate-snippets-rs)

pub(crate) fn diagnostic_display<T: fmt::Debug + fmt::Display>(
    d: &WithDiagnostic<T>,
    color: bool,
    f: &mut dyn fmt::Write,
    with_context: bool,
) -> fmt::Result {
    write!(f, "{}", d.call_stack())?;
    let annotation_label = format!("{}", d.inner());
    // I set color to false here to make the comparison easier with tests (coloring
    // adds in pretty strange unicode chars).
    let display_list = d.0.diagnostic.get_display_list(&annotation_label, color);
    writeln!(f, "{}", display_list)?;
    // Print out the `Caused by:` trace (if exists) and rust backtrace (if enabled).
    // The trace printed comes from an [`anyhow::Error`] that is not a [`Diagnostic`].
    if with_context {
        writeln!(f, "\n\n{:?}", d.inner())?;
    }

    Ok(())
}

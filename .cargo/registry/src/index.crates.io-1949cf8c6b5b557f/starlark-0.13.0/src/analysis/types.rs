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
use std::fmt::Display;
use std::path::Path;

use dupe::Dupe;
use serde::Serialize;

use crate::codemap::CodeMap;
use crate::codemap::FileSpan;
use crate::codemap::ResolvedSpan;
use crate::codemap::Span;

pub(crate) trait LintWarning: Display {
    fn severity(&self) -> EvalSeverity;
    fn short_name(&self) -> &'static str;
}

/// A private version of lint without the inner trait erased, useful so we can test
/// using full matching, but then erase the internal details when exporting to users.
#[derive(Debug)]
pub(crate) struct LintT<T> {
    pub location: FileSpan,
    pub original: String,
    pub problem: T,
}

/// A lint produced by `AstModule::lint`.
#[derive(Debug)]
pub struct Lint {
    /// Which code location does this lint refer to.
    pub location: FileSpan,
    /// kebab-case constant describing this issue, e.g. `missing-return`.
    pub short_name: String,
    /// Is this code highly-likely to be wrong, rather
    /// than merely stylistically non-ideal.
    pub severity: EvalSeverity,
    /// A description of the underlying problem.
    pub problem: String,
    /// The source code at [`location`](Lint::location).
    pub original: String,
}

impl Display for Lint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.location, self.problem)
    }
}

impl<T: Display> Display for LintT<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.location, self.problem)
    }
}

impl<T: LintWarning> LintT<T> {
    pub(crate) fn new(codemap: &CodeMap, span: Span, problem: T) -> Self {
        let location = codemap.file_span(span);
        Self {
            original: location.file.source_span(span).to_owned(),
            location,
            problem,
        }
    }

    pub(crate) fn erase(self) -> Lint {
        Lint {
            location: self.location,
            short_name: self.problem.short_name().to_owned(),
            severity: self.problem.severity(),
            problem: self.problem.to_string(),
            original: self.original,
        }
    }
}

/// A standardised set of severities.
#[derive(Debug, Serialize, Dupe, Clone, Copy)]
#[serde(rename_all = "lowercase")]
pub enum EvalSeverity {
    /// An error while the program was being parsed.
    Error,
    /// The program parsed, but might not execute without error.
    Warning,
    /// This could be changed, but is not a blocking problem.
    Advice,
    /// This should be ignored
    Disabled,
}

impl Display for EvalSeverity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            EvalSeverity::Error => "Error",
            EvalSeverity::Warning => "Warning",
            EvalSeverity::Advice => "Advice",
            EvalSeverity::Disabled => "Disabled",
        })
    }
}

#[derive(Debug, Clone)]
/// Potential problems that occurred while parsing a starlark program.
pub struct EvalMessage {
    /// The path to the starlark program
    pub path: String,
    /// If present, where in the program the problem occurred.
    pub span: Option<ResolvedSpan>,
    /// How severed the problem is.
    pub severity: EvalSeverity,
    /// The general name of the issue.
    pub name: String,
    /// The details of the issue, generally displayed to the user.
    pub description: String,
    /// The full error details.
    pub full_error_with_span: Option<String>,
    /// The text referred to by `.span`
    pub original: Option<String>,
}

impl Display for EvalMessage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}:", self.severity, self.path)?;
        if let Some(span) = self.span {
            write!(f, "{}", span)?;
        }
        write!(f, " {}", self.description)
    }
}

impl EvalMessage {
    /// Produce an `EvalMessage` from a `starlark::Error`
    pub fn from_error(file: &Path, err: &crate::Error) -> Self {
        if let Some(span) = err.span() {
            return Self::from_diagnostic(span, err.without_diagnostic(), err);
        }
        Self::from_any_error(file, err)
    }

    /// Create an `EvalMessage` from any kind of error
    ///
    /// Prefer to use `from_error` if at all possible.
    pub fn from_any_error(file: &Path, x: &impl std::fmt::Display) -> Self {
        Self {
            path: file.display().to_string(),
            span: None,
            severity: EvalSeverity::Error,
            name: "error".to_owned(),
            description: format!("{:#}", x),
            full_error_with_span: None,
            original: None,
        }
    }

    fn from_diagnostic(
        span: &FileSpan,
        message: impl std::fmt::Display,
        full_error: impl std::fmt::Display,
    ) -> Self {
        let original = span.source_span().to_owned();
        let resolved_span = span.resolve_span();
        Self {
            path: span.filename().to_owned(),
            span: Some(resolved_span),
            severity: EvalSeverity::Error,
            name: "error".to_owned(),
            description: format!("{:#}", message),
            full_error_with_span: Some(full_error.to_string()),
            original: Some(original),
        }
    }
}

impl From<Lint> for EvalMessage {
    fn from(x: Lint) -> Self {
        Self {
            path: x.location.filename().to_owned(),
            span: Some(x.location.resolve_span()),
            severity: x.severity,
            name: x.short_name,
            description: x.problem,
            full_error_with_span: None,
            original: Some(x.original),
        }
    }
}

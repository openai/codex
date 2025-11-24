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

use dupe::Dupe;

use crate::codemap::CodeMap;
use crate::codemap::FileSpan;
use crate::codemap::FileSpanRef;
use crate::codemap::Span;
use crate::values::FrozenRef;

#[derive(Debug, Copy, Clone, Dupe, PartialEq, Eq)]
pub(crate) struct FrozenFileSpan {
    file: FrozenRef<'static, CodeMap>,
    span: Span,
}

impl Display for FrozenFileSpan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        Display::fmt(&self.to_file_span(), f)
    }
}

impl Default for FrozenFileSpan {
    fn default() -> FrozenFileSpan {
        FrozenFileSpan::new(FrozenRef::new(CodeMap::empty_static()), Span::default())
    }
}

impl FrozenFileSpan {
    pub(crate) const fn new_unchecked(
        file: FrozenRef<'static, CodeMap>,
        span: Span,
    ) -> FrozenFileSpan {
        FrozenFileSpan { file, span }
    }

    pub(crate) fn new(file: FrozenRef<'static, CodeMap>, span: Span) -> FrozenFileSpan {
        // Check the span is valid: this will panic if the span is not valid.
        file.source_span(span);

        Self::new_unchecked(file, span)
    }

    pub(crate) fn file(&self) -> FrozenRef<'static, CodeMap> {
        self.file
    }

    pub(crate) fn span(&self) -> Span {
        self.span
    }

    pub(crate) fn end_span(&self) -> FrozenFileSpan {
        FrozenFileSpan {
            file: self.file,
            span: self.span.end_span(),
        }
    }

    pub(crate) fn file_span_ref(&self) -> FileSpanRef<'static> {
        FileSpanRef {
            file: self.file.as_ref(),
            span: self.span,
        }
    }

    pub(crate) fn to_file_span(&self) -> FileSpan {
        FileSpan {
            file: (*self.file).dupe(),
            span: self.span,
        }
    }

    pub(crate) fn merge(&self, other: &FrozenFileSpan) -> FrozenFileSpan {
        if self.file == other.file {
            FrozenFileSpan {
                file: self.file,
                span: self.span.merge(other.span),
            }
        } else {
            // We need to pick something if we merge two spans from different files.
            *self
        }
    }
}

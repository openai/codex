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

use annotate_snippets::display_list::DisplayList;
use annotate_snippets::display_list::FormatOptions;
use annotate_snippets::snippet::Annotation;
use annotate_snippets::snippet::AnnotationType;
use annotate_snippets::snippet::Slice;
use annotate_snippets::snippet::Snippet;
use annotate_snippets::snippet::SourceAnnotation;

use crate::codemap::FileSpanRef;
use crate::fast_string;

/// Gets annotated snippets.
pub fn span_display<'a>(
    span: Option<FileSpanRef<'a>>,
    annotation_label: &'a str,
    color: bool,
) -> impl Display + 'a {
    fn convert_span_to_slice<'a>(span: FileSpanRef<'a>) -> Slice<'a> {
        let region = span.resolve_span();

        // we want the source_span to capture any whitespace ahead of the diagnostic span to
        // get the column numbers correct in the DisplayList, and any trailing source code
        // on the last line for context.
        let first_line_span = span.file.line_span(region.begin.line);
        let last_line_span = span.file.line_span(region.end.line);
        let source_span = span.span.merge(first_line_span).merge(last_line_span);
        let source = span.file.source_span(source_span);

        // We want to highlight the span, which needs to be relative to source, and in
        // characters.
        // Our spans are in terms of bytes, but our resolved spans in terms of characters.
        let range_start_chars = region.begin.column;
        let range_len_chars = fast_string::len(span.source_span()).0;

        Slice {
            source,
            line_start: 1 + region.begin.line,
            origin: Some(span.file.filename()),
            fold: false,
            annotations: vec![SourceAnnotation {
                label: "",
                annotation_type: AnnotationType::Error,
                range: (range_start_chars, range_start_chars + range_len_chars),
            }],
        }
    }

    let slice = span.map(convert_span_to_slice);

    let snippet = Snippet {
        title: Some(Annotation {
            label: Some(annotation_label),
            id: None,
            annotation_type: AnnotationType::Error,
        }),
        footer: Vec::new(),
        slices: slice.map(|s| vec![s]).unwrap_or_default(),
        opt: FormatOptions {
            color,
            ..Default::default()
        },
    };

    DisplayList::from(snippet)
}

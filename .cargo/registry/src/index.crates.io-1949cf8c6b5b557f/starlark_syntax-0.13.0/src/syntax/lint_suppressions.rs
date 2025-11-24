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

use std::collections::HashMap;
use std::collections::HashSet;

use crate::codemap::CodeMap;
use crate::codemap::Pos;
use crate::codemap::Span;

static LINT_SUPPRESISON_PREFIX: &str = "starlark-lint-disable ";

#[derive(Debug, Clone)]
struct SuppressionInfo {
    /// The original span of the comment token containing the suppression
    token_span: Span,
    /// The span that this suppression effects
    effective_span: Span,
    /// Does the suppression cover the next line?
    suppress_next_line: bool,
}
#[derive(Debug, Clone)]
pub(crate) struct LintSuppressions {
    /// A map from lint short names to spans where they are suppressed
    suppressions: HashMap<String, Vec<SuppressionInfo>>,
}

impl LintSuppressions {
    /// Check if a given lint short_name and span is suppressed
    pub(crate) fn is_suppressed(&self, issue_short_name: &str, issue_span: Span) -> bool {
        self.suppressions
            .get(issue_short_name)
            .map(|suppression_spans| {
                suppression_spans.iter().any(
                    |SuppressionInfo {
                         token_span,
                         effective_span,
                         suppress_next_line,
                     }| {
                        // is this suppression that last thing in a suppress_next_line issue span? ...
                        if *suppress_next_line &&
                            // (issue_span includes line terminator)
                            (issue_span.end() - 1) == token_span.end()
                        {
                            // ... then issue is not suppressed
                            false
                        } else {
                            issue_span.intersects(*effective_span)
                        }
                    },
                )
            })
            == Some(true)
    }
}

/// State needed for parsing a block of comments
#[derive(Default)]
struct ParseState {
    token_spans: Vec<Span>,
    effective_spans: Vec<Span>,
    short_names: HashSet<String>,
    last_line: usize,
}

impl ParseState {
    fn is_empty(&self) -> bool {
        self.token_spans.is_empty()
            && self.effective_spans.is_empty()
            && self.short_names.is_empty()
    }
}

/// Parse lint suppressions for a module and build a LintSuppressions struct
pub(crate) struct LintSuppressionsBuilder {
    state: ParseState,
    suppressions: LintSuppressions,
}

impl LintSuppressionsBuilder {
    pub(crate) fn new() -> Self {
        Self {
            state: ParseState::default(),
            suppressions: LintSuppressions {
                suppressions: HashMap::new(),
            },
        }
    }

    /// Call for each comment in a block of comments
    pub(crate) fn parse_comment(
        &mut self,
        codemap: &CodeMap,
        comment: &str,
        start: usize,
        end: usize,
    ) {
        let parsed_short_names = parse_lint_suppressions(comment);
        if !parsed_short_names.is_empty() || !self.state.short_names.is_empty() {
            if let (Ok(start_pos), Ok(end_pos)) = (start.try_into(), end.try_into()) {
                let token_span = Span::new(Pos::new(start_pos), Pos::new(end_pos));
                let line = codemap.find_line(Pos::new(start_pos));
                let effective_span = codemap.line_span_trim_newline(line);
                self.state.short_names.extend(parsed_short_names);
                self.state.token_spans.push(token_span);
                self.state.effective_spans.push(effective_span);
                self.state.last_line = line;
            }
        }
    }

    /// Call after the last comment in a block of comments
    pub(crate) fn end_of_comment_block(&mut self, codemap: &CodeMap) {
        if !self.state.short_names.is_empty() {
            self.update_lint_suppressions(codemap);
        }
    }

    pub(crate) fn build(self) -> LintSuppressions {
        assert!(self.state.is_empty());
        self.suppressions
    }

    /// Update the line_suppressions hashmap with parsed lint suppressions for a block of comment
    /// Consumes and clears the ParseState
    fn update_lint_suppressions(&mut self, codemap: &CodeMap) {
        let state = std::mem::take(&mut self.state);
        let number_of_tokens = state.token_spans.len();
        let token_span = Span::merge_all(state.token_spans.into_iter());
        let mut effective_span = Span::merge_all(state.effective_spans.into_iter());
        // In case the suppression comment has preceding whitespace
        let source_before_token =
            codemap.source_span(Span::new(effective_span.begin(), token_span.begin()));
        let suppress_next_line = number_of_tokens > 1
            || effective_span == token_span
            || (effective_span.end() == token_span.end() && source_before_token.trim().is_empty());
        if suppress_next_line {
            // Expand the span to include the next line,
            // in case suppression was put on the line before the issue
            if let Some(next_line_span) = codemap.line_span_opt(state.last_line + 1) {
                effective_span = effective_span.merge(next_line_span);
            }
        }

        for name in state.short_names {
            self.suppressions
                .suppressions
                .entry(name)
                .or_default()
                .push(SuppressionInfo {
                    token_span,
                    effective_span,
                    suppress_next_line,
                });
        }
    }
}

/// Parse a single comment line and extract any lint suppressions.
fn parse_lint_suppressions(comment_line: &str) -> Vec<String> {
    let mut res = Vec::new();
    if let Some(short_names) = comment_line
        .trim_start()
        .strip_prefix(LINT_SUPPRESISON_PREFIX)
    {
        for name in short_names.split([' ', ',']) {
            let trimmed = name.trim();
            if !trimmed.is_empty() {
                res.push(trimmed.to_owned());
            }
        }
    }

    res
}

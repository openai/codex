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

use serde::Serialize;

use crate::errors::EvalMessage;
use crate::errors::EvalSeverity;

/// A JSON-deriving type that gives a stable interface to downstream types.
/// Do NOT change this type, change Message instead.
///
/// [Linter JSON format](https://www.internalfb.com/intern/wiki/Linting/adding-linters/).
#[derive(Debug, Clone, Serialize)]
pub struct LintMessage {
    path: String,
    line: Option<usize>,
    char: Option<usize>,
    code: String,
    severity: EvalSeverity,
    name: String,
    description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    original: Option<String>,
}

impl LintMessage {
    /// Construct from an [`EvalMessage`].
    pub fn new(x: EvalMessage) -> Self {
        Self {
            path: x.path,
            line: x.span.map(|x| x.begin.line + 1),
            char: x.span.map(|x| x.begin.column + 1),
            code: "STARLARK".to_owned(),
            severity: x.severity,
            name: x.name,
            description: Some(x.description),
            original: x.original,
        }
    }
}

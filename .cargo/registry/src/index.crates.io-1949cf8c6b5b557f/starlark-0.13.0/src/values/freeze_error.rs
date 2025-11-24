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

//! Error type for `Freeze` implementations. Freeze errors should only contain error messages
//! and error contexts as strings and no metadatas.
//! Conversion from anyhow is disallowed by design in order to enforce the above.

/// Alias for std::result::Result<T, FreezeError>
pub type FreezeResult<T> = std::result::Result<T, FreezeError>;

/// freeze_error type, only carries the original error message and potentially an array of contexts
#[derive(Debug)]
pub struct FreezeError {
    /// The base error message
    pub err_msg: String,
    /// The error contexts that are added to the error message
    pub contexts: Vec<String>,
}

// TODO(minglunli): Check how often this is called and if it's worth keeping
impl From<FreezeError> for anyhow::Error {
    #[cold]
    fn from(e: FreezeError) -> Self {
        let mut anyhow = anyhow::anyhow!(e.err_msg);

        for context in e.contexts.into_iter().rev() {
            anyhow = anyhow.context(context);
        }

        anyhow
    }
}

impl From<FreezeError> for starlark_syntax::Error {
    #[cold]
    fn from(e: FreezeError) -> Self {
        starlark_syntax::Error::new_kind(starlark_syntax::ErrorKind::Freeze(e.into()))
    }
}

impl FreezeError {
    /// Create a new freeze_error type
    pub fn new(err_msg: String) -> Self {
        Self {
            err_msg,
            contexts: Vec::new(),
        }
    }

    /// Add error contexts to freeze_error
    pub fn context(mut self, context: &str) -> Self {
        self.contexts.push(context.to_owned());
        self
    }
}

/// Provides the `context` method for `FreezeResult`.
///
/// This is designed to only be called on `FreezeResult` types due to the nature of freeze_error.
/// This is to prevent callers from accidentally expecting context to carry metadata.
pub trait FreezeErrorContext<T>: Sealed {
    /// Add a string error context to an existing `FreezeResult` type.
    #[track_caller]
    fn freeze_error_context(self, context: &str) -> FreezeResult<T>;
}

/// Protects against downstream implementations
pub trait Sealed: Sized {}

impl<T> Sealed for std::result::Result<T, FreezeError> {}

impl<T> FreezeErrorContext<T> for std::result::Result<T, FreezeError> {
    fn freeze_error_context(self, c: &str) -> FreezeResult<T> {
        match self {
            Ok(x) => Ok(x),
            Err(e) => Err(e.context(c)),
        }
    }
}

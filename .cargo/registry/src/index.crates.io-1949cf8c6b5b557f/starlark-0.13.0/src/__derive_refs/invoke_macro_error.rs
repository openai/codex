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

/// Trait used to convert error returned from native function into `starlark::Error`.
pub trait InvokeMacroError {
    fn into_starlark_error(self) -> crate::Error;
}

/// This implementation should not be used by starlark itself:
/// starlark native functions should not return `anyhow::Error`,
/// and should not convert to `ErrorKind::Native`.
impl InvokeMacroError for anyhow::Error {
    #[cold]
    fn into_starlark_error(self) -> crate::Error {
        crate::Error::new_native(self)
    }
}

impl InvokeMacroError for crate::Error {
    #[cold]
    fn into_starlark_error(self) -> crate::Error {
        self
    }
}

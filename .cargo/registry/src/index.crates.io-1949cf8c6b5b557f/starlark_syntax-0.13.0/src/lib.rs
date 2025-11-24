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

//! Starlark AST.

#![allow(clippy::comparison_chain)]
#![allow(clippy::comparison_to_empty)]
#![allow(clippy::len_without_is_empty)]
#![allow(clippy::needless_lifetimes)]
#![allow(clippy::new_ret_no_self)]
#![allow(clippy::should_implement_trait)]

pub use crate::error::Error;
pub use crate::error::ErrorKind;
pub use crate::error::StarlarkResultExt;

pub type Result<T> = std::result::Result<T, Error>;

pub mod call_stack;
pub mod codemap;
pub mod convert_indices;
pub(crate) mod cursors;
pub mod diagnostic;
pub mod dialect;
pub mod dot_format_parser;
pub mod error;
pub mod eval_exception;
pub mod fast_string;
pub mod frame;
pub mod golden_test_template;
pub mod lexer;
#[cfg(test)]
mod lexer_tests;
pub mod slice_vec_ext;
pub mod span_display;
pub mod syntax;

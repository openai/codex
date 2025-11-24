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

//! Utilities to test Starlark code execution, using the [`Assert`] type and top-level functions.
//!
//! There are two general approaches. You can either use the functions in this module directly, e.g.:
//!
//! ```
//! use starlark::assert;
//! assert::eq("1+2", "3");
//! ```
//!
//! Or create an [`Assert`] object, which supports the same assertions, but also let's you modify the
//! environment in which the tests are run, e.g.:
//!
//! ```
//! use starlark::assert::Assert;
//! use starlark::syntax::Dialect;
//!
//! let mut a = Assert::new();
//! a.dialect(&Dialect::Standard); // Use standard Starlark
//! a.eq("1+2", "3");
//! ```
//!
//! The tests in question may be run multiple times, in different modes, to maximise test coverage.
//! For example, execution tests are run at different garbage collection settings. Parsing tests are run
//! with both Unix and Windows newlines.

mod assert;
mod conformance;

pub use assert::*;

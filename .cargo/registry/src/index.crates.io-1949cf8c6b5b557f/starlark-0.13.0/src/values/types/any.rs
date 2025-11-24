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

//! A type [`StarlarkAny`] which can cheaply wrap any Rust value into a [`Value`].
//!
//! This is intended to be a low cost way to quickly wrap Rust types without much boilerplate.
//! For more advanced uses you should define an instance of [`StarlarkValue`].
//!
//! To use this type, usually you will return a [`StarlarkAny`] from a module function,
//! and consume it in another. As an example, we can cheaply wrap the
//! [`Duration`](std::time::Duration) type.
//!
//! ```
//! #[macro_use]
//! extern crate starlark;
//! # fn main() {
//! use std::fmt;
//! use std::time::Instant;
//!
//! use starlark::assert::Assert;
//! use starlark::environment::GlobalsBuilder;
//! use starlark::values::any::StarlarkAny;
//! use starlark::values::Value;
//!
//! #[derive(Debug)]
//! struct MyInstant(Instant);
//!
//! #[starlark_module]
//! fn globals(builder: &mut GlobalsBuilder) {
//!     fn start() -> anyhow::Result<StarlarkAny<MyInstant>> {
//!         Ok(StarlarkAny::new(MyInstant(Instant::now())))
//!     }
//!
//!     fn elapsed(x: Value) -> anyhow::Result<String> {
//!         Ok(StarlarkAny::<MyInstant>::get(x)
//!             .unwrap()
//!             .0
//!             .elapsed()
//!             .as_secs_f64()
//!             .to_string())
//!     }
//! }
//!
//! let mut a = Assert::new();
//! a.globals_add(globals);
//! a.pass(
//!     r#"
//! instant = start()
//! print(elapsed(instant))
//! "#,
//! );
//! # }
//! ```

use std::fmt;
use std::fmt::Debug;

use allocative::Allocative;
use starlark_derive::starlark_value;
use starlark_derive::NoSerialize;

use crate as starlark;
use crate::any::ProvidesStaticType;
use crate::values::AllocValue;
use crate::values::Heap;
use crate::values::StarlarkValue;
use crate::values::Value;
use crate::values::ValueLike;

/// A type that can be passed around as a Starlark [`Value`], but in most
/// ways is uninteresting/opaque to Starlark. Constructed with
/// [`new`](StarlarkAny::new) and decomposed with [`get`](StarlarkAny::get).
///
/// This is version for "simple" values (not requiring trace during GC).
/// For "complex" version check
/// [`StarlarkAnyComplex`](crate::values::types::any_complex::StarlarkAnyComplex).
#[derive(ProvidesStaticType, NoSerialize, Allocative, derive_more::Display)]
#[allocative(bound = "")]
#[display("{:?}", self)]
pub struct StarlarkAny<T: Debug + Send + Sync + 'static>(
    #[allocative(skip)] // TODO(nga): do not skip.
    pub  T,
);

#[starlark_value(type = "any")]
impl<'v, T: Debug + Send + Sync + 'static> StarlarkValue<'v> for StarlarkAny<T> {
    type Canonical = Self;
}

impl<'v, T: Debug + Send + Sync + 'static> AllocValue<'v> for StarlarkAny<T> {
    fn alloc_value(self, heap: &'v Heap) -> Value<'v> {
        heap.alloc_simple(self)
    }
}

impl<T: Debug + Send + Sync + 'static> Debug for StarlarkAny<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        Debug::fmt(&self.0, f)
    }
}

impl<T: Debug + Send + Sync + 'static> StarlarkAny<T> {
    /// Create a new [`StarlarkAny`] value. Such a value can be allocated on a heap with
    /// `heap.alloc(StarlarkAny::new(x))`.
    pub fn new(x: T) -> Self {
        StarlarkAny(x)
    }

    /// Extract from a [`Value`] that contains a [`StarlarkAny`] underneath. Returns [`None`] if
    /// the value does not match the expected type.
    pub fn get<'v>(x: Value<'v>) -> Option<&'v T> {
        let x: &StarlarkAny<T> = x.downcast_ref()?;
        Some(&x.0)
    }
}

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

//! The boolean type (`False` and `True`).
//!
//! Can be created with [`Value::new_bool`](crate::values::Value::new_bool)
//! and unwrapped with [`Value::unpack_bool`](crate::values::Value::unpack_bool).
//! Unlike most Starlark values, these aren't actually allocated on the [`Heap`](crate::values::Heap),
//! but as special values.

mod alloc;
pub(crate) mod globals;
mod type_repr;
mod unpack;
pub(crate) mod value;

pub use value::StarlarkBool;
pub use value::BOOL_TYPE;

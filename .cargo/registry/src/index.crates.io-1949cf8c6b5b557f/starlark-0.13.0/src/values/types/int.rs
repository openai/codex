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

//! The integer type.
//!
//! For small values, we try not to allocate on the [`Heap`](crate::values::Heap),
//! but instead use special values. If the value doesn't fit in the special representation,
//! we use [`BigInt`](num_bigint::BigInt) to store it.

pub(crate) mod globals;
mod i32;
pub(crate) mod inline_int;
pub(crate) mod int_or_big;
pub(crate) mod pointer_i32;
mod tests;

pub use pointer_i32::INT_TYPE;

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

//! The list type, an immutable sequence of values.

pub(crate) mod alloc;
pub(crate) mod globals;
pub(crate) mod refs;
pub(crate) mod rust_tuple;
pub(crate) mod unpack;
pub(crate) mod value;

pub use crate::values::types::tuple::alloc::AllocTuple;
pub use crate::values::types::tuple::refs::FrozenTupleRef;
pub use crate::values::types::tuple::refs::TupleRef;
pub use crate::values::types::tuple::unpack::UnpackTuple;

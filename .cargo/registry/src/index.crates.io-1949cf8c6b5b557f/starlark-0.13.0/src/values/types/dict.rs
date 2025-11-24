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

//! The dictionary type, a mutable associative-map, which iterates in insertion order.

mod alloc;

mod dict_type;
pub(crate) mod globals;
pub(crate) mod methods;
mod refs;
mod traits;
pub(crate) mod unpack;
pub(crate) mod value;

pub use crate::values::dict::alloc::AllocDict;
pub use crate::values::dict::dict_type::DictType;
pub use crate::values::dict::refs::DictMut;
pub use crate::values::dict::refs::DictRef;
pub use crate::values::dict::refs::FrozenDictRef;
pub use crate::values::dict::unpack::UnpackDictEntries;
pub use crate::values::dict::value::Dict;

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

//! The string type. All strings must be valid UTF8.

mod alloc_unpack;
pub(crate) mod dot_format;
pub(crate) mod globals;
pub(crate) mod intern;
pub(crate) mod interpolation;
pub(crate) mod iter;
pub(crate) mod methods;
pub(crate) mod repr;
pub(crate) mod simd;
pub(crate) mod str_type;

pub use crate::values::types::string::str_type::StarlarkStr;
pub use crate::values::types::string::str_type::STRING_TYPE;

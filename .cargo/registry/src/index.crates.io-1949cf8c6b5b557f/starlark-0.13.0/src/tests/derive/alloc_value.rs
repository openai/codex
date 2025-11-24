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

//! Tests for `#[derive(AllocValue)]`.

#![allow(dead_code)] // Only check it compiles.

use starlark_derive::AllocFrozenValue;

use crate as starlark;
use crate::values::type_repr::StarlarkTypeRepr;
use crate::values::AllocValue;

#[derive(StarlarkTypeRepr, AllocValue, AllocFrozenValue)]
enum AllocNoVariant {}

#[derive(StarlarkTypeRepr, AllocValue, AllocFrozenValue)]
enum AllocOneVariant {
    Int(u32),
}

#[derive(StarlarkTypeRepr, AllocValue, AllocFrozenValue)]
enum AllocTwoVariants {
    Int(u32),
    String(String),
}

#[derive(StarlarkTypeRepr, AllocValue, AllocFrozenValue)]
enum AllocWithLifetime<'v> {
    String(&'v str),
}

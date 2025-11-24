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

#![allow(dead_code)] // Only check it compiles.

use allocative::Allocative;
use starlark_derive::starlark_value;
use starlark_derive::NoSerialize;
use starlark_derive::ProvidesStaticType;

use crate as starlark;
use crate::values::StarlarkValue;
use crate::values::Value;

#[derive(
    Debug,
    derive_more::Display,
    NoSerialize,
    ProvidesStaticType,
    Allocative
)]
#[display("ValueWithLifetimeParam")]
struct ValueWithLifetimeParam<'v>(Value<'v>);
#[derive(
    Debug,
    derive_more::Display,
    NoSerialize,
    ProvidesStaticType,
    Allocative
)]
#[display("ValueWithoutParam")]
struct ValueWithoutParam(String);

#[starlark_value(type = "ValueWithLifetimeParam", StarlarkTypeRepr, UnpackValue)]
impl<'v> StarlarkValue<'v> for ValueWithLifetimeParam<'v> {}

#[starlark_value(type = "ValueWithoutParam", StarlarkTypeRepr, UnpackValue)]
impl<'v> StarlarkValue<'v> for ValueWithoutParam {}

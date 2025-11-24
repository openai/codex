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

use allocative::Allocative;
use starlark_derive::starlark_module;
use starlark_derive::starlark_value;
use starlark_derive::NoSerialize;
use starlark_derive::ProvidesStaticType;

use crate as starlark;
use crate::assert::Assert;
use crate::environment::GlobalsBuilder;
use crate::values::StarlarkValue;

#[derive(
    Debug,
    derive_more::Display,
    ProvidesStaticType,
    NoSerialize,
    Allocative
)]
#[display("foo")]
struct Foo;

#[starlark_value(type = "Foo")]
impl<'v> StarlarkValue<'v> for Foo {}

#[starlark_module]
fn type_annotation_functions(globals: &mut GlobalsBuilder) {
    #[starlark(as_type = Foo)]
    fn foo(x: i32) -> anyhow::Result<i32> {
        Ok(x)
    }
}

#[test]
fn test_type_annotation() {
    let mut a = Assert::new();
    a.globals_add(type_annotation_functions);
    a.eq("'Foo'", "foo.type");
}

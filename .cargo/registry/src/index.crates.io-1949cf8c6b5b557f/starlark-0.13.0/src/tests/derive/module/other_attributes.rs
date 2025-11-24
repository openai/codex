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

//! In these tests, parameters are declared as unused,
//! attributes should be preserved and no warnings should be emitted.

use starlark_derive::starlark_module;

use crate as starlark;
use crate::environment::GlobalsBuilder;
use crate::environment::MethodsBuilder;
use crate::values::none::NoneType;

#[starlark_module]
fn test_other_attributes_in_globals(globals: &mut GlobalsBuilder) {
    fn test_global(#[allow(unused_variables)] foo: u32) -> anyhow::Result<NoneType> {
        Ok(NoneType)
    }
}

#[starlark_module]
fn test_other_attributes_in_methods(methods: &mut MethodsBuilder) {
    fn test_method(#[allow(unused_variables)] this: u32) -> anyhow::Result<NoneType> {
        Ok(NoneType)
    }
}

#[starlark_module]
fn test_other_attributes_in_atributes(methods: &mut MethodsBuilder) {
    #[starlark(attribute)]
    fn test_attribute(
        // TODO(nga): this marker is no-op.
        #[allow(unused_variables)] this: u32,
    ) -> starlark::Result<NoneType> {
        Ok(NoneType)
    }
}

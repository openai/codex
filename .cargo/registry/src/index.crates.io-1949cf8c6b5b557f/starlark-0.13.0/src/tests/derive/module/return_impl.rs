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

use starlark_derive::starlark_module;

use crate as starlark;
use crate::environment::GlobalsBuilder;
use crate::environment::MethodsBuilder;
use crate::values::AllocValue;
use crate::values::Value;

#[starlark_module]
fn _test_return_impl_alloc_value(globals: &mut GlobalsBuilder) {
    fn func<'v>(v: Value<'v>) -> anyhow::Result<impl AllocValue<'v>> {
        Ok(v)
    }
}

#[starlark_module]
fn _test_return_impl_alloc_value_for_attr(methods: &mut MethodsBuilder) {
    #[starlark(attribute)]
    fn attr<'v>(this: Value<'v>) -> starlark::Result<impl AllocValue<'v>> {
        Ok(this)
    }
}

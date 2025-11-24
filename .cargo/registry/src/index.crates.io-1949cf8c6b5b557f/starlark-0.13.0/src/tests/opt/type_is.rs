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

//! Test for type-is optimizations.

use starlark_derive::starlark_module;

use crate as starlark;
use crate::assert::Assert;
use crate::environment::GlobalsBuilder;
use crate::eval::compiler::def::Def;
use crate::eval::compiler::def::FrozenDef;
use crate::eval::compiler::def_inline::InlineDefBody;
use crate::values::Value;
use crate::values::ValueLike;

#[starlark_module]
fn globals(builder: &mut GlobalsBuilder) {
    fn returns_type_is<'v>(value: Value<'v>) -> anyhow::Result<bool> {
        Ok(if let Some(def) = value.downcast_ref::<FrozenDef>() {
            matches!(
                def.def_info.inline_def_body,
                Some(InlineDefBody::ReturnTypeIs(..))
            )
        } else if let Some(def) = value.downcast_ref::<Def>() {
            matches!(
                def.def_info.inline_def_body,
                Some(InlineDefBody::ReturnTypeIs(..))
            )
        } else {
            panic!("not def")
        })
    }
}

#[test]
fn returns_type_is() {
    let mut a = Assert::new();
    a.globals_add(globals);

    a.module(
        "types.star",
        "\
def is_list(x):
  return type(x) == type([])
",
    );

    a.pass(
        "\
load('types.star', 'is_list')
assert_true(returns_type_is(is_list))
assert_true(is_list([]))
assert_false(is_list({}))
    ",
    );
}

#[test]
fn does_not_return_type_is() {
    let mut a = Assert::new();
    a.globals_add(globals);
    a.pass(
        "\
def is_not_list(x):
  return type(x) != type([])

def something_else(x, y):
  return type(x) == type([])

assert_false(returns_type_is(is_not_list))
assert_false(returns_type_is(something_else))
    ",
    );
}

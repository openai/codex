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

//! Expose starlark-rust internals in starlark.
//!
//! None of this code is meant to be used in production. Can be changed any time.

use starlark_derive::starlark_module;

use crate as starlark;
use crate::environment::GlobalsBuilder;
use crate::typing::Ty;
use crate::values::Value;

#[starlark_module]
fn starlark_rust_internal_members(globals: &mut GlobalsBuilder) {
    fn ty_of_value_debug(value: Value) -> anyhow::Result<String> {
        Ok(format!("{:?}", Ty::of_value(value)))
    }
}

pub(crate) fn register_internal(globals: &mut GlobalsBuilder) {
    globals.namespace_no_docs("starlark_rust_internal", |s| {
        starlark_rust_internal_members(s)
    });
}

#[cfg(test)]
mod tests {
    use crate::assert;

    #[test]
    fn test_ty_of_value_debug() {
        assert::pass("print(starlark_rust_internal.ty_of_value_debug(1))");
    }
}

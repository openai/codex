/*
 * Copyright 2019 The Starlark in Rust Authors.
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
use crate::values::bool::StarlarkBool;
use crate::values::Value;

#[starlark_module]
pub(crate) fn register_bool(globals: &mut GlobalsBuilder) {
    /// A boolean representing true.
    const True: bool = true;

    /// A boolean representing false.
    const False: bool = false;

    /// [bool](
    /// https://github.com/bazelbuild/starlark/blob/master/spec.md#bool
    /// ): returns the truth value of any starlark value.
    ///
    /// ```
    /// # starlark::assert::all_true(r#"
    /// bool() == False
    /// bool([]) == False
    /// bool([1]) == True
    /// bool(True) == True
    /// bool(False) == False
    /// bool(None) == False
    /// bool(bool) == True
    /// bool(1) == True
    /// bool(0) == False
    /// bool({}) == False
    /// bool({1:2}) == True
    /// bool(()) == False
    /// bool((1,)) == True
    /// bool("") == False
    /// bool("1") == True
    /// # "#);
    /// ```
    #[starlark(as_type = StarlarkBool, speculative_exec_safe)]
    fn bool(#[starlark(require = pos)] x: Option<Value>) -> anyhow::Result<bool> {
        match x {
            None => Ok(false),
            Some(x) => Ok(x.to_bool()),
        }
    }
}

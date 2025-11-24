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
use crate::values::types::num::value::Num;
use crate::values::types::num::value::NumRef;

#[starlark_module]
pub(crate) fn register_num(globals: &mut GlobalsBuilder) {
    /// Take the absolute value of an int.
    ///
    /// ```
    /// # starlark::assert::all_true(r#"
    /// abs(0)   == 0
    /// abs(-10) == 10
    /// abs(10)  == 10
    /// abs(10.0) == 10.0
    /// abs(-12.34) == 12.34
    /// # "#);
    /// ```
    fn abs(#[starlark(require = pos)] x: NumRef) -> anyhow::Result<Num> {
        match x {
            NumRef::Int(a) => Ok(Num::Int(a.abs())),
            NumRef::Float(a) => Ok(Num::Float(a.0.abs())),
        }
    }
}

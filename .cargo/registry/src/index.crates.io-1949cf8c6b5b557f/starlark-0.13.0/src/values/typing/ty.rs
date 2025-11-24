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

use allocative::Allocative;
use starlark_derive::starlark_value;
use starlark_derive::NoSerialize;
use starlark_derive::ProvidesStaticType;

use crate as starlark;
use crate::typing::Ty;
use crate::typing::TyBasic;
use crate::values::StarlarkValue;

/// Type of type.
#[doc(hidden)]
#[derive(
    Debug,
    derive_more::Display,
    Allocative,
    ProvidesStaticType,
    NoSerialize
)]
#[display("type")]
pub enum AbstractType {}

#[starlark_value(type = "type")]
impl<'v> StarlarkValue<'v> for AbstractType {
    fn get_type_starlark_repr() -> Ty {
        Ty::basic(TyBasic::Type)
    }

    fn eval_type(&self) -> Option<Ty> {
        // This is unreachable, but this function is needed
        // so `TyStarlarkValue` could think this is a type".
        match *self {}
    }
}

#[cfg(test)]
mod tests {
    use crate::assert;

    #[test]
    fn test_isinstance() {
        assert::is_true("isinstance(int, type)");
        assert::is_false("isinstance(1, type)");
        assert::is_true("isinstance(list[str], type)");
        assert::is_true("isinstance(eval_type(list), type)");
    }

    #[test]
    fn test_pass() {
        assert::pass(
            r#"
def accepts_type(t: type):
    pass

def test():
    accepts_type(int)
    accepts_type(list[str])
    accepts_type(None | int)

test()
"#,
        );
    }

    #[test]
    fn test_fail_compile_time() {
        assert::fail(
            r#"
def accepts_type(t: type):
    pass

def test():
    accepts_type(1)
"#,
            "Expected type `type` but got `int`",
        );
    }
}

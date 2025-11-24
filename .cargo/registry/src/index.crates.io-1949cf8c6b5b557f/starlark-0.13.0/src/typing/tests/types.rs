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

//! Type-related operations.

use crate::typing::tests::TypeCheck;

#[test]
fn test_type_alias() {
    TypeCheck::new().ty("x").check(
        "type_alias",
        r#"
MyList = list[int]

def f(x: MyList):
    pass
"#,
    );
}

#[test]
fn test_incorrect_type_dot() {
    TypeCheck::new().check(
        "incorrect_type_dot",
        r#"
def foo(x: list.foo.bar):
    pass
"#,
    );
}

#[test]
fn test_function_as_type_bit_or() {
    TypeCheck::new().ty("t").check(
        "function_as_type_bit_or",
        r#"
def test():
    # This test should work even if `t` is global. There's a bug in test framework somewhere.
    t = int | str
"#,
    );
}

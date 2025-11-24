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

//! Test for callables, but not calls.

use crate::typing::tests::TypeCheck;

#[test]
fn test_callable_with_args() {
    TypeCheck::new().check(
        "callable_with_args",
        r#"
def accept_f(x: typing.Callable[[int, str], str]):
    pass

def good_function(x: int, y: str) -> str:
    return ""

def bad_function(x: int, y: bool) -> str:
    return ""

def test():
    accept_f(good_function)
    accept_f(bad_function)
"#,
    );
}

#[test]
fn test_callable_named() {
    TypeCheck::new().check(
        "callable_named",
        r#"
def good_function_pos_or_named(x: str, y: int) -> None:
    pass

def good_function_named_only(*, x: str, y: int) -> None:
    pass

def bad_function_wrong_types(x: bool, y: list) -> None:
    pass

def bad_function_missing_params(x: str) -> None:
    pass

def bad_function_extra_params(x: str, y: int, z: int) -> None:
    pass

def test():
    accepts_callable_named_xy(good_function_pos_or_named)
    accepts_callable_named_xy(good_function_named_only)
    accepts_callable_named_xy(bad_function_wrong_types)
    accepts_callable_named_xy(bad_function_missing_params)
    accepts_callable_named_xy(bad_function_extra_params)
"#,
    );
}

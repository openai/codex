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

use crate::typing::tests::TypeCheck;

#[test]
fn test_int_mul_list() {
    // TODO(nga): fix.
    TypeCheck::new().ty("x").check(
        "int_mul_list",
        r#"
def test():
    x = 1 * ["a"]
"#,
    );
}

#[test]
fn test_list_append() {
    TypeCheck::new().ty("x").check(
        "list_append",
        r#"
def test():
    # Type of `x` should be inferred as list of either `int` or `str`.
    x = []
    x.append(1)
    x.append("")
"#,
    );
}

#[test]
fn test_list_append_bug() {
    // TODO(nga): fix.
    TypeCheck::new().ty("x").check(
        "list_append_bug",
        r#"
def test():
    x = []
    x.append(x)
"#,
    );
}

#[test]
fn test_list_function() {
    TypeCheck::new().ty("x").check(
        "list_function",
        r#"
def test():
    x = list([1, 2])
"#,
    );
}

#[test]
fn test_list_less() {
    TypeCheck::new().check(
        "list_less",
        r#"
def test(x: list[str], y: list[str]) -> bool:
    return x < y
"#,
    );
}

#[test]
fn test_list_bin_op() {
    TypeCheck::new().ty("x").ty("y").ty("z").check(
        "list_bin_op",
        r#"
def test(a: list[str]):
    x = a + a
    y = a * 3
    z = 3 * a
"#,
    );
}

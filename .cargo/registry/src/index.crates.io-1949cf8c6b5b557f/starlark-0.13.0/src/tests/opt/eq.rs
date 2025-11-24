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

//! Test for `a == b` optimizations.

use crate::tests::bc::golden::bc_golden_test;

#[test]
fn test_eq_int() {
    bc_golden_test(
        "eq_int",
        r#"
def test(x):
    return x == 10
"#,
    );
}

#[test]
fn test_eq_str() {
    bc_golden_test(
        "eq_str",
        r#"
def test(x):
    return x == "hello"
"#,
    );
}

#[test]
fn test_eq_short_str_is_ptr_eq() {
    bc_golden_test(
        "eq_short_str",
        r#"
def test(x):
    return x == "a"
"#,
    );
}

#[test]
fn test_eq_bool_is_ptr_eq() {
    bc_golden_test(
        "eq_bool",
        r#"
def test(x):
    return x == True
"#,
    );
}

/// Enum values do not override `equals` method, so we can use pointer equality.
#[test]
fn test_eq_enum_is_ptr_eq() {
    bc_golden_test(
        "eq_enum",
        r#"
Color = enum("RED", "GREEN", "BLUE")

def test(x):
    return x == Color("RED")
"#,
    );
}

#[test]
fn test_eq_const() {
    bc_golden_test(
        "eq_const",
        r#"
S = struct(a = 2)

def test(x):
    return x == S
"#,
    );
}

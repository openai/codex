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

//! Optimizer tests.

mod constant_folding;
mod def_inline;
mod eq;
mod if_rand;
mod list_add;
mod speculative_exec;
mod type_is;
mod types;

use crate::tests::bc::golden::bc_golden_test;

#[test]
fn test_type_is_inlined() {
    bc_golden_test(
        "opt_type_is_inlined",
        r#"
def is_list(x):
    return type(x) == type([])

def test(x):
    return is_list(x)
    "#,
    )
}

#[test]
fn test_private_forward_mutable_module_vars_inlined() {
    bc_golden_test(
        "opt_private_forward_mutable_module_vars_inlined",
        r#"
def test():
    # Reference to module variable should be replaced with constant
    return _private_forward_mutable

_private_forward_mutable = {1: 2}
"#,
    );
}

#[test]
fn test_same_module_struct_getattr_inlined() {
    bc_golden_test(
        "opt_same_module_struct_getattr_inlined",
        r#"
def test():
    return _s.f

_s = struct(f = 1)
"#,
    );
}

#[test]
fn test_list_plus_list() {
    bc_golden_test(
        "opt_list_plus_list",
        r#"
L = [1, 2]

def test():
    return L + [1]
"#,
    );
}

#[test]
fn test_empty_iterable_optimized_away() {
    bc_golden_test(
        "opt_empty_iterable_optimized_away",
        r#"
L = []
def test():
    for x in L:
        print(x)
"#,
    );
}

#[test]
fn test_unreachable_code_optimized_away() {
    bc_golden_test(
        "opt_unreachable_code_optimized_away",
        r#"
def test():
    if True:
        return
    fail("unreachable")
"#,
    );
}

#[test]
fn test_recursion() {
    bc_golden_test(
        "opt_recursion",
        // Test inlining does not fail here.
        "def test(): return test()",
    );
}

#[test]
fn test_mutual_recursion() {
    // Just check we do not enter an infinite recursion in the optimizer here.
    bc_golden_test(
        "opt_mutual_recursion",
        r#"
def test():
    return g()

def g():
    return test()
"#,
    );
}

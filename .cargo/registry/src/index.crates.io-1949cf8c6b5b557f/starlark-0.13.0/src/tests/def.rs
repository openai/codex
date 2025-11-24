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

//! Test for `def` and `lambda`.

use crate::assert;
use crate::assert::Assert;
use crate::environment::Module;
use crate::eval::Evaluator;

#[test]
fn test_lambda() {
    assert::is_true("(lambda x: x)(1) == 1");
    assert::is_true("(lambda x: (x == 1))(1)");
    assert::is_true(
        "
xs = [lambda x: x + y for y in [1,2,3]]
ys = [lambda x: x + y for y in [4,5,6]]
[xs[1](0),ys[1](0)] == [3,6]",
    );
}

#[test]
fn test_frozen_lambda() {
    let mut a = Assert::new();
    a.module(
        "lam",
        r#"
def my_func(a):
    return lambda b: a + b
add18 = my_func(18)
# This test used to fail if a GC happened, so add one
garbage_collect()
"#,
    );
    a.pass(
        r#"
load("lam", "add18")
assert_eq(add18(24), 42)
"#,
    );
}

#[test]
fn test_nested_def_1() {
    assert::is_true(
        "
def foo(x):
    def bar(y):
        return x+y
    return bar(x)
foo(8) == 16",
    );
}

#[test]
fn test_nested_def_2() {
    assert::is_true(
        "
def squarer():
    x = [0]
    def f():
        x[0] += 1
        return x[0]*x[0]
    return f
sq = squarer()
[sq(), sq(), sq(), sq()] == [1,4,9,16]",
    );
}

#[test]
fn test_nested_def_3() {
    assert::is_true(
        "
def f(x):
    def g(y):
        return lambda z: x + y + z
    return g
f(1)(2)(3) == 6",
    );
}

#[test]
fn test_lambda_capture_from_module() {
    assert::is_true(
        "
f = lambda y: x + y
x = 100
f(42) == 142
",
    );
}

#[test]
fn test_lambda_capture_from_def() {
    assert::is_true(
        "
def inside():
    f = lambda y: x + y
    x = 100
    return f(42) == 142
inside()
",
    );
}

#[test]
fn test_lambda_capture_reassigned_from_def() {
    assert::is_true(
        "
def inside():
    x = 100
    f = lambda y: x + y
    x = 200
    return f(42) == 242
inside()
",
    );
}

#[test]
fn test_def_freeze() {
    let mut a = Assert::new();
    a.module(
        "f.bzl",
        r#"
def f(g):
    g(1)"#,
    );
    a.is_true(
        r#"
load('f.bzl', 'f')
x = []
def g(y):
    x.append(y)
f(g)
x == [1]"#,
    );
}

#[test]
fn test_frozen_lambda_nest() {
    let mut a = Assert::new();
    let m = a.module(
        "a",
        r#"
def outer_function(x):
    return x["test"]

def function(x):
    def inner_function():
        return outer_function(x)
    return inner_function()

value = {"test": "hello"}
"#,
    );
    let f = m.get("function").unwrap();
    let x = m.get("value").unwrap();
    let module = Module::new();
    let mut eval = Evaluator::new(&module);
    let res = eval.eval_function(f.value(), &[x.value()], &[]).unwrap();
    assert_eq!(res.to_str(), "hello");
}

#[test]
fn test_context_captured() {
    let mut a = Assert::new();
    a.module("f.bzl", "x = 17\ndef f(): return x");
    // Import `f` but do not import `x`
    a.is_true("load('f.bzl', 'f')\nf() == 17");
}

#[test]
fn test_lambda_errors() {
    // Test from https://github.com/facebook/starlark-rust/issues/36
    assert::fail("lambda a,a:a", "duplicated parameter name");
}

#[test]
fn test_lambda_errors_nested() {
    // Test from https://issues.oss-fuzz.com/issues/369003809
    assert::fail("lambda: lambda a,a:a", "duplicated parameter name");
    assert::fail("[lambda a,a:a]", "duplicated parameter name");
}

#[test]
fn test_double_capture_and_freeze() {
    let mut a = Assert::new();
    a.module(
        "x.bzl",
        r#"
def f(x):
    # `x` is captured by `g` and then frozen.
    def g():
        # When `h` is instantiated, `x` is already captured and frozen.
        def h():
            return noop(x)
        return h

    return g

G = f(1)
     "#,
    );

    a.pass("load('x.bzl', 'G')\nG()");
}

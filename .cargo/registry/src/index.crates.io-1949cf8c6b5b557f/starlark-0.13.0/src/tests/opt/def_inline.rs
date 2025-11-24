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

//! Test function bodies inlined.

use crate::assert::Assert;
use crate::eval::bc::opcode::BcOpcode;
use crate::eval::compiler::def::FrozenDef;
use crate::tests::bc::golden::bc_golden_test;
use crate::values::ValueLike;

#[test]
fn test_def_const_inlined() {
    bc_golden_test(
        "def_inline_const_inlined",
        r#"
def trivial():
    return 10

def test():
    return trivial()
"#,
    )
}

#[test]
fn test_def_list_inlined() {
    bc_golden_test(
        "def_inline_list_inlined",
        r#"
def test():
    return returns_list()

# Also test function is inlined if it is defined after the caller.
def returns_list():
    return [10, True]
"#,
    )
}

#[test]
fn test_dict_inlined() {
    bc_golden_test(
        "def_inline_dict_inlined",
        r#"
def returns_dict():
    # This should fail at runtime.
    return {[]: 10}

def test():
    return returns_dict()
"#,
    );
}

#[test]
fn test_do_not_inline_functions_with_return_type() {
    bc_golden_test(
        "def_inline_return_type_inlined",
        r#"
def smth() -> str:
    return "10"

def test():
    # This call should not be inlined.
    return smth()
"#,
    );
}

#[test]
fn test_dict_inlined_call_stack() {
    let mut a = Assert::new();
    a.module("f.bzl", "def f(): return {[]: 10}");
    // For now inlining doesn't work within one module, so do different modules.
    let m_g = a.module("g.bzl", "load('f.bzl', 'f')\ndef g(): return f()");
    let m_h = a.module("h.bzl", "load('g.bzl', 'g')\ndef h(): return g()");

    // Check `f` is inlined into `g` and `h`.
    for (m, f) in [(m_g, "g"), (m_h, "h")] {
        let f = m.get(f).unwrap();
        let f = f.value().downcast_ref::<FrozenDef>().unwrap();
        assert_eq!(
            BcOpcode::ListNew,
            f.bc().instrs.opcodes().as_slice()[0],
            "in `{}`",
            f,
        );
    }

    let error = a.fail(
        r"
load('h.bzl', 'h')
h()
",
        "",
    );

    assert_eq!(
        r"
Traceback (most recent call last):
  * assert.bzl:3, in <module>
      h()
  * h.bzl.bzl:2, in h
      def h(): return g()
  * g.bzl.bzl:2, in g
      def g(): return f()
error: Value of type `list` is not hashable
 --> f.bzl.bzl:1:18
  |
1 | def f(): return {[]: 10}
  |                  ^^
  |
",
        &format!("\n{:#}", error)
    );
}

#[test]
fn test_do_not_inline_too_large_functions() {
    let mut a = Assert::new();
    a.module("a0.bzl", "def a0(): return noop()");
    for i in 1..100 {
        // `a_17()` is inlined into `a_18()`.
        // If inlining is not limited, this test will run for very long time
        // and eat up all the memory.
        let i_1 = i - 1;
        a.module(
            &format!("a{i}.bzl"),
            &format!(
                "load('a{i_1}.bzl', 'a{i_1}')\n\
                def a{i}(): return a{i_1}() or a{i_1}()"
            ),
        );
    }
}

#[test]
fn test_calls_with_const_args_inlined() {
    bc_golden_test(
        "def_inline_const_args_inlined",
        r#"
def foo(x, y):
    return noop(y, x)

def test():
    # This call should be inlined.
    return foo(10, True)
"#,
    );
}

#[test]
fn test_calls_with_locals_inlined() {
    bc_golden_test(
        "def_inline_locals_inlined",
        r#"
def foo(x, y):
    return noop(y, x)

def test(x, y):
    # This call should be inlined.
    return foo(y, x)
"#,
    );
}

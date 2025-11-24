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

//! Test call expression and parameter binding.

use crate::assert;
use crate::assert::Assert;

#[test]
fn funcall_test() {
    fn f(x: &str) -> String {
        format!(
            "
def f1():
  return 1

def f2(a): return a

def f3(a, b, c):
   return a + b + c

def f4(a, *args):
    r = a
    for i in args:
      r += i
    return r

def f5(a, **kwargs): return kwargs
def f6(*args): return args

def rec1(): rec1()

def rec2(): rec3()
def rec3(): rec4()
def rec4(): rec5()
def rec5(): rec6()
def rec6(): rec2()
{}",
            x
        )
    }
    assert::is_true(&f("(f1() == 1)"));
    assert::is_true(&f("(f2(2) == 2)"));
    assert::is_true(&f("(f3(1, 2, 3) == 6)"));
    assert::is_true(&f("(f4(1, 2, 3) == 6)"));
    assert::is_true(&f("(f5(2) == {})"));
    assert::is_true(&f("(f5(a=2) == {})"));
    assert::is_true(&f("(f5(1, b=2) == {'b': 2})"));
    assert::is_true(&f("(f6(1, 2, 3) == (1, 2, 3))"));
    // Recursion limit
    assert::fail(&f("rec1()"), "Starlark call stack overflow");
    assert::fail(&f("rec2()"), "Starlark call stack overflow");
    // multiple argument with the same name should not be allowed
    assert::fail("def f(a, a=2): pass", "duplicated parameter");
    // Invalid order of parameter
    assert::is_true("def f(a, *args, b): return b\nf(1, b=True)");
    assert::is_true("def f(a, *args, b=True): return b\nf(1)");
    assert::is_true("NAME=True\ndef f(*args, pkg=NAME, **kwargs): return pkg\nf()");
    assert::is_true("def f(*args, pkg=False, **kwargs): return pkg\nf(pkg=True)");
    assert::is_true("def f(a, b=1, *args, c=False): return c\nf(a=1,c=True)");
    assert::fail("def f(a, **kwargs, b=1): pass", "Parameter after kwargs");
    assert::fail(
        "def f(a, b=1, **kwargs, c=1): pass",
        "Parameter after kwargs",
    );
    assert::fail("def f(a, **kwargs, *args): pass", "parameter after another");
}

#[test]
fn funcall_extra_args_def() {
    fn f(x: &str) -> String {
        format!(
            "
def f3(a, b, c):
   return a + b + c
{}",
            x
        )
    }
    assert::fail(&f("noop(f3)(1,2,3,4)"), "extra positional");
    assert::fail(&f("noop(f3)(1,2)"), "Missing parameter");
    assert::fail(&f("noop(f3)(a=1, b=2)"), "Missing parameter");
    assert::fail(&f("noop(f3)(a=1, b=2, c=3, d=4)"), "extra named");
}

#[test]
fn test_repeated_parameters() {
    // Starlark requires both these types of errors are _static_ errors
    assert::fail("def f(x,x): pass", "duplicated parameter");
    assert::fail("def f(): pass\ndef g(): f(x=1,x=1)", "repeated named");
}

#[test]
fn test_bad_application() {
    assert::fail("noop(['1'])(2)", "not supported");
    assert::fail("noop('test')(2)", "not supported");
    assert::fail("noop(1 == 1)(2)", "not supported");
}

#[test]
fn test_extra_args_native() {
    // Check that extra native arguments fail.
    // In this module to use String functions as a test suite.
    assert::is_true(r#"("bonbon".find("on") == 1)"#);
    // Should fail because find declares #needle, so hide the parameter
    assert::fail(r#"("bonbon".find(needle = "on") == 1)"#, "extra named");
    assert::fail(r#""bonbon".find("on", 2, 3, 4)"#, "Wrong number of");
    assert::fail(r#""bonbon".find("on", needless="on")"#, "extra named");
    assert::fail(r#""bonbon".find()"#, "Wrong number of");
}

#[test]
fn test_insufficient_args_native() {
    assert::fails(
        "noop(filter)([])",
        &["Wrong number of positional", "expected 2", "got 1"],
    );
}

#[test]
fn test_parameter_defaults() {
    assert::is_true(
        "
def f(x=[x for x in [1]]):
    return x
f() == [1]",
    );
    assert::is_true(
        "
y = 7
def f(x=y):
    y = 1
    return x
f() == 7",
    );
    assert::is_true(
        "
def f(x, xs = []):
    xs.append(x)
    return xs
pre = str(f(8, [6,7]))
f(1)
post = str(f(2))
pre == '[6, 7, 8]' and post == '[1, 2]'",
    );
}

#[test]
fn test_parameter_defaults_frozen() {
    let mut a = Assert::new();
    // Frozen parameter defaults are meant to error on mutation, check that
    a.module("f.bzl", "def f(x, xs = []):\n xs.append(x)\n return xs");
    // It works if we call it with an explicit parameter
    a.is_true("load('f.bzl', 'f')\nf(1, [2]) == [2, 1]");
    // But fails if we don't, with a frozen error
    a.fail("load('f.bzl', 'f')\nf(1) == [1]", "Immutable");
}

#[test]
fn test_arguments() {
    fn f(x: &str) -> String {
        format!(
            "
def f(a, b, c=5):
    return a * b + c
def g(a=1, b=2):
    return a+b
def h(a=1, *, b=2):
    return a+b
{}",
            x
        )
    }
    assert::is_true(&f("f(*[2, 3]) == 11"));
    assert::is_true(&f("f(*[2, 3, 7]) == 13"));
    assert::fail(&f("f(*[2])"), "Missing parameter");
    assert::is_true(&f("f(**{'b':3, 'a':2}) == 11"));
    assert::is_true(&f("f(**{'c':7, 'a':2, 'b':3}) == 13"));
    assert::fail(&f("f(**{'a':2})"), "Missing parameter");
    assert::fail(&f("f(**{'c':7, 'a':2, 'b':3, 'd':5})"), "extra named");
    assert::fail(&f("noop(f)(1, a=1, b=2)"), "occurs more");
    assert::fail(&f("noop(g)(a=1,*[2])"), "occurs more");
    assert::fail(&f("noop(h)(1, 2)"), "extra positional");
    assert::is_true(&f("h(2, b=3) == 5"));
    assert::is_true(&f("h(a=2, b=3) == 5"));
    assert::fail(
        &f("def bad(x, *, *args):\n  pass"),
        "parameter after another",
    );
}

#[test]
fn test_argument_evaluation_order() {
    assert::pass(
        r#"
r = []

def id(x):
    r.append(x)
    return x

def f(*args, **kwargs):
    return (args, kwargs)

y = f(id(1), id(2), x=id(3), *[id(4)], **dict(z=id(5)))
assert_eq(y, ((1, 2, 4), dict(x=3, z=5)))
assert_eq(r, [1,2,3,4,5])
"#,
    );
}

#[test]
fn test_empty_args_kwargs() {
    // This was a bug that was introduced in the past, so make sure you don't forget
    assert::pass(
        r#"
def f(x, *args, **kwargs):
    assert_eq(args, ())
    assert_eq(kwargs, {})
f(1)
"#,
    );
    assert::fail(
        r#"
def f(x, *, y):
    pass
noop(f)(1)
"#,
        "Missing named-only parameter `y`",
    );
}

#[test]
fn test_non_optional_after_optional() {
    assert::pass(
        r#"
def f(*args, x, y = 42, z):
    return (args, x, y, z)
assert_eq(f(x = 1, z = 3), ((), 1, 42, 3))
assert_eq(f(2, 4, y = 7, x = 1, z = 3), ((2, 4), 1, 7, 3))
"#,
    );
}

#[test]
fn test_pos_only_pass() {
    assert::pass(
        r#"
def f(x, /, y):
    return x, y
assert_eq((1, 2), f(1, y=2))
"#,
    );
}

#[test]
fn test_pos_only_fail() {
    assert::fail(
        r#"
def f(x, /, y):
    return x, y
g = noop(f) # Hide from static type checker.
g(x=1, y=2)
"#,
        // TODO(nga): bad message.
        "Missing positional-only parameter `x` for call",
    );
}

// This test relies on stack behavior which does not hold when
// ASAN is enabled. See D47571173 for more context.
#[cfg_attr(rust_nightly, cfg(not(sanitize = "address")))]
#[test]
fn test_frame_size() {
    use starlark::values::list_or_tuple::UnpackListOrTuple;
    use starlark_derive::starlark_module;

    use crate as starlark;
    use crate::environment::GlobalsBuilder;
    use crate::values::UnpackValue;
    use crate::values::Value;

    #[starlark_module]
    fn natives(builder: &mut GlobalsBuilder) {
        fn stack_ptr(args: UnpackListOrTuple<Value>) -> anyhow::Result<usize> {
            drop(args);

            let x = std::hint::black_box(1);
            let ptr = &x;
            Ok(ptr as *const i32 as usize)
        }
    }

    let program = r#"
def f(x):
    return stack_ptr(x)

def g(x):
    noop(x)
    return f(x)

F_PTR = f([])
G_F_PTR = g([])
    "#;

    let mut a = Assert::new();
    a.globals_add(natives);
    let module = a.pass_module(program);
    let one = usize::unpack_value(module.get("F_PTR").unwrap().value())
        .unwrap()
        .unwrap();
    let two = usize::unpack_value(module.get("G_F_PTR").unwrap().value())
        .unwrap()
        .unwrap();
    assert!(
        two < one,
        "stack grows down everywhere we support starlark-rust"
    );
    // At the moment of writing it is about 1K in O1 and O2 modes, but about 10K in O0.
    // Note, actual frame size may be larger a frame contains for loops.
    let frame_native_size = one - two;
    assert!(frame_native_size > 20, "sanity check");
    assert!(
        frame_native_size < 20000,
        "native frame size is too large: {}, evaluation may result in native stack overflow",
        frame_native_size,
    );
}

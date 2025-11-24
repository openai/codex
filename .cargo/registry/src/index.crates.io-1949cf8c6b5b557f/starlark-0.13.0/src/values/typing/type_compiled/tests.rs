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

#![cfg(test)]

use crate::assert;
use crate::values::typing::type_compiled::compiled::TypeCompiled;
use crate::values::Heap;

#[test]
fn test_types() {
    let a = assert::Assert::new();
    a.is_true(
        r#"
def f(i: int) -> bool:
    return i == 3
f(8) == False"#,
    );

    // If the types are either malformed or runtime errors, it should fail
    a.fail("def f(i: made_up):\n pass", "Variable");
    a.fail(
        "def f(i: fail('bad')):\n pass",
        "call expression is not allowed in type expression",
    );

    // Type errors should be caught in arguments
    a.fail(
        "def f_runtime(i: bool):\n pass\nf_runtime(noop(1))",
        "Value `1` of type `int` does not match the type annotation `bool` for argument `i`",
    );
    a.fail(
        r#"
def f_compile_time(i: bool):
    pass
def g():
    f_compile_time(1)
"#,
        "Expected type `bool` but got `int`",
    );
    a.pass(
        r#"Foo = record(value=int)
def f(v: bool) -> Foo:
    return Foo(value=1)"#,
    );
    // Type errors should be caught in return positions
    a.fail(
        "def f_return_runtime() -> bool:\n return noop(1)\nf_return_runtime()",
        "Value `1` of type `int` does not match the type annotation `bool` for return type",
    );
    a.fail(
        r#"
def f_return_compile_time() -> bool:
    return 1
def g():
    f_return_compile_time()
"#,
        "Expected type `bool` but got `int`",
    );
    // And for functions without return
    // TODO(nga): should be compile-time error.
    a.fail(
        "def f_bool_none() -> bool:\n pass\nf_bool_none()",
        "Value `None` of type `NoneType` does not match the type annotation `bool` for return type",
    );
    // And for functions that return None implicitly or explicitly
    a.fails(
        "def f_none_bool_runtime() -> None:\n return noop(True)\nf_none_bool_runtime()",
        &["type annotation", "`None`", "`bool`", "return"],
    );
    a.fail(
        r#"
def f_none_bool_compile_time() -> None:
    return True
def g():
    f_none_bool_compile_time()
"#,
        "Expected type `None` but got `bool`",
    );
    a.pass("def f() -> None:\n pass\nf()");

    // The following are all valid types
    a.all_true(
        r#"
isinstance(1, int)
isinstance(True, bool)
isinstance(True, typing.Any)
isinstance(None, None)
isinstance(assert_type, typing.Callable)
isinstance([], list[int])
isinstance([], list[typing.Any])
isinstance([1, 2, 3], list[int])
isinstance(None, [None, int])
isinstance('test', int | str)
isinstance(('test', None), (str, None))
isinstance({"test": 1, "more": 2}, dict[str, int])
isinstance({1: 1, 2: 2}, dict[int, int])

not isinstance(1, None)
not isinstance((1, 1), str)
not isinstance('test', int | bool)
not isinstance([1,2,None], list[int])
not isinstance({"test": 1, 8: 2}, dict[str, int])
not isinstance({"test": 1, "more": None}, dict[str, int])

isinstance(1, typing.Any)
isinstance([1,2,"test"], list)
"#,
    );

    // Checking types fails for invalid types
    a.fail("isinstance(None, isinstance)", "not a valid type");
    a.fail("isinstance(None, [])", "cannot be used as type");
    a.fail(
        "isinstance(None, {'1': '', '2': ''})",
        "cannot be used as type",
    );

    // Should check the type of default parameters that aren't used
    a.fail(
        r#"
def foo(f: int = None):
    pass
"#,
        "`None` of type `NoneType` does not match the type annotation `int`",
    );
}

#[test]
fn test_new_syntax_without_dot_type_compile_time() {
    assert::pass(r"def f() -> int: return 17");
    assert::fail(
        r#"
def f() -> int: return 'tea'
"#,
        "Expected type `int` but got `str`",
    );
}

#[test]
fn test_new_syntax_without_dot_type_runtime() {
    assert::pass(
        r#"
def f() -> str: return noop('coke')
f()
"#,
    );
    assert::fail(
        r#"
def f() -> str: return noop(19)
f()
"#,
        "Value `19` of type `int`",
    );
}

#[test]
fn test_type_compiled_display() {
    fn t(expected: &str, ty0: &str) {
        let ty = assert::pass(ty0);
        let heap = Heap::new();
        let ty = unsafe { ty.unchecked_frozen_value() }.to_value();
        let ty = TypeCompiled::new(ty, &heap).unwrap();
        assert_eq!(expected, ty.to_string(), "for `{}`", ty0);
    }

    t("typing.Any", "typing.Any");
    t("list", "list");
    t("list", "list[typing.Any]");
    t("None", "None");
}

#[test]
fn test_type_compiled_starlark_api() {
    assert::eq("\"int\"", "repr(eval_type(int))");
    assert::eq("\"int | str\"", "repr(eval_type(int | str))");
    assert::is_true("eval_type(int).matches(1)");
    assert::is_true("not eval_type(int).matches([])");
    assert::pass("eval_type(int).check_matches(1)");
    assert::fail(
        "eval_type(int).check_matches([])",
        "Value of type `list` does not match type `int`: []",
    );
}

#[test]
fn test_eval_type_eval_type() {
    assert::is_true("isinstance(1, eval_type(eval_type(int)))");
}

#[test]
fn test_type_compiled_can_be_used_in_function_signature() {
    assert::pass(
        r#"
ty = eval_type(int)
def f(x: ty):
    pass

f(1)
"#,
    );
    assert::fail(
        r#"
ty = eval_type(int)
def f(x: ty):
    pass

# Runtime error.
f(noop("x"))
"#,
        "Value `x` of type `string` does not match the type annotation `int` for argument `x`",
    );
    assert::fail(
        r#"
ty = eval_type(int)
def f(x: ty):
    pass

def g():
    # Compile-time error.
    f("x")
"#,
        "Expected type `int` but got `str`",
    );
}

#[test]
fn test_isinstance() {
    assert::eq("True", "isinstance(1, int)");
    assert::eq("False", "isinstance(1, str)");
}

#[test]
fn test_new_list_dict_syntax_pass() {
    assert::pass(
        r#"
def uuu(x: list[int]):
    pass

uuu([1, 2, 3])
"#,
    );
}

#[test]
fn test_new_list_dict_syntax_fail_compile_time() {
    assert::fail(
        r#"
def uuu(x: list[int]):
    pass

def www():
    uuu(["mm"])
"#,
        "Expected type `list[int]` but got `list[str]`",
    );
}

#[test]
fn test_new_list_dict_syntax_fail_runtime() {
    assert::fail(
        r#"
def uuu(x: list[int]):
    pass

noop(uuu)(["mm"])
"#,
        r#"Value `["mm"]` of type `list` does not match"#,
    );
}

#[test]
fn test_bit_or() {
    let types = [
        ("int", "17"),
        ("str", "'x'"),
        ("None", "None"),
        ("list", "[]"),
        ("(str | int)", "19"),
    ];
    for (at, av) in &types {
        for (bt, bv) in &types {
            assert::is_true(&format!("isinstance({av}, {at} | {bt})"));
            assert::is_true(&format!("isinstance({bv}, {at} | {bt})"));
            assert::is_true(&format!("not isinstance(range(10), {at} | {bt})"));
        }
    }
}

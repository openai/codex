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

use crate::assert;
use crate::assert::Assert;
use crate::syntax::Dialect;
use crate::syntax::DialectTypes;

const PROGRAM: &str = "\
def f(x: int): pass

f(noop('s'))
";

#[test]
fn test_types_disable() {
    let mut dialect = Dialect::Standard;
    dialect.enable_types = DialectTypes::Disable;
    let mut a = Assert::new();
    a.dialect(&dialect);
    a.fail(PROGRAM, "type annotations are not allowed in this dialect");
}

#[test]
fn test_types_parse_only() {
    let mut dialect = Dialect::Standard;
    dialect.enable_types = DialectTypes::ParseOnly;
    let mut a = Assert::new();
    a.dialect(&dialect);
    a.pass(PROGRAM);
}

#[test]
fn test_types_enable() {
    let mut dialect = Dialect::Standard;
    dialect.enable_types = DialectTypes::Enable;
    let mut a = Assert::new();
    a.dialect(&dialect);
    a.fail(PROGRAM, "does not match the type annotation");
}

#[test]
fn test_type_assign_annotation() {
    assert::pass(
        r#"
x : str = "test"
xs: typing.Any = [1,2]
xs[0] : int = 4
"#,
    );
    assert::fail(
        "a, b : typing.Any = 1, 2",
        "not allowed on multiple assignments",
    );
    assert::fail(
        "a = 1\na : typing.Any += 1",
        "not allowed on augmented assignments",
    );
    assert::fail("a : str = noop(1)", "does not match the type annotation");
}

#[test]
fn test_only_globals_or_bultins_allowed() {
    assert::fail(
        r#"
def f():
    x = "str"
    def g(p: x): pass
"#,
        "Identifiers in type expressions can only refer globals or builtins: `x`",
    );
}

#[test]
fn test_typecheck_opt_in_self_check() {
    let mut a = Assert::new();
    a.disable_static_typechecking();
    a.pass(
        r#"
def f(x: int): pass
def g(): f("")
"#,
    );
}

/// Test `@starlark-rust: typecheck` enables typechecking when `Evaluator` does not.
#[test]
fn test_typecheck_opt_in() {
    let mut a = Assert::new();
    a.disable_static_typechecking();
    a.fail(
        r#"
# @starlark-rust: typecheck
def f(x: int): pass
def g(): f("")
"#,
        "Expected type `int` but got `str`",
    );
}

#[test]
fn test_string_lit_as_type() {
    assert::fail(
        r#"
def foo(x: ""): pass
"#,
        "string literal expression is not allowed in type expression",
    );
}

#[test]
fn test_string_const_as_type() {
    assert::fail(
        r#"
T = ""
def foo(x: T): pass
"#,
        "String literals are not allowed in type expressions",
    );
}

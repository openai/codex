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

//! Basic expression tests.

use crate::assert;
use crate::assert::Assert;
use crate::environment::GlobalsBuilder;
use crate::values::OwnedFrozenValue;

#[test]
fn arithmetic_test() {
    assert::is_true("(1 + 2 == 3)");
    assert::is_true("(1 * 2 == 2)");
    assert::is_true("(-1 * 2 == -2)");
    assert::is_true("(5 // 2 == 2)");
    assert::is_true("(5 % 2 == 1)");
}

#[test]
fn bitwise_test() {
    assert::all_true(
        r#"
3 & 6 == 2
3 & 6 == 2
-3 & 6 == 4
3 | 6 == 7
3 | -6 == -5
3 ^ 6 == 5
-3 ^ 6 == -5
-3 ^ -6 == 7
1 << 2 == 4
-1 << 2 == -4
1 >> 0 == 1
111 >> 2 == 27
~31 == -32
~-31 == 30
"#,
    );

    assert::fail("1 << -13", "Negative left shift");
    assert::fail("1 >> -13", "Negative right shift");
}

#[test]
fn test_operators() {
    assert::eq("1+------2", "3");
}

#[test]
fn test_equality() {
    assert::all_true(
        r#"
None == None
True == True
True != False
1 == 1
1 != 2
"test" == "test"
"test" != "x"
[1, 2] == [1, 2]
[1, 3] != [1, 2]
[1, 3] != [1, 3, 4]
(1, 2) == (1, 2)
(1, 3) != (1, 2)
noop((1, 3)) != (1, 3, 4)
noop(range(4)) == range(0, 4, 1)
noop(range(4)) != range(0, 4, 2)
noop(range(4)) != [0,1,2,3,4]
{1: 2} == {1: 2}
{1: 2} != {}
{1: 2, 3: 4} == {1: 2, 3: 4}
{1: 2, 3: 4} == {3: 4, 1: 2}  # Spec is a little ambiguous here
repr == repr
repr != str
[].clear != [1].clear
x = []; x.clear != x.clear
x = []; y = x.clear; y == y
x = repr; y = repr; x == y
"#,
    );
}

#[test]
fn test_frozen_equality() {
    let program = "(str, (), 1, range(4), True, None, [8], {'test':3})";
    let a = assert::pass(program);
    let b = assert::pass(program);
    assert_eq!(a.value(), b.value());

    let mut a = Assert::new();
    a.module("saved", &format!("val = {}", program));
    a.is_true(&format!("load('saved', 'val'); val == {}", program));
}

#[test]
fn test_equality_multiple_globals() {
    fn mk_repr() -> OwnedFrozenValue {
        let mut a = Assert::new();
        let globals = GlobalsBuilder::extended().build();
        a.globals(globals);
        a.pass("repr")
    }

    // Do things that compare by pointer still work if you
    // create fresh Globals for each of them.
    assert_eq!(mk_repr().value(), mk_repr().value());
}

#[test]
fn test_comparison() {
    assert::all_true(
        r#"
False < True
1 < 2
"test" < "x"
[1, 3] > [1, 2]
[1, 3] < [1, 3, 4]
(1, 3) > (1, 2)
(1, 3) < (1, 3, 4)
"#,
    );
    assert::fail("noop(None) < None", "`compare` not supported");
    assert::fail_skip_typecheck("(None, ) < (None, )", "`compare` not supported");
    assert::fail_skip_typecheck("x = (None,); x < x", "`compare` not supported");
    assert::fail_skip_typecheck("x = {}; x < x", "`compare` not supported");
    assert::fail_skip_typecheck("{} < {1: 2}", "`compare` not supported");
    assert::fail_skip_typecheck("range(1) < range(2)", "`compare` not supported");
    assert::fail_skip_typecheck("repr < str", "`compare` not supported");
}

#[test]
fn test_frozen_hash() {
    let exprs = &["\"test\"", "\"x\""];
    let mut a = Assert::new();
    // TODO(nga): fix and enable.
    a.disable_static_typechecking();
    a.module(
        "m",
        &format!(
            r#"
dict = {{x:len(x) for x in [{}]}}
"#,
            exprs.join(",")
        ),
    );
    a.pass(&format!(
        r#"
load('m', frozen_dict='dict')
values = [{}]
assert_eq(all([frozen_dict[x] != None for x in values]), True)
"#,
        exprs.join(","),
    ));
}

#[test]
fn test_compare() {
    assert::fail("noop(1) > False", "Operation `compare` not supported");
    assert::is_true("[1, 2] == [1, 2]");
    assert::is_true("noop(1) != True");
    assert::is_true("not (noop(None) == [1])");
    assert::is_true(
        r#"
xs = [1]
xs[0] = xs
xs == xs
"#,
    );
    assert::is_true(
        r#"
ys = [1]
xs = [ys]
ys[0] = xs
xs == xs
"#,
    );
    let mut a = Assert::new();
    // TODO(nga): fix and enable.
    a.disable_static_typechecking();
    a.fail(
        r#"
ys = [1]
xs = [ys]
ys[0] = xs
xs == ys
"#,
        "recursion",
    );
}

#[test]
fn test_not_in_unhashable() {
    // Note that [] can't be hashed
    assert::fail("[] not in {123: 456}", "not hashable");
}

#[test]
fn test_not_hashable() {
    assert::fail(
        r#"
x = {}
y = {}
x[y] = 1
"#,
        "not hashable",
    );
    assert::fail(
        r#"
x = {'x': 1}
y = {}
x.get(y)
"#,
        "not hashable",
    );
    assert::fail(
        r#"
x = {'x': 1}
y = {}
x[y]
"#,
        "not hashable",
    );
}

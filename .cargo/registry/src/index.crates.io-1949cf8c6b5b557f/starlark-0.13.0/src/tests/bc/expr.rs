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
use crate::tests::bc::golden::bc_golden_test;

#[test]
fn test_type() {
    bc_golden_test("expr_type", "def test(x): return type(x)");
}

#[test]
fn test_percent_s_one() {
    bc_golden_test("expr_percent_s_one", "def test(x): return '((%s))' % x");
}

#[test]
fn test_format_one() {
    bc_golden_test("expr_format_one", "def test(x): return '(({}))'.format(x)");
}

#[test]
fn test_percent_s_one_format_one_eval() {
    assert::pass(
        r#"
load("asserts.star", "asserts")

def test(x):
    return ("<{}>".format(x), "<%s>" % x)

asserts.eq(("<1>", "<1>"), test(1))
# Test format does not accidentally call `PercentSOne`.
asserts.eq(("<(1,)>", "<1>"), test((1,)))
"#,
    );
}

#[test]
fn test_spec_exec_list() {
    // `list` function is const-evaluated and the resulting list is compiled as list instruction.
    bc_golden_test("expr_spec_exec_list", "def test(): return list((10, 20))");
}

#[test]
fn test_call_maybe_known_method() {
    bc_golden_test("expr_call_maybe_known_method", "def test(x): x.append(1)");
}

#[test]
fn test_fstring() {
    bc_golden_test("expr_fstring", "def test(x): return f'test: {x}'");
}

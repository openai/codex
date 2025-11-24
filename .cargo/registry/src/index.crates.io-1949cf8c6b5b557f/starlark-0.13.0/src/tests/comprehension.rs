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

//! Test dict and list comprehension.

use crate::assert;

// comprehensions should work whether they are at the root, or under a def
// but these are actually quite different locations semantically, so test both
fn check_comp(lines: &[&str]) {
    // TODO(nga): typechecker is wrong here.
    assert::is_true_skip_typecheck(&lines.join("\n"));
    let (last, init) = lines.split_last().unwrap();
    assert::is_true_skip_typecheck(&format!(
        "def f():\n  {}\n  return {}\nf()",
        init.join("\n  "),
        last
    ));
}

#[test]
fn test_spec() {
    // From the Starlark spec
    check_comp(&["[x*x for x in [0,1,2,3,4]] == [0, 1, 4, 9, 16]"]);
    check_comp(&["[x*x for x in [0,1,2,3,4] if x%2 == 0] == [0, 4, 16]"]);
    check_comp(&[
        "[(x, y) for x in [0,1,2,3,4] if x%2 == 0 for y in [0,1,2,3,4] if y > x] == [(0, 1), (0, 2), (0, 3), (0, 4), (2, 3), (2, 4)]",
    ]);
    check_comp(&[r#"[x*y+z for (x, y), z in [((2, 3), 5), (("o", 2), "!")]] == [11, 'oo!']"#]);
    assert::fail("[x*x for x in 1, 2, 3]", "Parse error");
    check_comp(&["x = 1", "_ = [x for x in [2]]", "x == 1"]);
}

#[test]
fn test_scopes() {
    // In the (unnatural) examples below, the scope of the variables x, y, and z
    // is the entire comprehension block, except the operand of the first loop ([], [1] or w),
    // which is resolved in the enclosing environment. The second loop may thus refer
    // to variables defined by the third (z), even though such references would fail
    // if actually executed.
    check_comp(&["[1//0 for x in [] for y in z for z in ()] == []"]);
    assert::fail_skip_typecheck(
        "[1//0 for x in [1] for y in z for z in ()]",
        "Local variable `z` referenced before assignment",
    );
    assert::fail_skip_typecheck("[() for x in w for w in [1]]", "Variable `w` not found");
}

#[test]
fn test_dict() {
    // Dict comprehensions
    check_comp(&["{x: 1 for x in [0,1,2]} == {0: 1, 1: 1, 2: 1}"]);
}

#[test]
fn test_nested() {
    // Nested comprehensions
    check_comp(&["[[y for y in x] for x in [[1],[2,3]]] == [[1],[2,3]]"]);
    check_comp(&["[[x for x in x] for x in [[1],[2,3]]] == [[1],[2,3]]"]);
    check_comp(&["[x for x in [[1],[2,3]] for x in x if x >= 2] == [2,3]"]);
    check_comp(&[
        "items = {8: [1,2], 9: [3,4,6]}",
        "[[x for x in items[x] if x%2==0] for x in items] == [[2],[4,6]]",
    ]);
}

#[test]
fn test_sequential() {
    // Sequential comprehensions
    check_comp(&[
        "x = [x*x for x in [0,1,2,3,4]]",
        "[x*x for x in x] == [0, 1, 16, 81, 256]",
    ]);
}

#[test]
fn test_if_only() {
    // If only comprehensions are parse errors
    assert::fail("[1 if 0 == 0] == [0]", "Parse error");
}

#[test]
fn test_same_var_twice_in_assignment() {
    check_comp(&["[x for (x, x) in [(1, 2), (3, 4)]] == [2, 4]"]);
}

#[test]
fn test_same_var_in_two_fors() {
    check_comp(&["[x for x in [[1, 2], [3]] for x in x] == [1, 2, 3]"]);
}

#[test]
fn test_comprehension_blocks() {
    assert::fail_skip_typecheck(
        r#"
x = [1, 2]
res = [x for _ in [3] for x in x]
assert_eq(res, [1,2])
"#,
        "variable `x` referenced before assignment",
    );
}

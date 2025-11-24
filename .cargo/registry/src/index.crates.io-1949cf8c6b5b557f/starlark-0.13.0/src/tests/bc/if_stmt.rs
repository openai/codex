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

use crate::tests::bc::golden::bc_golden_test;

#[test]
fn test_if_x_and_true() {
    bc_golden_test(
        "if_stmt_if_x_and_true",
        "def test(x):\n  if x and True: noop()",
    );
}

#[test]
fn test_if_x_and_false() {
    bc_golden_test(
        "if_stmt_if_x_and_false",
        "def test(x):\n  if x and False: noop()",
    );
}

#[test]
fn test_if_x_or_true() {
    bc_golden_test(
        "if_stmt_if_x_or_true",
        "def test(x):\n  if x or True: noop()",
    );
}

#[test]
fn test_if_x_or_false() {
    bc_golden_test(
        "if_stmt_if_x_or_false",
        "def test(x):\n  if x or False: noop()",
    );
}

#[test]
fn test_if_true_and_x() {
    bc_golden_test(
        "if_stmt_if_true_and_x",
        "def test(x):\n  if True and x: noop()",
    );
}

#[test]
fn test_if_false_and_x() {
    bc_golden_test(
        "if_stmt_if_false_and_x",
        "def test(x):\n  if False and x: noop()",
    );
}

#[test]
fn test_if_true_or_x() {
    bc_golden_test(
        "if_stmt_if_true_or_x",
        "def test(x):\n  if True or x: noop()",
    );
}

#[test]
fn test_if_false_or_x() {
    bc_golden_test(
        "if_stmt_if_false_or_x",
        "def test(x):\n  if False or x: noop()",
    );
}

#[test]
fn test_if_else_x_and_y() {
    bc_golden_test(
        "if_stmt_if_else_x_and_y",
        "def test(x, y):\n  if x and y:\n    return 10\n  else:\n    return 20",
    );
}

#[test]
fn test_if_else_x_or_y() {
    bc_golden_test(
        "if_stmt_if_else_x_or_y",
        "def test(x, y):\n  if x or y:\n    return 10\n  else:\n    return 20",
    );
}

#[test]
fn test_and_stmt() {
    bc_golden_test("if_stmt_and_stmt", "def test(x):\n  x and noop()");
}

#[test]
fn test_or_stmt() {
    bc_golden_test("if_stmt_or_stmt", "def test(x):\n  x or noop()");
}

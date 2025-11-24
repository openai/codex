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
fn test_for() {
    bc_golden_test("for", "def test(x):\n  for i in x:\n    noop(i)");
}

#[test]
fn test_for_break() {
    bc_golden_test(
        "for_break",
        "def test(x):\n  for i in x:\n    if i: break\n    noop(i)",
    );
}

#[test]
fn test_for_continue() {
    bc_golden_test(
        "for_continue",
        "def test(x):\n  for i in x:\n    if i: continue\n    noop(i)",
    );
}

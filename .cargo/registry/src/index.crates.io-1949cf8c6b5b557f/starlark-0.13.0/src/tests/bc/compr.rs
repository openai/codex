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

//! Test compilation of comprehensions.

use crate::tests::bc::golden::bc_golden_test;

#[test]
fn test_no_loop_if_top_collection_is_empty() {
    bc_golden_test(
        "compr_no_loop_if_top_collection_is_empty",
        "def test(): return [x for x in []]",
    );
}

#[test]
fn test_no_loop_if_top_collection_is_empty_on_freeze() {
    // This function is not optimized to return a list on compilation,
    // because `L` is not evaluated yet.
    // But it eliminates the loop on freeze.
    bc_golden_test(
        "compr_no_loop_if_top_collection_is_empty_on_freeze",
        "def test(): return [x for x in D]\nD = {}",
    );
}

#[test]
fn test_if_true_clause() {
    bc_golden_test(
        "compr_if_true_clause",
        "def test(y): return [x for x in y if True]",
    );
}

#[test]
fn test_if_true_clause_on_freeze() {
    bc_golden_test(
        "compr_if_true_clause_on_freeze",
        "def test(y): return [x for x in y if C]\nC = False\nC = True",
    );
}

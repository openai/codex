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

use crate::typing::tests::TypeCheck;

#[test]
fn test_special_function_struct() {
    TypeCheck::new().ty("x").check(
        "struct",
        r#"
def test():
    x = struct(a = 1, b = "test")
"#,
    );
}

#[test]
fn test_special_function_zip() {
    TypeCheck::new().ty("x").check(
        "zip",
        r#"
def test():
    x = zip([1,2], [True, False], ["a", "b"])
"#,
    );
}

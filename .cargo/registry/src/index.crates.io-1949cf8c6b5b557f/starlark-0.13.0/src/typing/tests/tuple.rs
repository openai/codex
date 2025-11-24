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
fn test_tuple() {
    TypeCheck::new().check(
        "tuple",
        r#"
def empty_tuple_fixed_name() -> (): return tuple()
def empty_tuple_name_fixed() -> tuple: return ()
"#,
    );
}

#[test]
fn test_tuple_ellipsis() {
    TypeCheck::new().check(
        "tuple_ellipsis",
        r#"
def f(t: tuple[int, ...]) -> int:
    return t[0]

def g():
    # Good.
    f((1, 2, 3))

    # Bad.
    f((1, "x"))
"#,
    );
}

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

use std::fmt::Write;

use starlark_syntax::golden_test_template::golden_test_template;

use crate::analysis::unused_loads::remove::remove_unused_loads;

fn test_remove(name: &str, program: &str) {
    let program = program.trim();

    let mut out = String::new();

    writeln!(out, "Program:").unwrap();
    writeln!(out, "{}", program).unwrap();
    writeln!(out).unwrap();

    let removed = remove_unused_loads(name, program).unwrap();
    match removed {
        None => writeln!(out, "No unused loads").unwrap(),
        Some(removed) => {
            writeln!(out, "Removed unused loads:").unwrap();
            writeln!(out, "{}", removed).unwrap();
        }
    }

    golden_test_template(
        &format!("src/analysis/unused_loads/remove/{name}.golden"),
        &out,
    );
}

#[test]
fn test_remove_first_of_two() {
    test_remove(
        "remove_first_of_two",
        r#"
load("foo", "x", "y")
print(y)
"#,
    );
}

#[test]
fn test_remove_second_of_two() {
    test_remove(
        "remove_second_of_two",
        r#"
load("foo", "x", "y")
print(x)
"#,
    );
}

#[test]
fn test_remove_all() {
    test_remove(
        "remove_all",
        r#"
load("foo", "x", "y")
print("test")
"#,
    );
}

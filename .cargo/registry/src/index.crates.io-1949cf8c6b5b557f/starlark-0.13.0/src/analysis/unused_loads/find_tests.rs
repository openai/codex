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
use starlark_syntax::span_display::span_display;

use crate::analysis::unused_loads::find::find_unused_loads;

fn test_unused_loads(name: &str, program: &str) {
    let program = program.trim();
    let mut out = String::new();
    writeln!(out, "Program:").unwrap();
    writeln!(out, "{}", program).unwrap();
    writeln!(out).unwrap();

    let (codemap, unused_loads) = find_unused_loads(name, program).unwrap();
    if unused_loads.is_empty() {
        writeln!(out, "No unused loads").unwrap();
    } else {
        writeln!(out, "Unused loads:").unwrap();
        for load in unused_loads {
            let spans = if load.all_unused() {
                vec![load.load.span]
            } else {
                load.unused_args.iter().map(|x| x.span()).collect()
            };
            for span in spans {
                writeln!(out).unwrap();

                let span = codemap.file_span(span);
                let display = span_display(Some(span.as_ref()), "Unused load", false);
                write!(out, "{}", display).unwrap();
            }
        }
    }

    golden_test_template(
        &format!("src/analysis/unused_loads/find/{name}.golden"),
        &out,
    );
}

#[test]
fn test_simple() {
    test_unused_loads(
        "simple",
        r#"
load("foo", "x")
"#,
    );
}

#[test]
fn test_used_in_top_level_assignment() {
    test_unused_loads(
        "used_in_top_level_assignment",
        r#"
load("foo", "x")
y = x
"#,
    );
}

#[test]
fn test_one_of_two_unused() {
    test_unused_loads(
        "one_of_two_unused",
        r#"
load("foo", "x", "y")
print(x)
"#,
    );
}

#[test]
fn test_with_rename() {
    test_unused_loads(
        "with_rename",
        r#"
load("foo", x="y", z="w")
y = z
"#,
    );
}

#[test]
fn test_used_in_type_expr() {
    test_unused_loads(
        "used_in_type_expr",
        r#"
load("foo", "T")
load("bar", "U")

y: T = 1

def f(x: U):
    pass
"#,
    );
}

#[test]
fn test_unused_annotation_on_arg() {
    test_unused_loads(
        "unused_annotation_on_arg",
        r#"
load("foo",
    "T", # @unused
    "U",
     )
"#,
    );
}

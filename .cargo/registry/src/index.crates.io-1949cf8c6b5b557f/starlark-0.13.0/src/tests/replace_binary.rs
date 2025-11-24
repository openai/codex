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

//! Run Go implementation tests.

use std::collections::HashMap;

use starlark_syntax::syntax::AstModule;
use starlark_syntax::syntax::Dialect;

use crate::environment::Globals;
use crate::environment::Module;
use crate::eval::Evaluator;

#[test]
fn test_replace_binary() {
    let mut ast = AstModule::parse(
        "file.sky",
        r#"
def equals(a, b):
    return "(" + str(a) + " == " + str(b) + ")"

def my_subtract(a, b):
    return "(" + str(a) + " - " + str(b) + ")"

(7 + 8) - 9 == True
    "#
        .to_owned(),
        &Dialect::Standard,
    )
    .unwrap();
    ast.replace_binary_operators(&HashMap::from([
        ("==".to_owned(), "equals".to_owned()),
        ("-".to_owned(), "my_subtract".to_owned()),
    ]));
    let module = Module::new();
    let mut eval = Evaluator::new(&module);
    let v = eval.eval_module(ast, &Globals::standard()).unwrap();
    assert_eq!(v.unpack_str().unwrap(), "((15 - 9) == True)");
}

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

use std::cell::Cell;

use crate::codemap::FileSpanRef;
use crate::environment::Globals;
use crate::environment::Module;
use crate::eval::Evaluator;
use crate::syntax::AstModule;
use crate::syntax::Dialect;

#[test]
fn before_stmt() {
    let module = Module::new();
    let globals = Globals::new();
    let counter = Cell::new(0);
    let before_stmt = |_span: FileSpanRef, _eval: &mut Evaluator<'_, '_, '_>| {
        counter.set(counter.get() + 1);
    };

    let mut evaluator = Evaluator::new(&module);
    evaluator.before_stmt_fn(&before_stmt);

    // For a top-level statement, we get an additional before_stmt call for the possible gc.
    let program = "\
x = 1          # 0 + 1
def f():       # 1 + 1
  return x + 1 # 3
f()            # 2 + 1
";
    let ast = AstModule::parse("a.star", program.to_owned(), &Dialect::AllOptionsInternal).unwrap();
    evaluator.eval_module(ast, &globals).unwrap();
    assert_eq!(7, counter.get());
}

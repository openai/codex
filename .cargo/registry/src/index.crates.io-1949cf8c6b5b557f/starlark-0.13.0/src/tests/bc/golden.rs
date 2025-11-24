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

use std::fmt::Write;
use std::mem;

use starlark_syntax::golden_test_template::golden_test_template;

use crate::assert::Assert;
use crate::eval::compiler::def::FrozenDef;
use crate::syntax::Dialect;

fn test_function_bytecode(program: &str) -> String {
    let program = program.trim();

    let mut a = Assert::new();
    a.dialect(&Dialect::AllOptionsInternal);
    let def = a
        .module("instrs.star", program)
        .get("test")
        .unwrap()
        .downcast::<FrozenDef>()
        .unwrap();

    let mut golden = String::new();
    writeln!(golden, "{}", program).unwrap();
    writeln!(golden).unwrap();
    writeln!(golden, "# Bytecode:").unwrap();
    writeln!(golden).unwrap();
    writeln!(golden, "{}", def.bc().dump_debug().trim()).unwrap();
    golden
}

pub(crate) fn bc_golden_test(test_name: &str, program: &str) {
    if mem::size_of::<usize>() != mem::size_of::<u64>() {
        // Bytecode addresses are different on 32-bit platforms.
        // TODO(nga): still run evaluation on 32-bit platforms, without comparison.
        return;
    }

    let output = test_function_bytecode(program);

    golden_test_template(&format!("src/tests/bc/golden/{test_name}.golden"), &output);
}

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

use crate::environment::FrozenModule;
use crate::eval::compiler::def::FrozenDef;
use crate::values::FrozenHeapRef;
use crate::values::FrozenValueTyped;

impl FrozenModule {
    /// Print a lot of module internals for debugging.
    pub fn dump_debug(&self) -> String {
        let mut w = String::new();

        writeln!(w, "Eval duration: {:.3}s", self.eval_duration.as_secs_f64()).unwrap();
        writeln!(w, "Heap stats:").unwrap();
        w.push_str(&self.frozen_heap().dump_debug());

        for (name, value) in self.all_items() {
            // TODO(nga): this prints public, private and imported symbols.
            //   We only care about public and private symbols, but no imported.
            writeln!(w).unwrap();
            writeln!(w, "{} = {}", name, value).unwrap();
            if let Some(def) = FrozenValueTyped::<FrozenDef>::new(value) {
                def.dump_debug()
                    .lines()
                    .for_each(|line| writeln!(w, "  {}", line).unwrap());
            }
        }
        w
    }
}

impl FrozenHeapRef {
    fn dump_debug(&self) -> String {
        let mut w = String::new();
        writeln!(w, "Allocated bytes: {}", self.allocated_bytes()).unwrap();
        writeln!(w, "Available bytes: {}", self.available_bytes()).unwrap();
        w
    }
}

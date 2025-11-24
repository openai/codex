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

use starlark_derive::starlark_module;

use crate as starlark;
use crate::assert::Assert;
use crate::environment::GlobalsBuilder;
use crate::values::tuple::UnpackTuple;

#[starlark_module]
fn named_positional_functions(globals: &mut GlobalsBuilder) {
    fn positional(#[starlark(require = pos)] x: i32) -> anyhow::Result<i32> {
        Ok(x)
    }

    fn named(x: i32) -> anyhow::Result<i32> {
        Ok(x)
    }

    fn named_only(#[starlark(require = named)] x: i32) -> anyhow::Result<i32> {
        Ok(x)
    }

    fn named_after_args(
        #[starlark(args)] star_args: UnpackTuple<i32>,
        x: i32,
    ) -> anyhow::Result<i32> {
        Ok(x + star_args.items.iter().sum::<i32>())
    }

    // Same as above, but with explicit redundant annotation.
    fn named_after_args_explicitly_marked(
        #[starlark(args)] args: UnpackTuple<i32>,
        #[starlark(require = named)] x: i32,
    ) -> anyhow::Result<i32> {
        Ok(x + args.items.iter().sum::<i32>())
    }
}

#[test]
fn test_positional_only() {
    let mut a = Assert::new();
    a.globals_add(named_positional_functions);
    a.eq("17", "positional(17)");
    a.fail("noop(positional)(x=19)", "extra named parameter");
}

#[test]
fn test_named_can_be_called_as_both_named_and_positional() {
    let mut a = Assert::new();
    a.globals_add(named_positional_functions);
    a.eq("23", "named(x=23)");
    a.eq("29", "named(29)");
}

#[test]
fn test_named_only() {
    let mut a = Assert::new();
    a.globals_add(named_positional_functions);
    a.eq("31", "named_only(x=31)");
    a.fail("noop(named_only)(37)", "Missing named-only parameter");
}

#[test]
fn test_named_after_args() {
    let mut a = Assert::new();
    a.globals_add(named_positional_functions);
    a.eq("13", "named_after_args(1, 2, x=10)");
    a.fail(
        "noop(named_after_args)(1, 2, 3)",
        "Missing named-only parameter",
    );
}

#[test]
fn test_named_after_args_explicitly_marked() {
    let mut a = Assert::new();
    a.globals_add(named_positional_functions);
    a.eq("13", "named_after_args_explicitly_marked(1, 2, x=10)");
    a.fail(
        "noop(named_after_args_explicitly_marked)(1, 2, 3)",
        "Missing named-only parameter",
    );
}

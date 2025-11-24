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

//! Detect recursion when doing `repr` or `to_json`.

use std::cell::Cell;

use crate::collections::SmallSet;
use crate::hint::unlikely;
use crate::values::layout::pointer::RawPointer;
use crate::values::Value;

/// Pop the stack on drop.
pub(crate) struct ReprStackGuard;

/// Pop the stack on drop.
pub(crate) struct JsonStackGuard;

impl Drop for ReprStackGuard {
    fn drop(&mut self) {
        REPR_STACK.with(|repr_stack| {
            let mut stack = Cell::take(repr_stack);
            let popped = stack.pop();
            debug_assert!(popped.is_some());
            repr_stack.set(stack);
        })
    }
}

impl Drop for JsonStackGuard {
    fn drop(&mut self) {
        JSON_STACK.with(|json_stack| {
            let mut stack = Cell::take(json_stack);
            let popped = stack.pop();
            debug_assert!(popped.is_some());
            json_stack.set(stack);
        })
    }
}

/// Returned when `repr` is called recursively and a cycle is detected.
pub(crate) struct ReprCycle;

/// Returned when `to_json` is called recursively and a cycle is detected.
pub(crate) struct JsonCycle;

thread_local! {
    static REPR_STACK: Cell<SmallSet<RawPointer>> = const { Cell::new(SmallSet::new()) };
}

thread_local! {
    static JSON_STACK: Cell<SmallSet<RawPointer>> = const { Cell::new(SmallSet::new()) };
}

/// Push a value to the stack, return error if it is already on the stack.
pub(crate) fn repr_stack_push(value: Value) -> Result<ReprStackGuard, ReprCycle> {
    REPR_STACK.with(|repr_stack| {
        let mut stack = Cell::take(repr_stack);
        if unlikely(!stack.insert(value.ptr_value())) {
            repr_stack.set(stack);
            Err(ReprCycle)
        } else {
            repr_stack.set(stack);
            Ok(ReprStackGuard)
        }
    })
}

/// Push a value to the stack, return error if it is already on the stack.
pub(crate) fn json_stack_push(value: Value) -> Result<JsonStackGuard, JsonCycle> {
    JSON_STACK.with(|json_stack| {
        let mut stack = Cell::take(json_stack);
        if unlikely(!stack.insert(value.ptr_value())) {
            json_stack.set(stack);
            Err(JsonCycle)
        } else {
            json_stack.set(stack);
            Ok(JsonStackGuard)
        }
    })
}

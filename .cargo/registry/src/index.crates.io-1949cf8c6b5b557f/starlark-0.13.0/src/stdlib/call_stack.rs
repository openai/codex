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

//! Implementation of `call_stack` function.

use std::fmt;
use std::fmt::Display;
use std::fmt::Formatter;

use allocative::Allocative;
use starlark_derive::starlark_module;
use starlark_syntax::codemap::FileSpan;

use crate as starlark;
use crate::environment::GlobalsBuilder;
use crate::environment::Methods;
use crate::environment::MethodsBuilder;
use crate::environment::MethodsStatic;
use crate::eval::Evaluator;
use crate::values::none::NoneOr;
use crate::values::starlark_value;
use crate::values::AllocValue;
use crate::values::Heap;
use crate::values::NoSerialize;
use crate::values::ProvidesStaticType;
use crate::values::StarlarkValue;
use crate::values::Trace;
use crate::values::Value;

#[derive(ProvidesStaticType, Trace, Allocative, Debug, NoSerialize, Clone)]
/// A frame of the call-stack.
struct StackFrame {
    /// The name of the entry on the call-stack.
    name: String,
    /// The location of the definition, or [`None`] for native Rust functions.
    location: Option<FileSpan>,
}

#[starlark_value(type = "StackFrame", StarlarkTypeRepr, UnpackValue)]
impl<'v> StarlarkValue<'v> for StackFrame {
    fn get_methods() -> Option<&'static Methods> {
        static RES: MethodsStatic = MethodsStatic::new();
        RES.methods(stack_frame_methods)
    }
}

impl<'v> AllocValue<'v> for StackFrame {
    fn alloc_value(self, heap: &'v Heap) -> Value<'v> {
        heap.alloc_complex_no_freeze(self)
    }
}

impl Display for StackFrame {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "<StackFrame ...>")
    }
}

#[starlark_module]
fn stack_frame_methods(builder: &mut MethodsBuilder) {
    /// Returns the name of the entry on the call-stack.
    #[starlark(attribute)]
    fn func_name(this: &StackFrame) -> starlark::Result<String> {
        Ok(this.name.clone())
    }

    /// Returns a path of the module from which the entry was called, or [`None`] for native Rust functions.
    #[starlark(attribute)]
    fn module_path(this: &StackFrame) -> starlark::Result<NoneOr<String>> {
        match this.location {
            Some(ref location) => Ok(NoneOr::Other(location.file.filename().to_owned())),
            None => Ok(NoneOr::None),
        }
    }
}

#[starlark_module]
pub(crate) fn global(builder: &mut GlobalsBuilder) {
    /// Get a textual representation of the call stack.
    ///
    /// This is intended only for debugging purposes to display to a human and
    /// should not be considered stable or parseable.
    ///
    /// strip_frames will pop N frames from the top of the call stack, which can
    /// be useful to hide non-interesting lines - for example, strip_frames=1
    /// will hide the call to and location of `call_stack()` itself.
    fn call_stack(
        #[starlark(require=named, default = 0)] strip_frames: u32,
        eval: &mut Evaluator,
    ) -> anyhow::Result<String> {
        let mut stack = eval.call_stack();
        stack
            .frames
            .truncate(stack.frames.len().saturating_sub(strip_frames as usize));
        Ok(stack.to_string())
    }

    /// Get a structural representation of the n-th call stack frame.
    ///
    /// With `n=0` returns `call_stack_frame` itself.
    /// Returns `None` if `n` is greater than or equal to the stack size.
    fn call_stack_frame(
        #[starlark(require = pos)] n: u32,
        eval: &mut Evaluator,
    ) -> anyhow::Result<NoneOr<StackFrame>> {
        let stack = eval.call_stack();
        let n = n as usize;
        if n >= stack.frames.len() {
            return Ok(NoneOr::None);
        }
        match stack.frames.get(stack.frames.len() - n - 1) {
            Some(frame) => Ok(NoneOr::Other(StackFrame {
                name: frame.name.clone(),
                location: frame.location.clone(),
            })),

            None => Ok(NoneOr::None),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::global;
    use crate::assert::Assert;

    #[test]
    fn test_simple() {
        let mut a = Assert::new();
        a.globals_add(global);
        a.is_true(
            r#"
def foo():
    return bar()

def bar():
    s = call_stack()
    return all([
        "foo()" in s,
        "bar()" in s,
        "call_stack()" in s,
    ])

foo()
            "#,
        );
    }

    #[test]
    fn test_strip_one() {
        let mut a = Assert::new();
        a.globals_add(global);
        a.is_true(
            r#"
def foo():
    return bar()

def bar():
    s = call_stack(strip_frames=1)
    return all([
        "foo()" in s,
        "bar()" in s,
        "call_stack()" not in s,
    ])

foo()
            "#,
        );
    }

    #[test]
    fn test_strip_all() {
        let mut a = Assert::new();
        a.globals_add(global);
        a.is_true(
            r#"
def foo():
    return bar()

def bar():
    s = call_stack(strip_frames=10)
    return not bool(s)

foo()
            "#,
        );
    }

    #[test]
    fn test_call_stack_frame() {
        let mut a = Assert::new();
        a.globals_add(global);
        a.is_true(
            r#"
def foo():
    return bar()

def bar():
    return all([
            "call_stack_frame" == call_stack_frame(0).func_name,
            "assert.bzl" == call_stack_frame(0).module_path,
            "bar" == call_stack_frame(1).func_name,
            "assert.bzl" == call_stack_frame(1).module_path,
            "foo" == call_stack_frame(2).func_name,
            "assert.bzl" == call_stack_frame(2).module_path,
            None == call_stack_frame(3),
            None == call_stack_frame(4),
        ])

foo()
            "#,
        );
    }
}

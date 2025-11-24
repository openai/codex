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

//! Starlark call stack.

// FIXME: I think we should rewrite the CallStack stuff entirely:
// * Don't keep a call stack, just a call stack depth.
// * When people did StackGuard.inc just do CallStack.inc, we need less info
// once it's an int so can reuse.
// * When an exception happens, decorate it with the call stack on the way back
//   up, in eval_call.

use std::fmt;
use std::fmt::Debug;
use std::fmt::Display;

use crate::frame::Frame;

pub const CALL_STACK_TRACEBACK_PREFIX: &str = "Traceback (most recent call last):";

/// Owned call stack.
#[derive(Debug, Default, Clone, PartialEq, Eq, Hash)]
pub struct CallStack {
    /// The frames.
    pub frames: Vec<Frame>,
}

impl CallStack {
    /// Is the call stack empty?
    pub fn is_empty(&self) -> bool {
        self.frames.is_empty()
    }

    /// Take the contained frames.
    pub fn into_frames(self) -> Vec<Frame> {
        self.frames
    }
}

impl Display for CallStack {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if !self.frames.is_empty() {
            // Match Python output.
            writeln!(f, "{}", CALL_STACK_TRACEBACK_PREFIX)?;
            // TODO(nga): use real module name.
            let mut prev = "<module>";
            for x in &self.frames {
                x.write_two_lines("  ", prev, f)?;
                prev = &x.name;
            }
        }
        Ok(())
    }
}

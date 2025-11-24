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

use crate::eval::bc::addr::BcAddr;
use crate::eval::runtime::frame_span::FrameSpan;
use crate::values::FrozenRef;
use crate::values::FrozenStringValue;

/// Slow instruction arg: stored in the end of bytecode,
/// expensive to access. Used to implement errors.
#[derive(Default, Debug)]
pub(crate) struct BcInstrSlowArg {
    /// Instruction code span.
    pub(crate) span: FrameSpan,
    /// Spans when an instruction needs multiple spans.
    pub(crate) spans: Vec<FrameSpan>,
}

#[derive(Debug, Default)]
pub(crate) struct BcInstrEndArg {
    /// Offset of end instruction.
    pub(crate) end_addr: BcAddr,
    /// Spans of all instructions.
    pub(crate) slow_args: Vec<(BcAddr, BcInstrSlowArg)>,
    /// Frame local names.
    pub(crate) local_names: FrozenRef<'static, [FrozenStringValue]>,
}

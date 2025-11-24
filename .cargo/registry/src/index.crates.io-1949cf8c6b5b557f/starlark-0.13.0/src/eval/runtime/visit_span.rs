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

use starlark_syntax::syntax::def::DefParamIndices;
use starlark_syntax::syntax::def::DefRegularParamMode;

use crate::collections::symbol::symbol::Symbol;
use crate::environment::slots::ModuleSlotId;
use crate::eval::compiler::expr::CompareOp;
use crate::eval::compiler::span::IrSpanned;
use crate::eval::runtime::frame_span::FrameSpan;
use crate::values::typing::type_compiled::compiled::TypeCompiled;
use crate::values::FrozenRef;
use crate::values::FrozenValue;
use crate::values::FrozenValueTyped;
use crate::values::StarlarkValue;

/// Visitor for code spans in the IR.
pub(crate) trait VisitSpanMut {
    fn visit_spans(&mut self, visitor: &mut impl FnMut(&mut FrameSpan));
}

impl<V: VisitSpanMut> VisitSpanMut for IrSpanned<V> {
    fn visit_spans(&mut self, visitor: &mut impl FnMut(&mut FrameSpan)) {
        visitor(&mut self.span);
        self.node.visit_spans(visitor);
    }
}

impl VisitSpanMut for FrozenValue {
    fn visit_spans(&mut self, _visitor: &mut impl FnMut(&mut FrameSpan)) {}
}

impl VisitSpanMut for TypeCompiled<FrozenValue> {
    fn visit_spans(&mut self, _visitor: &mut impl FnMut(&mut FrameSpan)) {}
}

impl VisitSpanMut for String {
    fn visit_spans(&mut self, _visitor: &mut impl FnMut(&mut FrameSpan)) {}
}

impl VisitSpanMut for bool {
    fn visit_spans(&mut self, _visitor: &mut impl FnMut(&mut FrameSpan)) {}
}

impl VisitSpanMut for u32 {
    fn visit_spans(&mut self, _visitor: &mut impl FnMut(&mut FrameSpan)) {}
}

impl VisitSpanMut for ModuleSlotId {
    fn visit_spans(&mut self, _visitor: &mut impl FnMut(&mut FrameSpan)) {}
}

impl VisitSpanMut for CompareOp {
    fn visit_spans(&mut self, _visitor: &mut impl FnMut(&mut FrameSpan)) {}
}

impl<V: VisitSpanMut> VisitSpanMut for Box<V> {
    fn visit_spans(&mut self, visitor: &mut impl FnMut(&mut FrameSpan)) {
        (**self).visit_spans(visitor);
    }
}

impl<T: StarlarkValue<'static>> VisitSpanMut for FrozenValueTyped<'static, T> {
    fn visit_spans(&mut self, _visitor: &mut impl FnMut(&mut FrameSpan)) {}
}

impl<T> VisitSpanMut for FrozenRef<'static, T> {
    fn visit_spans(&mut self, _visitor: &mut impl FnMut(&mut FrameSpan)) {}
}

impl VisitSpanMut for Symbol {
    fn visit_spans(&mut self, _visitor: &mut impl FnMut(&mut FrameSpan)) {}
}

impl<A: VisitSpanMut, B: VisitSpanMut> VisitSpanMut for (A, B) {
    fn visit_spans(&mut self, visitor: &mut impl FnMut(&mut FrameSpan)) {
        self.0.visit_spans(visitor);
        self.1.visit_spans(visitor);
    }
}

impl<A: VisitSpanMut, B: VisitSpanMut, C: VisitSpanMut> VisitSpanMut for (A, B, C) {
    fn visit_spans(&mut self, visitor: &mut impl FnMut(&mut FrameSpan)) {
        self.0.visit_spans(visitor);
        self.1.visit_spans(visitor);
        self.2.visit_spans(visitor);
    }
}

impl<A: VisitSpanMut, B: VisitSpanMut, C: VisitSpanMut, D: VisitSpanMut> VisitSpanMut
    for (A, B, C, D)
{
    fn visit_spans(&mut self, visitor: &mut impl FnMut(&mut FrameSpan)) {
        self.0.visit_spans(visitor);
        self.1.visit_spans(visitor);
        self.2.visit_spans(visitor);
        self.3.visit_spans(visitor);
    }
}

impl<V: VisitSpanMut> VisitSpanMut for Vec<V> {
    fn visit_spans(&mut self, visitor: &mut impl FnMut(&mut FrameSpan)) {
        for v in self {
            v.visit_spans(visitor);
        }
    }
}

impl<V: VisitSpanMut> VisitSpanMut for Option<V> {
    fn visit_spans(&mut self, visitor: &mut impl FnMut(&mut FrameSpan)) {
        if let Some(v) = self {
            v.visit_spans(visitor);
        }
    }
}

impl VisitSpanMut for DefRegularParamMode {
    fn visit_spans(&mut self, _visitor: &mut impl FnMut(&mut FrameSpan)) {}
}

impl VisitSpanMut for DefParamIndices {
    fn visit_spans(&mut self, _visitor: &mut impl FnMut(&mut FrameSpan)) {}
}

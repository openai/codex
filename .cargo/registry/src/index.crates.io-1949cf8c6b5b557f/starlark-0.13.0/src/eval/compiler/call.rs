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

//! Compile function calls.

use starlark_derive::VisitSpanMut;
use starlark_syntax::slice_vec_ext::VecExt;

use crate::collections::symbol::symbol::Symbol;
use crate::eval::compiler::args::ArgsCompiledValue;
use crate::eval::compiler::def_inline::local_as_value::local_as_value;
use crate::eval::compiler::def_inline::InlineDefBody;
use crate::eval::compiler::def_inline::InlineDefCallSite;
use crate::eval::compiler::expr::Builtin1;
use crate::eval::compiler::expr::ExprCompiled;
use crate::eval::compiler::opt_ctx::OptCtx;
use crate::eval::compiler::span::IrSpanned;
use crate::eval::runtime::frame_span::FrameSpan;
use crate::eval::runtime::inlined_frame::InlinedFrameAlloc;
use crate::eval::runtime::visit_span::VisitSpanMut;
use crate::values::enumeration::FrozenEnumType;
use crate::values::string::dot_format::parse_format_one;
use crate::values::FrozenStringValue;
use crate::values::FrozenValue;
use crate::values::Value;

#[derive(Clone, Debug, VisitSpanMut)]
pub(crate) struct CallCompiled {
    pub(crate) fun: IrSpanned<ExprCompiled>,
    pub(crate) args: ArgsCompiledValue,
}

impl CallCompiled {
    pub(crate) fn new_method(
        span: FrameSpan,
        this: IrSpanned<ExprCompiled>,
        field: &Symbol,
        getattr_span: FrameSpan,
        args: ArgsCompiledValue,
        ctx: &mut OptCtx,
    ) -> ExprCompiled {
        if let Some(this) = this.as_value() {
            if let Some(v) = ExprCompiled::compile_time_getattr(this, field, ctx) {
                let v = ExprCompiled::Value(v);
                let v = IrSpanned {
                    span: getattr_span,
                    node: v,
                };
                return CallCompiled::call(span, v, args, ctx);
            }
        }

        ExprCompiled::Call(Box::new(IrSpanned {
            span,
            node: CallCompiled {
                fun: IrSpanned {
                    span: getattr_span,
                    node: ExprCompiled::dot(this, field, ctx),
                },
                args,
            },
        }))
    }

    /// If this call expression is `len(x)`, return `x`.
    pub(crate) fn as_len(&self) -> Option<&IrSpanned<ExprCompiled>> {
        if !self.fun.is_fn_len() {
            return None;
        }
        self.args.one_pos()
    }

    /// If this call expression is `type(x)`, return `x`.
    pub(crate) fn as_type(&self) -> Option<&IrSpanned<ExprCompiled>> {
        if !self.fun.is_fn_type() {
            return None;
        }
        self.args.one_pos()
    }

    /// If this call expression is `isinstance(x, t)`, return `(x, t)`.
    pub(crate) fn as_isinstance(&self) -> Option<(&IrSpanned<ExprCompiled>, FrozenValue)> {
        if !self.fun.is_fn_isinstance() {
            return None;
        }
        let (x, t) = self.args.two_pos()?;
        let t = t.as_value()?;
        Some((x, t))
    }

    /// This call is infallible and has no side effects.
    pub(crate) fn is_pure_infallible(&self) -> bool {
        match self.as_type() {
            Some(arg) => arg.is_pure_infallible(),
            None => false,
        }
    }

    /// This call is a method call.
    pub(crate) fn method(&self) -> Option<(&IrSpanned<ExprCompiled>, &Symbol, &ArgsCompiledValue)> {
        match &self.fun.node {
            ExprCompiled::Builtin1(Builtin1::Dot(name), expr) => Some((expr, name, &self.args)),
            _ => None,
        }
    }

    /// Try to inline a function like `lambda x: type(x) == "y"`.
    fn try_type_is(fun: &ExprCompiled, args: &ArgsCompiledValue) -> Option<ExprCompiled> {
        let fun = fun.as_frozen_def()?;
        let pos = args.one_pos()?;
        if let Some(InlineDefBody::ReturnTypeIs(t)) = &fun.def_info.inline_def_body {
            Some(ExprCompiled::type_is(pos.clone(), *t))
        } else {
            None
        }
    }

    /// Inline calls to functions which are safe to inline.
    fn try_inline(
        span: FrameSpan,
        fun: &ExprCompiled,
        args: &ArgsCompiledValue,
        ctx: &mut OptCtx,
    ) -> Option<IrSpanned<ExprCompiled>> {
        let fun = fun.as_frozen_def()?;

        if fun.parameters.has_args_or_kwargs() {
            // Functions with `*args` or `**kwargs` are not marked safe to inline,
            // but it is safer to also handle it explicitly here.
            return None;
        }

        let expr = if let Some(InlineDefBody::ReturnSafeToInlineExpr(expr)) =
            &fun.def_info.inline_def_body
        {
            expr
        } else {
            return None;
        };

        let param_count = ctx.param_count;
        let expr_to_value = |expr: &ExprCompiled| -> Option<Value> {
            match expr {
                ExprCompiled::Value(v) => Some(v.to_value()),
                ExprCompiled::Local(local) if local.0 < param_count => {
                    // Definitely assigned local variable.
                    //
                    // Consider this example:
                    // ```
                    // def foo(x): bar() + x
                    // ```
                    // We can inline calls like `foo(x)` when `x` is definitely assigned,
                    // but if `x` is not we cannot do that, because
                    // we should emit `x` is not assigned error before call to `bar()`
                    // which may fail. We can implement inlining of variables which
                    // may be not assigned, or inlining of any expression arguments,
                    // but more work is needed for that.
                    Some(local_as_value(*local)?.to_value())
                }
                _ => None,
            }
        };

        args.all_values_generic(expr_to_value, |arguments| {
            let mut slots = vec![None; fun.parameters.len()];
            fun.parameters
                .collect(arguments.frozen_to_v(), &mut slots, ctx.heap())
                .ok()?;

            let slots = slots
                .into_try_map(|value| {
                    // Value must be set, but better ignore optimization here than panic.
                    let value = value.ok_or(())?;
                    // Everything should be frozen here, but if not,
                    // it is safer to abandon optimization.
                    value.unpack_frozen().ok_or(())
                })
                .ok()?;

            let mut expr = IrSpanned {
                span,
                node: expr.node.clone(),
            };
            let mut span_alloc = InlinedFrameAlloc::new(ctx.frozen_heap());
            expr.visit_spans(&mut |expr_span: &mut FrameSpan| {
                expr_span
                    .inlined_frames
                    .inline_into(span, fun.to_frozen_value(), &mut span_alloc);
            });
            InlineDefCallSite { ctx, slots: &slots }.inline(&expr).ok()
        })?
    }

    fn try_spec_exec(
        span: FrameSpan,
        fun: &ExprCompiled,
        args: &ArgsCompiledValue,
        ctx: &mut OptCtx,
    ) -> Option<ExprCompiled> {
        let fun = fun.as_value()?;

        if !fun.speculative_exec_safe() {
            return None;
        }

        let eval = ctx.eval()?;

        // Only if all call arguments are frozen values.
        args.all_values(|arguments| {
            let v = fun.to_value().invoke(arguments.frozen_to_v(), eval).ok()?;
            ExprCompiled::try_value(span, v, eval.module_env.frozen_heap())
        })?
    }

    // Optimize `MyEnum(arg)`.
    fn try_enum_value(
        fun: &IrSpanned<ExprCompiled>,
        args: &ArgsCompiledValue,
    ) -> Option<ExprCompiled> {
        let fun = fun.as_value()?.downcast_frozen_ref::<FrozenEnumType>()?;
        let arg = args.one_pos()?.as_value()?;
        Some(ExprCompiled::Value(
            fun.value.construct(arg.to_value()).ok()?,
        ))
    }

    // Optimize `"aaa{}bbb".format(arg)`.
    fn try_format(
        fun: &IrSpanned<ExprCompiled>,
        args: &ArgsCompiledValue,
        ctx: &mut OptCtx,
    ) -> Option<ExprCompiled> {
        let fun = fun.as_frozen_bound_method()?;
        let format = FrozenStringValue::new(fun.this)?;
        if fun.method.name != "format" {
            return None;
        }
        let arg = args.one_pos()?;

        let (before, after) = parse_format_one(&format)?;

        let before = ctx.frozen_heap().alloc_str_intern(&before);
        let after = ctx.frozen_heap().alloc_str_intern(&after);
        Some(ExprCompiled::format_one(before, arg.clone(), after, ctx))
    }

    pub(crate) fn call(
        span: FrameSpan,
        fun: IrSpanned<ExprCompiled>,
        args: ArgsCompiledValue,
        ctx: &mut OptCtx,
    ) -> ExprCompiled {
        if let Some(type_is) = CallCompiled::try_type_is(&fun, &args) {
            return type_is;
        }

        if let Some(inline) = CallCompiled::try_inline(span, &fun, &args, ctx) {
            return inline.node;
        }

        if fun.is_fn_len() {
            if let Some(arg) = args.one_pos() {
                return ExprCompiled::len(span, arg.clone());
            }
        }

        if fun.is_fn_type() {
            if let Some(arg) = args.one_pos() {
                return ExprCompiled::typ(span, arg.clone());
            }
        }

        if let Some(r) = CallCompiled::try_enum_value(&fun, &args) {
            return r;
        }

        if let Some(r) = CallCompiled::try_spec_exec(span, &fun, &args, ctx) {
            return r;
        }

        if let Some(r) = CallCompiled::try_format(&fun, &args, ctx) {
            return r;
        }

        if let ExprCompiled::Builtin1(Builtin1::Dot(field), this) = &fun.node {
            return CallCompiled::new_method(span, (**this).clone(), field, fun.span, args, ctx);
        }

        ExprCompiled::Call(Box::new(IrSpanned {
            span,
            node: CallCompiled { fun, args },
        }))
    }
}

impl IrSpanned<CallCompiled> {
    pub(crate) fn optimize(&self, ctx: &mut OptCtx) -> ExprCompiled {
        let CallCompiled { fun: expr, args } = &self.node;
        let expr = expr.optimize(ctx);
        let args = args.optimize(ctx);
        CallCompiled::call(self.span, expr, args, ctx)
    }
}

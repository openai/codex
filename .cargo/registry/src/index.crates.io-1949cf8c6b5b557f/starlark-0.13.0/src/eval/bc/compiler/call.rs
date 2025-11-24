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

use either::Either;

use crate::collections::symbol::symbol::Symbol;
use crate::eval::bc::call::BcCallArgsFull;
use crate::eval::bc::call::BcCallArgsPos;
use crate::eval::bc::compiler::expr::write_expr_opt;
use crate::eval::bc::compiler::expr::write_exprs;
use crate::eval::bc::instr_impl::InstrCall;
use crate::eval::bc::instr_impl::InstrCallFrozen;
use crate::eval::bc::instr_impl::InstrCallFrozenDef;
use crate::eval::bc::instr_impl::InstrCallFrozenDefPos;
use crate::eval::bc::instr_impl::InstrCallFrozenNative;
use crate::eval::bc::instr_impl::InstrCallFrozenNativePos;
use crate::eval::bc::instr_impl::InstrCallFrozenPos;
use crate::eval::bc::instr_impl::InstrCallMaybeKnownMethod;
use crate::eval::bc::instr_impl::InstrCallMaybeKnownMethodPos;
use crate::eval::bc::instr_impl::InstrCallMethod;
use crate::eval::bc::instr_impl::InstrCallMethodPos;
use crate::eval::bc::instr_impl::InstrCallPos;
use crate::eval::bc::instr_impl::InstrIsInstance;
use crate::eval::bc::instr_impl::InstrLen;
use crate::eval::bc::instr_impl::InstrType;
use crate::eval::bc::native_function::BcNativeFunction;
use crate::eval::bc::stack_ptr::BcSlotOut;
use crate::eval::bc::writer::BcWriter;
use crate::eval::compiler::args::ArgsCompiledValue;
use crate::eval::compiler::call::CallCompiled;
use crate::eval::compiler::def::FrozenDef;
use crate::eval::compiler::expr::ExprCompiled;
use crate::eval::compiler::span::IrSpanned;
use crate::eval::runtime::frame_span::FrameSpan;
use crate::values::function::NativeFunction;
use crate::values::types::known_methods::get_known_method;
use crate::values::typing::type_compiled::compiled::TypeCompiled;
use crate::values::FrozenValue;
use crate::values::FrozenValueTyped;

impl ArgsCompiledValue {
    /// After evaluation of function arguments like `foo(a, b=c[d], **e)`,
    /// variables `a`, `b`, `c`, `d`, and `e` are definitely assigned.
    fn mark_definitely_assigned_after(&self, bc: &mut BcWriter) {
        let ArgsCompiledValue {
            pos_named,
            names,
            args,
            kwargs,
        } = self;
        for n in pos_named {
            n.mark_definitely_assigned_after(bc);
        }
        let _ = names;
        if let Some(args) = args {
            args.mark_definitely_assigned_after(bc);
        }
        if let Some(kwargs) = kwargs {
            kwargs.mark_definitely_assigned_after(bc);
        }
    }

    fn write_bc(&self, bc: &mut BcWriter, k: impl FnOnce(BcCallArgsFull<Symbol>, &mut BcWriter)) {
        write_exprs(&self.pos_named, bc, |pos_named, bc| {
            write_expr_opt(&self.args, bc, |args, bc| {
                write_expr_opt(&self.kwargs, bc, |kwargs, bc| {
                    let args_full = BcCallArgsFull {
                        pos_named,
                        names: self.names.clone().into_boxed_slice(),
                        args,
                        kwargs,
                    };
                    k(args_full, bc)
                })
            })
        });
    }
}

impl CallCompiled {
    /// After evaluation of call like `a[b](c.d)`,
    /// variables `a`, `b`, and `c` are definitely assigned.
    pub(crate) fn mark_definitely_assigned_after(&self, bc: &mut BcWriter) {
        let CallCompiled { fun, args } = self;
        fun.mark_definitely_assigned_after(bc);
        args.mark_definitely_assigned_after(bc);
    }
}

impl IrSpanned<CallCompiled> {
    fn write_args(
        args: &ArgsCompiledValue,
        bc: &mut BcWriter,
        k: impl FnOnce(Either<BcCallArgsPos, BcCallArgsFull<Symbol>>, &mut BcWriter),
    ) {
        if let Some(pos) = args.pos_only() {
            write_exprs(pos, bc, |pos, bc| {
                let args = Either::Left(BcCallArgsPos { pos });
                k(args, bc)
            })
        } else {
            args.write_bc(bc, |args_full, bc| {
                let args = Either::Right(args_full);
                k(args, bc)
            })
        }
    }

    fn write_call_frozen(
        span: FrameSpan,
        fun: FrozenValue,
        args: &ArgsCompiledValue,
        target: BcSlotOut,
        bc: &mut BcWriter,
    ) {
        let file_span = bc.alloc_file_span(span);
        if let Some(fun) = FrozenValueTyped::<FrozenDef>::new(fun) {
            Self::write_args(args, bc, |args, bc| match args {
                Either::Left(npops) => {
                    bc.write_instr::<InstrCallFrozenDefPos>(span, (fun, npops, file_span, target))
                }
                Either::Right(args) => bc.write_instr::<InstrCallFrozenDef>(
                    span,
                    (fun, args.resolve(fun.as_ref()), file_span, target),
                ),
            })
        } else if let Some(fun) = FrozenValueTyped::<NativeFunction>::new(fun) {
            let fun = BcNativeFunction::new(fun);
            Self::write_args(args, bc, |args, bc| match args {
                Either::Left(npops) => {
                    bc.write_instr::<InstrCallFrozenNativePos>(
                        span,
                        (fun, npops, file_span, target),
                    );
                }
                Either::Right(args) => {
                    bc.write_instr::<InstrCallFrozenNative>(span, (fun, args, file_span, target));
                }
            })
        } else {
            Self::write_args(args, bc, |args, bc| match args {
                Either::Left(npops) => {
                    bc.write_instr::<InstrCallFrozenPos>(span, (fun, npops, file_span, target));
                }
                Either::Right(args) => {
                    bc.write_instr::<InstrCallFrozen>(span, (fun, args, file_span, target));
                }
            })
        }
    }

    fn write_call_method(
        target: BcSlotOut,
        span: FrameSpan,
        this: &IrSpanned<ExprCompiled>,
        symbol: &Symbol,
        args: &ArgsCompiledValue,
        bc: &mut BcWriter,
    ) {
        this.write_bc_cb(bc, |this, bc| {
            let file_span = bc.alloc_file_span(span);
            let known_method = get_known_method(symbol.as_str());
            if let Some(pos) = args.pos_only() {
                write_exprs(pos, bc, |pos, bc| {
                    if let Some(known_method) = known_method {
                        bc.write_instr::<InstrCallMaybeKnownMethodPos>(
                            span,
                            (
                                this,
                                symbol.clone(),
                                known_method,
                                BcCallArgsPos { pos },
                                file_span,
                                target,
                            ),
                        );
                    } else {
                        bc.write_instr::<InstrCallMethodPos>(
                            span,
                            (
                                this,
                                symbol.clone(),
                                BcCallArgsPos { pos },
                                file_span,
                                target,
                            ),
                        );
                    }
                });
            } else {
                args.write_bc(bc, |args, bc| {
                    if let Some(known_method) = known_method {
                        bc.write_instr::<InstrCallMaybeKnownMethod>(
                            span,
                            (this, symbol.clone(), known_method, args, file_span, target),
                        );
                    } else {
                        bc.write_instr::<InstrCallMethod>(
                            span,
                            (this, symbol.clone(), args, file_span, target),
                        );
                    }
                })
            }
        })
    }

    pub(crate) fn write_bc(&self, target: BcSlotOut, bc: &mut BcWriter) {
        if let Some(arg) = self.as_len() {
            return arg.write_bc_cb(bc, |arg, bc| {
                bc.write_instr::<InstrLen>(self.span, (arg, target));
            });
        } else if let Some(arg) = self.as_type() {
            return arg.write_bc_cb(bc, |arg, bc| {
                bc.write_instr::<InstrType>(self.span, (arg, target));
            });
        } else if let Some((x, t)) = self.as_isinstance() {
            if let Ok(t) = TypeCompiled::new_frozen(t, bc.heap) {
                return x.write_bc_cb(bc, |x, bc| {
                    bc.write_instr::<InstrIsInstance>(self.span, (x, t, target));
                });
            }
        }

        let span = self.span;
        let file_span = bc.alloc_file_span(span);
        match self.method() {
            None => match self.fun.as_value() {
                Some(f) => Self::write_call_frozen(span, f, &self.args, target, bc),
                None => {
                    self.fun.write_bc_cb(bc, |fun, bc| {
                        Self::write_args(&self.args, bc, |args, bc| match args {
                            Either::Left(npops) => bc
                                .write_instr::<InstrCallPos>(span, (fun, npops, file_span, target)),
                            Either::Right(args) => {
                                bc.write_instr::<InstrCall>(span, (fun, args, file_span, target));
                            }
                        })
                    });
                }
            },
            Some((this, symbol, args)) => {
                Self::write_call_method(target, span, this, symbol, args, bc)
            }
        }
    }
}

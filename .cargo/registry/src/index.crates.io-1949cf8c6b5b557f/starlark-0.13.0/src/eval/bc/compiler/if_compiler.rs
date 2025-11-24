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

use crate::eval::bc::instrs::PatchAddr;
use crate::eval::bc::writer::BcWriter;
use crate::eval::compiler::expr::Builtin1;
use crate::eval::compiler::expr::ExprCompiled;
use crate::eval::compiler::expr::ExprLogicalBinOp;
use crate::eval::compiler::expr::MaybeNot;
use crate::eval::compiler::span::IrSpanned;

/// Common code for compiling if statements and if expressions.
pub(crate) fn write_if_else(
    c: &IrSpanned<ExprCompiled>,
    t: impl FnOnce(&mut BcWriter),
    f: impl FnOnce(&mut BcWriter),
    bc: &mut BcWriter,
) {
    write_if_else_impl(c, MaybeNot::Id, t, Some(f), bc);
}

/// Common code for compiling if statements and if conditions in comprehensions.
pub(crate) fn write_if_then(
    c: &IrSpanned<ExprCompiled>,
    maybe_not: MaybeNot,
    t: impl FnOnce(&mut BcWriter),
    bc: &mut BcWriter,
) {
    // Deal with some typechecker issue.
    fn wr<T, F>(c: &IrSpanned<ExprCompiled>, maybe_not: MaybeNot, t: T, _f: F, bc: &mut BcWriter)
    where
        T: FnOnce(&mut BcWriter),
        F: FnOnce(&mut BcWriter),
    {
        write_if_else_impl::<T, F>(c, maybe_not, t, None, bc);
    }

    wr(c, maybe_not, t, |_| unreachable!(), bc);
}

/// Common code for writing if-then or if-then-else expression or statement.
fn write_if_else_impl<T, F>(
    cond: &IrSpanned<ExprCompiled>,
    maybe_not: MaybeNot,
    t: T,
    f: Option<F>,
    bc: &mut BcWriter,
) where
    T: FnOnce(&mut BcWriter),
    F: FnOnce(&mut BcWriter),
{
    let mut then_addrs = Vec::new();
    let mut else_addrs = Vec::new();

    write_cond(cond, maybe_not, &mut then_addrs, &mut else_addrs, bc);

    let definitely_assigned = bc.save_definitely_assigned();

    bc.patch_addrs(then_addrs);
    t(bc);
    if let Some(f) = f {
        let end_addr = bc.write_br(cond.span);

        bc.restore_definitely_assigned(definitely_assigned.clone());

        bc.patch_addrs(else_addrs);
        f(bc);

        bc.patch_addr(end_addr);
    } else {
        bc.patch_addrs(else_addrs);
    }

    bc.restore_definitely_assigned(definitely_assigned);
}

/// Write boolean binary condition.
///
/// The condition is: `maybe_not(x bin_op y)`.
///
/// See `write_cond` for semantics of `t`, `f` parameters.
fn write_cond_bin_op(
    x: &IrSpanned<ExprCompiled>,
    y: &IrSpanned<ExprCompiled>,
    bin_op: ExprLogicalBinOp,
    maybe_not: MaybeNot,
    t: &mut Vec<PatchAddr>,
    f: &mut Vec<PatchAddr>,
    bc: &mut BcWriter,
) {
    if (bin_op == ExprLogicalBinOp::And) == (maybe_not == MaybeNot::Id) {
        // This branch handles either of expressions:
        // expression   | bin_op | maybe_not
        // --------------+-------+----------
        // x and y      | and    | id
        // not (x or y) | or     | not

        let mut x_skip = Vec::new();
        write_cond(x, maybe_not, &mut x_skip, f, bc);
        bc.patch_addrs(x_skip);

        write_cond(y, maybe_not, t, f, bc);
    } else {
        // This branch handles either of expressions:
        // expression    | bin_op | maybe_not
        // --------------+-----+--+----------
        // x or y        | or     | id
        // not (x and y) | and    | not

        let mut x_skip = Vec::new();
        write_cond(x, maybe_not.negate(), &mut x_skip, t, bc);
        bc.patch_addrs(x_skip);

        write_cond(y, maybe_not, t, f, bc);
    }
}

/// Write if condition bytecode.
///
/// The condition is `maybe_not(cond)`.
///
/// This function assumes there are two address:
/// * address of then block
/// * address of else block
///
/// Generated code will:
/// * jump to else address if condition is false
/// * jump to then address **or** fall through if condition is true
///
/// This function will populate `t` and `f` with addresses of instructions
/// which jump to then or else block respectively. Caller needs to patch these.
fn write_cond(
    cond: &IrSpanned<ExprCompiled>,
    maybe_not: MaybeNot,
    t: &mut Vec<PatchAddr>,
    f: &mut Vec<PatchAddr>,
    bc: &mut BcWriter,
) {
    match &cond.node {
        ExprCompiled::Builtin1(Builtin1::Not, cond) => {
            write_cond(cond, maybe_not.negate(), t, f, bc);
        }
        ExprCompiled::LogicalBinOp(op, x_y) => {
            let (x, y) = &**x_y;
            write_cond_bin_op(x, y, *op, maybe_not, t, f, bc);
        }
        _ => {
            cond.write_bc_cb(bc, |cond_slot, bc| {
                let addr = match maybe_not {
                    MaybeNot::Id => bc.write_if_not_br(cond_slot, cond.span),
                    MaybeNot::Not => bc.write_if_br(cond_slot, cond.span),
                };
                f.push(addr);
            });
        }
    }
}

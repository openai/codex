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

use crate::typing::Ty;
use crate::typing::TyBasic;
use crate::typing::TypingBinOp;

pub(crate) enum NumTy {
    Int,
    Float,
}

enum NumRhsTy {
    Num(NumTy),
    Any,
}

fn int_or_float() -> Ty {
    Ty::union2(Ty::int(), Ty::float())
}

/// Group of operators sharing the typing behavior.
enum BinOpClass {
    /// If any operand is a float, the result is a float.
    Add,
    /// Result is always a float.
    Div,
    /// Only supported for integers.
    BitAnd,
    /// Not supported.
    In,
    /// Supported.
    Less,
}

pub(crate) fn typecheck_num_bin_op(lhs: NumTy, op: TypingBinOp, rhs: &TyBasic) -> Option<Ty> {
    let rhs = if rhs == &TyBasic::Any {
        NumRhsTy::Any
    } else if rhs == &TyBasic::int() {
        NumRhsTy::Num(NumTy::Int)
    } else if rhs == &TyBasic::float() {
        NumRhsTy::Num(NumTy::Float)
    } else {
        return None;
    };

    let op = match op {
        TypingBinOp::Add
        | TypingBinOp::Sub
        | TypingBinOp::Mul
        | TypingBinOp::FloorDiv
        | TypingBinOp::Percent => BinOpClass::Add,
        TypingBinOp::Div => BinOpClass::Div,
        TypingBinOp::BitOr
        | TypingBinOp::BitXor
        | TypingBinOp::BitAnd
        | TypingBinOp::LeftShift
        | TypingBinOp::RightShift => BinOpClass::BitAnd,
        TypingBinOp::In => BinOpClass::In,
        TypingBinOp::Less => BinOpClass::Less,
    };

    match (lhs, op, rhs) {
        (_, BinOpClass::In, _) => None,
        (_, BinOpClass::Less, _) => Some(Ty::bool()),
        (NumTy::Float, BinOpClass::Add, _) => Some(Ty::float()),
        (NumTy::Int, BinOpClass::Add, NumRhsTy::Num(NumTy::Int)) => Some(Ty::int()),
        (_, BinOpClass::Add, NumRhsTy::Num(NumTy::Float)) => Some(Ty::float()),
        (_, BinOpClass::Add, NumRhsTy::Any) => Some(int_or_float()),
        (_, BinOpClass::Div, _) => Some(Ty::float()),
        (NumTy::Int, BinOpClass::BitAnd, NumRhsTy::Num(NumTy::Int) | NumRhsTy::Any) => {
            Some(Ty::int())
        }
        (_, BinOpClass::BitAnd, _) => None,
    }
}

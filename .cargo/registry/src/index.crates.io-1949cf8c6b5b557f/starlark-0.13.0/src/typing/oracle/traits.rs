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

// This makes for a better API.
#![allow(clippy::result_unit_err)]

use dupe::Dupe;

/// Unary operator for typechecker.
#[derive(Copy, Clone, Dupe, Eq, PartialEq, derive_more::Display, Debug)]
pub enum TypingUnOp {
    /// `+`.
    #[display("+")]
    Plus,
    /// `+`.
    #[display("-")]
    Minus,
    /// `~`.
    #[display("~")]
    BitNot,
}

/// Binary operator for typechecker.
#[derive(Copy, Clone, Dupe, Eq, PartialEq, derive_more::Display, Debug)]
pub enum TypingBinOp {
    /// `+`.
    #[display("+")]
    Add,
    /// `-`.
    #[display("-")]
    Sub,
    /// `/`.
    #[display("/")]
    Div,
    /// `//`.
    #[display("/")]
    FloorDiv,
    /// `*`.
    #[display("*")]
    Mul,
    /// `%`.
    #[display("%")]
    Percent,
    /// `y in x`.
    #[display("in")]
    In,
    /// `|`.
    #[display("|")]
    BitOr,
    /// `^`.
    #[display("^")]
    BitXor,
    /// `&`.
    #[display("&")]
    BitAnd,
    /// `<`.
    #[display("<")]
    Less,
    /// `<<`.
    #[display("<<")]
    LeftShift,
    /// `>>`.
    #[display(">>")]
    RightShift,
}

impl TypingBinOp {
    /// Result type is always `bool`.
    pub(crate) fn always_bool(self) -> bool {
        matches!(self, TypingBinOp::In | TypingBinOp::Less)
    }
}

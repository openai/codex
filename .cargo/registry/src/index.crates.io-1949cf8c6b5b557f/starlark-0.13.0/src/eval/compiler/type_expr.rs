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

use starlark_derive::VisitSpanMut;

use crate::eval::compiler::expr::ExprCompiled;
use crate::eval::compiler::span::IrSpanned;
use crate::values::typing::TypeCompiled;

/// IR expression in type position.
#[derive(Clone, Debug, VisitSpanMut)]
pub(crate) struct TypeExprCompiled {
    expr: IrSpanned<ExprCompiled>,
}

impl TypeExprCompiled {
    pub(crate) fn new_expr(expr: IrSpanned<ExprCompiled>) -> TypeExprCompiled {
        TypeExprCompiled { expr }
    }

    /// True if type is known to match any type.
    pub(crate) fn is_wildcard(&self) -> bool {
        if let Some(value) = self.expr.as_value() {
            TypeCompiled::is_wildcard_value(value.to_value())
        } else {
            false
        }
    }
}

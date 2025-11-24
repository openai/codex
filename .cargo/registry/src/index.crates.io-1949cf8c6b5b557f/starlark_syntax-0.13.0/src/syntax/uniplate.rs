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

// These are more readable for formulaic code like Uniplate
#![allow(clippy::many_single_char_names)]
// Sometimes we need |x| f(x) to do type/lifetime conversion, sometimes we don't.
// Most consistent to use the closure everywhere.
#![allow(clippy::redundant_closure)]

use crate::syntax::ast::AssignP;
use crate::syntax::ast::AssignTargetP;
use crate::syntax::ast::AstAssignIdentP;
use crate::syntax::ast::AstExprP;
use crate::syntax::ast::AstIdentP;
use crate::syntax::ast::AstPayload;
use crate::syntax::ast::AstStmtP;
use crate::syntax::ast::AstTypeExprP;
use crate::syntax::ast::ClauseP;
use crate::syntax::ast::DefP;
use crate::syntax::ast::ExprP;
use crate::syntax::ast::ForClauseP;
use crate::syntax::ast::ForP;
use crate::syntax::ast::LambdaP;
use crate::syntax::ast::ParameterP;
use crate::syntax::ast::StmtP;
use crate::syntax::ast::TypeExprP;

pub enum Visit<'a, P: AstPayload> {
    Stmt(&'a AstStmtP<P>),
    Expr(&'a AstExprP<P>),
}

pub enum VisitMut<'a, P: AstPayload> {
    Stmt(&'a mut AstStmtP<P>),
    Expr(&'a mut AstExprP<P>),
}

impl<'a, P: AstPayload> Visit<'a, P> {
    pub fn visit_children(&self, mut f: impl FnMut(Visit<'a, P>)) {
        match self {
            Self::Stmt(x) => x.visit_children(f),
            Self::Expr(x) => x.visit_expr(|x| f(Visit::Expr(x))),
        }
    }

    pub fn visit_children_err<E>(
        &self,
        mut f: impl FnMut(Visit<'a, P>) -> Result<(), E>,
    ) -> Result<(), E> {
        match self {
            Self::Stmt(x) => x.visit_children_err(f),
            Self::Expr(x) => x.visit_expr_err(|x| f(Visit::Expr(x))),
        }
    }
}

impl<P: AstPayload> DefP<P> {
    fn visit_children<'a>(&'a self, mut f: impl FnMut(Visit<'a, P>)) {
        let DefP {
            name: _,
            params,
            return_type,
            body,
            payload: _,
        } = self;
        params
            .iter()
            .for_each(|x| x.visit_expr(|x| f(Visit::Expr(x))));
        return_type
            .iter()
            .for_each(|x| x.visit_expr(|x| f(Visit::Expr(x))));
        f(Visit::Stmt(body));
    }

    pub fn visit_children_err<'a, E>(
        &'a self,
        mut f: impl FnMut(Visit<'a, P>) -> Result<(), E>,
    ) -> Result<(), E> {
        let mut result = Ok(());
        self.visit_children(|x| {
            if result.is_ok() {
                result = f(x);
            }
        });
        result
    }
}

impl<P: AstPayload> StmtP<P> {
    pub fn visit_children<'a>(&'a self, mut f: impl FnMut(Visit<'a, P>)) {
        match self {
            StmtP::Statements(xs) => xs.iter().for_each(|x| f(Visit::Stmt(x))),
            StmtP::If(condition, then_block) => {
                f(Visit::Expr(condition));
                f(Visit::Stmt(then_block));
            }
            StmtP::IfElse(condition, then_block_else_block) => {
                let (then_block, else_block) = &**then_block_else_block;
                f(Visit::Expr(condition));
                f(Visit::Stmt(then_block));
                f(Visit::Stmt(else_block));
            }
            StmtP::Def(def) => def.visit_children(f),
            StmtP::For(ForP { var, over, body }) => {
                var.visit_expr(|x| f(Visit::Expr(x)));
                f(Visit::Expr(over));
                f(Visit::Stmt(body));
            }
            // Nothing else contains nested statements
            StmtP::Break => {}
            StmtP::Continue => {}
            StmtP::Pass => {}
            StmtP::Return(ret) => {
                ret.iter().for_each(|x| f(Visit::Expr(x)));
            }
            StmtP::Expression(e) => f(Visit::Expr(e)),
            StmtP::Assign(AssignP { lhs, ty, rhs }) => {
                lhs.visit_expr(|x| f(Visit::Expr(x)));
                ty.iter().for_each(|x| x.visit_expr(|x| f(Visit::Expr(x))));
                f(Visit::Expr(rhs));
            }
            StmtP::AssignModify(lhs, _, rhs) => {
                lhs.visit_expr(|x| f(Visit::Expr(x)));
                f(Visit::Expr(rhs));
            }
            StmtP::Load(..) => {}
        }
    }

    pub fn visit_children_mut<'a>(&'a mut self, mut f: impl FnMut(VisitMut<'a, P>)) {
        match self {
            StmtP::Statements(xs) => xs.iter_mut().for_each(|x| f(VisitMut::Stmt(x))),
            StmtP::If(condition, then_block) => {
                f(VisitMut::Expr(condition));
                f(VisitMut::Stmt(then_block));
            }
            StmtP::IfElse(condition, then_block_else_block) => {
                let (then_block, else_block) = &mut **then_block_else_block;
                f(VisitMut::Expr(condition));
                f(VisitMut::Stmt(then_block));
                f(VisitMut::Stmt(else_block));
            }
            StmtP::Def(DefP {
                name: _,
                params,
                return_type,
                body,
                payload: _,
            }) => {
                params
                    .iter_mut()
                    .for_each(|x| x.visit_expr_mut(|x| f(VisitMut::Expr(x))));
                return_type
                    .iter_mut()
                    .for_each(|x| x.visit_expr_mut(|x| f(VisitMut::Expr(x))));
                f(VisitMut::Stmt(body));
            }
            StmtP::For(ForP { var, over, body }) => {
                var.visit_expr_mut(|x| f(VisitMut::Expr(x)));
                f(VisitMut::Expr(over));
                f(VisitMut::Stmt(body));
            }
            // Nothing else contains nested statements
            StmtP::Break => {}
            StmtP::Continue => {}
            StmtP::Pass => {}
            StmtP::Return(ret) => {
                ret.iter_mut().for_each(|x| f(VisitMut::Expr(x)));
            }
            StmtP::Expression(e) => f(VisitMut::Expr(e)),
            StmtP::Assign(AssignP { lhs, ty, rhs }) => {
                lhs.visit_expr_mut(|x| f(VisitMut::Expr(x)));
                ty.iter_mut()
                    .for_each(|x| x.visit_expr_mut(|x| f(VisitMut::Expr(x))));
                f(VisitMut::Expr(rhs));
            }
            StmtP::AssignModify(lhs, _, rhs) => {
                lhs.visit_expr_mut(|x| f(VisitMut::Expr(x)));
                f(VisitMut::Expr(rhs));
            }
            StmtP::Load(..) => {}
        }
    }

    pub fn visit_children_err<'a, E>(
        &'a self,
        mut f: impl FnMut(Visit<'a, P>) -> Result<(), E>,
    ) -> Result<(), E> {
        let mut result = Ok(());
        self.visit_children(|x| {
            if result.is_ok() {
                result = f(x);
            }
        });
        result
    }

    pub fn visit_children_err_mut<'a, E>(
        &'a mut self,
        mut f: impl FnMut(VisitMut<'a, P>) -> Result<(), E>,
    ) -> Result<(), E> {
        let mut result = Ok(());
        self.visit_children_mut(|x| {
            if result.is_ok() {
                result = f(x);
            }
        });
        result
    }

    pub fn visit_stmt<'a>(&'a self, mut f: impl FnMut(&'a AstStmtP<P>)) {
        self.visit_children(|x| match x {
            Visit::Stmt(x) => f(x),
            Visit::Expr(_) => {} // Nothing to do
        })
    }

    pub fn visit_stmt_mut<'a>(&'a mut self, mut f: impl FnMut(&'a mut AstStmtP<P>)) {
        self.visit_children_mut(|x| match x {
            VisitMut::Stmt(x) => f(x),
            VisitMut::Expr(_) => {} // Nothing to do
        })
    }

    pub fn visit_expr<'a>(&'a self, mut f: impl FnMut(&'a AstExprP<P>)) {
        // Note the &mut impl on f, it's subtle, see
        // https://stackoverflow.com/questions/54613966/error-reached-the-recursion-limit-while-instantiating-funcclosure
        fn pick<'a, P: AstPayload>(x: Visit<'a, P>, f: &mut impl FnMut(&'a AstExprP<P>)) {
            match x {
                Visit::Stmt(x) => x.visit_children(|x| pick(x, f)),
                Visit::Expr(x) => f(x),
            }
        }
        self.visit_children(|x| pick(x, &mut f))
    }

    pub fn visit_expr_mut<'a>(&'a mut self, mut f: impl FnMut(&'a mut AstExprP<P>)) {
        // Note the &mut impl on f, it's subtle, see
        // https://stackoverflow.com/questions/54613966/error-reached-the-recursion-limit-while-instantiating-funcclosure
        fn pick<'a, P: AstPayload>(x: VisitMut<'a, P>, f: &mut impl FnMut(&'a mut AstExprP<P>)) {
            match x {
                VisitMut::Stmt(x) => x.visit_children_mut(|x| pick(x, f)),
                VisitMut::Expr(x) => f(x),
            }
        }
        self.visit_children_mut(|x| pick(x, &mut f))
    }

    pub fn visit_expr_result<'a, E>(
        &'a self,
        mut f: impl FnMut(&'a AstExprP<P>) -> Result<(), E>,
    ) -> Result<(), E> {
        let mut result = Ok(());
        self.visit_expr(|x| {
            if result.is_ok() {
                result = f(x);
            }
        });
        result
    }

    pub fn visit_stmt_result<E>(
        &self,
        mut f: impl FnMut(&AstStmtP<P>) -> Result<(), E>,
    ) -> Result<(), E> {
        let mut result = Ok(());
        let f2 = |x: &AstStmtP<P>| {
            if result.is_ok() {
                result = f(x);
            }
        };
        self.visit_stmt(f2);
        result
    }

    /// Visit all type expressions in this statement and its children.
    pub fn visit_type_expr_err_mut<E>(
        &mut self,
        f: &mut impl FnMut(&mut AstTypeExprP<P>) -> Result<(), E>,
    ) -> Result<(), E> {
        match self {
            StmtP::Def(def) => {
                for param in &mut def.params {
                    if let (_, Some(ty), _) = param.split_mut() {
                        f(ty)?;
                    }
                }
                if let Some(ty) = &mut def.return_type {
                    f(ty)?;
                }
            }
            StmtP::Assign(assign) => {
                if let Some(ty) = &mut assign.ty {
                    f(ty)?;
                }
            }
            _ => {}
        }
        self.visit_children_err_mut(|visit| match visit {
            VisitMut::Stmt(stmt) => stmt.visit_type_expr_err_mut(f),
            VisitMut::Expr(expr) => expr.visit_type_expr_err_mut(f),
        })
    }

    /// Visit all identifiers in read position recursively.
    pub fn visit_ident<E>(
        &self,
        mut f: impl FnMut(&AstIdentP<P>) -> Result<(), E>,
    ) -> Result<(), E> {
        self.visit_expr_result(|expr| expr.visit_ident(&mut f))
    }
}

impl<P: AstPayload> ParameterP<P> {
    // Split a parameter into name, type, default value
    pub fn split(
        &self,
    ) -> (
        Option<&AstAssignIdentP<P>>,
        Option<&AstTypeExprP<P>>,
        Option<&AstExprP<P>>,
    ) {
        match self {
            ParameterP::Normal(a, b, None) | ParameterP::Args(a, b) | ParameterP::KwArgs(a, b) => {
                (Some(a), b.as_ref().map(|x| &**x), None)
            }
            ParameterP::Normal(a, b, Some(c)) => (Some(a), b.as_ref().map(|x| &**x), Some(&**c)),
            ParameterP::NoArgs | ParameterP::Slash => (None, None, None),
        }
    }

    // Split a parameter into name, type, default value
    pub fn split_mut(
        &mut self,
    ) -> (
        Option<&mut AstAssignIdentP<P>>,
        Option<&mut AstTypeExprP<P>>,
        Option<&mut AstExprP<P>>,
    ) {
        match self {
            ParameterP::Normal(a, b, None) | ParameterP::Args(a, b) | ParameterP::KwArgs(a, b) => {
                (Some(a), b.as_mut().map(|x| &mut **x), None)
            }
            ParameterP::Normal(a, b, Some(c)) => {
                (Some(a), b.as_mut().map(|x| &mut **x), Some(&mut **c))
            }
            ParameterP::NoArgs | ParameterP::Slash => (None, None, None),
        }
    }

    pub fn visit_expr<'a>(&'a self, mut f: impl FnMut(&'a AstExprP<P>)) {
        let (_, typ, def) = self.split();
        typ.iter().for_each(|x| x.visit_expr(&mut f));
        def.iter().for_each(|x| f(x));
    }

    pub fn visit_expr_mut<'a>(&'a mut self, mut f: impl FnMut(&'a mut AstExprP<P>)) {
        let (_, typ, def) = self.split_mut();
        if let Some(typ) = typ {
            typ.visit_expr_mut(&mut f);
        }
        if let Some(def) = def {
            f(def);
        }
    }
}

impl<P: AstPayload> ExprP<P> {
    pub fn visit_expr<'a>(&'a self, mut f: impl FnMut(&'a AstExprP<P>)) {
        match self {
            ExprP::Tuple(xs) => xs.iter().for_each(|x| f(x)),
            ExprP::Dot(x, _) => f(x),
            ExprP::Call(a, b) => {
                f(a);
                b.args.iter().for_each(|x| f(x.expr()));
            }
            ExprP::Index(a_b) => {
                let (a, b) = &**a_b;
                f(a);
                f(b);
            }
            ExprP::Index2(a_i0_i1) => {
                let (a, i0, i1) = &**a_i0_i1;
                f(a);
                f(i0);
                f(i1);
            }
            ExprP::Slice(a, b, c, d) => {
                f(a);
                b.iter().for_each(|x| f(x));
                c.iter().for_each(|x| f(x));
                d.iter().for_each(|x| f(x));
            }
            ExprP::Identifier(..) => {}
            ExprP::Lambda(LambdaP {
                params,
                body,
                payload: _,
            }) => {
                params.iter().for_each(|x| x.visit_expr(|x| f(x)));
                f(body);
            }
            ExprP::Literal(_) => {}
            ExprP::Not(x) => f(x),
            ExprP::Minus(x) => f(x),
            ExprP::Plus(x) => f(x),
            ExprP::BitNot(x) => f(x),
            ExprP::Op(x, _, y) => {
                f(x);
                f(y);
            }
            ExprP::If(a_b_c) => {
                let (a, b, c) = &**a_b_c;
                f(a);
                f(b);
                f(c);
            }
            ExprP::List(x) => x.iter().for_each(|x| f(x)),
            ExprP::Dict(x) => x.iter().for_each(|(x, y)| {
                f(x);
                f(y);
            }),
            ExprP::ListComprehension(x, for_, y) => {
                for_.visit_expr(|x| f(x));
                y.iter().for_each(|x| x.visit_expr(|x| f(x)));
                f(x);
            }
            ExprP::DictComprehension(x, for_, y) => {
                for_.visit_expr(|x| f(x));
                y.iter().for_each(|x| x.visit_expr(|x| f(x)));
                f(&x.0);
                f(&x.1);
            }
            ExprP::FString(fstring) => {
                for expr in &fstring.expressions {
                    f(expr);
                }
            }
        }
    }

    /// Visit children expressions.
    pub fn visit_expr_err<'a, E>(
        &'a self,
        mut f: impl FnMut(&'a AstExprP<P>) -> Result<(), E>,
    ) -> Result<(), E> {
        let mut ok = Ok(());
        self.visit_expr(|x| {
            if ok.is_ok() {
                ok = f(x);
            }
        });
        ok
    }

    pub fn visit_expr_err_mut<'a, E>(
        &'a mut self,
        mut f: impl FnMut(&'a mut AstExprP<P>) -> Result<(), E>,
    ) -> Result<(), E> {
        let mut ok = Ok(());
        self.visit_expr_mut(|x| {
            if ok.is_ok() {
                ok = f(x);
            }
        });
        ok
    }

    pub fn visit_expr_mut<'a>(&'a mut self, mut f: impl FnMut(&'a mut AstExprP<P>)) {
        match self {
            ExprP::Tuple(xs) => xs.iter_mut().for_each(|x| f(x)),
            ExprP::Dot(x, _) => f(x),
            ExprP::Call(a, b) => {
                f(a);
                b.args.iter_mut().for_each(|x| f(x.expr_mut()));
            }
            ExprP::Index(a_b) => {
                let (a, b) = &mut **a_b;
                f(a);
                f(b);
            }
            ExprP::Index2(a_i0_i1) => {
                let (a, i0, i1) = &mut **a_i0_i1;
                f(a);
                f(i0);
                f(i1);
            }
            ExprP::Slice(a, b, c, d) => {
                f(a);
                b.iter_mut().for_each(|x| f(x));
                c.iter_mut().for_each(|x| f(x));
                d.iter_mut().for_each(|x| f(x));
            }
            ExprP::Identifier(..) => {}
            ExprP::Lambda(LambdaP {
                params,
                body,
                payload: _,
            }) => {
                params.iter_mut().for_each(|x| x.visit_expr_mut(|x| f(x)));
                f(body);
            }
            ExprP::Literal(_) => {}
            ExprP::Not(x) => f(x),
            ExprP::Minus(x) => f(x),
            ExprP::Plus(x) => f(x),
            ExprP::BitNot(x) => f(x),
            ExprP::Op(x, _, y) => {
                f(x);
                f(y);
            }
            ExprP::If(a_b_c) => {
                let (a, b, c) = &mut **a_b_c;
                f(a);
                f(b);
                f(c);
            }
            ExprP::List(x) => x.iter_mut().for_each(|x| f(x)),
            ExprP::Dict(x) => x.iter_mut().for_each(|(x, y)| {
                f(x);
                f(y);
            }),
            ExprP::ListComprehension(x, for_, y) => {
                for_.visit_expr_mut(|x| f(x));
                y.iter_mut().for_each(|x| x.visit_expr_mut(|x| f(x)));
                f(x);
            }
            ExprP::DictComprehension(x, for_, y) => {
                for_.visit_expr_mut(|x| f(x));
                y.iter_mut().for_each(|x| x.visit_expr_mut(|x| f(x)));
                f(&mut x.0);
                f(&mut x.1);
            }
            ExprP::FString(fstring) => {
                for expr in &mut fstring.expressions {
                    f(expr);
                }
            }
        }
    }

    fn visit_type_expr_err_mut<E>(
        &mut self,
        f: &mut impl FnMut(&mut AstTypeExprP<P>) -> Result<(), E>,
    ) -> Result<(), E> {
        if let ExprP::Lambda(lambda) = self {
            for param in &mut lambda.params {
                if let (_, Some(ty), _) = param.split_mut() {
                    f(ty)?;
                }
            }
        }
        self.visit_expr_err_mut(|expr| expr.visit_type_expr_err_mut(f))
    }

    /// Visit all identifiers in read position recursively.
    fn visit_ident<E>(&self, f: &mut impl FnMut(&AstIdentP<P>) -> Result<(), E>) -> Result<(), E> {
        if let ExprP::Identifier(ident) = self {
            f(ident)?;
        }
        self.visit_expr_err(|expr| expr.visit_ident(f))
    }
}

impl<P: AstPayload> TypeExprP<P> {
    fn visit_expr<'a>(&'a self, mut f: impl FnMut(&'a AstExprP<P>)) {
        f(&self.expr)
    }

    fn visit_expr_mut<'a>(&'a mut self, mut f: impl FnMut(&'a mut AstExprP<P>)) {
        f(&mut self.expr)
    }
}

impl<P: AstPayload> AssignTargetP<P> {
    pub fn visit_expr<'a>(&'a self, mut f: impl FnMut(&'a AstExprP<P>)) {
        fn recurse<'a, P: AstPayload>(
            x: &'a AssignTargetP<P>,
            f: &mut impl FnMut(&'a AstExprP<P>),
        ) {
            match x {
                AssignTargetP::Tuple(xs) => xs.iter().for_each(|x| recurse(x, f)),
                AssignTargetP::Dot(a, _) => f(a),
                AssignTargetP::Index(a_b) => {
                    let (a, b) = &**a_b;
                    f(a);
                    f(b);
                }
                AssignTargetP::Identifier(..) => {}
            }
        }
        recurse(self, &mut f)
    }

    pub fn visit_expr_mut<'a>(&'a mut self, mut f: impl FnMut(&'a mut AstExprP<P>)) {
        fn recurse<'a, P: AstPayload>(
            x: &'a mut AssignTargetP<P>,
            f: &mut impl FnMut(&'a mut AstExprP<P>),
        ) {
            match x {
                AssignTargetP::Tuple(ref mut xs) => xs.iter_mut().for_each(|x| recurse(&mut *x, f)),
                AssignTargetP::Dot(a, _) => f(a),
                AssignTargetP::Index(a_b) => {
                    let (a, b) = &mut **a_b;
                    f(a);
                    f(b);
                }
                AssignTargetP::Identifier(..) => {}
            }
        }
        recurse(self, &mut f)
    }

    /// Assuming this expression was on the left-hand-side of an assignment,
    /// visit all the names that are bound by this assignment.
    /// Note that assignments like `x[i] = n` don't bind any names.
    pub fn visit_lvalue<'a>(&'a self, mut f: impl FnMut(&'a AstAssignIdentP<P>)) {
        fn recurse<'a, P: AstPayload>(
            x: &'a AssignTargetP<P>,
            f: &mut impl FnMut(&'a AstAssignIdentP<P>),
        ) {
            match x {
                AssignTargetP::Identifier(x) => f(x),
                AssignTargetP::Tuple(xs) => xs.iter().for_each(|x| recurse(x, f)),
                _ => {}
            }
        }
        recurse(self, &mut f)
    }

    pub fn visit_lvalue_mut<'a>(&'a mut self, mut f: impl FnMut(&'a mut AstAssignIdentP<P>)) {
        fn recurse<'a, P: AstPayload>(
            x: &'a mut AssignTargetP<P>,
            f: &mut impl FnMut(&'a mut AstAssignIdentP<P>),
        ) {
            match x {
                AssignTargetP::Identifier(x) => f(x),
                AssignTargetP::Tuple(xs) => xs.iter_mut().for_each(|x| recurse(x, f)),
                _ => {}
            }
        }
        recurse(self, &mut f)
    }
}

impl<P: AstPayload> ForClauseP<P> {
    pub fn visit_expr<'a>(&'a self, mut f: impl FnMut(&'a AstExprP<P>)) {
        self.var.visit_expr(&mut f);
        f(&self.over);
    }

    pub fn visit_expr_mut<'a>(&'a mut self, mut f: impl FnMut(&'a mut AstExprP<P>)) {
        self.var.visit_expr_mut(&mut f);
        f(&mut self.over);
    }
}

impl<P: AstPayload> ClauseP<P> {
    pub fn visit_expr<'a>(&'a self, mut f: impl FnMut(&'a AstExprP<P>)) {
        match self {
            ClauseP::For(x) => x.visit_expr(f),
            ClauseP::If(x) => f(x),
        }
    }

    pub fn visit_expr_mut<'a>(&'a mut self, mut f: impl FnMut(&'a mut AstExprP<P>)) {
        match self {
            ClauseP::For(x) => x.visit_expr_mut(f),
            ClauseP::If(x) => f(x),
        }
    }
}

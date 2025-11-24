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

//! Map AST payload.

use crate::codemap::Spanned;
use crate::slice_vec_ext::VecExt;
use crate::syntax::ast::ArgumentP;
use crate::syntax::ast::AssignIdentP;
use crate::syntax::ast::AssignP;
use crate::syntax::ast::AssignTargetP;
use crate::syntax::ast::AstPayload;
use crate::syntax::ast::CallArgsP;
use crate::syntax::ast::ClauseP;
use crate::syntax::ast::DefP;
use crate::syntax::ast::ExprP;
use crate::syntax::ast::FStringP;
use crate::syntax::ast::ForClauseP;
use crate::syntax::ast::ForP;
use crate::syntax::ast::IdentP;
use crate::syntax::ast::LambdaP;
use crate::syntax::ast::LoadArgP;
use crate::syntax::ast::LoadP;
use crate::syntax::ast::ParameterP;
use crate::syntax::ast::StmtP;
use crate::syntax::ast::TypeExprP;

pub trait AstPayloadFunction<A: AstPayload, B: AstPayload> {
    fn map_load(&mut self, import_path: &str, a: A::LoadPayload) -> B::LoadPayload;
    fn map_ident(&mut self, a: A::IdentPayload) -> B::IdentPayload;
    fn map_ident_assign(&mut self, a: A::IdentAssignPayload) -> B::IdentAssignPayload;
    fn map_def(&mut self, a: A::DefPayload) -> B::DefPayload;
    fn map_type_expr(&mut self, a: A::TypeExprPayload) -> B::TypeExprPayload;
}

impl<A: AstPayload> LoadArgP<A> {
    pub fn into_map_payload<B: AstPayload>(
        self,
        f: &mut impl AstPayloadFunction<A, B>,
    ) -> LoadArgP<B> {
        let LoadArgP {
            local,
            their: remote,
            comma,
        } = self;
        LoadArgP {
            local: local.into_map_payload(f),
            their: remote,
            comma,
        }
    }
}

impl<A: AstPayload> LoadP<A> {
    pub fn into_map_payload<B: AstPayload>(
        self,
        f: &mut impl AstPayloadFunction<A, B>,
    ) -> LoadP<B> {
        let LoadP {
            module,
            args,
            payload,
        } = self;
        let payload = f.map_load(&module.node, payload);
        LoadP {
            module,
            args: args.into_map(|a| a.into_map_payload(f)),
            payload,
        }
    }
}

impl<A: AstPayload> AssignP<A> {
    pub fn into_map_payload<B: AstPayload>(
        self,
        f: &mut impl AstPayloadFunction<A, B>,
    ) -> AssignP<B> {
        let AssignP { lhs, ty, rhs } = self;
        AssignP {
            lhs: lhs.into_map_payload(f),
            ty: ty.map(|ty| ty.into_map_payload(f)),
            rhs: rhs.into_map_payload(f),
        }
    }
}

impl<A: AstPayload> ForP<A> {
    pub fn into_map_payload<B: AstPayload>(self, f: &mut impl AstPayloadFunction<A, B>) -> ForP<B> {
        let ForP { var, over, body } = self;
        ForP {
            var: var.into_map_payload(f),
            over: over.into_map_payload(f),
            body: Box::new(body.into_map_payload(f)),
        }
    }
}

impl<A: AstPayload> StmtP<A> {
    pub fn into_map_payload<B: AstPayload>(
        self,
        f: &mut impl AstPayloadFunction<A, B>,
    ) -> StmtP<B> {
        match self {
            StmtP::Break => StmtP::Break,
            StmtP::Continue => StmtP::Continue,
            StmtP::Pass => StmtP::Pass,
            StmtP::Return(None) => StmtP::Return(None),
            StmtP::Return(Some(e)) => StmtP::Return(Some(e.into_map_payload(f))),
            StmtP::Expression(e) => StmtP::Expression(e.into_map_payload(f)),
            StmtP::Assign(assign) => StmtP::Assign(assign.into_map_payload(f)),
            StmtP::AssignModify(lhs, op, rhs) => StmtP::AssignModify(
                lhs.into_map_payload(f),
                op,
                Box::new(rhs.into_map_payload(f)),
            ),
            StmtP::Statements(stmts) => {
                StmtP::Statements(stmts.into_map(|s| s.into_map_payload(f)))
            }
            StmtP::If(cond, then_block) => StmtP::If(
                cond.into_map_payload(f),
                Box::new(then_block.into_map_payload(f)),
            ),
            StmtP::IfElse(cond, then_block_else_block) => {
                let (then_block, else_block) = *then_block_else_block;
                StmtP::IfElse(
                    cond.into_map_payload(f),
                    Box::new((
                        then_block.into_map_payload(f),
                        else_block.into_map_payload(f),
                    )),
                )
            }
            StmtP::For(fr) => StmtP::For(fr.into_map_payload(f)),
            StmtP::Def(DefP {
                name,
                params,
                return_type,
                body,
                payload,
            }) => StmtP::Def(DefP {
                name: name.into_map_payload(f),
                params: params.into_map(|p| p.into_map_payload(f)),
                return_type: return_type.map(|ret| Box::new(ret.into_map_payload(f))),
                body: Box::new(body.into_map_payload(f)),
                payload: f.map_def(payload),
            }),
            StmtP::Load(load) => StmtP::Load(load.into_map_payload(f)),
        }
    }
}

impl<A: AstPayload> ExprP<A> {
    pub fn into_map_payload<B: AstPayload>(
        self,
        f: &mut impl AstPayloadFunction<A, B>,
    ) -> ExprP<B> {
        match self {
            ExprP::Tuple(exprs) => ExprP::Tuple(exprs.into_map(|e| e.into_map_payload(f))),
            ExprP::Dot(object, field) => ExprP::Dot(Box::new(object.into_map_payload(f)), field),
            ExprP::Call(ca, args) => ExprP::Call(
                Box::new(ca.into_map_payload(f)),
                CallArgsP {
                    args: args.args.into_map(|a| a.into_map_payload(f)),
                },
            ),
            ExprP::Index(array_index) => {
                let (array, index) = *array_index;
                ExprP::Index(Box::new((
                    array.into_map_payload(f),
                    index.into_map_payload(f),
                )))
            }
            ExprP::Index2(a_i0_i1) => {
                let (array, i0, i1) = *a_i0_i1;
                ExprP::Index2(Box::new((
                    array.into_map_payload(f),
                    i0.into_map_payload(f),
                    i1.into_map_payload(f),
                )))
            }
            ExprP::Slice(x, a, b, c) => ExprP::Slice(
                Box::new(x.into_map_payload(f)),
                a.map(|e| Box::new(e.into_map_payload(f))),
                b.map(|e| Box::new(e.into_map_payload(f))),
                c.map(|e| Box::new(e.into_map_payload(f))),
            ),
            ExprP::Identifier(id) => ExprP::Identifier(id.into_map_payload(f)),
            ExprP::Lambda(LambdaP {
                params,
                body,
                payload,
            }) => ExprP::Lambda(LambdaP {
                params: params.into_map(|p| p.into_map_payload(f)),
                body: Box::new(body.into_map_payload(f)),
                payload: f.map_def(payload),
            }),
            ExprP::Literal(l) => ExprP::Literal(l),
            ExprP::Not(e) => ExprP::Not(Box::new(e.into_map_payload(f))),
            ExprP::Minus(e) => ExprP::Minus(Box::new(e.into_map_payload(f))),
            ExprP::Plus(e) => ExprP::Plus(Box::new(e.into_map_payload(f))),
            ExprP::BitNot(e) => ExprP::BitNot(Box::new(e.into_map_payload(f))),
            ExprP::Op(l, op, r) => ExprP::Op(
                Box::new(l.into_map_payload(f)),
                op,
                Box::new(r.into_map_payload(f)),
            ),
            ExprP::If(a_b_c) => {
                let (a, b, c) = *a_b_c;
                ExprP::If(Box::new((
                    a.into_map_payload(f),
                    b.into_map_payload(f),
                    c.into_map_payload(f),
                )))
            }
            ExprP::List(es) => ExprP::List(es.into_map(|e| e.into_map_payload(f))),
            ExprP::Dict(kvs) => {
                ExprP::Dict(kvs.into_map(|(k, v)| (k.into_map_payload(f), v.into_map_payload(f))))
            }
            ExprP::ListComprehension(e, c0, cs) => ExprP::ListComprehension(
                Box::new(e.into_map_payload(f)),
                Box::new(c0.into_map_payload(f)),
                cs.into_map(|c| c.into_map_payload(f)),
            ),
            ExprP::DictComprehension(k_v, c0, cs) => {
                let (k, v) = *k_v;
                ExprP::DictComprehension(
                    Box::new((k.into_map_payload(f), v.into_map_payload(f))),
                    Box::new(c0.into_map_payload(f)),
                    cs.into_map(|c| c.into_map_payload(f)),
                )
            }
            ExprP::FString(fstring) => ExprP::FString(fstring.into_map_payload(f)),
        }
    }
}

impl<A: AstPayload> TypeExprP<A> {
    pub fn into_map_payload<B: AstPayload>(
        self,
        f: &mut impl AstPayloadFunction<A, B>,
    ) -> TypeExprP<B> {
        let TypeExprP { expr, payload } = self;
        TypeExprP {
            expr: expr.map(|e| e.into_map_payload(f)),
            payload: f.map_type_expr(payload),
        }
    }
}

impl<A: AstPayload> AssignTargetP<A> {
    pub fn into_map_payload<B: AstPayload>(
        self,
        f: &mut impl AstPayloadFunction<A, B>,
    ) -> AssignTargetP<B> {
        match self {
            AssignTargetP::Tuple(args) => {
                AssignTargetP::Tuple(args.into_map(|a| a.into_map_payload(f)))
            }
            AssignTargetP::Index(array_index) => {
                let (array, index) = *array_index;
                AssignTargetP::Index(Box::new((
                    array.into_map_payload(f),
                    index.into_map_payload(f),
                )))
            }
            AssignTargetP::Dot(object, field) => {
                AssignTargetP::Dot(Box::new(object.into_map_payload(f)), field)
            }
            AssignTargetP::Identifier(ident) => {
                AssignTargetP::Identifier(ident.into_map_payload(f))
            }
        }
    }
}

impl<A: AstPayload> AssignIdentP<A> {
    pub fn into_map_payload<B: AstPayload>(
        self,
        f: &mut impl AstPayloadFunction<A, B>,
    ) -> AssignIdentP<B> {
        let AssignIdentP {
            ident: s,
            payload: p,
        } = self;
        AssignIdentP {
            ident: s,
            payload: f.map_ident_assign(p),
        }
    }
}

impl<A: AstPayload> IdentP<A> {
    pub fn into_map_payload<B: AstPayload>(
        self,
        f: &mut impl AstPayloadFunction<A, B>,
    ) -> IdentP<B> {
        let IdentP {
            ident: s,
            payload: p,
        } = self;
        IdentP {
            ident: s,
            payload: f.map_ident(p),
        }
    }
}

impl<A: AstPayload> ParameterP<A> {
    pub fn into_map_payload<B: AstPayload>(
        self,
        f: &mut impl AstPayloadFunction<A, B>,
    ) -> ParameterP<B> {
        match self {
            ParameterP::Normal(name, ty, defa) => ParameterP::Normal(
                name.into_map_payload(f),
                ty.map(|defa| Box::new(defa.into_map_payload(f))),
                defa.map(|defa| Box::new(defa.into_map_payload(f))),
            ),
            ParameterP::NoArgs => ParameterP::NoArgs,
            ParameterP::Slash => ParameterP::Slash,
            ParameterP::Args(name, ty) => ParameterP::Args(
                name.into_map_payload(f),
                ty.map(|defa| Box::new(defa.into_map_payload(f))),
            ),
            ParameterP::KwArgs(name, ty) => ParameterP::KwArgs(
                name.into_map_payload(f),
                ty.map(|defa| Box::new(defa.into_map_payload(f))),
            ),
        }
    }
}

impl<A: AstPayload> ArgumentP<A> {
    pub fn into_map_payload<B: AstPayload>(
        self,
        f: &mut impl AstPayloadFunction<A, B>,
    ) -> ArgumentP<B> {
        match self {
            ArgumentP::Positional(e) => ArgumentP::Positional(e.into_map_payload(f)),
            ArgumentP::Named(n, e) => ArgumentP::Named(n, e.into_map_payload(f)),
            ArgumentP::Args(e) => ArgumentP::Args(e.into_map_payload(f)),
            ArgumentP::KwArgs(e) => ArgumentP::KwArgs(e.into_map_payload(f)),
        }
    }
}

impl<A: AstPayload> ClauseP<A> {
    pub fn into_map_payload<B: AstPayload>(
        self,
        f: &mut impl AstPayloadFunction<A, B>,
    ) -> ClauseP<B> {
        match self {
            ClauseP::For(c) => ClauseP::For(c.into_map_payload(f)),
            ClauseP::If(e) => ClauseP::If(e.into_map_payload(f)),
        }
    }
}

impl<A: AstPayload> ForClauseP<A> {
    pub fn into_map_payload<B: AstPayload>(
        self,
        f: &mut impl AstPayloadFunction<A, B>,
    ) -> ForClauseP<B> {
        ForClauseP {
            var: self.var.into_map_payload(f),
            over: self.over.into_map_payload(f),
        }
    }
}

impl<A: AstPayload> FStringP<A> {
    pub fn into_map_payload<B: AstPayload>(
        self,
        f: &mut impl AstPayloadFunction<A, B>,
    ) -> FStringP<B> {
        let Self {
            format,
            expressions,
        } = self;

        FStringP {
            format,
            expressions: expressions.into_map(|id| id.into_map_payload(f)),
        }
    }
}

macro_rules! ast_payload_map_stub {
    ($ast_type:ident, $ext:ident) => {
        pub trait $ext<A: AstPayload> {
            fn into_map_payload<B: AstPayload>(
                self,
                f: &mut impl AstPayloadFunction<A, B>,
            ) -> Spanned<$ast_type<B>>;
        }

        impl<A: AstPayload> $ext<A> for Spanned<$ast_type<A>> {
            fn into_map_payload<B: AstPayload>(
                self,
                f: &mut impl AstPayloadFunction<A, B>,
            ) -> Spanned<$ast_type<B>> {
                let Spanned { span, node } = self;
                Spanned {
                    span,
                    node: node.into_map_payload(f),
                }
            }
        }
    };
}

ast_payload_map_stub!(ExprP, ExprPExt);
ast_payload_map_stub!(TypeExprP, TypeExprPExt);
ast_payload_map_stub!(AssignTargetP, AssignTargetPExt);
ast_payload_map_stub!(AssignIdentP, AssignIdentPExt);
ast_payload_map_stub!(IdentP, IdentPExt);
ast_payload_map_stub!(ParameterP, ParameterPExt);
ast_payload_map_stub!(ArgumentP, ArgumentPExt);
ast_payload_map_stub!(StmtP, StmtPExt);
ast_payload_map_stub!(FStringP, FStringPExt);

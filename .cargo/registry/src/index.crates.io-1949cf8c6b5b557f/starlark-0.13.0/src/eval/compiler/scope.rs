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

pub(crate) mod payload;
pub(crate) mod scope_resolver_globals;
mod tests;

use std::collections::HashMap;
use std::iter;
use std::marker::PhantomData;
use std::mem;

use dupe::Dupe;
use starlark_derive::VisitSpanMut;
use starlark_map::small_map;
use starlark_map::small_map::SmallMap;
use starlark_syntax::eval_exception::EvalException;
use starlark_syntax::syntax::ast::AssignIdent;
use starlark_syntax::syntax::ast::AssignP;
use starlark_syntax::syntax::ast::AssignTarget;
use starlark_syntax::syntax::ast::AstAssignIdentP;
use starlark_syntax::syntax::ast::AstStmt;
use starlark_syntax::syntax::ast::ClauseP;
use starlark_syntax::syntax::ast::DefP;
use starlark_syntax::syntax::ast::ExprP;
use starlark_syntax::syntax::ast::ForClauseP;
use starlark_syntax::syntax::ast::ForP;
use starlark_syntax::syntax::ast::LambdaP;
use starlark_syntax::syntax::ast::LoadArgP;
use starlark_syntax::syntax::ast::Stmt;
use starlark_syntax::syntax::ast::StmtP;
use starlark_syntax::syntax::ast::Visibility;
use starlark_syntax::syntax::top_level_stmts::top_level_stmts_mut;
use starlark_syntax::syntax::uniplate::VisitMut;

use crate::codemap::CodeMap;
use crate::codemap::Span;
use crate::environment::names::MutableNames;
use crate::environment::slots::ModuleSlotId;
use crate::environment::Module;
use crate::errors::did_you_mean::did_you_mean;
use crate::eval::compiler::def::CopySlotFromParent;
use crate::eval::compiler::scope::payload::CstAssignIdent;
use crate::eval::compiler::scope::payload::CstAssignTarget;
use crate::eval::compiler::scope::payload::CstExpr;
use crate::eval::compiler::scope::payload::CstIdent;
use crate::eval::compiler::scope::payload::CstParameter;
use crate::eval::compiler::scope::payload::CstPayload;
use crate::eval::compiler::scope::payload::CstStmt;
use crate::eval::compiler::scope::payload::CstStmtFromAst;
use crate::eval::compiler::scope::payload::CstTypeExpr;
use crate::eval::compiler::scope::scope_resolver_globals::ScopeResolverGlobals;
use crate::eval::runtime::slots::LocalSlotIdCapturedOrNot;
use crate::syntax::Dialect;
use crate::typing::error::InternalError;
use crate::typing::Interface;
use crate::values::FrozenHeap;
use crate::values::FrozenRef;
use crate::values::FrozenStringValue;
use crate::values::FrozenValue;

#[derive(Debug, thiserror::Error)]
enum ScopeError {
    #[error("Variable `{0}` not found")]
    VariableNotFound(String),
    #[error("Variable `{0}` not found, did you mean `{1}`?")]
    VariableNotFoundDidYouMean(String, String),
    #[error("Identifiers in type expressions can only refer globals or builtins: `{0}`")]
    TypeExpressionGlobalOrBuiltin(String),
}

impl From<ScopeError> for crate::Error {
    fn from(e: ScopeError) -> Self {
        crate::Error::new_kind(crate::ErrorKind::Scope(anyhow::Error::new(e)))
    }
}

/// All scopes and bindings in a module.
struct ModuleScopeBuilder<'a> {
    scope_data: ModuleScopeData<'a>,
    module: &'a MutableNames,
    frozen_heap: &'a FrozenHeap,
    module_bindings: SmallMap<FrozenStringValue, BindingId>,
    // The first scope is a module-level scope (including comprehensions in module scope).
    // The rest are scopes for functions (which include their comprehensions).
    locals: Vec<ScopeId>,
    unscopes: Vec<Unscope>,
    codemap: FrozenRef<'static, CodeMap>,
    globals: ScopeResolverGlobals,
    errors: Vec<EvalException>,
    top_level_stmt_count: usize,
}

pub(crate) struct ModuleScopes<'f> {
    pub(crate) scope_data: ModuleScopeData<'f>,
    pub(crate) module_slot_count: u32,
    pub(crate) cst: CstStmt,
    /// Number of top-level statements in the module.
    pub(crate) top_level_stmt_count: usize,
}

struct UnscopeBinding {
    /// Variable mappings in local scope are overwritten by comprehension variables.
    ///
    /// When we enter comprehension, we replace local scope variable slots with comprehension
    /// scope slots. This field stores the original slot in the local scope,
    /// or `None` if there was no mapping for the variable.
    ///
    /// When we pop the comprehension scope, we restore the mapping from this value.
    undo: Option<(LocalSlotIdCapturedOrNot, BindingId)>,
}

#[derive(Default)]
struct Unscope(SmallMap<FrozenStringValue, UnscopeBinding>);

#[derive(Default, Debug)]
pub(crate) struct ScopeNames<'f> {
    /// `Some` when scope is initialized.
    /// For module scope, the value is zero.
    pub param_count: Option<u32>,
    /// Slots this scope uses, including for parameters and `parent`.
    /// Indexed by [`LocalSlotId`], values are variable names.
    pub used: Vec<FrozenStringValue>,
    /// The names that are in this scope
    pub mp: SmallMap<FrozenStringValue, (LocalSlotIdCapturedOrNot, BindingId)>,
    /// Slots to copy from the parent.
    /// Module-level identifiers are not copied over, to avoid excess copying.
    pub parent: Vec<CopySlotFromParent>,
    /// We store frozen strings.
    _heap: PhantomData<&'f ()>,
}

impl<'f> ScopeNames<'f> {
    fn set_param_count(&mut self, param_count: u32) {
        assert!(self.param_count.is_none());
        self.param_count = Some(param_count);
    }

    pub(crate) fn param_count(&self) -> u32 {
        self.param_count
            .expect("param_count must be set during analysis")
    }

    fn copy_parent(
        &mut self,
        parent_slot: LocalSlotIdCapturedOrNot,
        binding_id: BindingId,
        name: FrozenStringValue,
    ) -> LocalSlotIdCapturedOrNot {
        assert!(self.get_name(name).is_none()); // Or we'll be overwriting our variable
        let res = self.add_name(name, binding_id);
        self.parent.push(CopySlotFromParent {
            parent: parent_slot,
            child: res,
        });
        res
    }

    fn next_slot(&mut self, name: FrozenStringValue) -> LocalSlotIdCapturedOrNot {
        let res = LocalSlotIdCapturedOrNot(self.used.len().try_into().unwrap());
        self.used.push(name);
        res
    }

    fn add_name(
        &mut self,
        name: FrozenStringValue,
        binding_id: BindingId,
    ) -> LocalSlotIdCapturedOrNot {
        let slot = self.next_slot(name);
        let old = self.mp.insert_hashed(name.get_hashed(), (slot, binding_id));
        assert!(old.is_none());
        slot
    }

    fn add_scoped(
        &mut self,
        name: FrozenStringValue,
        binding_id: BindingId,
        unscope: &mut Unscope,
    ) -> LocalSlotIdCapturedOrNot {
        let slot = self.next_slot(name);
        let undo = match self.mp.get_mut_hashed(name.get_hashed().as_ref()) {
            Some(v) => {
                let old = *v;
                *v = (slot, binding_id);
                Some(old)
            }
            None => {
                self.mp.insert_hashed(name.get_hashed(), (slot, binding_id));
                None
            }
        };
        assert!(
            unscope
                .0
                .insert_hashed(name.get_hashed(), UnscopeBinding { undo })
                .is_none()
        );
        slot
    }

    fn unscope(&mut self, unscope: Unscope) {
        for (name, UnscopeBinding { undo }) in unscope.0 {
            match undo {
                None => {
                    self.mp.shift_remove(&name);
                }
                Some(v) => *self.mp.get_mut(&name).unwrap() = v,
            }
        }
    }

    fn get_name(&self, name: FrozenStringValue) -> Option<(LocalSlotIdCapturedOrNot, BindingId)> {
        self.mp.get_hashed(name.get_hashed().as_ref()).copied()
    }
}

#[derive(Copy, Clone, Dupe, Debug)]
pub(crate) enum Slot {
    /// Top-level module scope.
    Module(ModuleSlotId),
    /// Local scope, always mutable.
    Local(LocalSlotIdCapturedOrNot),
}

#[derive(Clone, Copy, Dupe)]
enum ResolveIdentScope {
    /// Resolving normal identifier.
    Any,
    /// Resolving identifier in type expression.
    GlobalForTypeExpression,
}

impl<'f> ModuleScopeBuilder<'f> {
    fn top_scope_id(&self) -> ScopeId {
        *self.locals.last().unwrap()
    }

    fn scope_at_level(&self, level: usize) -> &ScopeNames<'f> {
        let scope_id = self.locals[level];
        self.scope_data.get_scope(scope_id)
    }

    fn scope_at_level_mut(&mut self, level: usize) -> &mut ScopeNames<'f> {
        let scope_id = self.locals[level];
        self.scope_data.mut_scope(scope_id)
    }

    /// Resolve symbols in a module.
    ///
    /// Checks all the symbols are resolved to locals/globals/captured/etc.
    /// Do not check types yet. But validate type expressions.
    ///
    /// This function does not fail, errors are stored in the `errors` field.
    fn enter_module(
        module: &'f MutableNames,
        frozen_heap: &'f FrozenHeap,
        loads: &HashMap<String, Interface>,
        stmt: AstStmt,
        globals: ScopeResolverGlobals,
        codemap: FrozenRef<'static, CodeMap>,
        dialect: &Dialect,
    ) -> (CstStmt, ModuleScopeBuilder<'f>) {
        let mut scope_data = ModuleScopeData::new();
        let scope_id = scope_data.new_scope().0;
        let mut cst = CstStmt::from_ast(stmt, &mut scope_data, loads);

        let mut top_level_stmts = top_level_stmts_mut(&mut cst);

        // Not really important, sanity check
        assert_eq!(scope_id, ScopeId::module());

        scope_data.mut_scope(scope_id).set_param_count(0);

        let mut locals: SmallMap<FrozenStringValue, _> = SmallMap::new();

        let existing_module_names_and_visibilites = module.all_names_and_visibilities();
        for (name, vis) in existing_module_names_and_visibilites.iter() {
            let (binding_id, _binding) = scope_data.new_binding(
                *name,
                BindingSource::FromModule,
                *vis,
                AssignCount::AtMostOnce,
            );
            locals.insert_hashed(name.get_hashed(), binding_id);
        }

        for stmt in top_level_stmts.iter_mut() {
            Stmt::collect_defines(
                stmt,
                InLoop::No,
                &mut scope_data,
                frozen_heap,
                &mut locals,
                dialect,
            );
        }

        let mut module_bindings = SmallMap::new();
        for (x, binding_id) in locals {
            let binding = scope_data.mut_binding(binding_id);
            let slot = module.add_name_visibility(x, binding.vis);
            binding.init_slot(Slot::Module(slot), &codemap).unwrap();
            let old_binding = module_bindings.insert_hashed(x.get_hashed(), binding_id);
            assert!(old_binding.is_none());
        }

        // Here we traverse the AST second time to collect scopes of defs
        for stmt in top_level_stmts.iter_mut() {
            ModuleScopeBuilder::collect_defines_recursively(
                &mut scope_data,
                stmt,
                frozen_heap,
                dialect,
                &codemap,
            );
        }
        let mut scope = ModuleScopeBuilder {
            scope_data,
            frozen_heap,
            module,
            module_bindings,
            locals: vec![scope_id],
            unscopes: Vec::new(),
            codemap,
            globals,
            errors: Vec::new(),
            top_level_stmt_count: top_level_stmts.len(),
        };
        for stmt in top_level_stmts.iter_mut() {
            scope.resolve_idents(stmt);
        }
        (cst, scope)
    }
}

impl<'f> ModuleScopeBuilder<'f> {
    // Number of module slots I need, a struct holding all scopes, and module bindings.
    fn exit_module(
        mut self,
    ) -> (
        u32,
        ModuleScopeData<'f>,
        SmallMap<FrozenStringValue, BindingId>,
    ) {
        assert!(self.locals.len() == 1);
        assert!(self.unscopes.is_empty());
        let scope_id = self.locals.pop().unwrap();
        assert!(scope_id == ScopeId::module());
        let scope = self.scope_data.get_scope(scope_id);
        assert!(scope.parent.is_empty());
        (
            self.module.slot_count(),
            self.scope_data,
            self.module_bindings,
        )
    }
}

impl<'f> ModuleScopes<'f> {
    pub(crate) fn check_module_err(
        module: &'f MutableNames,
        frozen_heap: &'f FrozenHeap,
        loads: &HashMap<String, Interface>,
        stmt: AstStmt,
        globals: ScopeResolverGlobals,
        codemap: FrozenRef<'static, CodeMap>,
        dialect: &Dialect,
    ) -> crate::Result<ModuleScopes<'f>> {
        let (errors, scopes) =
            ModuleScopes::check_module(module, frozen_heap, loads, stmt, globals, codemap, dialect);
        if let Some(error) = errors.into_iter().next() {
            return Err(error.into_error());
        }
        Ok(scopes)
    }

    pub(crate) fn check_module(
        module: &'f MutableNames,
        frozen_heap: &'f FrozenHeap,
        loads: &HashMap<String, Interface>,
        stmt: AstStmt,
        globals: ScopeResolverGlobals,
        codemap: FrozenRef<'static, CodeMap>,
        dialect: &Dialect,
    ) -> (Vec<EvalException>, ModuleScopes<'f>) {
        let (stmt, mut scope) = ModuleScopeBuilder::enter_module(
            module,
            frozen_heap,
            loads,
            stmt,
            globals,
            codemap,
            dialect,
        );
        let top_level_stmt_count = scope.top_level_stmt_count;
        let errors = mem::take(&mut scope.errors);
        let (module_slot_count, scope_data, _module_bindings) = scope.exit_module();
        (
            errors,
            ModuleScopes {
                cst: stmt,
                scope_data,
                module_slot_count,
                top_level_stmt_count,
            },
        )
    }
}

impl<'f> ModuleScopeBuilder<'f> {
    fn collect_defines_in_def(
        scope_data: &mut ModuleScopeData,
        scope_id: ScopeId,
        params: &mut [CstParameter],
        body: Option<&mut CstStmt>,

        frozen_heap: &FrozenHeap,
        dialect: &Dialect,
        codemap: &CodeMap,
    ) {
        let params: Vec<&mut AstAssignIdentP<_>> = params
            .iter_mut()
            .filter_map(|p| p.node.split_mut().0)
            .collect::<Vec<_>>();
        scope_data
            .mut_scope(scope_id)
            .set_param_count(params.len().try_into().unwrap());
        let mut locals: SmallMap<FrozenStringValue, _> = SmallMap::new();
        for p in params {
            let name = frozen_heap.alloc_str_intern(&p.ident);
            // Subtle invariant: the slots for the params must be ordered and at the
            // beginning
            let binding_id = scope_data
                .new_binding(
                    name,
                    BindingSource::Source(p.span),
                    Visibility::Public,
                    AssignCount::AtMostOnce,
                )
                .0;
            p.payload = Some(binding_id);
            let old_local = locals.insert_hashed(name.get_hashed(), binding_id);
            assert!(old_local.is_none());
        }
        if let Some(code) = body {
            Stmt::collect_defines(
                code,
                InLoop::No,
                scope_data,
                frozen_heap,
                &mut locals,
                dialect,
            );
        }
        for (name, binding_id) in locals.into_iter() {
            let slot = scope_data.mut_scope(scope_id).add_name(name, binding_id);
            let binding = scope_data.mut_binding(binding_id);
            binding.init_slot(Slot::Local(slot), codemap).unwrap();
        }
    }

    fn collect_defines_recursively(
        scope_data: &mut ModuleScopeData,
        code: &mut CstStmt,

        frozen_heap: &FrozenHeap,
        dialect: &Dialect,
        codemap: &CodeMap,
    ) {
        if let StmtP::Def(DefP {
            name: _,
            params,
            return_type: _,
            body,
            payload: scope_id,
        }) = &mut code.node
        {
            // Here we traverse the AST twice: once for this def scope,
            // second time below for nested defs.
            Self::collect_defines_in_def(
                scope_data,
                *scope_id,
                params,
                Some(body),
                frozen_heap,
                dialect,
                codemap,
            );
        }

        code.visit_children_mut(&mut |visit| match visit {
            VisitMut::Expr(e) => Self::collect_defines_recursively_in_expr(
                scope_data,
                e,
                frozen_heap,
                dialect,
                codemap,
            ),
            VisitMut::Stmt(s) => {
                Self::collect_defines_recursively(scope_data, s, frozen_heap, dialect, codemap)
            }
        });
    }

    fn collect_defines_recursively_in_expr(
        scope_data: &mut ModuleScopeData,
        code: &mut CstExpr,

        frozen_heap: &FrozenHeap,
        dialect: &Dialect,
        codemap: &CodeMap,
    ) {
        if let ExprP::Lambda(LambdaP {
            params,
            body: _,
            payload: scope_id,
        }) = &mut code.node
        {
            Self::collect_defines_in_def(
                scope_data,
                *scope_id,
                params,
                None,
                frozen_heap,
                dialect,
                codemap,
            );
        }

        code.visit_expr_mut(|e| {
            Self::collect_defines_recursively_in_expr(scope_data, e, frozen_heap, dialect, codemap)
        });
    }

    fn resolve_idents(&mut self, code: &mut CstStmt) {
        match &mut code.node {
            StmtP::Def(DefP {
                name: _,
                params,
                return_type,
                body,
                payload: scope_id,
            }) => self.resolve_idents_in_def(
                *scope_id,
                params,
                return_type.as_mut().map(|r| &mut **r),
                Some(body),
                None,
            ),
            StmtP::Assign(AssignP { lhs, ty, rhs }) => {
                self.resolve_idents_in_assign(lhs);
                if let Some(ty) = ty {
                    self.resolve_idents_in_type_expr(ty);
                }
                self.resolve_idents_in_expr(rhs);
            }
            _ => code.visit_children_mut(|visit| match visit {
                VisitMut::Stmt(stmt) => self.resolve_idents(stmt),
                VisitMut::Expr(expr) => self.resolve_idents_in_expr(expr),
            }),
        }
    }

    fn resolve_idents_in_assign(&mut self, assign: &mut CstAssignTarget) {
        assign.visit_expr_mut(|expr| self.resolve_idents_in_expr(expr));
    }

    fn resolve_idents_in_def(
        &mut self,
        scope_id: ScopeId,
        params: &mut [CstParameter],
        ret: Option<&mut CstTypeExpr>,
        body_stmt: Option<&mut CstStmt>,
        body_expr: Option<&mut CstExpr>,
    ) {
        for param in params {
            let (_, ty, def) = param.split_mut();
            if let Some(ty) = ty {
                self.resolve_idents_in_type_expr(ty);
            }
            if let Some(def) = def {
                self.resolve_idents_in_expr(def);
            }
        }
        if let Some(ret) = ret {
            self.resolve_idents_in_type_expr(ret);
        }

        self.enter_def(scope_id);
        if let Some(body_stmt) = body_stmt {
            self.resolve_idents(body_stmt);
        }
        if let Some(body_expr) = body_expr {
            self.resolve_idents_in_expr(body_expr);
        }
        self.exit_def();
    }

    fn resolve_idents_in_expr_impl(&mut self, scope: ResolveIdentScope, expr: &mut CstExpr) {
        match &mut expr.node {
            ExprP::Identifier(ident) => self.resolve_ident(scope, ident),
            ExprP::Lambda(LambdaP {
                params,
                body,
                payload: scope_id,
            }) => self.resolve_idents_in_def(*scope_id, params, None, None, Some(body)),
            ExprP::ListComprehension(expr, first_for, clauses) => {
                self.resolve_idents_in_compr(&mut [expr], first_for, clauses)
            }
            ExprP::DictComprehension(k_v, first_for, clauses) => {
                let (k, v) = &mut **k_v;
                self.resolve_idents_in_compr(&mut [k, v], first_for, clauses)
            }
            _ => expr.visit_expr_mut(|expr| self.resolve_idents_in_expr_impl(scope, expr)),
        }
    }

    fn resolve_idents_in_expr(&mut self, expr: &mut CstExpr) {
        self.resolve_idents_in_expr_impl(ResolveIdentScope::Any, expr);
    }

    fn resolve_idents_in_type_expr(&mut self, expr: &mut CstTypeExpr) {
        self.resolve_idents_in_expr_impl(
            ResolveIdentScope::GlobalForTypeExpression,
            &mut expr.node.expr,
        );
    }

    fn current_scope_all_visible_names_for_did_you_mean(&self) -> Option<Vec<String>> {
        // It is OK to return non-unique identifiers
        let mut r: Vec<String> = Vec::new();
        for &scope_id in self.locals.iter().rev() {
            let scope = self.scope_data.get_scope(scope_id);
            r.extend(scope.mp.keys().map(|s| s.as_str().to_owned()));
        }
        r.extend(self.module_bindings.keys().map(|s| s.as_str().to_owned()));
        r.extend(self.globals.names()?);
        Some(r)
    }

    #[cold]
    fn variable_not_found_err(&self, ident: &CstIdent) -> EvalException {
        let variants = self
            .current_scope_all_visible_names_for_did_you_mean()
            .unwrap_or_default();
        let better = did_you_mean(
            ident.node.ident.as_str(),
            variants.iter().map(|s| s.as_str()),
        );
        EvalException::new(
            match better {
                Some(better) => ScopeError::VariableNotFoundDidYouMean(
                    ident.node.ident.clone(),
                    better.to_owned(),
                ),
                None => ScopeError::VariableNotFound(ident.node.ident.clone()),
            }
            .into(),
            ident.span,
            &self.codemap,
        )
    }

    fn resolve_ident(&mut self, scope: ResolveIdentScope, ident: &mut CstIdent) {
        assert!(ident.node.payload.is_none());
        let resolved = match self.get_name(self.frozen_heap.alloc_str_intern(&ident.node.ident)) {
            None => {
                // Must be a global, since we know all variables
                match self.globals.get_global(&ident.node.ident) {
                    None => {
                        self.errors.push(self.variable_not_found_err(ident));
                        return;
                    }
                    Some(v) => ResolvedIdent::Global(v),
                }
            }
            Some((slot, binding_id)) => ResolvedIdent::Slot(slot, binding_id),
        };
        match scope {
            ResolveIdentScope::Any => {}
            ResolveIdentScope::GlobalForTypeExpression => match resolved {
                ResolvedIdent::Slot(Slot::Local(_), _) => {
                    self.errors.push(EvalException::new(
                        ScopeError::TypeExpressionGlobalOrBuiltin(ident.node.ident.clone()).into(),
                        ident.span,
                        &self.codemap,
                    ));
                    return;
                }
                ResolvedIdent::Slot(Slot::Module(_), _) => {}
                ResolvedIdent::Global(_) => {}
            },
        }
        ident.node.payload = Some(resolved);
    }

    fn resolve_idents_in_compr(
        &mut self,
        exprs: &mut [&mut CstExpr],
        first_for: &mut ForClauseP<CstPayload>,
        clauses: &mut [ClauseP<CstPayload>],
    ) {
        // First for is resolved in outer scope
        self.resolve_idents_in_for_clause(first_for);

        self.enter_compr();

        // Add identifiers to compr scope

        self.add_compr(
            iter::once(&mut first_for.var).chain(clauses.iter_mut().filter_map(
                |clause| match clause {
                    ClauseP::For(for_clause) => Some(&mut for_clause.var),
                    ClauseP::If(..) => None,
                },
            )),
        );

        // Now resolve idents in compr scope

        for clause in clauses.iter_mut() {
            match clause {
                ClauseP::For(for_clause) => self.resolve_idents_in_for_clause(for_clause),
                ClauseP::If(cond) => self.resolve_idents_in_expr(cond),
            }
        }

        // Finally, resolve the item expression

        for expr in exprs {
            self.resolve_idents_in_expr(expr);
        }

        self.exit_compr();
    }

    fn resolve_idents_in_for_clause(&mut self, for_clause: &mut ForClauseP<CstPayload>) {
        self.resolve_idents_in_expr(&mut for_clause.over);
        self.resolve_idents_in_assign(&mut for_clause.var);
    }

    pub fn enter_def(&mut self, scope_id: ScopeId) {
        assert!(scope_id != ScopeId::module());
        self.locals.push(scope_id);
    }

    // Which slots to grab from the current scope to the parent scope, size of your
    // self scope Future state: Should return the slots to use from the parent
    // scope
    pub fn exit_def(&mut self) -> &mut ScopeNames<'f> {
        let scope_id = self.locals.pop().unwrap();
        self.scope_data.mut_scope(scope_id)
    }

    fn enter_compr(&mut self) {
        self.unscopes.push(Unscope::default());
    }

    fn add_compr<'x>(&mut self, var: impl IntoIterator<Item = &'x mut CstAssignTarget>) {
        let scope_id = self.top_scope_id();
        let mut locals = SmallMap::new();
        for var in var {
            AssignTarget::collect_defines_lvalue(
                var,
                InLoop::Yes,
                &mut self.scope_data,
                self.frozen_heap,
                &mut locals,
            );
        }
        for (name, binding_id) in locals.into_iter() {
            let slot = self.scope_data.mut_scope(scope_id).add_scoped(
                name,
                binding_id,
                self.unscopes.last_mut().unwrap(),
            );
            let binding = self.scope_data.mut_binding(binding_id);
            binding.init_slot(Slot::Local(slot), &self.codemap).unwrap();
        }
    }

    fn exit_compr(&mut self) {
        self.scope_data
            .mut_scope(self.top_scope_id())
            .unscope(self.unscopes.pop().unwrap());
    }

    fn get_name(&mut self, name: FrozenStringValue) -> Option<(Slot, BindingId)> {
        // look upwards to find the first place the variable occurs
        // then copy that variable downwards
        for i in (0..self.locals.len()).rev() {
            if let Some((mut v, binding_id)) = self.scope_at_level(i).get_name(name) {
                if i + 1 != self.locals.len() {
                    self.scope_data.mut_binding(binding_id).captured = Captured::Yes;
                }
                for j in (i + 1)..self.locals.len() {
                    v = self.scope_at_level_mut(j).copy_parent(v, binding_id, name);
                }
                return Some((Slot::Local(v), binding_id));
            }
        }
        let binding_id = self
            .module_bindings
            .get_hashed(name.get_hashed().as_ref())
            .copied();
        match binding_id {
            Some(binding_id) => {
                let binding = self.scope_data.mut_binding(binding_id);
                if self.locals.len() > 1 {
                    binding.captured = Captured::Yes;
                }
                let slot = binding.resolved_slot(&self.codemap).unwrap();
                assert!(matches!(slot, Slot::Module(_)));
                Some((slot, binding_id))
            }
            None => None,
        }
    }
}

/// While performing analysis.
#[derive(Copy, Clone, Dupe)]
enum InLoop {
    /// Current statement has an enclosing loop in the current scope.
    Yes,
    /// Current statement has no enclosing loop in the current scope.
    No,
}

trait StmtCollectDefines {
    fn collect_defines<'a>(
        stmt: &'a mut CstStmt,
        in_loop: InLoop,
        scope_data: &mut ModuleScopeData,
        frozen_heap: &FrozenHeap,
        result: &mut SmallMap<FrozenStringValue, BindingId>,
        dialect: &Dialect,
    );
}

impl StmtCollectDefines for Stmt {
    // Collect all the variables that are defined in this scope
    fn collect_defines<'a>(
        stmt: &'a mut CstStmt,
        in_loop: InLoop,
        scope_data: &mut ModuleScopeData,
        frozen_heap: &FrozenHeap,
        result: &mut SmallMap<FrozenStringValue, BindingId>,
        dialect: &Dialect,
    ) {
        match &mut stmt.node {
            StmtP::Assign(AssignP { lhs: dest, .. }) | StmtP::AssignModify(dest, _, _) => {
                AssignTarget::collect_defines_lvalue(
                    dest,
                    in_loop,
                    scope_data,
                    frozen_heap,
                    result,
                );
            }
            StmtP::For(ForP { var, over: _, body }) => {
                AssignTarget::collect_defines_lvalue(
                    var,
                    InLoop::Yes,
                    scope_data,
                    frozen_heap,
                    result,
                );
                StmtP::collect_defines(body, InLoop::Yes, scope_data, frozen_heap, result, dialect);
            }
            StmtP::Def(DefP { name, .. }) => AssignIdent::collect_assign_ident(
                name,
                in_loop,
                Visibility::Public,
                scope_data,
                frozen_heap,
                result,
            ),
            StmtP::Load(load) => {
                // TODO(nga): visibility does not belong to AST.
                let vis = match dialect.enable_load_reexport {
                    true => Visibility::Public,
                    false => Visibility::Private,
                };
                for LoadArgP { local, .. } in &mut load.args {
                    let mut vis = vis;
                    if Module::default_visibility(&local.ident) == Visibility::Private {
                        vis = Visibility::Private;
                    }
                    AssignIdent::collect_assign_ident(
                        local,
                        in_loop,
                        vis,
                        scope_data,
                        frozen_heap,
                        result,
                    );
                }
            }
            stmt => stmt.visit_stmt_mut(|x| {
                Stmt::collect_defines(x, in_loop, scope_data, frozen_heap, result, dialect)
            }),
        }
    }
}

trait AssignIdentCollect {
    fn collect_assign_ident<'a>(
        assign: &'a mut CstAssignIdent,
        in_loop: InLoop,
        vis: Visibility,
        scope_data: &mut ModuleScopeData,
        frozen_heap: &FrozenHeap,
        result: &mut SmallMap<FrozenStringValue, BindingId>,
    );
}

impl AssignIdentCollect for AssignIdent {
    fn collect_assign_ident<'a>(
        assign: &'a mut CstAssignIdent,
        in_loop: InLoop,
        vis: Visibility,
        scope_data: &mut ModuleScopeData,
        frozen_heap: &FrozenHeap,
        result: &mut SmallMap<FrozenStringValue, BindingId>,
    ) {
        // Helper function to untangle lifetimes: we read and modify `assign` fields.
        fn assign_ident_impl<'b>(
            name: FrozenStringValue,
            span: Span,

            binding: &'b mut Option<BindingId>,
            in_loop: InLoop,
            mut vis: Visibility,
            scope_data: &mut ModuleScopeData,
            result: &mut SmallMap<FrozenStringValue, BindingId>,
        ) {
            assert!(
                binding.is_none(),
                "binding can be assigned only once: `{}`",
                name.as_str()
            );
            if vis == Visibility::Public {
                vis = Module::default_visibility(&name);
            }
            match result.entry_hashed(name.get_hashed()) {
                small_map::Entry::Occupied(e) => {
                    let prev_binding_id = *e.get();
                    let prev_binding = scope_data.mut_binding(prev_binding_id);
                    // If we are in the map as Public and Private, then Public wins.
                    // Everything but Load is definitely Public.
                    // So only insert if it wasn't already there.
                    if vis == Visibility::Public {
                        prev_binding.vis = Visibility::Public;
                    }
                    prev_binding.assign_count = AssignCount::Any;
                    *binding = Some(prev_binding_id);
                }
                small_map::Entry::Vacant(e) => {
                    let assign_count = match in_loop {
                        InLoop::Yes => AssignCount::Any,
                        InLoop::No => AssignCount::AtMostOnce,
                    };
                    let (new_binding_id, _) = scope_data.new_binding(
                        name,
                        BindingSource::Source(span),
                        vis,
                        assign_count,
                    );
                    e.insert(new_binding_id);
                    *binding = Some(new_binding_id);
                }
            };
        }
        assign_ident_impl(
            frozen_heap.alloc_str_intern(&assign.node.ident),
            assign.span,
            &mut assign.node.payload,
            in_loop,
            vis,
            scope_data,
            result,
        );
    }
}

trait AssignTargetCollectDefinesLvalue {
    fn collect_defines_lvalue<'a>(
        expr: &'a mut CstAssignTarget,
        in_loop: InLoop,
        scope_data: &mut ModuleScopeData,
        frozen_heap: &FrozenHeap,
        result: &mut SmallMap<FrozenStringValue, BindingId>,
    );
}

impl AssignTargetCollectDefinesLvalue for AssignTarget {
    // Collect variables defined in an expression on the LHS of an assignment (or
    // for variable etc)
    fn collect_defines_lvalue<'a>(
        expr: &'a mut CstAssignTarget,
        in_loop: InLoop,
        scope_data: &mut ModuleScopeData,
        frozen_heap: &FrozenHeap,
        result: &mut SmallMap<FrozenStringValue, BindingId>,
    ) {
        expr.node.visit_lvalue_mut(|x| {
            AssignIdent::collect_assign_ident(
                x,
                in_loop,
                Visibility::Public,
                scope_data,
                frozen_heap,
                result,
            )
        });
    }
}

/// Storage of objects referenced by AST.
#[derive(Default)]
pub(crate) struct ModuleScopeData<'f> {
    /// Bindings by id.
    bindings: Vec<Binding<'f>>,
    /// Scopes by id.
    scopes: Vec<ScopeNames<'f>>,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum AssignCount {
    /// Variable is assigned at most once during the execution of the scope.
    AtMostOnce,
    /// Variable may be assigned more than once during execution of the scope.
    Any,
}

/// Was a binding captured by nested def or lambda scopes?
#[derive(Debug, Copy, Clone, Dupe, Eq, PartialEq, VisitSpanMut)]
pub(crate) enum Captured {
    Yes,
    No,
}

#[derive(Debug)]
pub(crate) enum BindingSource {
    /// Variable is defined in the source of the module.
    Source(Span),
    /// Variable came from `Module`, not defined in the source file.
    FromModule,
}

/// Binding defines a place for a variable.
///
/// For example, in code `x = 1; x = 2`, there's one binding for name `x`.
///
/// In code `x = 1; def f(): x = 2`, there are two bindings for name `x`.
#[derive(Debug)]
pub(crate) struct Binding<'f> {
    pub(crate) name: FrozenStringValue,
    pub(crate) source: BindingSource,
    pub(crate) vis: Visibility,
    /// `slot` is `None` when it is not initialized yet.
    /// When analysis is completed, `slot` is always `Some`.
    slot: Option<Slot>,
    pub(crate) assign_count: AssignCount,
    // Whether a variable defined in a scope gets captured in nested def or lambda scope.
    // (Comprehension scopes do not count, because they are considered
    // local by the runtime and do not allocate a frame).
    pub(crate) captured: Captured,
    _marker: PhantomData<&'f ()>,
}

impl<'f> Binding<'f> {
    fn new(
        name: FrozenStringValue,
        source: BindingSource,
        vis: Visibility,
        assign_count: AssignCount,
    ) -> Binding<'f> {
        Binding {
            name,
            source,
            vis,
            slot: None,
            assign_count,
            captured: Captured::No,
            _marker: PhantomData,
        }
    }

    fn span(&self) -> Span {
        match self.source {
            BindingSource::Source(span) => span,
            BindingSource::FromModule => Span::default(),
        }
    }

    /// Get resolved slot after analysis is completed.
    pub(crate) fn resolved_slot(&self, codemap: &CodeMap) -> Result<Slot, InternalError> {
        match self.slot {
            Some(slot) => Ok(slot),
            None => Err(InternalError::msg(
                "slot is not resolved",
                self.span(),
                codemap,
            )),
        }
    }

    /// Initialize the slot during analysis.
    pub(crate) fn init_slot(&mut self, slot: Slot, codemap: &CodeMap) -> Result<(), InternalError> {
        match mem::replace(&mut self.slot, Some(slot)) {
            Some(_) => Err(InternalError::msg(
                "slot is already assigned",
                self.span(),
                codemap,
            )),
            None => Ok(()),
        }
    }
}

/// If of a binding within current module.
#[derive(Copy, Clone, Dupe, Debug, Hash, PartialEq, Eq, Ord, PartialOrd)]
pub(crate) struct BindingId(usize);

/// Id of a scope within current module.
#[derive(Copy, Clone, Dupe, Debug, Eq, PartialEq)]
pub(crate) struct ScopeId(usize);

impl ScopeId {
    pub(crate) fn module() -> ScopeId {
        ScopeId(0)
    }
}

impl<'f> ModuleScopeData<'f> {
    pub(crate) fn new() -> ModuleScopeData<'f> {
        ModuleScopeData::default()
    }

    pub(crate) fn get_binding(&self, BindingId(id): BindingId) -> &Binding<'f> {
        &self.bindings[id]
    }

    fn mut_binding(&mut self, BindingId(id): BindingId) -> &mut Binding<'f> {
        &mut self.bindings[id]
    }

    fn new_binding(
        &mut self,
        name: FrozenStringValue,
        source: BindingSource,
        vis: Visibility,
        assigned_count: AssignCount,
    ) -> (BindingId, &mut Binding<'f>) {
        let binding_id = BindingId(self.bindings.len());
        self.bindings
            .push(Binding::new(name, source, vis, assigned_count));
        (binding_id, self.bindings.last_mut().unwrap())
    }

    pub(crate) fn get_scope(&self, ScopeId(id): ScopeId) -> &ScopeNames<'f> {
        &self.scopes[id]
    }

    pub(crate) fn mut_scope(&mut self, ScopeId(id): ScopeId) -> &mut ScopeNames<'f> {
        &mut self.scopes[id]
    }

    pub(crate) fn new_scope(&mut self) -> (ScopeId, &mut ScopeNames<'f>) {
        let scope_id = ScopeId(self.scopes.len());
        self.scopes.push(ScopeNames::default());
        (scope_id, self.scopes.last_mut().unwrap())
    }

    /// Get resolved slot for assigning identifier.
    pub(crate) fn get_assign_ident_slot(
        &self,
        ident: &CstAssignIdent,
        codemap: &CodeMap,
    ) -> (Slot, Captured) {
        let binding_id = ident.payload.expect("binding not assigned for ident");
        let binding = self.get_binding(binding_id);
        let slot = binding.resolved_slot(codemap).unwrap();
        (slot, binding.captured)
    }
}

#[derive(Debug, Clone, Dupe, Copy)]
pub(crate) enum ResolvedIdent {
    Slot(Slot, BindingId),
    Global(FrozenValue),
}

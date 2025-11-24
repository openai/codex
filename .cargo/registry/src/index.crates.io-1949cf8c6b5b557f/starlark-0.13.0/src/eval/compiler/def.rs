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

//! Implementation of `def`.

use std::cell::UnsafeCell;
use std::collections::HashMap;
use std::fmt;
use std::fmt::Write;
use std::ptr;

use allocative::Allocative;
use derivative::Derivative;
use derive_more::Display;
use dupe::Dupe;
use once_cell::sync::Lazy;
use starlark_derive::starlark_value;
use starlark_derive::NoSerialize;
use starlark_derive::VisitSpanMut;
use starlark_map::StarlarkHasher;
use starlark_syntax::eval_exception::EvalException;
use starlark_syntax::slice_vec_ext::SliceExt;
use starlark_syntax::syntax::def::DefParam;
use starlark_syntax::syntax::def::DefParamIndices;
use starlark_syntax::syntax::def::DefParamKind;
use starlark_syntax::syntax::def::DefParams;

use crate as starlark;
use crate::any::ProvidesStaticType;
use crate::codemap::CodeMap;
use crate::codemap::Spanned;
use crate::collections::Hashed;
use crate::const_frozen_string;
use crate::docs::DocFunction;
use crate::docs::DocItem;
use crate::docs::DocMember;
use crate::docs::DocString;
use crate::docs::DocStringKind;
use crate::environment::FrozenModuleData;
use crate::environment::Globals;
use crate::eval::bc::bytecode::Bc;
use crate::eval::bc::frame::alloca_frame;
use crate::eval::compiler::def_inline::inline_def_body;
use crate::eval::compiler::def_inline::InlineDefBody;
use crate::eval::compiler::error::CompilerInternalError;
use crate::eval::compiler::expr::ExprCompiled;
use crate::eval::compiler::opt_ctx::OptCtx;
use crate::eval::compiler::scope::payload::CstAssignIdent;
use crate::eval::compiler::scope::payload::CstParameter;
use crate::eval::compiler::scope::payload::CstPayload;
use crate::eval::compiler::scope::payload::CstStmt;
use crate::eval::compiler::scope::payload::CstTypeExpr;
use crate::eval::compiler::scope::Captured;
use crate::eval::compiler::scope::ScopeId;
use crate::eval::compiler::span::IrSpanned;
use crate::eval::compiler::stmt::OptimizeOnFreezeContext;
use crate::eval::compiler::stmt::StmtCompileContext;
use crate::eval::compiler::stmt::StmtsCompiled;
use crate::eval::compiler::Compiler;
use crate::eval::runtime::arguments::ArgumentsImpl;
use crate::eval::runtime::arguments::ResolvedArgName;
use crate::eval::runtime::evaluator::Evaluator;
use crate::eval::runtime::frame_span::FrameSpan;
use crate::eval::runtime::frozen_file_span::FrozenFileSpan;
use crate::eval::runtime::params::spec::ParametersSpec;
use crate::eval::runtime::profile::instant::ProfilerInstant;
use crate::eval::runtime::slots::LocalSlotId;
use crate::eval::runtime::slots::LocalSlotIdCapturedOrNot;
use crate::eval::Arguments;
use crate::starlark_complex_values;
use crate::typing::callable_param::ParamIsRequired;
use crate::typing::ParamSpec;
use crate::typing::Ty;
use crate::util::arc_str::ArcStr;
use crate::values::frozen_ref::AtomicFrozenRefOption;
use crate::values::function::FUNCTION_TYPE;
use crate::values::typing::type_compiled::compiled::TypeCompiled;
use crate::values::Freeze;
use crate::values::FreezeResult;
use crate::values::Freezer;
use crate::values::FrozenHeap;
use crate::values::FrozenRef;
use crate::values::FrozenStringValue;
use crate::values::FrozenValue;
use crate::values::Heap;
use crate::values::StarlarkValue;
use crate::values::Trace;
use crate::values::Tracer;
use crate::values::Value;
use crate::values::ValueLike;

#[derive(thiserror::Error, Debug)]
enum DefError {
    #[error("Function has no type, while function was compiled with return type (internal error)")]
    CheckReturnTypeNoType,
}

/// Store frozen `StmtCompiled`.
/// This is initialized in `post_freeze`.
struct StmtCompiledCell {
    cell: UnsafeCell<Bc>,
}

unsafe impl<'v> Trace<'v> for StmtCompiledCell {
    fn trace(&mut self, _: &Tracer<'v>) {
        // Bytecode contains only frozen values.
    }
}

unsafe impl Sync for StmtCompiledCell {}
unsafe impl Send for StmtCompiledCell {}

impl StmtCompiledCell {
    fn new() -> StmtCompiledCell {
        StmtCompiledCell {
            cell: UnsafeCell::new(Bc::default()),
        }
    }

    /// This function is unsafe if other thread is executing the stmt.
    unsafe fn set(&self, value: Bc) {
        ptr::drop_in_place(self.cell.get());
        ptr::write(self.cell.get(), value);
    }

    fn get(&self) -> &Bc {
        unsafe { &*self.cell.get() }
    }
}

#[derive(Clone, Debug, VisitSpanMut)]
pub(crate) struct ParameterName {
    pub(crate) name: String,
    captured: Captured,
}

#[derive(Clone, Debug, VisitSpanMut)]
pub(crate) enum ParameterCompiled<T> {
    Normal(
        /// Name.
        ParameterName,
        /// Type.
        Option<TypeCompiled<FrozenValue>>,
        /// Default value.
        Option<T>,
    ),
    Args(ParameterName, Option<TypeCompiled<FrozenValue>>),
    KwArgs(ParameterName, Option<TypeCompiled<FrozenValue>>),
}

impl<T> ParameterCompiled<T> {
    pub(crate) fn map_expr<U>(&self, f: impl FnMut(&T) -> U) -> ParameterCompiled<U> {
        match self {
            ParameterCompiled::Normal(n, o, t) => {
                ParameterCompiled::Normal(n.clone(), *o, t.as_ref().map(f))
            }
            ParameterCompiled::Args(n, o) => ParameterCompiled::Args(n.clone(), *o),
            ParameterCompiled::KwArgs(n, o) => ParameterCompiled::KwArgs(n.clone(), *o),
        }
    }

    pub(crate) fn accepts_positional(&self) -> bool {
        match self {
            ParameterCompiled::Normal(..) => true,
            _ => false,
        }
    }

    pub(crate) fn captured(&self) -> Captured {
        self.name_ty().0.captured
    }

    pub(crate) fn name_ty(&self) -> (&ParameterName, Option<TypeCompiled<FrozenValue>>) {
        match self {
            Self::Normal(n, t, _) => (n, *t),
            Self::Args(n, t) => (n, *t),
            Self::KwArgs(n, t) => (n, *t),
        }
    }

    pub(crate) fn has_type(&self) -> bool {
        match self.name_ty() {
            (_, Some(_)) => true,
            _ => false,
        }
    }

    pub(crate) fn ty(&self) -> Ty {
        match self.name_ty() {
            (_, Some(t)) => t.as_ty().clone(),
            (_, None) => Ty::any(),
        }
    }

    pub(crate) fn required(&self) -> ParamIsRequired {
        match self {
            ParameterCompiled::Normal(_, _, None) => ParamIsRequired::Yes,
            ParameterCompiled::Normal(_, _, Some(_)) => ParamIsRequired::No,
            ParameterCompiled::Args(..) => ParamIsRequired::No,
            ParameterCompiled::KwArgs(..) => ParamIsRequired::No,
        }
    }

    pub(crate) fn is_star_or_star_star(&self) -> bool {
        matches!(
            self,
            ParameterCompiled::Args(_, _) | ParameterCompiled::KwArgs(_, _)
        )
    }
}

#[derive(Debug, Clone, VisitSpanMut)]
pub(crate) struct ParametersCompiled<T> {
    pub(crate) params: Vec<IrSpanned<ParameterCompiled<T>>>,
    pub(crate) indices: DefParamIndices,
}

impl<T> ParametersCompiled<T> {
    /// How many expressions this parameters references (default values and types).
    pub(crate) fn count_exprs(&self) -> u32 {
        let mut count = 0;
        for p in &self.params {
            p.map_expr(|_e| count += 1);
        }
        count
    }

    /// How many parameter variables?
    pub(crate) fn count_param_variables(&self) -> u32 {
        self.params.len().try_into().unwrap()
    }

    /// Any parameter has type annotation?
    pub(crate) fn has_types(&self) -> bool {
        self.params.iter().any(|p| p.has_type())
    }

    /// Has `*args` or `*kwargs` parameter? `*` is fine.
    pub(crate) fn has_args_or_kwargs(&self) -> bool {
        self.params.iter().any(|p| {
            matches!(
                p.node,
                ParameterCompiled::Args(..) | ParameterCompiled::KwArgs(..)
            )
        })
    }

    pub(crate) fn parameter_captures(&self) -> Vec<LocalSlotId> {
        self.params
            .iter()
            .enumerate()
            .filter_map(|(i, p)| {
                if p.captured() == Captured::Yes {
                    Some(LocalSlotId(i as u32))
                } else {
                    None
                }
            })
            .collect()
    }

    pub(crate) fn to_ty_params(&self) -> ParamSpec {
        ParamSpec::new_parts(
            self.indices.pos_only().map(|i| {
                let p = &self.params[i].node;
                (p.required(), p.ty())
            }),
            self.indices.pos_or_named().map(|i| {
                let p = &self.params[i].node;
                (
                    ArcStr::from(p.name_ty().0.name.as_str()),
                    p.required(),
                    p.ty(),
                )
            }),
            self.indices.args.map(|i| {
                let p = &self.params[i as usize].node;
                p.ty()
            }),
            self.indices.named_only(self.params.len()).map(|i| {
                let p = &self.params[i].node;
                (
                    ArcStr::from(p.name_ty().0.name.as_str()),
                    p.required(),
                    p.ty(),
                )
            }),
            self.indices.kwargs.map(|i| {
                let p = &self.params[i as usize].node;
                p.ty()
            }),
        )
        // TODO(nga): do not unwrap.
        .unwrap()
    }
}

/// Copy local variable slot to nested function.
#[derive(Debug, Clone, Dupe)]
pub(crate) struct CopySlotFromParent {
    /// Slot in the outer function.
    pub(crate) parent: LocalSlotIdCapturedOrNot,
    /// Slot in the nested function.
    pub(crate) child: LocalSlotIdCapturedOrNot,
}

/// Static info for `def`, `lambda` or module.
#[derive(Derivative, Display)]
#[derivative(Debug)]
#[display("DefInfo")]
pub(crate) struct DefInfo {
    pub(crate) name: FrozenStringValue,
    /// Span of function signature.
    pub(crate) signature_span: FrozenFileSpan,
    /// Indices of parameters, which are captured in nested defs.
    parameter_captures: FrozenRef<'static, [LocalSlotId]>,
    /// Type of this function, for the typechecker.
    ty: Ty,
    /// Codemap of the file where the function is declared.
    pub(crate) codemap: FrozenRef<'static, CodeMap>,
    /// The raw docstring pulled out of the AST.
    pub(crate) docstring: Option<String>,
    /// Slots this scope uses, including for parameters and `parent`.
    /// Indexed by [`LocalSlotId`], values are variable names.
    pub(crate) used: FrozenRef<'static, [FrozenStringValue]>,
    /// Slots to copy from the parent.
    /// Module-level identifiers are not copied over, to avoid excess copying.
    pub(crate) parent: FrozenRef<'static, [CopySlotFromParent]>,
    /// Statement compiled for non-frozen def.
    #[derivative(Debug = "ignore")]
    stmt_compiled: Bc,
    // The compiled expression for the body of this definition, to be run
    // after the parameters are evaluated.
    #[derivative(Debug = "ignore")]
    body_stmts: StmtsCompiled,
    /// How to compile the statement on freeze.
    stmt_compile_context: StmtCompileContext,
    /// Function can be inlined.
    pub(crate) inline_def_body: Option<InlineDefBody>,
    /// Globals captured during function or module creation.
    /// Only needed for debugger evaluation.
    pub(crate) globals: FrozenRef<'static, Globals>,
}

impl DefInfo {
    pub(crate) fn empty() -> FrozenRef<'static, DefInfo> {
        static EMPTY: Lazy<DefInfo> = Lazy::new(|| DefInfo {
            name: const_frozen_string!("<empty>"),
            signature_span: FrozenFileSpan::default(),
            parameter_captures: FrozenRef::new(&[]),
            ty: Ty::any(),
            codemap: FrozenRef::new(CodeMap::empty_static()),
            docstring: None,
            used: FrozenRef::new(&[]),
            parent: FrozenRef::new(&[]),
            stmt_compiled: Bc::default(),
            body_stmts: StmtsCompiled::empty(),
            stmt_compile_context: StmtCompileContext::default(),
            inline_def_body: None,
            globals: FrozenRef::new(Globals::empty()),
        });
        FrozenRef::new(&EMPTY)
    }

    pub(crate) fn for_module(
        codemap: FrozenRef<'static, CodeMap>,
        local_names: FrozenRef<'static, [FrozenStringValue]>,
        parent: FrozenRef<'static, [CopySlotFromParent]>,
        globals: FrozenRef<'static, Globals>,
    ) -> DefInfo {
        DefInfo {
            name: const_frozen_string!("<module>"),
            signature_span: FrozenFileSpan::default(),
            parameter_captures: FrozenRef::new(&[]),
            ty: Ty::any(),
            codemap,
            docstring: None,
            used: local_names,
            parent,
            stmt_compiled: Bc::default(),
            body_stmts: StmtsCompiled::empty(),
            stmt_compile_context: StmtCompileContext::default(),
            inline_def_body: None,
            globals,
        }
    }
}

#[derive(Clone, Debug, VisitSpanMut)]
pub(crate) struct DefCompiled {
    pub(crate) function_name: String,
    pub(crate) params: ParametersCompiled<IrSpanned<ExprCompiled>>,
    pub(crate) return_type: Option<TypeCompiled<FrozenValue>>,
    pub(crate) info: FrozenRef<'static, DefInfo>,
}

impl Compiler<'_, '_, '_, '_> {
    fn parameter_name(&mut self, ident: &CstAssignIdent) -> ParameterName {
        let binding_id = ident.payload.expect("no binding for parameter");
        let binding = self.scope_data.get_binding(binding_id);
        ParameterName {
            name: ident.node.ident.clone(),
            captured: binding.captured,
        }
    }

    fn parameter(
        &mut self,
        x: &Spanned<DefParam<'_, CstPayload>>,
    ) -> Result<IrSpanned<ParameterCompiled<IrSpanned<ExprCompiled>>>, CompilerInternalError> {
        let span = FrameSpan::new(FrozenFileSpan::new(self.codemap, x.span));
        let parameter_name = self.parameter_name(x.ident);
        Ok(IrSpanned {
            span,
            node: match &x.node.kind {
                DefParamKind::Regular(_mode, default_value) => ParameterCompiled::Normal(
                    parameter_name,
                    self.expr_for_type(x.ty).map(|t| t.node),
                    default_value.as_ref().map(|d| self.expr(d)).transpose()?,
                ),
                DefParamKind::Args => ParameterCompiled::Args(
                    parameter_name,
                    self.expr_for_type(x.ty).map(|t| t.node),
                ),
                DefParamKind::Kwargs => ParameterCompiled::KwArgs(
                    parameter_name,
                    self.expr_for_type(x.ty).map(|t| t.node),
                ),
            },
        })
    }

    pub fn function(
        &mut self,
        name: &str,
        signature_span: FrozenFileSpan,
        scope_id: ScopeId,
        params: &[CstParameter],
        return_type: Option<&CstTypeExpr>,
        suite: &CstStmt,
    ) -> Result<ExprCompiled, CompilerInternalError> {
        let file = self.codemap.file_span(suite.span);
        let function_name = format!("{}.{}", file.file.filename(), name);
        let name = self.eval.frozen_heap().alloc_str_intern(name);

        let DefParams { params, indices } = match DefParams::unpack(params, &self.codemap) {
            Ok(def_params) => def_params,
            Err(e) => return Err(CompilerInternalError::from_eval_exception(e)),
        };

        // The parameters run in the scope of the parent, so compile them with the outer
        // scope
        let params: Vec<_> = params
            .iter()
            .map(|x| self.parameter(x))
            .collect::<Result<_, CompilerInternalError>>()?;
        let params = ParametersCompiled { params, indices };
        let return_type = self.expr_for_type(return_type).map(|t| t.node);

        let ty = Ty::function(
            params.to_ty_params(),
            return_type.map_or(Ty::any(), |t| t.as_ty().clone()),
        );

        self.enter_scope(scope_id);

        let docstring = DocString::extract_raw_starlark_docstring(suite);
        let body = self.stmt(suite, false)?;
        let scope_id = self.exit_scope();
        let scope_names = self.scope_data.get_scope(scope_id);

        let has_types = return_type.is_some() || params.has_types();

        let inline_def_body = if has_types {
            // It is harder to inline if a function declares parameter types or return type.
            None
        } else {
            inline_def_body(&params, &body)
        };

        let param_count = params.count_param_variables();

        let used = self.eval.frozen_heap().alloc_any_slice(&scope_names.used);
        let info = self.eval.module_env.frozen_heap().alloc_any(DefInfo {
            name,
            signature_span,
            parameter_captures: self
                .eval
                .frozen_heap()
                .alloc_any_slice(&params.parameter_captures()),
            ty,
            codemap: self.codemap,
            docstring,
            used,
            parent: self.eval.frozen_heap().alloc_any_slice(&scope_names.parent),
            stmt_compiled: body.as_bc(
                &self.compile_context(return_type.is_some()),
                used,
                param_count,
                self.eval.module_env.frozen_heap(),
            ),
            body_stmts: body,
            inline_def_body,
            stmt_compile_context: self.compile_context(return_type.is_some()),
            globals: self.globals,
        });

        Ok(ExprCompiled::Def(DefCompiled {
            function_name,
            params,
            return_type,
            info,
        }))
    }
}

/// Starlark function internal representation and implementation of
/// [`StarlarkValue`].
#[derive(Derivative, NoSerialize, ProvidesStaticType, Trace, Allocative)]
#[derivative(Debug)]
pub(crate) struct DefGen<V> {
    pub(crate) parameters: ParametersSpec<V>, // The parameters, **kwargs etc including defaults (which are evaluated afresh each time)
    /// Indices of parameters, which are captured in nested defs.
    /// This is a copy of `DefInfo.parameter_captures`.
    parameter_captures: FrozenRef<'static, [LocalSlotId]>,
    // The types of the parameters.
    // (Sparse indexed array, (0, argm T) implies parameter 0 named arg must have type T).
    parameter_types: Vec<(LocalSlotId, String, TypeCompiled<FrozenValue>)>,
    pub(crate) return_type: Option<TypeCompiled<FrozenValue>>, // The return type annotation for the function
    /// Data created during function compilation but before function instantiation.
    /// `DefInfo` can be shared by multiple `def` instances, for example,
    /// `lambda` functions can be instantiated multiple times.
    pub(crate) def_info: FrozenRef<'static, DefInfo>,
    /// Any variables captured from the outer scope (nested def/lambda).
    /// Values are either [`Value`] or [`FrozenValue`] pointing respectively to
    /// [`ValueCaptured`] or [`FrozenValueCaptured`].
    captured: Vec<V>,
    // Important to ignore these field as it probably references DefGen in a cycle
    #[derivative(Debug = "ignore")]
    /// A reference to the module where the function is defined after the module has been frozen.
    /// When the module is not frozen yet, this field contains `None`, and function's module
    /// can be accessed from evaluator's module.
    #[allocative(skip)]
    pub(crate) module: AtomicFrozenRefOption<FrozenModuleData>,
    /// This field is only used in `FrozenDef`. It is populated in `post_freeze`.
    #[derivative(Debug = "ignore")]
    #[allocative(skip)]
    optimized_on_freeze_stmt: StmtCompiledCell,
}

impl<V> Display for DefGen<V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.parameters.signature())
    }
}

pub(crate) type Def<'v> = DefGen<Value<'v>>;
pub(crate) type FrozenDef = DefGen<FrozenValue>;

starlark_complex_values!(Def);

impl<'v> Def<'v> {
    pub(crate) fn new(
        parameters: ParametersSpec<Value<'v>>,
        parameter_types: Vec<(LocalSlotId, String, TypeCompiled<FrozenValue>)>,
        return_type: Option<TypeCompiled<FrozenValue>>,
        stmt: FrozenRef<'static, DefInfo>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<Value<'v>> {
        let captured = stmt
            .parent
            .as_ref()
            .map(|copy| eval.clone_slot_capture(copy, &stmt));
        Ok(eval.heap().alloc(Self {
            parameters,
            parameter_captures: stmt.parameter_captures,
            parameter_types,
            return_type,
            captured,
            module: AtomicFrozenRefOption::new(eval.top_frame_def_frozen_module(false)?),
            optimized_on_freeze_stmt: StmtCompiledCell::new(),
            def_info: stmt,
        }))
    }
}

impl<'v> Freeze for Def<'v> {
    type Frozen = FrozenDef;

    fn freeze(self, freezer: &Freezer) -> FreezeResult<Self::Frozen> {
        let parameters = self.parameters.freeze(freezer)?;
        let parameter_types = self.parameter_types.freeze(freezer)?;
        let return_type = self.return_type.freeze(freezer)?;
        let captured = self.captured.try_map(|x| x.freeze(freezer))?;
        let module = AtomicFrozenRefOption::new(self.module.load_relaxed());
        Ok(FrozenDef {
            parameters,
            parameter_captures: self.parameter_captures,
            parameter_types,
            return_type,
            def_info: self.def_info,
            captured,
            module,
            optimized_on_freeze_stmt: self.optimized_on_freeze_stmt,
        })
    }
}

pub(crate) trait DefLike<'v> {
    const FROZEN: bool;
}

impl<'v> DefLike<'v> for DefGen<Value<'v>> {
    const FROZEN: bool = false;
}

impl<'v> DefLike<'v> for DefGen<FrozenValue> {
    const FROZEN: bool = true;
}

#[starlark_value(type = FUNCTION_TYPE)]
impl<'v, V: ValueLike<'v>> StarlarkValue<'v> for DefGen<V>
where
    Self: ProvidesStaticType<'v> + DefLike<'v>,
{
    fn name_for_call_stack(&self, _me: Value<'v>) -> String {
        self.def_info.name.as_str().to_owned()
    }

    fn invoke(
        &self,
        me: Value<'v>,
        args: &Arguments<'v, '_>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> crate::Result<Value<'v>> {
        self.invoke_impl(me, &args.0, eval)
    }

    fn documentation(&self) -> DocItem {
        let mut parameter_types = vec![Ty::any(); self.parameters.len()];
        for (idx, _, ty) in &self.parameter_types {
            // Local slot number for parameter is the same as parameter index.
            parameter_types[idx.0 as usize] = ty.as_ty().clone();
        }

        let return_type = self.return_type.map_or(Ty::any(), |r| r.as_ty().clone());

        let function_docs = DocFunction::from_docstring(
            DocStringKind::Starlark,
            self.parameters
                .documentation(parameter_types, HashMap::new()),
            return_type,
            self.def_info.docstring.as_ref().map(String::as_ref),
        );

        DocItem::Member(DocMember::Function(function_docs))
    }

    fn typechecker_ty(&self) -> Option<Ty> {
        Some(self.def_info.ty.clone())
    }

    fn write_hash(&self, hasher: &mut StarlarkHasher) -> crate::Result<()> {
        // It's hard to come up with a good hash here, but let's at least make an effort.
        self.def_info.name.write_hash(hasher)
    }
}

impl<'v, V: ValueLike<'v>> DefGen<V>
where
    Self: DefLike<'v>,
{
    pub(crate) fn bc(&self) -> &Bc {
        if Self::FROZEN {
            self.optimized_on_freeze_stmt.get()
        } else {
            &self.def_info.stmt_compiled
        }
    }

    fn check_parameter_types(&self, eval: &mut Evaluator<'v, '_, '_>) -> crate::Result<()> {
        let start = if eval.typecheck_profile.enabled {
            Some(ProfilerInstant::now())
        } else {
            None
        };
        for (i, arg_name, ty) in &self.parameter_types {
            match eval.current_frame.get_slot(i.to_captured_or_not()) {
                None => {
                    panic!("Not allowed optional unassigned with type annotations on them")
                }
                Some(v) => ty.check_type(v, Some(arg_name))?,
            }
        }
        if let Some(start) = start {
            eval.typecheck_profile
                .add(self.def_info.name, start.elapsed());
        }
        Ok(())
    }

    pub(crate) fn check_return_type(
        &self,
        ret: Value<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> crate::Result<()> {
        let return_type_ty: TypeCompiled<FrozenValue> = self
            .return_type
            .ok_or_else(|| crate::Error::new_other(DefError::CheckReturnTypeNoType))?;
        let start = if eval.typecheck_profile.enabled {
            Some(ProfilerInstant::now())
        } else {
            None
        };
        return_type_ty.check_type(ret, None)?;
        if let Some(start) = start {
            eval.typecheck_profile
                .add(self.def_info.name, start.elapsed());
        }
        Ok(())
    }

    #[inline(always)]
    fn invoke_impl<'a, A: ArgumentsImpl<'v, 'a>>(
        &self,
        me: Value<'v>,
        args: &A,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> crate::Result<Value<'v>>
    where
        'v: 'a,
    {
        let bc = self.bc();
        alloca_frame(
            eval,
            bc.local_count,
            bc.max_stack_size,
            bc.max_loop_depth,
            |eval| {
                // SAFETY: `slots` is unique: `alloca_frame` just allocated the frame,
                //   so there are no references to the frame except `eval.current_frame`.
                //   We use `slots` only in `collect_inline`,
                //   which does not have access to `eval` thus cannot access the frame indirectly.
                let slots = unsafe { eval.current_frame.locals_mut() };
                self.parameters.collect_inline(args, slots, eval.heap())?;
                self.invoke_raw(me, eval)
            },
        )
    }

    pub(crate) fn invoke_with_args<'a, A: ArgumentsImpl<'v, 'a>>(
        &self,
        me: Value<'v>,
        args: &A,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> crate::Result<Value<'v>>
    where
        'v: 'a,
    {
        // This is trivial function which delegates to `invoke_impl`.
        // `invoke_impl` is called from two places,
        // giving this function different name makes this function easier to see in profiler.
        self.invoke_impl(me, args, eval)
    }

    /// Invoke the function, assuming that:
    /// * the frame has been allocated and stored in `eval.current_frame`
    /// * the arguments have been collected into the frame
    #[inline(always)]
    fn invoke_raw(
        &self,
        me: Value<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> crate::Result<Value<'v>> {
        // println!("invoking {}", self.def.stmt.name.node);

        if !self.parameter_types.is_empty() {
            self.check_parameter_types(eval)?;
        }

        // Parameters are collected into local slots without captures
        // (to avoid even more branches in parameter capture),
        // and this loop wraps captured parameters.
        for &captured in &*self.parameter_captures {
            eval.wrap_local_slot_captured(captured);
        }

        // Copy over the parent slots.
        // Explicitly check `self.captured` is not empty to avoid accessing
        // self.def_info.scope_names which is two indirections.
        if !self.captured.is_empty() {
            for (copy, captured) in self.def_info.parent.iter().zip(self.captured.iter()) {
                eval.current_frame.set_slot(copy.child, captured.to_value());
            }
        }

        if Self::FROZEN {
            debug_assert!(self.module.load_relaxed().is_some());
        }

        eval.eval_bc(me, self.bc())
            .map_err(EvalException::into_error)
    }

    pub(crate) fn resolve_arg_name(&self, name: Hashed<&str>) -> ResolvedArgName {
        self.parameters.resolve_name(name)
    }

    pub(crate) fn dump_debug(&self) -> String {
        let mut w = String::new();
        writeln!(w, "Bytecode:").unwrap();
        self.bc()
            .dump_debug()
            .lines()
            .for_each(|l| writeln!(w, "  {}", l).unwrap());
        w
    }
}

impl FrozenDef {
    pub(crate) fn post_freeze(
        &self,
        module: FrozenRef<FrozenModuleData>,
        heap: &Heap,
        frozen_heap: &FrozenHeap,
    ) {
        // Module passed to this function is not always module where the function is declared:
        // A function can be created in a frozen module and frozen later in another module.
        // `def_module` variable contains a module where this `def` is declared.
        let def_module = match self.module.load_relaxed() {
            None => {
                self.module.store_relaxed(module);
                module
            }
            Some(module) => module,
        };

        // Now perform the optimization of function body with fully frozen module:
        // all module variables are frozen, so we can inline more aggressively.
        let body_optimized = self
            .def_info
            .body_stmts
            .optimize(&mut OptCtx::new(
                &mut OptimizeOnFreezeContext {
                    module: def_module.as_ref(),
                    heap,
                    frozen_heap,
                },
                self.parameters.len().try_into().unwrap(),
            ))
            .as_bc(
                &self.def_info.stmt_compile_context,
                self.def_info.used,
                self.parameters.len() as u32,
                frozen_heap,
            );

        // Store the optimized body.
        // This is (relatively) safe because we know that during freeze
        // nobody has a reference to stmt: nobody is executing this `def`.
        unsafe {
            self.optimized_on_freeze_stmt.set(body_optimized);
        }
    }
}

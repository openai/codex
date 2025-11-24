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

use std::fmt;
use std::fmt::Debug;
use std::fmt::Display;
use std::sync::Arc;

use allocative::Allocative;
use dupe::Dupe;
use dupe::IterDupedExt;
use either::Either;
use starlark_derive::Trace;
use starlark_syntax::codemap::CodeMap;
use starlark_syntax::codemap::Span;
use starlark_syntax::codemap::Spanned;

use crate as starlark;
use crate::__derive_refs::components::NativeCallableComponents;
use crate::eval::compiler::small_vec_1::SmallVec1;
use crate::typing::arc_ty::ArcTy;
use crate::typing::basic::TyBasic;
use crate::typing::call_args::TyCallArgs;
use crate::typing::callable::TyCallable;
use crate::typing::custom::TyCustom;
use crate::typing::custom::TyCustomImpl;
use crate::typing::error::TypingNoContextError;
use crate::typing::function::TyCustomFunction;
use crate::typing::function::TyCustomFunctionImpl;
use crate::typing::function::TyFunction;
use crate::typing::small_arc_vec::SmallArcVec1;
use crate::typing::starlark_value::TyStarlarkValue;
use crate::typing::structs::TyStruct;
use crate::typing::tuple::TyTuple;
use crate::typing::ParamSpec;
use crate::typing::TypingOracleCtx;
use crate::values::bool::StarlarkBool;
use crate::values::typing::never::TypingNever;
use crate::values::StarlarkValue;
use crate::values::Value;

/// A typing operation wasn't able to produce a precise result,
/// so made some kind of approximation.
#[derive(Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct Approximation {
    /// The category of the approximation, e.g. `"Unknown type"`.
    pub category: &'static str,
    /// The precise details of this approximation, e.g. which type was unknown.
    pub message: String,
}

impl Approximation {
    /// Create a new [`Approximation`].
    pub fn new(category: &'static str, message: impl Debug) -> Self {
        Self {
            category,
            message: format!("{:?}", message),
        }
    }
}

impl Display for Approximation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Approximation: {} = {:?}", self.category, self.message)
    }
}

/// A Starlark type.
#[derive(
    Debug, Clone, Dupe, PartialEq, Eq, Hash, PartialOrd, Ord, Allocative, Trace
)]
pub struct Ty {
    /// A series of alternative types.
    ///
    /// When typechecking, we try all alternatives, and if at least one of them
    /// succeeds, then the whole expression is considered to be a success.
    ///
    /// For example, when typechecking:
    ///
    /// ```python
    /// x = ... # string or int
    /// y = ... # string
    /// x + y   # `int + string` fails, but `string + string` succeeds,
    ///         # so the whole expression is typechecked successfully as `string`
    /// ```
    ///
    /// This is different handling of union types than in TypeScript for example,
    /// TypeScript would consider such expression to be an error.
    alternatives: SmallArcVec1<TyBasic>,
}

fn merge_adjacent<T>(xs: Vec<T>, f: impl Fn(T, T) -> Either<T, (T, T)>) -> SmallVec1<T> {
    let mut res = SmallVec1::new();
    let mut last = None;
    for x in xs {
        match last {
            None => last = Some(x),
            Some(l) => match f(l, x) {
                Either::Left(x) => last = Some(x),
                Either::Right((l, x)) => {
                    res.push(l);
                    last = Some(x)
                }
            },
        }
    }
    if let Some(l) = last {
        res.push(l)
    }
    res
}

impl Ty {
    /// Create a [`Ty::any`], but tagged in such a way it can easily be found.
    pub fn todo() -> Self {
        Ty::any()
    }

    /// Turn a type back into a name, potentially erasing some structure.
    /// E.g. the type `[bool]` would return `list`.
    /// Types like [`Ty::any`] will return `None`.
    pub fn as_name(&self) -> Option<&str> {
        match self.alternatives.as_slice() {
            [x] => x.as_name(),
            _ => None,
        }
    }

    /// This type is `TyStarlarkValue`.
    pub(crate) fn is_starlark_value(&self) -> Option<TyStarlarkValue> {
        match self.iter_union() {
            [TyBasic::StarlarkValue(x)] => Some(*x),
            _ => None,
        }
    }

    pub(crate) const fn basic(basic: TyBasic) -> Self {
        Ty {
            alternatives: SmallArcVec1::one(basic),
        }
    }

    /// "any" type: can hold any value.
    pub const fn any() -> Self {
        Ty::basic(TyBasic::Any)
    }

    /// Never type: can hold no value.
    pub const fn never() -> Self {
        Ty {
            alternatives: SmallArcVec1::empty(),
        }
    }

    /// Create a `None` type.
    pub const fn none() -> Self {
        Ty::basic(TyBasic::none())
    }

    /// Create a boolean type.
    pub const fn bool() -> Self {
        Ty::starlark_value::<StarlarkBool>()
    }

    /// Create the int type.
    pub const fn int() -> Self {
        Ty::basic(TyBasic::int())
    }

    /// Create a float type.
    pub const fn float() -> Self {
        Ty::basic(TyBasic::float())
    }

    /// Create a string type.
    pub const fn string() -> Self {
        Ty::basic(TyBasic::string())
    }

    /// Create a list type.
    pub fn list(element: Ty) -> Self {
        Ty::basic(TyBasic::list(element))
    }

    pub(crate) fn any_list() -> Self {
        Self::list(Ty::any())
    }

    /// Create a iterable type.
    pub fn iter(item: Ty) -> Self {
        Ty::basic(TyBasic::iter(item))
    }

    /// Create a dictionary type.
    pub fn dict(key: Ty, value: Ty) -> Self {
        Ty::basic(TyBasic::dict(key, value))
    }

    pub(crate) fn any_dict() -> Self {
        Self::dict(Ty::any(), Ty::any())
    }

    /// Create a set type.
    pub fn set(item: Ty) -> Self {
        Ty::basic(TyBasic::set(item))
    }

    pub(crate) fn any_set() -> Self {
        Self::set(Ty::any())
    }

    /// Create a tuple of two elements
    pub fn tuple2(a: Ty, b: Ty) -> Self {
        Ty::tuple(vec![a, b])
    }

    /// Create a tuple of given elements.
    pub fn tuple(elems: Vec<Ty>) -> Self {
        Ty::basic(TyBasic::Tuple(TyTuple::Elems(elems.into())))
    }

    /// Tuple where elements are unknown.
    pub(crate) fn any_tuple() -> Self {
        Self::tuple_of(Ty::any())
    }

    pub(crate) fn tuple_of(item: Ty) -> Self {
        Ty::basic(TyBasic::Tuple(TyTuple::Of(ArcTy::new(item))))
    }

    /// Create a function type.
    pub fn function(params: ParamSpec, result: Ty) -> Self {
        Self::ty_function(TyFunction::new(params, result))
    }

    /// Create a function type.
    pub fn callable(params: ParamSpec, result: Ty) -> Self {
        Ty::basic(TyBasic::Callable(TyCallable::new(params, result)))
    }

    /// Create a function type.
    pub fn ty_function(f: TyFunction) -> Self {
        Self::custom(TyCustomFunction(f))
    }

    /// Create a function, where the first argument is the result of `.type`.
    pub fn ctor_function(type_attr: Ty, params: ParamSpec, result: Ty) -> Self {
        Self::custom(TyCustomFunction(TyFunction::new_with_type_attr(
            params, result, type_attr,
        )))
    }

    /// Function type that accepts any arguments and returns any result.
    pub(crate) fn any_callable() -> Self {
        Ty::basic(TyBasic::Callable(TyCallable::any()))
    }

    pub(crate) fn any_struct() -> Self {
        Ty::custom(TyStruct::any())
    }

    /// Type from the implementation of [`StarlarkValue`].
    pub const fn starlark_value<'v, T: StarlarkValue<'v>>() -> Self {
        Ty::basic(TyBasic::starlark_value::<T>())
    }

    pub(crate) fn is_any(&self) -> bool {
        self == &Ty::any()
    }

    pub(crate) fn is_never(&self) -> bool {
        self.alternatives.is_empty()
    }

    pub(crate) fn is_list(&self) -> bool {
        matches!(self.alternatives.as_slice(), [TyBasic::List(_)])
    }

    pub(crate) fn is_function(&self) -> bool {
        self.as_name() == Some("function")
    }

    /// If this type is function, return the function type.
    ///
    /// This is exposed for buck2 providers implementation,
    /// probably it does not do what you think.
    pub fn as_function(&self) -> Option<&TyFunction> {
        match self.iter_union() {
            [x] => x.as_function(),
            _ => None,
        }
    }

    /// Create a unions type, which will be normalised before being created.
    pub fn unions(xs: Vec<Self>) -> Self {
        // Handle common cases first.

        if xs.iter().any(|x| x.is_any()) {
            return Ty::any();
        }

        fn next_skip_never<I: Iterator<Item = Ty>>(iter: &mut I) -> Option<Ty> {
            iter.find(|x| !x.is_never())
        }

        let mut xs = xs.into_iter();
        let Some(x0) = next_skip_never(&mut xs) else {
            return Ty::never();
        };
        let Some(x1) = next_skip_never(&mut xs) else {
            return x0;
        };
        if xs.len() == 0 && x0 == x1 {
            return x0;
        }

        // Now default slow version.

        let xs = xs.as_slice();
        let mut xs: Vec<TyBasic> = [x0, x1]
            .iter()
            .chain(xs)
            .flat_map(|x| x.iter_union())
            .duped()
            .collect();
        xs.sort();
        xs.dedup();
        // Try merging adjacent elements
        let xs = merge_adjacent(xs, |x, y| match (x, y) {
            (TyBasic::List(x), TyBasic::List(y)) => {
                Either::Left(TyBasic::List(ArcTy::union2(x, y)))
            }
            (TyBasic::Dict(x_k, x_v), TyBasic::Dict(y_k, y_v)) => Either::Left(TyBasic::Dict(
                ArcTy::union2(x_k, y_k),
                ArcTy::union2(x_v, y_v),
            )),
            (TyBasic::Custom(x), TyBasic::Custom(y)) => match TyCustom::union2(x, y) {
                Ok(u) => Either::Left(TyBasic::Custom(u)),
                Err((x, y)) => Either::Right((TyBasic::Custom(x), TyBasic::Custom(y))),
            },
            xy => Either::Right(xy),
        });

        Ty {
            alternatives: xs.into_iter().collect(),
        }
    }

    /// Iterate over the types within a union, pretending the type is a singleton union if not a union.
    pub fn iter_union(&self) -> &[TyBasic] {
        &self.alternatives
    }

    /// Apply typechecking operation for each alternative.
    ///
    /// If at least one was successful, return the union of all successful results.
    pub(crate) fn typecheck_union_simple(
        &self,
        typecheck: impl Fn(&TyBasic) -> Result<Ty, TypingNoContextError>,
    ) -> Result<Ty, TypingNoContextError> {
        if self.is_any() || self.is_never() {
            Ok(self.dupe())
        } else {
            match self.iter_union() {
                // Optimize common case.
                [x] => typecheck(x),
                xs => {
                    let mut good = Vec::with_capacity(self.alternatives.len());
                    for basic in xs {
                        match typecheck(basic) {
                            Ok(ty) => good.push(ty),
                            Err(TypingNoContextError) => {}
                        }
                    }
                    if good.is_empty() {
                        Err(TypingNoContextError)
                    } else {
                        Ok(Ty::unions(good))
                    }
                }
            }
        }
    }

    /// Create a union of two entries.
    pub fn union2(a: Self, b: Self) -> Self {
        // Handle fast cases first.
        // Optimizations, semantically identical to default implementation.
        if a.is_any() || b.is_any() {
            Ty::any()
        } else if a == b {
            a
        } else if a.is_never() {
            b
        } else if b.is_never() {
            a
        } else {
            Ty::unions(vec![a, b])
        }
    }

    /// Create a custom type.
    /// This is called from generated code.
    pub fn custom(t: impl TyCustomImpl) -> Self {
        Ty::basic(TyBasic::Custom(TyCustom(Arc::new(t))))
    }

    /// Create a custom function type.
    pub fn custom_function(f: impl TyCustomFunctionImpl) -> Self {
        Ty::custom(TyCustomFunction(f))
    }

    /// Typechecker type of value.
    pub fn of_value(value: Value) -> Ty {
        if let Some(t) = value.get_ref().typechecker_ty() {
            t
        } else {
            value.get_type_starlark_repr()
        }
    }

    /// Check if the value of this type can be called with given arguments and expected return type.
    #[must_use]
    pub(crate) fn check_call<'a>(
        &self,
        pos: impl IntoIterator<Item = Ty>,
        named: impl IntoIterator<Item = (&'a str, Ty)>,
        args: Option<Ty>,
        kwargs: Option<Ty>,
        expected_return_type: Ty,
    ) -> bool {
        let oracle = TypingOracleCtx {
            codemap: CodeMap::empty_static(),
        };
        let Ok(ret) = oracle.validate_call(
            Span::default(),
            self,
            &TyCallArgs {
                pos: pos
                    .into_iter()
                    .map(|p| Spanned {
                        span: Span::default(),
                        node: p,
                    })
                    .collect(),
                named: named
                    .into_iter()
                    .map(|p| Spanned {
                        span: Span::default(),
                        node: p,
                    })
                    .collect(),
                args: args.map(|t| Spanned {
                    span: Span::default(),
                    node: t,
                }),
                kwargs: kwargs.map(|t| Spanned {
                    span: Span::default(),
                    node: t,
                }),
            },
        ) else {
            return false;
        };
        let Ok(ok) = oracle.intersects(&ret, &expected_return_type) else {
            return false;
        };
        ok
    }

    pub(crate) fn check_intersects(&self, other: &Ty) -> crate::Result<bool> {
        let oracle = TypingOracleCtx {
            codemap: CodeMap::empty_static(),
        };
        match oracle.intersects(self, other) {
            Ok(ok) => Ok(ok),
            Err(e) => Err(e.into_error()),
        }
    }

    pub(crate) fn from_native_callable_components(
        comp: &NativeCallableComponents,
        as_type: Option<Ty>,
    ) -> starlark::Result<Self> {
        let result = comp.return_type.clone();

        let params = comp.param_spec.param_spec();

        match as_type {
            None => Ok(Ty::function(params, result)),
            Some(type_attr) => Ok(Ty::ctor_function(type_attr, params, result)),
        }
    }

    pub(crate) fn fmt_with_config(
        &self,
        f: &mut fmt::Formatter<'_>,
        config: &TypeRenderConfig,
    ) -> fmt::Result {
        match self.iter_union() {
            [] => write!(f, "{}", TypingNever::TYPE),
            xs => {
                for (i, x) in xs.iter().enumerate() {
                    if i != 0 {
                        write!(f, " | ")?;
                    }
                    x.fmt_with_config(f, config)?;
                }
                Ok(())
            }
        }
    }

    pub(crate) fn display_with<'a>(&'a self, config: &'a TypeRenderConfig) -> TyDisplay<'a> {
        TyDisplay { ty: self, config }
    }
}

/// Configuration for rendering types.
pub enum TypeRenderConfig {
    /// Uses the default rendering configuration.
    Default,
    /// Uses for linked type in doc
    LinkedType {
        /// The function to render linked type element
        render_linked_ty_starlark_value: Box<dyn Fn(&TyStarlarkValue) -> String>,
    },
}

pub(crate) struct TyDisplay<'a> {
    ty: &'a Ty,
    config: &'a TypeRenderConfig,
}

impl Display for TyDisplay<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.ty.fmt_with_config(f, self.config)
    }
}

impl Display for Ty {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.fmt_with_config(f, &TypeRenderConfig::Default)
    }
}

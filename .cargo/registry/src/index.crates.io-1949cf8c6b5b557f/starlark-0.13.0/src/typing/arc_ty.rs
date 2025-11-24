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
use std::fmt::Display;
use std::fmt::Formatter;
use std::ops::Deref;
use std::sync::Arc;

use allocative::Allocative;
use dupe::Dupe;

use crate::typing::ty::TypeRenderConfig;
use crate::typing::Ty;

#[derive(Dupe, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Allocative)]
enum ArcTyInner {
    // These are shortcuts to avoid allocations for common cases.
    Any,
    Never,
    Str,
    Int,
    Bool,
    None,
    /// Default implementation.
    Arc(Arc<Ty>),
}

impl Display for ArcTyInner {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            ArcTyInner::Any => Display::fmt(&Ty::any(), f),
            ArcTyInner::Never => Display::fmt(&Ty::never(), f),
            ArcTyInner::Str => Display::fmt(&Ty::string(), f),
            ArcTyInner::Int => Display::fmt(&Ty::int(), f),
            ArcTyInner::Bool => Display::fmt(&Ty::bool(), f),
            ArcTyInner::None => Display::fmt(&Ty::none(), f),
            ArcTyInner::Arc(ty) => Display::fmt(ty, f),
        }
    }
}

/// Wrapper for `Ty` which is smaller than `Ty`.
#[derive(
    Dupe,
    Clone,
    Eq,
    PartialEq,
    Ord,
    PartialOrd,
    Hash,
    derive_more::Display,
    Debug,
    Allocative
)]
pub struct ArcTy(ArcTyInner);

impl ArcTy {
    pub(crate) fn any() -> ArcTy {
        ArcTy(ArcTyInner::Any)
    }

    pub(crate) fn new(ty: Ty) -> ArcTy {
        if ty.is_any() {
            ArcTy::any()
        } else if ty.is_never() {
            ArcTy(ArcTyInner::Never)
        } else if ty == Ty::string() {
            ArcTy(ArcTyInner::Str)
        } else if ty == Ty::int() {
            ArcTy(ArcTyInner::Int)
        } else if ty == Ty::bool() {
            ArcTy(ArcTyInner::Bool)
        } else if ty == Ty::none() {
            ArcTy(ArcTyInner::None)
        } else {
            ArcTy(ArcTyInner::Arc(Arc::new(ty)))
        }
    }

    pub(crate) fn to_ty(&self) -> Ty {
        Ty::clone(self)
    }

    pub(crate) fn union2(a: ArcTy, b: ArcTy) -> ArcTy {
        if a == b {
            a.dupe()
        } else {
            ArcTy::new(Ty::union2(a.to_ty(), b.to_ty()))
        }
    }

    pub(crate) fn display_with<'a>(&'a self, config: &'a TypeRenderConfig) -> ArcTyDisplay<'a> {
        ArcTyDisplay { ty: self, config }
    }
}

impl Deref for ArcTy {
    type Target = Ty;

    fn deref(&self) -> &Self::Target {
        match &self.0 {
            ArcTyInner::Any => {
                static ANY: Ty = Ty::any();
                &ANY
            }
            ArcTyInner::Never => {
                static NEVER: Ty = Ty::never();
                &NEVER
            }
            ArcTyInner::Str => {
                static STR: Ty = Ty::string();
                &STR
            }
            ArcTyInner::Int => {
                static INT: Ty = Ty::int();
                &INT
            }
            ArcTyInner::Bool => {
                static BOOL: Ty = Ty::bool();
                &BOOL
            }
            ArcTyInner::None => {
                static NONE: Ty = Ty::none();
                &NONE
            }
            ArcTyInner::Arc(ty) => ty,
        }
    }
}

pub(crate) struct ArcTyDisplay<'a> {
    ty: &'a ArcTy,
    config: &'a TypeRenderConfig,
}

impl Display for ArcTyDisplay<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        self.ty.deref().fmt_with_config(f, self.config)
    }
}

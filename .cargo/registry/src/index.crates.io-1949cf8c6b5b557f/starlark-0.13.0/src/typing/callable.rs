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

use std::fmt::Display;
use std::sync::OnceLock;

use allocative::Allocative;
use dupe::Dupe;
use starlark_syntax::codemap::Span;

use crate::typing::call_args::TyCallArgs;
use crate::typing::error::TypingOrInternalError;
use crate::typing::ty::TypeRenderConfig;
use crate::typing::ParamSpec;
use crate::typing::Ty;
use crate::typing::TypingOracleCtx;
use crate::util::arc_or_static::ArcOrStatic;

#[derive(Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Allocative)]
struct TyCallableInner {
    params: ParamSpec,
    result: Ty,
}

/// `typing.Callable`.
#[derive(Debug, Dupe, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Allocative)]
pub struct TyCallable {
    inner: ArcOrStatic<TyCallableInner>,
}

impl TyCallable {
    /// Create a new callable type.
    pub fn new(params: ParamSpec, result: Ty) -> TyCallable {
        TyCallable {
            inner: ArcOrStatic::new(TyCallableInner { params, result }),
        }
    }

    pub(crate) fn validate_call(
        &self,
        span: Span,
        args: &TyCallArgs,
        oracle: TypingOracleCtx,
    ) -> Result<Ty, TypingOrInternalError> {
        oracle.validate_fn_call(span, self, args)
    }

    pub(crate) fn params(&self) -> &ParamSpec {
        &self.inner.params
    }

    pub(crate) fn result(&self) -> &Ty {
        &self.inner.result
    }

    pub(crate) fn any() -> TyCallable {
        static INNER: OnceLock<TyCallableInner> = OnceLock::new();
        TyCallable {
            inner: ArcOrStatic::new_static(INNER.get_or_init(|| TyCallableInner {
                params: ParamSpec::any(),
                result: Ty::any(),
            })),
        }
    }

    pub(crate) fn fmt_with_config(
        &self,
        f: &mut std::fmt::Formatter<'_>,
        config: &TypeRenderConfig,
    ) -> std::fmt::Result {
        if self.params() == &ParamSpec::any() && self.result() == &Ty::any() {
            write!(f, "typing.Callable")?;
        } else {
            write!(f, "typing.Callable[")?;
            if self.params().is_any() {
                write!(f, "...")?;
            } else if let Some(pos) = self.params().all_required_pos_only() {
                write!(f, "[")?;
                for (i, p) in pos.iter().enumerate() {
                    if i != 0 {
                        write!(f, ", ")?;
                    }
                    p.fmt_with_config(f, config)?;
                }
                write!(f, "]")?;
            } else {
                write!(f, "\"{}\"", self.params().display_with(config))?;
            }
            write!(f, ", {}]", self.result().display_with(config))?;
        }
        Ok(())
    }
}

impl Display for TyCallable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.fmt_with_config(f, &TypeRenderConfig::Default)
    }
}

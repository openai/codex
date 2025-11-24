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
use std::sync::Arc;

use allocative::Allocative;
use dupe::Dupe;
use starlark_map::sorted_map::SortedMap;

use crate::typing::custom::TyCustomImpl;
use crate::typing::error::TypingNoContextError;
use crate::typing::error::TypingNoContextOrInternalError;
use crate::typing::Ty;
use crate::typing::TyBasic;
use crate::typing::TypingBinOp;
use crate::typing::TypingOracleCtx;
use crate::util::arc_str::ArcStr;
use crate::values::structs::StructRef;
use crate::values::typing::type_compiled::alloc::TypeMatcherAlloc;
use crate::values::typing::type_compiled::matcher::TypeMatcher;
use crate::values::Value;

/// Struct type.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Allocative)]
pub struct TyStruct {
    /// The fields that are definitely present in the struct, with their types.
    pub(crate) fields: SortedMap<ArcStr, Ty>,
    /// [`true`] if there might be additional fields not captured above,
    /// [`false`] if this struct has no extra members.
    pub(crate) extra: bool,
}

impl TyStruct {
    /// Any struct.
    pub fn any() -> TyStruct {
        TyStruct {
            fields: SortedMap::new(),
            extra: true,
        }
    }
}

impl TyCustomImpl for TyStruct {
    fn as_name(&self) -> Option<&str> {
        Some("struct")
    }

    fn bin_op(
        &self,
        bin_op: TypingBinOp,
        rhs: &TyBasic,
        ctx: &TypingOracleCtx,
    ) -> Result<Ty, TypingNoContextOrInternalError> {
        match bin_op {
            TypingBinOp::Less => {
                // TODO(nga): do not clone.
                if ctx.intersects_basic(&TyBasic::custom(self.clone()), rhs)? {
                    Ok(Ty::bool())
                } else {
                    Err(TypingNoContextOrInternalError::Typing)
                }
            }
            _ => Err(TypingNoContextOrInternalError::Typing),
        }
    }

    fn attribute(&self, attr: &str) -> Result<Ty, TypingNoContextError> {
        match self.fields.get(attr) {
            Some(ty) => Ok(ty.clone()),
            None if self.extra => Ok(Ty::any()),
            _ => Err(TypingNoContextError),
        }
    }

    fn union2(a: Arc<Self>, b: Arc<Self>) -> Result<Arc<Self>, (Arc<Self>, Arc<Self>)> {
        if a == b {
            // Fast path.
            Ok(a)
        } else if a.extra == b.extra && itertools::equal(a.fields.keys(), b.fields.keys()) {
            let mut fields = Vec::new();
            for ((a_k, a_v), (b_k, b_v)) in a.fields.iter().zip(&b.fields) {
                assert_eq!(a_k, b_k);
                fields.push((a_k.dupe(), Ty::union2(a_v.clone(), b_v.clone())));
            }
            Ok(Arc::new(TyStruct {
                fields: SortedMap::from_iter(fields),
                extra: a.extra,
            }))
        } else {
            Err((a, b))
        }
    }

    fn matcher<T: TypeMatcherAlloc>(&self, factory: T) -> T::Result {
        #[derive(Allocative, Eq, PartialEq, Hash, Debug, Clone, Copy, Dupe)]
        struct StructMatcher;

        impl TypeMatcher for StructMatcher {
            fn matches(&self, value: Value) -> bool {
                StructRef::is_instance(value)
            }
        }

        factory.alloc(StructMatcher)
    }
}

impl Display for TyStruct {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let TyStruct { fields, extra } = self;
        display_container::fmt_container(
            f,
            "struct(",
            ")",
            display_container::iter_display_chain(
                fields.iter().map(|(k, v)| format!("{} = {}", k, v)),
                extra.then_some(".."),
            ),
        )
    }
}

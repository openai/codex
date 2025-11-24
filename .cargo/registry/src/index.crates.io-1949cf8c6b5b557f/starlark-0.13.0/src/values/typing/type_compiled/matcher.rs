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

use std::fmt::Debug;

use allocative::Allocative;

use crate::typing::custom::TyCustom;
use crate::values::typing::type_compiled::alloc::TypeMatcherAlloc;
use crate::values::typing::type_compiled::type_matcher_factory::TypeMatcherFactory;
use crate::values::Value;

/// Runtime type matcher. E.g. when `isinstance(1, int)` is called,
/// implementation of `TypeMatcher` for `int` is used.
pub trait TypeMatcher: Allocative + Debug + Clone + Sized + Send + Sync + 'static {
    /// Check if the value matches the type.
    fn matches(&self, value: Value) -> bool;
    /// True if this matcher matches any value.
    fn is_wildcard(&self) -> bool {
        false
    }
}

pub(crate) trait TypeMatcherDyn: Debug + Allocative + Send + Sync + 'static {
    fn matches_dyn(&self, value: Value) -> bool;
    fn is_wildcard_dyn(&self) -> bool;

    fn to_box(&self) -> TypeMatcherBox;
}

impl<T: TypeMatcher> TypeMatcherDyn for T {
    fn matches_dyn(&self, value: Value) -> bool {
        TypeMatcher::matches(self, value)
    }

    fn is_wildcard_dyn(&self) -> bool {
        TypeMatcher::is_wildcard(self)
    }

    fn to_box(&self) -> TypeMatcherBox {
        TypeMatcherBox::new(self.clone())
    }
}

#[derive(Debug, Allocative)]
pub(crate) struct TypeMatcherBox(pub(crate) Box<dyn TypeMatcherDyn>);

impl TypeMatcherBox {
    pub(crate) fn new<T: TypeMatcher>(matcher: T) -> TypeMatcherBox {
        TypeMatcherBox(Box::new(matcher))
    }
}

impl Clone for TypeMatcherBox {
    fn clone(&self) -> Self {
        self.0.to_box()
    }
}

impl TypeMatcher for TypeMatcherBox {
    fn matches(&self, value: Value) -> bool {
        self.0.matches_dyn(value)
    }

    fn is_wildcard(&self) -> bool {
        self.0.is_wildcard_dyn()
    }
}

/// Type allocator which allocates `TypeMatcher` into `TypeMatcherBox`.
pub(crate) struct TypeMatcherBoxAlloc;

impl TypeMatcherAlloc for TypeMatcherBoxAlloc {
    type Result = TypeMatcherBox;

    fn alloc<T: TypeMatcher>(self, matcher: T) -> Self::Result {
        TypeMatcherBox::new(matcher)
    }

    fn custom(self, custom: &TyCustom) -> Self::Result {
        custom.matcher_with_box()
    }

    fn from_type_matcher_factory(self, factory: &TypeMatcherFactory) -> Self::Result {
        factory.factory.matcher_box()
    }
}

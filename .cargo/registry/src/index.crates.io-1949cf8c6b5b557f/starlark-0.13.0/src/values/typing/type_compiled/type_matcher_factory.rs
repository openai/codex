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
use std::sync::Arc;

use allocative::Allocative;
use dupe::Dupe;

use crate::values::typing::type_compiled::alloc::TypeMatcherAlloc;
use crate::values::typing::type_compiled::compiled::TypeCompiled;
use crate::values::typing::type_compiled::factory::TypeCompiledFactory;
use crate::values::typing::type_compiled::matcher::TypeMatcher;
use crate::values::typing::type_compiled::matcher::TypeMatcherBox;
use crate::values::typing::type_compiled::matcher::TypeMatcherBoxAlloc;
use crate::values::Value;

#[derive(Allocative, Debug)]
struct TypeMatcherFactoryImpl<M: TypeMatcher> {
    matcher: M,
}

pub(crate) trait TypeMatcherFactoryDyn: Allocative + Debug + Send + Sync + 'static {
    fn matcher_box(&self) -> TypeMatcherBox;
    fn type_compiled<'v>(&self, factory: TypeCompiledFactory<'_, 'v>) -> TypeCompiled<Value<'v>>;
}

impl<M: TypeMatcher> TypeMatcherFactoryDyn for TypeMatcherFactoryImpl<M> {
    fn matcher_box(&self) -> TypeMatcherBox {
        TypeMatcherBoxAlloc.alloc(self.matcher.clone())
    }

    fn type_compiled<'v>(&self, factory: TypeCompiledFactory<'_, 'v>) -> TypeCompiled<Value<'v>> {
        factory.alloc(self.matcher.clone())
    }
}

/// Boxed `TypeMatcher`.
#[derive(Dupe, Clone, Allocative, Debug)]
pub struct TypeMatcherFactory {
    pub(crate) factory: Arc<dyn TypeMatcherFactoryDyn>,
}

impl TypeMatcherFactory {
    /// Create a new `TypeMatcherFactory` from a `TypeMatcher`.
    pub fn new(matcher: impl TypeMatcher) -> TypeMatcherFactory {
        TypeMatcherFactory {
            factory: Arc::new(TypeMatcherFactoryImpl { matcher }),
        }
    }
}

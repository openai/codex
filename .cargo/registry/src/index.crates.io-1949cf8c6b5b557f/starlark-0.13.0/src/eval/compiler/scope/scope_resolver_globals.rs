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

use crate::const_frozen_string;
use crate::environment::Globals;
use crate::values::FrozenRef;
use crate::values::FrozenValue;

pub(crate) struct ScopeResolverGlobals {
    /// None if unknown.
    pub(crate) globals: Option<FrozenRef<'static, Globals>>,
}

impl ScopeResolverGlobals {
    pub(crate) fn unknown() -> ScopeResolverGlobals {
        ScopeResolverGlobals { globals: None }
    }

    pub(crate) fn get_global(&self, name: &str) -> Option<FrozenValue> {
        match self.globals {
            Some(globals) => globals.get_frozen(name),
            None => Some(const_frozen_string!("unknown-global").to_frozen_value()),
        }
    }

    pub(crate) fn names(&self) -> Option<Vec<String>> {
        self.globals
            .map(|g| g.names().map(|s| s.as_str().to_owned()).collect())
    }
}

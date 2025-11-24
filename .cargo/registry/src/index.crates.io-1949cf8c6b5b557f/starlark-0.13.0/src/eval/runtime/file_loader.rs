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

//! Define variants of the evaluation function with different support
//! for the `load(...)` statement.

use std::collections::HashMap;

use dupe::Dupe;

use crate::environment::FrozenModule;

/// A trait for turning a `path` given by a `load()` statement into a [`FrozenModule`].
pub trait FileLoader {
    /// Open the file given by the load statement `path`.
    fn load(&self, path: &str) -> crate::Result<FrozenModule>;
}

/// [`FileLoader`] that looks up modules by name from a [`HashMap`].
///
/// A list of all load statements can be obtained through
/// [`AstModule::loads`](crate::syntax::AstModule::loads).
/// This struct will raise an error if any requested files are not available.
pub struct ReturnFileLoader<'a> {
    /// Map from module name (first argument to `load` statement) to the actual module.
    pub modules: &'a HashMap<&'a str, &'a FrozenModule>,
}

impl<'a> FileLoader for ReturnFileLoader<'a> {
    fn load(&self, path: &str) -> crate::Result<FrozenModule> {
        match self.modules.get(path) {
            Some(v) => Ok((*v).dupe()),
            None => Err(crate::Error::new_other(anyhow::anyhow!(
                "ReturnFileLoader does not know the module `{}`",
                path
            ))),
        }
    }
}

/// Same as [`ReturnFileLoader`], but does not require fighting the borrow checker.
#[cfg(test)]
pub(crate) struct ReturnOwnedFileLoader {
    pub(crate) modules: HashMap<String, FrozenModule>,
}

#[cfg(test)]
impl FileLoader for ReturnOwnedFileLoader {
    fn load(&self, path: &str) -> crate::Result<FrozenModule> {
        match self.modules.get(path) {
            Some(v) => Ok(v.dupe()),
            None => Err(crate::Error::new_other(anyhow::anyhow!(
                "ReturnOwnedFileLoader does not know the module `{}`",
                path
            ))),
        }
    }
}

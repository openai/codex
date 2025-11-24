/*
 * Copyright 2018 The Starlark in Rust Authors.
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

//! Types representing Starlark modules ([`Module`] and [`FrozenModule`]) and global variables ([`Globals`]).
//!
//! Global functions and values are stored in [`Globals`], which are typically
//! built using [`GlobalsBuilder`].
//! User executions store their values in a [`Module`], which have to be converted to a
//! [`FrozenModule`] using [`freeze`](Module::freeze) before they can be `load()`'d as a dependency.

mod globals;
mod methods;
mod module_dump;
mod modules;
pub(crate) mod names;
pub(crate) mod slots;

pub use globals::*;
pub use methods::*;
pub use modules::*;
use thiserror::Error;

#[derive(Debug, Error)]
enum EnvironmentError {
    /// Cannot import private symbol, i.e. underscore prefixed
    #[error("Cannot import private symbol `{0}`")]
    CannotImportPrivateSymbol(String),
    #[error("Module has no symbol `{0}`")]
    ModuleHasNoSymbol(String),
    #[error("Module has no symbol `{0}`, did you mean `{1}`?")]
    ModuleHasNoSymbolDidYouMean(String, String),
    #[error("Module symbol `{0}` is not exported")]
    ModuleSymbolIsNotExported(String),
}

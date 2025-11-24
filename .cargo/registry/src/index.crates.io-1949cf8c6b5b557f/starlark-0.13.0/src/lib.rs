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

//! A [Starlark interpreter in Rust](https://github.com/facebook/starlark-rust).
//! Starlark is a deterministic version of Python, with [a specification](https://github.com/bazelbuild/starlark/blob/master/spec.md),
//! used by (amongst others) the [Buck](https://buck.build) and [Bazel](https://bazel.build) build systems.
//!
//! To evaluate a simple file:
//!
//! ```
//! # fn run() -> starlark::Result<()> {
//! use starlark::environment::Globals;
//! use starlark::environment::Module;
//! use starlark::eval::Evaluator;
//! use starlark::syntax::AstModule;
//! use starlark::syntax::Dialect;
//! use starlark::values::Value;
//!
//! let content = r#"
//! def hello():
//!    return "hello"
//! hello() + " world!"
//! "#;
//!
//! // We first parse the content, giving a filename and the Starlark
//! // `Dialect` we'd like to use (we pick standard).
//! let ast: AstModule =
//!     AstModule::parse("hello_world.star", content.to_owned(), &Dialect::Standard)?;
//!
//! // We create a `Globals`, defining the standard library functions available.
//! // The `standard` function uses those defined in the Starlark specification.
//! let globals: Globals = Globals::standard();
//!
//! // We create a `Module`, which stores the global variables for our calculation.
//! let module: Module = Module::new();
//!
//! // We create an evaluator, which controls how evaluation occurs.
//! let mut eval: Evaluator = Evaluator::new(&module);
//!
//! // And finally we evaluate the code using the evaluator.
//! let res: Value = eval.eval_module(ast, &globals)?;
//! assert_eq!(res.unpack_str(), Some("hello world!"));
//! # Ok(())
//! # }
//! # fn main(){ run().unwrap(); }
//! ```
//!
//! From this example there are lots of ways to extend it, which we do so below.
//!
//! ## Call Rust functions from Starlark
//!
//! We want to define a function in Rust (that computes quadratics), and then call it from Starlark.
//! We define the function using the [`#[starlark_module]`](macro@starlark_module) attribute, and add it to
//! a [`Globals`](environment::Globals) object.
//!
//! ```
//! #[macro_use]
//! extern crate starlark;
//! # fn run() -> starlark::Result<()> {
//! use starlark::environment::{GlobalsBuilder, Module};
//! use starlark::eval::Evaluator;
//! use starlark::syntax::{AstModule, Dialect};
//! use starlark::values::Value;
//!
//! // This defines the function that is visible to Starlark
//! #[starlark_module]
//! fn starlark_quadratic(builder: &mut GlobalsBuilder) {
//!     fn quadratic(a: i32, b: i32, c: i32, x: i32) -> anyhow::Result<i32> {
//!         Ok(a * x * x + b * x + c)
//!     }
//! }
//!
//! // We build our globals to make the function available in Starlark
//! let globals = GlobalsBuilder::new().with(starlark_quadratic).build();
//! let module = Module::new();
//! let mut eval = Evaluator::new(&module);
//!
//! // Let's test calling the function from Starlark code
//! let starlark_code = r#"
//! quadratic(4, 2, 1, x = 8)
//! "#;
//!
//! let ast = AstModule::parse("quadratic.star", starlark_code.to_owned(), &Dialect::Standard)?;
//! let res = eval.eval_module(ast, &globals)?;
//! assert_eq!(res.unpack_i32(), Some(273)); // Verify that we got an `int` return value of 4 * 8^2 + 2 * 8 + 1 = 273
//! # Ok(())
//! # }
//! # fn main(){ run().unwrap(); }
//! ```
//!
//! ## Collect Starlark values
//!
//! If we want to use Starlark as an enhanced JSON, we can define an `emit` function
//! to "write out" a JSON value, and use the [`Evaluator.extra`](eval::Evaluator::extra) field to store it.
//!
//! ```
//! #[macro_use]
//! extern crate starlark;
//! # fn run() -> starlark::Result<()> {
//! use std::cell::RefCell;
//!
//! use starlark::any::ProvidesStaticType;
//! use starlark::environment::GlobalsBuilder;
//! use starlark::environment::Module;
//! use starlark::eval::Evaluator;
//! use starlark::syntax::AstModule;
//! use starlark::syntax::Dialect;
//! use starlark::values::none::NoneType;
//! use starlark::values::Value;
//! use starlark::values::ValueLike;
//!
//! let content = r#"
//! emit(1)
//! emit(["test"])
//! emit({"x": "y"})
//! "#;
//!
//! // Define a store in which to accumulate JSON strings
//! #[derive(Debug, ProvidesStaticType, Default)]
//! struct Store(RefCell<Vec<String>>);
//!
//! impl Store {
//!     fn add(&self, x: String) {
//!         self.0.borrow_mut().push(x)
//!     }
//! }
//!
//! #[starlark_module]
//! fn starlark_emit(builder: &mut GlobalsBuilder) {
//!     fn emit(x: Value, eval: &mut Evaluator) -> anyhow::Result<NoneType> {
//!         // We modify extra (which we know is a Store) and add the JSON of the
//!         // value the user gave.
//!         eval.extra
//!             .unwrap()
//!             .downcast_ref::<Store>()
//!             .unwrap()
//!             .add(x.to_json()?);
//!         Ok(NoneType)
//!     }
//! }
//!
//! let ast = AstModule::parse("json.star", content.to_owned(), &Dialect::Standard)?;
//! // We build our globals adding some functions we wrote
//! let globals = GlobalsBuilder::new().with(starlark_emit).build();
//! let module = Module::new();
//! let store = Store::default();
//! {
//!     let mut eval = Evaluator::new(&module);
//!     // We add a reference to our store
//!     eval.extra = Some(&store);
//!     eval.eval_module(ast, &globals)?;
//! }
//! assert_eq!(&*store.0.borrow(), &["1", "[\"test\"]", "{\"x\":\"y\"}"]);
//! # Ok(())
//! # }
//! # fn main(){ run().unwrap(); }
//! ```
//!
//! ## Enable Starlark extensions (e.g. types)
//!
//! Our Starlark supports a number of extensions, including type annotations, which are
//! controlled by the [`Dialect`](syntax::Dialect) type.
//!
//! ```
//! # fn run() -> starlark::Result<()> {
//! use starlark::environment::Globals;
//! use starlark::environment::Module;
//! use starlark::eval::Evaluator;
//! use starlark::syntax::AstModule;
//! use starlark::syntax::Dialect;
//! use starlark::syntax::DialectTypes;
//!
//! let content = r#"
//! def takes_int(x: int):
//!     pass
//! takes_int("test")
//! "#;
//!
//! // Make the dialect enable types
//! let dialect = Dialect {
//!     enable_types: DialectTypes::Enable,
//!     ..Dialect::Standard
//! };
//! // We could equally have done `dialect = Dialect::Extended`.
//! let ast = AstModule::parse("json.star", content.to_owned(), &dialect)?;
//! let globals = Globals::standard();
//! let module = Module::new();
//! let mut eval = Evaluator::new(&module);
//! let res = eval.eval_module(ast, &globals);
//! // We expect this to fail, since it is a type violation
//! assert!(
//!     res.unwrap_err()
//!         .to_string()
//!         .contains("Value `test` of type `string` does not match the type annotation `int`")
//! );
//! # Ok(())
//! # }
//! # fn main(){ run().unwrap(); }
//! ```
//!
//! ## Enable the `load` statement
//!
//! You can have Starlark load files imported by the user.
//! That requires that the loaded modules are first frozen with [`Module.freeze`](environment::Module::freeze).
//! There is no requirement that the files are on disk, but that would be a common pattern.
//!
//! ```
//! # fn run() -> starlark::Result<()> {
//! use starlark::environment::FrozenModule;
//! use starlark::environment::Globals;
//! use starlark::environment::Module;
//! use starlark::eval::Evaluator;
//! use starlark::eval::ReturnFileLoader;
//! use starlark::syntax::AstModule;
//! use starlark::syntax::Dialect;
//!
//! // Get the file contents (for the demo), in reality use `AstModule::parse_file`.
//! fn get_source(file: &str) -> &str {
//!     match file {
//!         "a.star" => "a = 7",
//!         "b.star" => "b = 6",
//!         _ => {
//!             r#"
//! load('a.star', 'a')
//! load('b.star', 'b')
//! ab = a * b
//! "#
//!         }
//!     }
//! }
//!
//! fn get_module(file: &str) -> starlark::Result<FrozenModule> {
//!     let ast = AstModule::parse(file, get_source(file).to_owned(), &Dialect::Standard)?;
//!
//!     // We can get the loaded modules from `ast.loads`.
//!     // And ultimately produce a `loader` capable of giving those modules to Starlark.
//!     let mut loads = Vec::new();
//!     for load in ast.loads() {
//!         loads.push((load.module_id.to_owned(), get_module(load.module_id)?));
//!     }
//!     let modules = loads.iter().map(|(a, b)| (a.as_str(), b)).collect();
//!     let mut loader = ReturnFileLoader { modules: &modules };
//!
//!     let globals = Globals::standard();
//!     let module = Module::new();
//!     {
//!         let mut eval = Evaluator::new(&module);
//!         eval.set_loader(&mut loader);
//!         eval.eval_module(ast, &globals)?;
//!     }
//!     // After creating a module we freeze it, preventing further mutation.
//!     // It can now be used as the input for other Starlark modules.
//!     Ok(module.freeze()?)
//! }
//!
//! let ab = get_module("ab.star")?;
//! assert_eq!(ab.get("ab").unwrap().unpack_i32(), Some(42));
//! # Ok(())
//! # }
//! # fn main(){ run().unwrap(); }
//! ```
//!
//! ## Call a Starlark function from Rust
//!
//! You can extract functions from Starlark, and call them from Rust, using [`eval_function`](eval::Evaluator::eval_function).
//!
//! ```
//! # fn run() -> starlark::Result<()> {
//! use starlark::environment::Globals;
//! use starlark::environment::Module;
//! use starlark::eval::Evaluator;
//! use starlark::syntax::AstModule;
//! use starlark::syntax::Dialect;
//! use starlark::values::Value;
//!
//! let content = r#"
//! def quadratic(a, b, c, x):
//!     return a*x*x + b*x + c
//! quadratic
//! "#;
//!
//! let ast = AstModule::parse("quadratic.star", content.to_owned(), &Dialect::Standard)?;
//! let globals = Globals::standard();
//! let module = Module::new();
//! let mut eval = Evaluator::new(&module);
//! let quad = eval.eval_module(ast, &globals)?;
//! let heap = module.heap();
//! let res = eval.eval_function(
//!     quad,
//!     &[heap.alloc(4), heap.alloc(2), heap.alloc(1)],
//!     &[("x", heap.alloc(8))],
//! )?;
//! assert_eq!(res.unpack_i32(), Some(273));
//! # Ok(())
//! # }
//! # fn main(){ run().unwrap(); }
//! ```
//!
//! ## Defining Rust objects that are used from Starlark
//!
//! Finally, we can define our own types in Rust which live in the Starlark heap.
//! Such types are relatively complex, see the details at [`StarlarkValue`](values::StarlarkValue).
//!
//! ```
//! # fn run() -> starlark::Result<()> {
//! use std::fmt::Display;
//! use std::fmt::Write;
//! use std::fmt::{self};
//!
//! use allocative::Allocative;
//! use starlark::environment::Globals;
//! use starlark::environment::Module;
//! use starlark::eval::Evaluator;
//! use starlark::starlark_simple_value;
//! use starlark::syntax::AstModule;
//! use starlark::syntax::Dialect;
//! use starlark::values::Heap;
//! use starlark::values::NoSerialize;
//! use starlark::values::ProvidesStaticType;
//! use starlark::values::StarlarkValue;
//! use starlark::values::Value;
//! use starlark::values::ValueError;
//! use starlark::values::ValueLike;
//! use starlark_derive::starlark_value;
//!
//! // Define complex numbers
//! #[derive(Debug, PartialEq, Eq, ProvidesStaticType, NoSerialize, Allocative)]
//! struct Complex {
//!     real: i32,
//!     imaginary: i32,
//! }
//! starlark_simple_value!(Complex);
//!
//! impl Display for Complex {
//!     fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
//!         write!(f, "{} + {}i", self.real, self.imaginary)
//!     }
//! }
//!
//! #[starlark_value(type = "complex")]
//! impl<'v> StarlarkValue<'v> for Complex {
//!     // How we add them
//!     fn add(&self, rhs: Value<'v>, heap: &'v Heap) -> Option<starlark::Result<Value<'v>>> {
//!         if let Some(rhs) = rhs.downcast_ref::<Self>() {
//!             Some(Ok(heap.alloc(Complex {
//!                 real: self.real + rhs.real,
//!                 imaginary: self.imaginary + rhs.imaginary,
//!             })))
//!         } else {
//!             None
//!         }
//!     }
//! }
//!
//! let content = "str(a + b)";
//!
//! let ast = AstModule::parse("complex.star", content.to_owned(), &Dialect::Standard)?;
//! let globals = Globals::standard();
//! let module = Module::new();
//! // We inject some complex numbers into the module before we start.
//! let a = module.heap().alloc(Complex {
//!     real: 1,
//!     imaginary: 8,
//! });
//! module.set("a", a);
//! let b = module.heap().alloc(Complex {
//!     real: 4,
//!     imaginary: 2,
//! });
//! module.set("b", b);
//! let mut eval = Evaluator::new(&module);
//! let res = eval.eval_module(ast, &globals)?;
//! assert_eq!(res.unpack_str(), Some("5 + 10i"));
//! # Ok(())
//! # }
//! # fn main(){ run().unwrap(); }
//! ```

// Features we use
#![allow(stable_features)]
#![allow(unknown_lints)] // for clippy::tuple_array_conversions
#![cfg_attr(rust_nightly, allow(internal_features))]
#![cfg_attr(rust_nightly, feature(const_type_id))]
#![cfg_attr(rust_nightly, feature(core_intrinsics))]
#![cfg_attr(rust_nightly, feature(cfg_sanitize))]
#![cfg_attr(rust_nightly, feature(const_type_name))]
// Good reasons
#![allow(clippy::needless_return)] // Mixing explicit returns with implicit ones sometimes looks odd
#![allow(clippy::new_ret_no_self)] // We often return Value, even though its morally a Self
// Disagree these are good hints
#![allow(clippy::comparison_chain)]
#![allow(clippy::comparison_to_empty)]
#![allow(clippy::enum_variant_names)]
#![allow(clippy::float_cmp)]
#![allow(clippy::if_same_then_else)]
#![allow(clippy::len_without_is_empty)]
#![allow(clippy::manual_map)]
#![allow(clippy::manual_range_contains)]
#![allow(clippy::match_like_matches_macro)]
#![allow(clippy::missing_safety_doc)]
#![allow(clippy::needless_lifetimes)]
#![allow(clippy::new_without_default)]
#![allow(clippy::should_implement_trait)]
#![allow(clippy::single_match)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::trivially_copy_pass_by_ref)]
#![allow(clippy::tuple_array_conversions)]
#![allow(clippy::type_complexity)]
#![allow(clippy::wrong_self_convention)]
// FIXME: Temporary
#![allow(clippy::useless_transmute)] // Seems to be a clippy bug, but we should be using less transmute anyway
#![allow(clippy::zero_repeat_side_effects)]
#![deny(missing_docs)]

mod macros;

pub use starlark_derive::starlark_module;
pub use starlark_syntax::codemap;
pub use starlark_syntax::Error;
pub use starlark_syntax::ErrorKind;
pub use starlark_syntax::Result;
pub use starlark_syntax::StarlarkResultExt;
pub use stdlib::PrintHandler;

pub mod analysis;
pub mod any;
pub mod assert;
pub mod collections;
pub mod debug;
pub mod docs;
pub mod environment;
pub mod errors;
pub mod eval;
mod private;
pub mod read_line;
mod sealed;
pub mod syntax;
pub mod typing;

pub(crate) mod cast;
mod hint;
mod stdlib;
pub mod util;
pub mod values;
pub mod wasm;

pub mod coerce;
#[cfg(test)]
mod tests;

/// __macro_refs allows us to reference other crates in macro rules without users needing to be
///  aware of those dependencies. We make them public here and then can reference them like
///  `$crate::__macro_refs::foo`.
#[doc(hidden)]
pub mod __macro_refs {
    pub use either::Either;
    pub use paste::item;

    pub use crate::coerce::coerce;
}

pub mod __derive_refs;

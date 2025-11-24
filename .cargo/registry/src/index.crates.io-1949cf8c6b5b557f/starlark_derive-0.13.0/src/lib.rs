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

//! A proc-macro for writing functions in Rust that can be called from Starlark.

#[allow(unused_extern_crates)] // proc_macro is very special
extern crate proc_macro;

use proc_macro::TokenStream;

mod alloc_value;
mod any_lifetime;
mod attrs;
mod bc;
mod coerce;
mod freeze;
mod module;
mod serde;
mod starlark_type_repr;
mod starlark_value;
mod trace;
mod unpack_value;
mod util;
mod v_lifetime;
mod visit_span;
mod vtable;

/// Write Starlark modules concisely in Rust syntax.
///
/// For example:
///
/// ```ignore
/// #[starlark_module]
/// fn global(builder: &mut GlobalsBuilder) {
///     fn cc_binary(name: &str, srcs: Vec<&str>) -> String {
///         Ok(format!("{:?} {:?}", name, srcs))
///     }
/// }
/// ```
///
/// Parameters operate as named parameters of a given type. Each parameter will be unpacked with `UnpackValue`, unless:
///
/// * It is type `Option`, in which case it will be considered optional.
/// * It is a single argument of type `Arguments`, in which case all arguments will be passed together with minimal interpretation.
///
/// There are a number of attributes that can be add to each parameter by writing attributes before the
/// parameter name:
///
/// * `#[starlark(default = "a default")]` - provide a deafult for the parameter if it is omitted.
/// * `#[starlark(require = pos)]` - require the parameter to be passed by position, not named.
/// * `#[starlark(require = named)]` - require the parameter to be passed by name, not by position.
/// * `#[starlark(args)]` - treat the argument as `*args` in Starlark, receiving all additional positional arguments as a tuple.
/// * `#[starlark(kwargs)]` - treat the argument as `**kwargs` in Starlark, receiving all additional named arguments as a dictionary.
///
/// There are a number of attributes that can be applied to the entire function by writing attributes
/// before the `fn` of the function:
///
/// * `#[starlark(attribute_type = "foo")]` - if the function has `.type` applied, return this string. Usually used on
///   constructor functions so that `ctor.type` can be used in Starlark code.
/// * `#[starlark(return_type = "foo")]` - the return type of the function used for documention.
/// * `#[starlark(speculative_exec_safe)]` - the function
///   is considered safe to execute speculatively: the function should have
///   no global side effects, should not panic, and should finish in reasonable time.
///   The evaluator may invoke such functions early to generate more efficient code.
/// * `#[starlark(attribute)]` to turn the name into
///   an attribute on the value. Such a function must take exactly one argument, namely a value
///   of the type you have attached it to.
///
/// Multiple attributes can be specified either separately `#[starlark(require = named)] #[starlark(default = "")]` or
/// separated with a comman `#[starlark(require = named, default = "")]`.
///
/// There are two special arguments, distinguished by their type, which provides access to interpreter state:
///
/// * `heap: &'v Heap` gives access to the Starlark heap, for allocating things.
/// * `eval: &mut Evaluator<'v, '_, '_>` gives access to the Starlark evaluator, which can be used to look at interpreter state.
///
/// A module can be used to define globals (with `GlobalsBuilder`) or methods on an object (with `MethodsBuilder`).
/// In the case of methods, the first argument to each function will be the object itself, typically named `this`.
///
/// All these functions interoperate properly with `dir()`, `getattr()` and `hasattr()`.
///
/// If a desired function name is also a Rust keyword, use the `r#` prefix, e.g. `r#type`.
///
/// As a more complex example:
///
/// ```ignore
/// #[starlark_module]
/// fn methods(builder: &mut MethodsBuilder) {
///     fn r#enum<'v>(
///         this: Value<'v>,
///         #[starlark(require = named, default = 3)] index: i32,
///         heap: &'v Heap,
///     ) -> anyhow::Result<StringValue<'v>> {
///         Ok(heap.alloc_str(&format!("{this} {index}")))
///     }
/// }
/// ```
///
/// This defines a method such that when attached to an object `object.enum(index = 12)` will
/// return the string of the object and the index.
#[proc_macro_attribute]
pub fn starlark_module(attr: TokenStream, input: TokenStream) -> TokenStream {
    module::starlark_module(attr, input)
}

/// Stubs for Starlark bytecode interpreter.
#[proc_macro_attribute]
pub fn starlark_internal_bc(
    attr: proc_macro::TokenStream,
    input: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    bc::starlark_internal_bc(attr, input)
}

#[proc_macro_attribute]
pub fn starlark_internal_vtable(
    attr: proc_macro::TokenStream,
    input: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    vtable::starlark_internal_vtable(attr, input)
}

#[proc_macro_derive(VisitSpanMut)]
pub fn derive_visit_span_mut(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    visit_span::derive_visit_span_mut(input)
}

/// Derive the `Trace` trait.
#[proc_macro_derive(Trace, attributes(trace))]
pub fn derive_trace(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    trace::derive_trace(input)
}

/// Derive the `Freeze` trait.
#[proc_macro_derive(Freeze, attributes(freeze))]
pub fn derive_freeze(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    freeze::derive_freeze(input)
}

/// Derive the `NoSerialize` trait for serde.
#[proc_macro_derive(NoSerialize)]
pub fn derive_no_serialize(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    serde::derive_no_serialize(input)
}

/// Derive the `StarlarkTypeRepr` trait.
#[proc_macro_derive(StarlarkTypeRepr)]
pub fn derive_starlark_type_repr(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    starlark_type_repr::derive_starlark_type_repr(input)
}

/// Derive the `UnpackValue` trait.
#[proc_macro_derive(UnpackValue)]
pub fn derive_unpack_value(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    unpack_value::derive_unpack_value(input)
}

/// Derive the `AllocValue` trait.
#[proc_macro_derive(AllocValue)]
pub fn derive_alloc_value(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    alloc_value::derive_alloc_value(input)
}

/// Derive the `AllocFrozenValue` trait.
#[proc_macro_derive(AllocFrozenValue)]
pub fn derive_alloc_frozen_value(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    alloc_value::derive_alloc_frozen_value(input)
}

/// Derive accessor methods that are designed to be used from {has,get,dir}_attr
/// in an `impl StarlarkValue` block. All fields in the struct that are not
/// marked with #[starlark(skip)] are exported to Starlark code as attributes.
/// NOTE: Any usage must also call `starlark_attrs!()` in the impl block for
/// `StarlarkValue`, otherwise the generated attr methods will not be used.
#[proc_macro_derive(StarlarkAttrs, attributes(starlark))]
pub fn derive_starlark_attrs(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    attrs::derive_attrs(input)
}

/// Generate `{has,get,dir}_attr` in the `StarlarkValue` impl block that proxy
/// to the ones generated by `derive(StarlarkAttrs)`
#[proc_macro]
pub fn starlark_attrs(_: proc_macro::TokenStream) -> proc_macro::TokenStream {
    attrs::starlark_attrs()
}

/// Generate missing elements of `StarlarkValue` trait when this attribute
/// is applied to an impl block of `StarlarkValue`.
#[proc_macro_attribute]
pub fn starlark_value(
    attr: proc_macro::TokenStream,
    input: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    starlark_value::derive_starlark_value(attr, input)
}

/// Derive the `ProvidesStaticType` trait. Requires the type has no type arguments, no constant arguments,
/// and at most one lifetime argument.
#[proc_macro_derive(ProvidesStaticType)]
pub fn derive_provides_static_type(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    any_lifetime::derive_provides_static_type(input)
}

// Derive the `Coerce` trait.
#[proc_macro_derive(Coerce)]
pub fn derive_coerce(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    coerce::derive_coerce(input)
}

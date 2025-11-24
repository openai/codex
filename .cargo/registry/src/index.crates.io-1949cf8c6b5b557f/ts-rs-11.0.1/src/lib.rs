//! <h1 align="center" style="padding-top: 0; margin-top: 0;">
//! <img width="150px" src="https://raw.githubusercontent.com/Aleph-Alpha/ts-rs/main/logo.png" alt="logo">
//! <br/>
//! ts-rs
//! </h1>
//! <p align="center">
//! Generate typescript type declarations from rust types
//! </p>
//!
//! <div align="center">
//! <!-- Github Actions -->
//! <img src="https://img.shields.io/github/actions/workflow/status/Aleph-Alpha/ts-rs/test.yml?branch=main" alt="actions status" />
//! <a href="https://crates.io/crates/ts-rs">
//! <img src="https://img.shields.io/crates/v/ts-rs.svg?style=flat-square"
//! alt="Crates.io version" />
//! </a>
//! <a href="https://docs.rs/ts-rs">
//! <img src="https://img.shields.io/badge/docs-latest-blue.svg?style=flat-square"
//! alt="docs.rs docs" />
//! </a>
//! <a href="https://crates.io/crates/ts-rs">
//! <img src="https://img.shields.io/crates/d/ts-rs.svg?style=flat-square"
//! alt="Download" />
//! </a>
//! </div>
//!
//! ## Why?
//! When building a web application in rust, data structures have to be shared between backend and frontend.
//! Using this library, you can easily generate TypeScript bindings to your rust structs & enums so that you can keep your
//! types in one place.
//!
//! ts-rs might also come in handy when working with webassembly.
//!
//! ## How?
//! ts-rs exposes a single trait, `TS`. Using a derive macro, you can implement this interface for your types.
//! Then, you can use this trait to obtain the TypeScript bindings.
//! We recommend doing this in your tests.
//! [See the example](https://github.com/Aleph-Alpha/ts-rs/blob/main/example/src/lib.rs) and [the docs](https://docs.rs/ts-rs/latest/ts_rs/).
//!
//! ## Get started
//! ```toml
//! [dependencies]
//! ts-rs = "10.1"
//! ```
//!
//! ```rust
//! use ts_rs::TS;
//!
//! #[derive(TS)]
//! #[ts(export)]
//! struct User {
//!     user_id: i32,
//!     first_name: String,
//!     last_name: String,
//! }
//! ```
//!
//! When running `cargo test` or `cargo test export_bindings`, the TypeScript bindings will be exported to the file `bindings/User.ts`
//! and will contain the following code:
//!
//! ```ts
//! export type User = { user_id: number, first_name: string, last_name: string, };
//! ```
//!
//! ## Features
//! - generate type declarations from rust structs
//! - generate union declarations from rust enums
//! - inline types
//! - flatten structs/types
//! - generate necessary imports when exporting to multiple files
//! - serde compatibility
//! - generic types
//! - support for ESM imports
//!
//! ## cargo features
//! | **Feature**        | **Description**                                                                                                                                                                                           |
//! |:-------------------|-----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|
//! | serde-compat       | **Enabled by default** <br/>See the *"serde compatibility"* section below for more information.                                                                                                           |
//! | format             | Enables formatting of the generated TypeScript bindings. <br/>Currently, this unfortunately adds quite a few dependencies.                                                                                |
//! | no-serde-warnings  | By default, warnings are printed during build if unsupported serde attributes are encountered. <br/>Enabling this feature silences these warnings.                                                        |
//! | import-esm         | When enabled,`import` statements in the generated file will have the `.js` extension in the end of the path to conform to the ES Modules spec. <br/> Example: `import { MyStruct } from "./my_struct.js"` |
//! | serde-json-impl    | Implement `TS` for types from *serde_json*                                                                                                                                                                |
//! | chrono-impl        | Implement `TS` for types from *chrono*                                                                                                                                                                    |
//! | bigdecimal-impl    | Implement `TS` for types from *bigdecimal*                                                                                                                                                                |
//! | url-impl           | Implement `TS` for types from *url*                                                                                                                                                                       |
//! | uuid-impl          | Implement `TS` for types from *uuid*                                                                                                                                                                      |
//! | bson-uuid-impl     | Implement `TS` for *bson::oid::ObjectId* and *bson::uuid*                                                                                                                                                 |
//! | bytes-impl         | Implement `TS` for types from *bytes*                                                                                                                                                                     |
//! | indexmap-impl      | Implement `TS` for types from *indexmap*                                                                                                                                                                  |
//! | ordered-float-impl | Implement `TS` for types from *ordered_float*                                                                                                                                                             |
//! | heapless-impl      | Implement `TS` for types from *heapless*                                                                                                                                                                  |
//! | semver-impl        | Implement `TS` for types from *semver*                                                                                                                                                                    |
//! | smol_str-impl      | Implement `TS` for types from *smol_str*                                                                                                                                                                    |
//! | tokio-impl         | Implement `TS` for types from *tokio*                                                                                                                                                                    |
//!
//! <br/>
//!
//! If there's a type you're dealing with which doesn't implement `TS`, use either
//! `#[ts(as = "..")]` or `#[ts(type = "..")]`, or open a PR.
//!
//! ## `serde` compatability
//! With the `serde-compat` feature (enabled by default), serde attributes can be parsed for enums and structs.
//! Supported serde attributes:
//! - `rename`
//! - `rename-all`
//! - `rename-all-fields`
//! - `tag`
//! - `content`
//! - `untagged`
//! - `skip`
//! - `skip_serializing`
//! - `skip_serializing_if`
//! - `flatten`
//! - `default`
//!
//! Note: `skip_serializing` and `skip_serializing_if` only have an effect when used together with
//! `#[serde(default)]`.
//!
//! Note: `skip_deserializing` is ignored. If you wish to exclude a field
//! from the generated type, but cannot use `#[serde(skip)]`, use `#[ts(skip)]` instead.
//!
//! When ts-rs encounters an unsupported serde attribute, a warning is emitted, unless the feature `no-serde-warnings` is enabled.
//!
//! ## Contributing
//! Contributions are always welcome!
//! Feel free to open an issue, discuss using GitHub discussions or open a PR.
//! [See CONTRIBUTING.md](https://github.com/Aleph-Alpha/ts-rs/blob/main/CONTRIBUTING.md)
//!
//! ## MSRV
//! The Minimum Supported Rust Version for this crate is 1.78.0

use std::{
    any::TypeId,
    collections::{BTreeMap, BTreeSet, HashMap, HashSet},
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4, SocketAddrV6},
    num::{
        NonZeroI128, NonZeroI16, NonZeroI32, NonZeroI64, NonZeroI8, NonZeroIsize, NonZeroU128,
        NonZeroU16, NonZeroU32, NonZeroU64, NonZeroU8, NonZeroUsize,
    },
    ops::{Range, RangeInclusive},
    path::{Path, PathBuf},
};

pub use ts_rs_macros::TS;

pub use crate::export::ExportError;

#[cfg(feature = "chrono-impl")]
mod chrono;
mod export;
#[cfg(feature = "serde-json-impl")]
mod serde_json;
#[cfg(feature = "tokio-impl")]
mod tokio;

/// A type which can be represented in TypeScript.  
/// Most of the time, you'd want to derive this trait instead of implementing it manually.  
/// ts-rs comes with implementations for all primitives, most collections, tuples,
/// arrays and containers.
///
/// ### exporting
/// Because Rusts procedural macros are evaluated before other compilation steps, TypeScript
/// bindings __cannot__ be exported during compile time.
///
/// Bindings can be exported within a test, which ts-rs generates for you by adding `#[ts(export)]`
/// to a type you wish to export to a file.  
/// When `cargo test` is run, all types annotated with `#[ts(export)]` and all of their
/// dependencies will be written to `TS_RS_EXPORT_DIR`, or `./bindings` by default.
///
/// For each individual type, path and filename within the output directory can be changed using
/// `#[ts(export_to = "...")]`. By default, the filename will be derived from the name of the type.
///
/// If, for some reason, you need to do this during runtime or cannot use `#[ts(export)]`, bindings
/// can be exported manually:
///
/// | Function              | Includes Dependencies | To                 |
/// |-----------------------|-----------------------|--------------------|
/// | [`TS::export`]        | ❌                    | `TS_RS_EXPORT_DIR` |
/// | [`TS::export_all`]    | ✔️                    | `TS_RS_EXPORT_DIR` |
/// | [`TS::export_all_to`] | ✔️                    | _custom_           |
///
/// ### serde compatibility
/// By default, the feature `serde-compat` is enabled.
/// ts-rs then parses serde attributes and adjusts the generated typescript bindings accordingly.
/// Not all serde attributes are supported yet - if you use an unsupported attribute, you'll see a
/// warning.
///
/// ### container attributes
/// attributes applicable for both structs and enums
///
/// - **`#[ts(crate = "..")]`**
///   Generates code which references the module passed to it instead of defaulting to `::ts_rs`
///   This is useful for cases where you have to re-export the crate.
///
/// - **`#[ts(export)]`**  
///   Generates a test which will export the type, by default to `bindings/<name>.ts` when running
///   `cargo test`. The default base directory can be overridden with the `TS_RS_EXPORT_DIR` environment variable.
///   Adding the variable to a project's [config.toml](https://doc.rust-lang.org/cargo/reference/config.html#env) can
///   make it easier to manage.
///   ```toml
///   # <project-root>/.cargo/config.toml
///   [env]
///   TS_RS_EXPORT_DIR = { value = "<OVERRIDE_DIR>", relative = true }
///   ```
///   <br/>
///
/// - **`#[ts(export_to = "..")]`**  
///   Specifies where the type should be exported to. Defaults to `<name>.ts`.  
///   The path given to the `export_to` attribute is relative to the `TS_RS_EXPORT_DIR` environment variable,
///   or, if `TS_RS_EXPORT_DIR` is not set, to `./bindings`  
///   If the provided path ends in a trailing `/`, it is interpreted as a directory.  
///   This attribute also accepts arbitrary expressions.  
///   Note that you need to add the `export` attribute as well, in order to generate a test which exports the type.
///   <br/><br/>
///
/// - **`#[ts(as = "..")]`**  
///   Overrides the type used in Typescript, using the provided Rust type instead.  
///   This is useful when you have a custom serializer and deserializer and don't want to implement `TS` manually
///   <br/><br/>
///
/// - **`#[ts(type = "..")]`**  
///   Overrides the type used in TypeScript.  
///   This is useful when you have a custom serializer and deserializer and don't want to implement `TS` manually
///   <br/><br/>
///
/// - **`#[ts(rename = "..")]`**  
///   Sets the typescript name of the generated type.  
///   Also accepts expressions, e.g `#[ts(rename = module_path!().rsplit_once("::").unwrap().1)]`.
///   <br/><br/>
///
/// - **`#[ts(rename_all = "..")]`**  
///   Rename all fields/variants of the type.  
///   Valid values are `lowercase`, `UPPERCASE`, `camelCase`, `snake_case`, `PascalCase`, `SCREAMING_SNAKE_CASE`, "kebab-case" and "SCREAMING-KEBAB-CASE"
///   <br/><br/>
///
/// - **`#[ts(concrete(..)]`**  
///   Disables one ore more generic type parameters by specifying a concrete type for them.  
///   The resulting TypeScript definition will not be generic over these parameters and will use the
///   provided type instead.  
///   This is especially useful for generic types containing associated types. Since TypeScript does
///   not have an equivalent construct to associated types, we cannot generate a generic definition
///   for them. Using `#[ts(concrete(..)]`, we can however generate a non-generic definition.
///   Example:
///   ```
///   # use ts_rs::TS;
///   ##[derive(TS)]
///   ##[ts(concrete(I = std::vec::IntoIter<String>))]
///   struct SearchResult<I: Iterator>(Vec<I::Item>);
///   // will always generate `type SearchResult = Array<String>`.
///   ```
///   <br/><br/>
///
/// - **`#[ts(bound)]`**
///   Override the bounds generated on the `TS` implementation for this type. This is useful in
///   combination with `#[ts(concrete)]`, when the type's generic parameters aren't directly used
///   in a field or variant.
///
///   Example:
///   ```
///   # use ts_rs::TS;
///
///   trait Container {
///       type Value: TS;
///   }
///
///   struct MyContainer;
///
///   ##[derive(TS)]
///   struct MyValue;
///
///   impl Container for MyContainer {
///       type Value = MyValue;
///   }
///
///   ##[derive(TS)]
///   ##[ts(export, concrete(C = MyContainer))]
///   struct Inner<C: Container> {
///       value: C::Value,
///   }
///
///   ##[derive(TS)]
///   // Without `#[ts(bound)]`, `#[derive(TS)]` would generate an unnecessary
///   // `C: TS` bound
///   ##[ts(export, concrete(C = MyContainer), bound = "C::Value: TS")]
///   struct Outer<C: Container> {
///       inner: Inner<C>,
///   }
///   ```
///   <br/><br/>
///
/// ### struct attributes
/// - **`#[ts(tag = "..")]`**  
///   Include the structs name (or value of `#[ts(rename = "..")]`) as a field with the given key.
///   <br/><br/>
///
/// - **`#[ts(optional_fields)]`**  
///   Makes all `Option<T>` fields in a struct optional.
///   If `#[ts(optional_fields)]` is present, `t?: T` is generated for every `Option<T>` field of the struct.  
///   If `#[ts(optional_fields = nullable)]` is present, `t?: T | null` is generated for every `Option<T>` field of the struct.  
///   <br/><br/>
///
/// ### struct field attributes
///
/// - **`#[ts(type = "..")]`**  
///   Overrides the type used in TypeScript.  
///   This is useful when there's a type for which you cannot derive `TS`.
///   <br/><br/>
///
/// - **`#[ts(as = "..")]`**  
///   Overrides the type of the annotated field, using the provided Rust type instead.
///   This is useful when there's a type for which you cannot derive `TS`.  
///   `_` may be used to refer to the type of the field, e.g `#[ts(as = "Option<_>")]`.
///   <br/><br/>
///
/// - **`#[ts(rename = "..")]`**  
///   Renames this field. To rename all fields of a struct, see the container attribute `#[ts(rename_all = "..")]`.
///   <br/><br/>
///
/// - **`#[ts(inline)]`**  
///   Inlines the type of this field, replacing its name with its definition.
///   <br/><br/>
///
/// - **`#[ts(skip)]`**  
///   Skips this field, omitting it from the generated *TypeScript* type.
///   <br/><br/>
///
/// - **`#[ts(optional)]`**  
///   May be applied on a struct field of type `Option<T>`. By default, such a field would turn into `t: T | null`.  
///   If `#[ts(optional)]` is present, `t?: T` is generated instead.  
///   If `#[ts(optional = nullable)]` is present, `t?: T | null` is generated.  
///   `#[ts(optional = false)]` can override the behaviour for this field if `#[ts(optional_fields)]`
///   is present on the struct itself.
///   <br/><br/>
///
/// - **`#[ts(flatten)]`**  
///   Flatten this field, inlining all the keys of the field's type into its parent.
///   <br/><br/>
///   
/// ### enum attributes
///
/// - **`#[ts(tag = "..")]`**  
///   Changes the representation of the enum to store its tag in a separate field.  
///   See [the serde docs](https://serde.rs/enum-representations.html) for more information.
///   <br/><br/>
///
/// - **`#[ts(content = "..")]`**  
///   Changes the representation of the enum to store its content in a separate field.  
///   See [the serde docs](https://serde.rs/enum-representations.html) for more information.
///   <br/><br/>
///
/// - **`#[ts(untagged)]`**  
///   Changes the representation of the enum to not include its tag.  
///   See [the serde docs](https://serde.rs/enum-representations.html) for more information.
///   <br/><br/>
///
/// - **`#[ts(rename_all = "..")]`**  
///   Rename all variants of this enum.  
///   Valid values are `lowercase`, `UPPERCASE`, `camelCase`, `snake_case`, `PascalCase`, `SCREAMING_SNAKE_CASE`, "kebab-case" and "SCREAMING-KEBAB-CASE"
///   <br/><br/>
///
/// - **`#[ts(rename_all_fields = "..")]`**  
///   Renames the fields of all the struct variants of this enum. This is equivalent to using
///   `#[ts(rename_all = "..")]` on all of the enum's variants.
///   Valid values are `lowercase`, `UPPERCASE`, `camelCase`, `snake_case`, `PascalCase`, `SCREAMING_SNAKE_CASE`, "kebab-case" and "SCREAMING-KEBAB-CASE"
///   <br/><br/>
///  
/// ### enum variant attributes
///
/// - **`#[ts(rename = "..")]`**  
///   Renames this variant. To rename all variants of an enum, see the container attribute `#[ts(rename_all = "..")]`.  
///   This attribute also accepts expressions, e.g `#[ts(rename = module_path!().rsplit_once("::").unwrap().1)]`.
///   <br/><br/>
///
/// - **`#[ts(skip)]`**  
///   Skip this variant, omitting it from the generated *TypeScript* type.
///   <br/><br/>
///
/// - **`#[ts(untagged)]`**  
///   Changes this variant to be treated as if the enum was untagged, regardless of the enum's tag
///   and content attributes
///   <br/><br/>
///
/// - **`#[ts(rename_all = "..")]`**  
///   Renames all the fields of a struct variant.
///   Valid values are `lowercase`, `UPPERCASE`, `camelCase`, `snake_case`, `PascalCase`, `SCREAMING_SNAKE_CASE`, "kebab-case" and "SCREAMING-KEBAB-CASE"
///   <br/><br/>
pub trait TS {
    /// If this type does not have generic parameters, then `WithoutGenerics` should just be `Self`.
    /// If the type does have generic parameters, then all generic parameters must be replaced with
    /// a dummy type, e.g `ts_rs::Dummy` or `()`.
    /// The only requirement for these dummy types is that `EXPORT_TO` must be `None`.
    ///
    /// # Example:
    /// ```
    /// use ts_rs::TS;
    /// struct GenericType<A, B>(A, B);
    /// impl<A, B> TS for GenericType<A, B> {
    ///     type WithoutGenerics = GenericType<ts_rs::Dummy, ts_rs::Dummy>;
    ///     type OptionInnerType = Self;
    ///     // ...
    ///     # fn decl() -> String { todo!() }
    ///     # fn decl_concrete() -> String { todo!() }
    ///     # fn name() -> String { todo!() }
    ///     # fn inline() -> String { todo!() }
    ///     # fn inline_flattened() -> String { todo!() }
    /// }
    /// ```
    type WithoutGenerics: TS + ?Sized;

    /// If the implementing type is `std::option::Option<T>`, then this associated type is set to `T`.
    /// All other implementations of `TS` should set this type to `Self` instead.
    type OptionInnerType: ?Sized;

    #[doc(hidden)]
    const IS_OPTION: bool = false;

    /// JSDoc comment to describe this type in TypeScript - when `TS` is derived, docs are
    /// automatically read from your doc comments or `#[doc = ".."]` attributes
    fn docs() -> Option<String> {
        None
    }

    /// Identifier of this type, excluding generic parameters.
    fn ident() -> String {
        // by default, fall back to `TS::name()`.
        let name = <Self as crate::TS>::name();

        match name.find('<') {
            Some(i) => name[..i].to_owned(),
            None => name,
        }
    }

    /// Declaration of this type, e.g. `type User = { user_id: number, ... }`.
    /// This function will panic if the type has no declaration.
    ///
    /// If this type is generic, then all provided generic parameters will be swapped for
    /// placeholders, resulting in a generic typescript definition.
    /// Both `SomeType::<i32>::decl()` and `SomeType::<String>::decl()` will therefore result in
    /// the same TypeScript declaration `type SomeType<A> = ...`.
    fn decl() -> String;

    /// Declaration of this type using the supplied generic arguments.
    /// The resulting TypeScript definition will not be generic. For that, see `TS::decl()`.
    /// If this type is not generic, then this function is equivalent to `TS::decl()`.
    fn decl_concrete() -> String;

    /// Name of this type in TypeScript, including generic parameters
    fn name() -> String;

    /// Formats this types definition in TypeScript, e.g `{ user_id: number }`.
    /// This function will panic if the type cannot be inlined.
    fn inline() -> String;

    /// Flatten a type declaration.  
    /// This function will panic if the type cannot be flattened.
    fn inline_flattened() -> String;

    /// Iterates over all dependency of this type.
    fn visit_dependencies(_: &mut impl TypeVisitor)
    where
        Self: 'static,
    {
    }

    /// Iterates over all type parameters of this type.
    fn visit_generics(_: &mut impl TypeVisitor)
    where
        Self: 'static,
    {
    }

    /// Resolves all dependencies of this type recursively.
    fn dependencies() -> Vec<Dependency>
    where
        Self: 'static,
    {
        let mut deps: Vec<Dependency> = vec![];
        struct Visit<'a>(&'a mut Vec<Dependency>);
        impl TypeVisitor for Visit<'_> {
            fn visit<T: TS + 'static + ?Sized>(&mut self) {
                if let Some(dep) = Dependency::from_ty::<T>() {
                    self.0.push(dep);
                }
            }
        }
        <Self as crate::TS>::visit_dependencies(&mut Visit(&mut deps));

        deps
    }

    /// Manually export this type to the filesystem.
    /// To export this type together with all of its dependencies, use [`TS::export_all`].
    ///
    /// # Automatic Exporting
    /// Types annotated with `#[ts(export)]`, together with all of their dependencies, will be
    /// exported automatically whenever `cargo test` is run.  
    /// In that case, there is no need to manually call this function.
    ///
    /// # Target Directory
    /// The target directory to which the type will be exported may be changed by setting the
    /// `TS_RS_EXPORT_DIR` environment variable. By default, `./bindings` will be used.
    ///
    /// To specify a target directory manually, use [`TS::export_all_to`], which also exports all
    /// dependencies.
    ///
    /// To alter the filename or path of the type within the target directory,
    /// use `#[ts(export_to = "...")]`.
    fn export() -> Result<(), ExportError>
    where
        Self: 'static,
    {
        let path = <Self as crate::TS>::default_output_path()
            .ok_or_else(std::any::type_name::<Self>)
            .map_err(ExportError::CannotBeExported)?;

        export::export_to::<Self, _>(path)
    }

    /// Manually export this type to the filesystem, together with all of its dependencies.  
    /// To export only this type, without its dependencies, use [`TS::export`].
    ///
    /// # Automatic Exporting
    /// Types annotated with `#[ts(export)]`, together with all of their dependencies, will be
    /// exported automatically whenever `cargo test` is run.  
    /// In that case, there is no need to manually call this function.
    ///
    /// # Target Directory
    /// The target directory to which the types will be exported may be changed by setting the
    /// `TS_RS_EXPORT_DIR` environment variable. By default, `./bindings` will be used.
    ///
    /// To specify a target directory manually, use [`TS::export_all_to`].
    ///
    /// To alter the filenames or paths of the types within the target directory,
    /// use `#[ts(export_to = "...")]`.
    fn export_all() -> Result<(), ExportError>
    where
        Self: 'static,
    {
        export::export_all_into::<Self>(&*export::default_out_dir())
    }

    /// Manually export this type into the given directory, together with all of its dependencies.  
    /// To export only this type, without its dependencies, use [`TS::export`].
    ///
    /// Unlike [`TS::export_all`], this function disregards `TS_RS_EXPORT_DIR`, using the provided
    /// directory instead.
    ///
    /// To alter the filenames or paths of the types within the target directory,
    /// use `#[ts(export_to = "...")]`.
    ///
    /// # Automatic Exporting
    /// Types annotated with `#[ts(export)]`, together with all of their dependencies, will be
    /// exported automatically whenever `cargo test` is run.  
    /// In that case, there is no need to manually call this function.
    fn export_all_to(out_dir: impl AsRef<Path>) -> Result<(), ExportError>
    where
        Self: 'static,
    {
        export::export_all_into::<Self>(out_dir)
    }

    /// Manually generate bindings for this type, returning a [`String`].  
    /// This function does not format the output, even if the `format` feature is enabled.
    ///
    /// # Automatic Exporting
    /// Types annotated with `#[ts(export)]`, together with all of their dependencies, will be
    /// exported automatically whenever `cargo test` is run.  
    /// In that case, there is no need to manually call this function.
    fn export_to_string() -> Result<String, ExportError>
    where
        Self: 'static,
    {
        export::export_to_string::<Self>()
    }

    /// Returns the output path to where `T` should be exported.  
    /// The returned path does _not_ include the base directory from `TS_RS_EXPORT_DIR`.  
    ///
    /// To get the output path containing `TS_RS_EXPORT_DIR`, use [`TS::default_output_path`].
    ///
    /// When deriving `TS`, the output path can be altered using `#[ts(export_to = "...")]`.  
    /// See the documentation of [`TS`] for more details.
    ///
    /// The output of this function depends on the environment variable `TS_RS_EXPORT_DIR`, which is
    /// used as base directory. If it is not set, `./bindings` is used as default directory.
    ///
    /// If `T` cannot be exported (e.g because it's a primitive type), this function will return
    /// `None`.
    fn output_path() -> Option<PathBuf> {
        None
    }

    /// Returns the output path to where `T` should be exported.  
    ///
    /// The output of this function depends on the environment variable `TS_RS_EXPORT_DIR`, which is
    /// used as base directory. If it is not set, `./bindings` is used as default directory.
    ///
    /// To get the output path relative to `TS_RS_EXPORT_DIR` and without reading the environment
    /// variable, use [`TS::output_path`].
    ///
    /// When deriving `TS`, the output path can be altered using `#[ts(export_to = "...")]`.  
    /// See the documentation of [`TS`] for more details.
    ///
    /// If `T` cannot be exported (e.g because it's a primitive type), this function will return
    /// `None`.
    fn default_output_path() -> Option<PathBuf> {
        Some(export::default_out_dir().join(<Self as crate::TS>::output_path()?))
    }
}

/// A visitor used to iterate over all dependencies or generics of a type.
/// When an instance of [`TypeVisitor`] is passed to [`TS::visit_dependencies`] or
/// [`TS::visit_generics`], the [`TypeVisitor::visit`] method will be invoked for every dependency
/// or generic parameter respectively.
pub trait TypeVisitor: Sized {
    fn visit<T: TS + 'static + ?Sized>(&mut self);
}

/// A typescript type which is depended upon by other types.
/// This information is required for generating the correct import statements.
#[derive(Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct Dependency {
    /// Type ID of the rust type
    pub type_id: TypeId,
    /// Name of the type in TypeScript
    pub ts_name: String,
    /// Path to where the type would be exported. By default, a filename is derived from the types
    /// name, which can be customized with `#[ts(export_to = "..")]`.  
    /// This path does _not_ include a base directory.
    pub output_path: PathBuf,
}

impl Dependency {
    /// Constructs a [`Dependency`] from the given type `T`.
    /// If `T` is not exportable (meaning `T::EXPORT_TO` is `None`), this function will return
    /// `None`
    pub fn from_ty<T: TS + 'static + ?Sized>() -> Option<Self> {
        let output_path = <T as crate::TS>::output_path()?;
        Some(Dependency {
            type_id: TypeId::of::<T>(),
            ts_name: <T as crate::TS>::ident(),
            output_path,
        })
    }
}

#[doc(hidden)]
#[diagnostic::on_unimplemented(
    message = "`#[ts(optional)]` can only be used on fields of type `Option`",
    note = "`#[ts(optional)]` was used on a field of type {Self}, which is not permitted",
    label = "`#[ts(optional)]` is not allowed on field of type {Self}"
)]
pub trait IsOption {}

impl<T> IsOption for Option<T> {}

// generate impls for primitive types
macro_rules! impl_primitives {
    ($($($ty:ty),* => $l:literal),*) => { $($(
        impl TS for $ty {
            type WithoutGenerics = Self;
            type OptionInnerType = Self;
            fn name() -> String { $l.to_owned() }
            fn inline() -> String { <Self as $crate::TS>::name() }
            fn inline_flattened() -> String { panic!("{} cannot be flattened", <Self as $crate::TS>::name()) }
            fn decl() -> String { panic!("{} cannot be declared", <Self as $crate::TS>::name()) }
            fn decl_concrete() -> String { panic!("{} cannot be declared", <Self as $crate::TS>::name()) }
        }
    )*)* };
}
// generate impls for tuples
macro_rules! impl_tuples {
    ( impl $($i:ident),* ) => {
        impl<$($i: TS),*> TS for ($($i,)*) {
            type WithoutGenerics = (Dummy, );
            type OptionInnerType = Self;
            fn name() -> String {
                format!("[{}]", [$(<$i as $crate::TS>::name()),*].join(", "))
            }
            fn inline() -> String {
                panic!("tuple cannot be inlined!");
            }
            fn visit_generics(v: &mut impl TypeVisitor)
            where
                Self: 'static
            {
                $(
                    v.visit::<$i>();
                    <$i as $crate::TS>::visit_generics(v);
                )*
            }
            fn inline_flattened() -> String { panic!("tuple cannot be flattened") }
            fn decl() -> String { panic!("tuple cannot be declared") }
            fn decl_concrete() -> String { panic!("tuple cannot be declared") }
        }
    };
    ( $i2:ident $(, $i:ident)* ) => {
        impl_tuples!(impl $i2 $(, $i)* );
        impl_tuples!($($i),*);
    };
    () => {};
}

// generate impls for wrapper types
macro_rules! impl_wrapper {
    ($($t:tt)*) => {
        $($t)* {
            type WithoutGenerics = Self;
            type OptionInnerType = Self;
            fn name() -> String { <T as $crate::TS>::name() }
            fn inline() -> String { <T as $crate::TS>::inline() }
            fn inline_flattened() -> String { <T as $crate::TS>::inline_flattened() }
            fn visit_dependencies(v: &mut impl TypeVisitor)
            where
                Self: 'static,
            {
                <T as $crate::TS>::visit_dependencies(v);
            }

            fn visit_generics(v: &mut impl TypeVisitor)
            where
                Self: 'static,
            {
                <T as $crate::TS>::visit_generics(v);
                v.visit::<T>();
            }
            fn decl() -> String { panic!("wrapper type cannot be declared") }
            fn decl_concrete() -> String { panic!("wrapper type cannot be declared") }
        }
    };
}

// implement TS for the $shadow, deferring to the impl $s
macro_rules! impl_shadow {
    (as $s:ty: $($impl:tt)*) => {
        $($impl)* {
            type WithoutGenerics = <$s as $crate::TS>::WithoutGenerics;
            type OptionInnerType = <$s as $crate::TS>::OptionInnerType;
            fn ident() -> String { <$s as $crate::TS>::ident() }
            fn name() -> String { <$s as $crate::TS>::name() }
            fn inline() -> String { <$s as $crate::TS>::inline() }
            fn inline_flattened() -> String { <$s as $crate::TS>::inline_flattened() }
            fn visit_dependencies(v: &mut impl $crate::TypeVisitor)
            where
                Self: 'static,
            {
                <$s as $crate::TS>::visit_dependencies(v);
            }
            fn visit_generics(v: &mut impl $crate::TypeVisitor)
            where
                Self: 'static,
            {
                <$s as $crate::TS>::visit_generics(v);
            }
            fn decl() -> String { <$s as $crate::TS>::decl() }
            fn decl_concrete() -> String { <$s as $crate::TS>::decl_concrete() }
            fn output_path() -> Option<std::path::PathBuf> { <$s as $crate::TS>::output_path() }
        }
    };
}

impl<T: TS> TS for Option<T> {
    type WithoutGenerics = Self;
    type OptionInnerType = T;
    const IS_OPTION: bool = true;

    fn name() -> String {
        format!("{} | null", <T as crate::TS>::name())
    }

    fn inline() -> String {
        format!("{} | null", <T as crate::TS>::inline())
    }

    fn visit_dependencies(v: &mut impl TypeVisitor)
    where
        Self: 'static,
    {
        <T as crate::TS>::visit_dependencies(v);
    }

    fn visit_generics(v: &mut impl TypeVisitor)
    where
        Self: 'static,
    {
        <T as crate::TS>::visit_generics(v);
        v.visit::<T>();
    }

    fn decl() -> String {
        panic!("{} cannot be declared", <Self as crate::TS>::name())
    }

    fn decl_concrete() -> String {
        panic!("{} cannot be declared", <Self as crate::TS>::name())
    }

    fn inline_flattened() -> String {
        panic!("{} cannot be flattened", <Self as crate::TS>::name())
    }
}

impl<T: TS, E: TS> TS for Result<T, E> {
    type WithoutGenerics = Result<Dummy, Dummy>;
    type OptionInnerType = Self;

    fn name() -> String {
        format!(
            "{{ Ok : {} }} | {{ Err : {} }}",
            <T as crate::TS>::name(),
            <E as crate::TS>::name()
        )
    }

    fn inline() -> String {
        format!(
            "{{ Ok : {} }} | {{ Err : {} }}",
            <T as crate::TS>::inline(),
            <E as crate::TS>::inline()
        )
    }

    fn visit_dependencies(v: &mut impl TypeVisitor)
    where
        Self: 'static,
    {
        <T as crate::TS>::visit_dependencies(v);
        <E as crate::TS>::visit_dependencies(v);
    }

    fn visit_generics(v: &mut impl TypeVisitor)
    where
        Self: 'static,
    {
        <T as crate::TS>::visit_generics(v);
        v.visit::<T>();
        <E as crate::TS>::visit_generics(v);
        v.visit::<E>();
    }

    fn decl() -> String {
        panic!("{} cannot be declared", <Self as crate::TS>::name())
    }

    fn decl_concrete() -> String {
        panic!("{} cannot be declared", <Self as crate::TS>::name())
    }

    fn inline_flattened() -> String {
        panic!("{} cannot be flattened", <Self as crate::TS>::name())
    }
}

impl<T: TS> TS for Vec<T> {
    type WithoutGenerics = Vec<Dummy>;
    type OptionInnerType = Self;

    fn ident() -> String {
        "Array".to_owned()
    }

    fn name() -> String {
        format!("Array<{}>", <T as crate::TS>::name())
    }

    fn inline() -> String {
        format!("Array<{}>", <T as crate::TS>::inline())
    }

    fn visit_dependencies(v: &mut impl TypeVisitor)
    where
        Self: 'static,
    {
        <T as crate::TS>::visit_dependencies(v);
    }

    fn visit_generics(v: &mut impl TypeVisitor)
    where
        Self: 'static,
    {
        <T as crate::TS>::visit_generics(v);
        v.visit::<T>();
    }

    fn decl() -> String {
        panic!("{} cannot be declared", <Self as crate::TS>::name())
    }

    fn decl_concrete() -> String {
        panic!("{} cannot be declared", <Self as crate::TS>::name())
    }

    fn inline_flattened() -> String {
        panic!("{} cannot be flattened", <Self as crate::TS>::name())
    }
}

// Arrays longer than this limit will be emitted as Array<T>
const ARRAY_TUPLE_LIMIT: usize = 64;
impl<T: TS, const N: usize> TS for [T; N] {
    type WithoutGenerics = [Dummy; N];
    type OptionInnerType = Self;

    fn name() -> String {
        if N > ARRAY_TUPLE_LIMIT {
            return <Vec<T> as crate::TS>::name();
        }

        format!(
            "[{}]",
            (0..N)
                .map(|_| <T as crate::TS>::name())
                .collect::<Box<[_]>>()
                .join(", ")
        )
    }

    fn inline() -> String {
        if N > ARRAY_TUPLE_LIMIT {
            return <Vec<T> as crate::TS>::inline();
        }

        format!(
            "[{}]",
            (0..N)
                .map(|_| <T as crate::TS>::inline())
                .collect::<Box<[_]>>()
                .join(", ")
        )
    }

    fn visit_dependencies(v: &mut impl TypeVisitor)
    where
        Self: 'static,
    {
        <T as crate::TS>::visit_dependencies(v);
    }

    fn visit_generics(v: &mut impl TypeVisitor)
    where
        Self: 'static,
    {
        <T as crate::TS>::visit_generics(v);
        v.visit::<T>();
    }

    fn decl() -> String {
        panic!("{} cannot be declared", <Self as crate::TS>::name())
    }

    fn decl_concrete() -> String {
        panic!("{} cannot be declared", <Self as crate::TS>::name())
    }

    fn inline_flattened() -> String {
        panic!("{} cannot be flattened", <Self as crate::TS>::name())
    }
}

impl<K: TS, V: TS, H> TS for HashMap<K, V, H> {
    type WithoutGenerics = HashMap<Dummy, Dummy>;
    type OptionInnerType = Self;

    fn ident() -> String {
        panic!()
    }

    fn name() -> String {
        format!(
            "{{ [key in {}]?: {} }}",
            <K as crate::TS>::name(),
            <V as crate::TS>::name()
        )
    }

    fn inline() -> String {
        format!(
            "{{ [key in {}]?: {} }}",
            <K as crate::TS>::inline(),
            <V as crate::TS>::inline()
        )
    }

    fn visit_dependencies(v: &mut impl TypeVisitor)
    where
        Self: 'static,
    {
        <K as crate::TS>::visit_dependencies(v);
        <V as crate::TS>::visit_dependencies(v);
    }

    fn visit_generics(v: &mut impl TypeVisitor)
    where
        Self: 'static,
    {
        <K as crate::TS>::visit_generics(v);
        v.visit::<K>();
        <V as crate::TS>::visit_generics(v);
        v.visit::<V>();
    }

    fn decl() -> String {
        panic!("{} cannot be declared", <Self as crate::TS>::name())
    }

    fn decl_concrete() -> String {
        panic!("{} cannot be declared", <Self as crate::TS>::name())
    }

    fn inline_flattened() -> String {
        format!(
            "({{ [key in {}]?: {} }})",
            <K as crate::TS>::inline(),
            <V as crate::TS>::inline()
        )
    }
}

impl<I: TS> TS for Range<I> {
    type WithoutGenerics = Range<Dummy>;
    type OptionInnerType = Self;

    fn name() -> String {
        format!(
            "{{ start: {}, end: {}, }}",
            <I as crate::TS>::name(),
            <I as crate::TS>::name()
        )
    }

    fn visit_dependencies(v: &mut impl TypeVisitor)
    where
        Self: 'static,
    {
        <I as crate::TS>::visit_dependencies(v);
    }

    fn visit_generics(v: &mut impl TypeVisitor)
    where
        Self: 'static,
    {
        <I as crate::TS>::visit_generics(v);
        v.visit::<I>();
    }

    fn decl() -> String {
        panic!("{} cannot be declared", <Self as crate::TS>::name())
    }

    fn decl_concrete() -> String {
        panic!("{} cannot be declared", <Self as crate::TS>::name())
    }

    fn inline() -> String {
        panic!("{} cannot be inlined", <Self as crate::TS>::name())
    }

    fn inline_flattened() -> String {
        panic!("{} cannot be flattened", <Self as crate::TS>::name())
    }
}

impl_shadow!(as Range<I>: impl<I: TS> TS for RangeInclusive<I>);
impl_shadow!(as Vec<T>: impl<T: TS, H> TS for HashSet<T, H>);
impl_shadow!(as Vec<T>: impl<T: TS> TS for BTreeSet<T>);
impl_shadow!(as HashMap<K, V>: impl<K: TS, V: TS> TS for BTreeMap<K, V>);
impl_shadow!(as Vec<T>: impl<T: TS> TS for [T]);

impl_wrapper!(impl<T: TS + ?Sized> TS for &T);
impl_wrapper!(impl<T: TS + ?Sized> TS for Box<T>);
impl_wrapper!(impl<T: TS + ?Sized> TS for std::sync::Arc<T>);
impl_wrapper!(impl<T: TS + ?Sized> TS for std::rc::Rc<T>);
impl_wrapper!(impl<'a, T: TS + ToOwned + ?Sized> TS for std::borrow::Cow<'a, T>);
impl_wrapper!(impl<T: TS> TS for std::cell::Cell<T>);
impl_wrapper!(impl<T: TS> TS for std::cell::RefCell<T>);
impl_wrapper!(impl<T: TS> TS for std::sync::Mutex<T>);
impl_wrapper!(impl<T: TS> TS for std::sync::RwLock<T>);
impl_wrapper!(impl<T: TS + ?Sized> TS for std::sync::Weak<T>);
impl_wrapper!(impl<T: TS> TS for std::marker::PhantomData<T>);

impl_tuples!(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10);

#[cfg(feature = "bigdecimal-impl")]
impl_primitives! { bigdecimal::BigDecimal => "string" }

#[cfg(feature = "smol_str-impl")]
impl_primitives! { smol_str::SmolStr => "string" }

#[cfg(feature = "uuid-impl")]
impl_primitives! { uuid::Uuid => "string" }

#[cfg(feature = "url-impl")]
impl_primitives! { url::Url => "string" }

#[cfg(feature = "ordered-float-impl")]
impl_primitives! { ordered_float::OrderedFloat<f32> => "number" }

#[cfg(feature = "ordered-float-impl")]
impl_primitives! { ordered_float::OrderedFloat<f64> => "number" }

#[cfg(feature = "bson-uuid-impl")]
impl_primitives! { bson::oid::ObjectId => "string" }

#[cfg(feature = "bson-uuid-impl")]
impl_primitives! { bson::Uuid => "string" }

#[cfg(feature = "indexmap-impl")]
impl_shadow!(as Vec<T>: impl<T: TS> TS for indexmap::IndexSet<T>);

#[cfg(feature = "indexmap-impl")]
impl_shadow!(as HashMap<K, V>: impl<K: TS, V: TS> TS for indexmap::IndexMap<K, V>);

#[cfg(feature = "heapless-impl")]
impl_shadow!(as Vec<T>: impl<T: TS, const N: usize> TS for heapless::Vec<T, N>);

#[cfg(feature = "semver-impl")]
impl_primitives! { semver::Version => "string" }

#[cfg(feature = "bytes-impl")]
mod bytes {
    use super::TS;

    impl_shadow!(as Vec<u8>: impl TS for bytes::Bytes);
    impl_shadow!(as Vec<u8>: impl TS for bytes::BytesMut);
}

impl_primitives! {
    u8, i8, NonZeroU8, NonZeroI8,
    u16, i16, NonZeroU16, NonZeroI16,
    u32, i32, NonZeroU32, NonZeroI32,
    usize, isize, NonZeroUsize, NonZeroIsize, f32, f64 => "number",
    u64, i64, NonZeroU64, NonZeroI64,
    u128, i128, NonZeroU128, NonZeroI128 => "bigint",
    bool => "boolean",
    char, Path, PathBuf, String, str,
    Ipv4Addr, Ipv6Addr, IpAddr, SocketAddrV4, SocketAddrV6, SocketAddr => "string",
    () => "null"
}

#[rustfmt::skip]
pub(crate) use impl_primitives;
#[rustfmt::skip]
pub(crate) use impl_shadow;
#[rustfmt::skip]
pub(crate) use impl_wrapper;

#[doc(hidden)]
#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq, Ord, PartialOrd)]
pub struct Dummy;

impl std::fmt::Display for Dummy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl TS for Dummy {
    type WithoutGenerics = Self;
    type OptionInnerType = Self;

    fn name() -> String {
        "Dummy".to_owned()
    }

    fn decl() -> String {
        panic!("{} cannot be declared", <Self as crate::TS>::name())
    }

    fn decl_concrete() -> String {
        panic!("{} cannot be declared", <Self as crate::TS>::name())
    }

    fn inline() -> String {
        panic!("{} cannot be inlined", <Self as crate::TS>::name())
    }

    fn inline_flattened() -> String {
        panic!("{} cannot be flattened", <Self as crate::TS>::name())
    }
}

/// Formats rust doc comments, turning them into a JSDoc comments.
/// Expects a `&[&str]` where each element corresponds to the value of one `#[doc]` attribute.
/// This work is deferred to runtime, allowing expressions in `#[doc]`, e.g `#[doc = file!()]`.
#[doc(hidden)]
pub fn format_docs(docs: &[&str]) -> String {
    match docs {
        // No docs
        [] => String::new(),

        // Multi-line block doc comment (/** ... */)
        [doc] if doc.contains('\n') => format!("/**{doc}*/\n"),

        // Regular doc comment(s) (///) or single line block doc comment
        _ => {
            let mut buffer = String::from("/**\n");
            let mut lines = docs.iter().peekable();

            while let Some(line) = lines.next() {
                buffer.push_str(" *");
                buffer.push_str(line);

                if lines.peek().is_some() {
                    buffer.push('\n');
                }
            }
            buffer.push_str("\n */\n");
            buffer
        }
    }
}

//! [![github]](https://github.com/dtolnay/syn)&ensp;[![crates-io]](https://crates.io/crates/syn)&ensp;[![docs-rs]](crate)
//!
//! [github]: https://img.shields.io/badge/github-8da0cb?style=for-the-badge&labelColor=555555&logo=github
//! [crates-io]: https://img.shields.io/badge/crates.io-fc8d62?style=for-the-badge&labelColor=555555&logo=rust
//! [docs-rs]: https://img.shields.io/badge/docs.rs-66c2a5?style=for-the-badge&labelColor=555555&logo=docs.rs
//!
//! <br>
//!
//! Syn is a parsing library for parsing a stream of Rust tokens into a syntax
//! tree of Rust source code.
//!
//! Currently this library is geared toward use in Rust procedural macros, but
//! contains some APIs that may be useful more generally.
//!
//! - **Data structures** — Syn provides a complete syntax tree that can
//!   represent any valid Rust source code. The syntax tree is rooted at
//!   [`syn::File`] which represents a full source file, but there are other
//!   entry points that may be useful to procedural macros including
//!   [`syn::Item`], [`syn::Expr`] and [`syn::Type`].
//!
//! - **Derives** — Of particular interest to derive macros is
//!   [`syn::DeriveInput`] which is any of the three legal input items to a
//!   derive macro. An example below shows using this type in a library that can
//!   derive implementations of a user-defined trait.
//!
//! - **Parsing** — Parsing in Syn is built around [parser functions] with the
//!   signature `fn(ParseStream) -> Result<T>`. Every syntax tree node defined
//!   by Syn is individually parsable and may be used as a building block for
//!   custom syntaxes, or you may dream up your own brand new syntax without
//!   involving any of our syntax tree types.
//!
//! - **Location information** — Every token parsed by Syn is associated with a
//!   `Span` that tracks line and column information back to the source of that
//!   token. These spans allow a procedural macro to display detailed error
//!   messages pointing to all the right places in the user's code. There is an
//!   example of this below.
//!
//! - **Feature flags** — Functionality is aggressively feature gated so your
//!   procedural macros enable only what they need, and do not pay in compile
//!   time for all the rest.
//!
//! [`syn::File`]: File
//! [`syn::Item`]: Item
//! [`syn::Expr`]: Expr
//! [`syn::Type`]: Type
//! [`syn::DeriveInput`]: DeriveInput
//! [parser functions]: mod@parse
//!
//! <br>
//!
//! # Example of a derive macro
//!
//! The canonical derive macro using Syn looks like this. We write an ordinary
//! Rust function tagged with a `proc_macro_derive` attribute and the name of
//! the trait we are deriving. Any time that derive appears in the user's code,
//! the Rust compiler passes their data structure as tokens into our macro. We
//! get to execute arbitrary Rust code to figure out what to do with those
//! tokens, then hand some tokens back to the compiler to compile into the
//! user's crate.
//!
//! [`TokenStream`]: proc_macro::TokenStream
//!
//! ```toml
//! [dependencies]
//! syn = "1.0"
//! quote = "1.0"
//!
//! [lib]
//! proc-macro = true
//! ```
//!
//! ```
//! # extern crate proc_macro;
//! #
//! use proc_macro::TokenStream;
//! use quote::quote;
//! use syn::{parse_macro_input, DeriveInput};
//!
//! # const IGNORE_TOKENS: &str = stringify! {
//! #[proc_macro_derive(MyMacro)]
//! # };
//! pub fn my_macro(input: TokenStream) -> TokenStream {
//!     // Parse the input tokens into a syntax tree
//!     let input = parse_macro_input!(input as DeriveInput);
//!
//!     // Build the output, possibly using quasi-quotation
//!     let expanded = quote! {
//!         // ...
//!     };
//!
//!     // Hand the output tokens back to the compiler
//!     TokenStream::from(expanded)
//! }
//! ```
//!
//! The [`heapsize`] example directory shows a complete working implementation
//! of a derive macro. It works on any Rust compiler 1.31+. The example derives
//! a `HeapSize` trait which computes an estimate of the amount of heap memory
//! owned by a value.
//!
//! [`heapsize`]: https://github.com/dtolnay/syn/tree/master/examples/heapsize
//!
//! ```
//! pub trait HeapSize {
//!     /// Total number of bytes of heap memory owned by `self`.
//!     fn heap_size_of_children(&self) -> usize;
//! }
//! ```
//!
//! The derive macro allows users to write `#[derive(HeapSize)]` on data
//! structures in their program.
//!
//! ```
//! # const IGNORE_TOKENS: &str = stringify! {
//! #[derive(HeapSize)]
//! # };
//! struct Demo<'a, T: ?Sized> {
//!     a: Box<T>,
//!     b: u8,
//!     c: &'a str,
//!     d: String,
//! }
//! ```
//!
//! <p><br></p>
//!
//! # Spans and error reporting
//!
//! The token-based procedural macro API provides great control over where the
//! compiler's error messages are displayed in user code. Consider the error the
//! user sees if one of their field types does not implement `HeapSize`.
//!
//! ```
//! # const IGNORE_TOKENS: &str = stringify! {
//! #[derive(HeapSize)]
//! # };
//! struct Broken {
//!     ok: String,
//!     bad: std::thread::Thread,
//! }
//! ```
//!
//! By tracking span information all the way through the expansion of a
//! procedural macro as shown in the `heapsize` example, token-based macros in
//! Syn are able to trigger errors that directly pinpoint the source of the
//! problem.
//!
//! ```text
//! error[E0277]: the trait bound `std::thread::Thread: HeapSize` is not satisfied
//!  --> src/main.rs:7:5
//!   |
//! 7 |     bad: std::thread::Thread,
//!   |     ^^^^^^^^^^^^^^^^^^^^^^^^ the trait `HeapSize` is not implemented for `Thread`
//! ```
//!
//! <br>
//!
//! # Parsing a custom syntax
//!
//! The [`lazy-static`] example directory shows the implementation of a
//! `functionlike!(...)` procedural macro in which the input tokens are parsed
//! using Syn's parsing API.
//!
//! [`lazy-static`]: https://github.com/dtolnay/syn/tree/master/examples/lazy-static
//!
//! The example reimplements the popular `lazy_static` crate from crates.io as a
//! procedural macro.
//!
//! ```
//! # macro_rules! lazy_static {
//! #     ($($tt:tt)*) => {}
//! # }
//! #
//! lazy_static! {
//!     static ref USERNAME: Regex = Regex::new("^[a-z0-9_-]{3,16}$").unwrap();
//! }
//! ```
//!
//! The implementation shows how to trigger custom warnings and error messages
//! on the macro input.
//!
//! ```text
//! warning: come on, pick a more creative name
//!   --> src/main.rs:10:16
//!    |
//! 10 |     static ref FOO: String = "lazy_static".to_owned();
//!    |                ^^^
//! ```
//!
//! <br>
//!
//! # Testing
//!
//! When testing macros, we often care not just that the macro can be used
//! successfully but also that when the macro is provided with invalid input it
//! produces maximally helpful error messages. Consider using the [`trybuild`]
//! crate to write tests for errors that are emitted by your macro or errors
//! detected by the Rust compiler in the expanded code following misuse of the
//! macro. Such tests help avoid regressions from later refactors that
//! mistakenly make an error no longer trigger or be less helpful than it used
//! to be.
//!
//! [`trybuild`]: https://github.com/dtolnay/trybuild
//!
//! <br>
//!
//! # Debugging
//!
//! When developing a procedural macro it can be helpful to look at what the
//! generated code looks like. Use `cargo rustc -- -Zunstable-options
//! --pretty=expanded` or the [`cargo expand`] subcommand.
//!
//! [`cargo expand`]: https://github.com/dtolnay/cargo-expand
//!
//! To show the expanded code for some crate that uses your procedural macro,
//! run `cargo expand` from that crate. To show the expanded code for one of
//! your own test cases, run `cargo expand --test the_test_case` where the last
//! argument is the name of the test file without the `.rs` extension.
//!
//! This write-up by Brandon W Maister discusses debugging in more detail:
//! [Debugging Rust's new Custom Derive system][debugging].
//!
//! [debugging]: https://quodlibetor.github.io/posts/debugging-rusts-new-custom-derive-system/
//!
//! <br>
//!
//! # Optional features
//!
//! Syn puts a lot of functionality behind optional features in order to
//! optimize compile time for the most common use cases. The following features
//! are available.
//!
//! - **`derive`** *(enabled by default)* — Data structures for representing the
//!   possible input to a derive macro, including structs and enums and types.
//! - **`full`** — Data structures for representing the syntax tree of all valid
//!   Rust source code, including items and expressions.
//! - **`parsing`** *(enabled by default)* — Ability to parse input tokens into
//!   a syntax tree node of a chosen type.
//! - **`printing`** *(enabled by default)* — Ability to print a syntax tree
//!   node as tokens of Rust source code.
//! - **`visit`** — Trait for traversing a syntax tree.
//! - **`visit-mut`** — Trait for traversing and mutating in place a syntax
//!   tree.
//! - **`fold`** — Trait for transforming an owned syntax tree.
//! - **`clone-impls`** *(enabled by default)* — Clone impls for all syntax tree
//!   types.
//! - **`extra-traits`** — Debug, Eq, PartialEq, Hash impls for all syntax tree
//!   types.
//! - **`proc-macro`** *(enabled by default)* — Runtime dependency on the
//!   dynamic library libproc_macro from rustc toolchain.

// Syn types in rustdoc of other crates get linked to here.
#![doc(html_root_url = "https://docs.rs/syn/1.0.109")]
#![cfg_attr(doc_cfg, feature(doc_cfg))]
#![allow(non_camel_case_types)]
#![allow(
    clippy::bool_to_int_with_if,
    clippy::cast_lossless,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_ptr_alignment,
    clippy::default_trait_access,
    clippy::doc_markdown,
    clippy::expl_impl_clone_on_copy,
    clippy::explicit_auto_deref,
    clippy::if_not_else,
    clippy::inherent_to_string,
    clippy::items_after_statements,
    clippy::large_enum_variant,
    clippy::manual_assert,
    clippy::match_on_vec_items,
    clippy::match_same_arms,
    clippy::match_wildcard_for_single_variants, // clippy bug: https://github.com/rust-lang/rust-clippy/issues/6984
    clippy::missing_errors_doc,
    clippy::missing_panics_doc,
    clippy::module_name_repetitions,
    clippy::must_use_candidate,
    clippy::needless_doctest_main,
    clippy::needless_pass_by_value,
    clippy::never_loop,
    clippy::redundant_else,
    clippy::return_self_not_must_use,
    clippy::similar_names,
    clippy::single_match_else,
    clippy::too_many_arguments,
    clippy::too_many_lines,
    clippy::trivially_copy_pass_by_ref,
    clippy::unnecessary_unwrap,
    clippy::used_underscore_binding,
    clippy::wildcard_imports
)]

#[cfg(all(
    not(all(target_arch = "wasm32", any(target_os = "unknown", target_os = "wasi"))),
    feature = "proc-macro"
))]
extern crate proc_macro;
extern crate proc_macro2;

#[cfg(feature = "printing")]
extern crate quote;

#[macro_use]
mod macros;

#[cfg(feature = "parsing")]
#[macro_use]
mod group;

#[macro_use]
pub mod token;

mod ident;
pub use crate::ident::Ident;

#[cfg(any(feature = "full", feature = "derive"))]
mod attr;
#[cfg(any(feature = "full", feature = "derive"))]
pub use crate::attr::{
    AttrStyle, Attribute, AttributeArgs, Meta, MetaList, MetaNameValue, NestedMeta,
};

mod bigint;

#[cfg(any(feature = "full", feature = "derive"))]
mod data;
#[cfg(any(feature = "full", feature = "derive"))]
pub use crate::data::{
    Field, Fields, FieldsNamed, FieldsUnnamed, Variant, VisCrate, VisPublic, VisRestricted,
    Visibility,
};

#[cfg(any(feature = "full", feature = "derive"))]
mod expr;
#[cfg(feature = "full")]
pub use crate::expr::{
    Arm, FieldValue, GenericMethodArgument, Label, MethodTurbofish, RangeLimits,
};
#[cfg(any(feature = "full", feature = "derive"))]
pub use crate::expr::{
    Expr, ExprArray, ExprAssign, ExprAssignOp, ExprAsync, ExprAwait, ExprBinary, ExprBlock,
    ExprBox, ExprBreak, ExprCall, ExprCast, ExprClosure, ExprContinue, ExprField, ExprForLoop,
    ExprGroup, ExprIf, ExprIndex, ExprLet, ExprLit, ExprLoop, ExprMacro, ExprMatch, ExprMethodCall,
    ExprParen, ExprPath, ExprRange, ExprReference, ExprRepeat, ExprReturn, ExprStruct, ExprTry,
    ExprTryBlock, ExprTuple, ExprType, ExprUnary, ExprUnsafe, ExprWhile, ExprYield, Index, Member,
};

#[cfg(any(feature = "full", feature = "derive"))]
mod generics;
#[cfg(any(feature = "full", feature = "derive"))]
pub use crate::generics::{
    BoundLifetimes, ConstParam, GenericParam, Generics, LifetimeDef, PredicateEq,
    PredicateLifetime, PredicateType, TraitBound, TraitBoundModifier, TypeParam, TypeParamBound,
    WhereClause, WherePredicate,
};
#[cfg(all(any(feature = "full", feature = "derive"), feature = "printing"))]
pub use crate::generics::{ImplGenerics, Turbofish, TypeGenerics};

#[cfg(feature = "full")]
mod item;
#[cfg(feature = "full")]
pub use crate::item::{
    FnArg, ForeignItem, ForeignItemFn, ForeignItemMacro, ForeignItemStatic, ForeignItemType,
    ImplItem, ImplItemConst, ImplItemMacro, ImplItemMethod, ImplItemType, Item, ItemConst,
    ItemEnum, ItemExternCrate, ItemFn, ItemForeignMod, ItemImpl, ItemMacro, ItemMacro2, ItemMod,
    ItemStatic, ItemStruct, ItemTrait, ItemTraitAlias, ItemType, ItemUnion, ItemUse, Receiver,
    Signature, TraitItem, TraitItemConst, TraitItemMacro, TraitItemMethod, TraitItemType, UseGlob,
    UseGroup, UseName, UsePath, UseRename, UseTree,
};

#[cfg(feature = "full")]
mod file;
#[cfg(feature = "full")]
pub use crate::file::File;

mod lifetime;
pub use crate::lifetime::Lifetime;

mod lit;
pub use crate::lit::{
    Lit, LitBool, LitByte, LitByteStr, LitChar, LitFloat, LitInt, LitStr, StrStyle,
};

#[cfg(any(feature = "full", feature = "derive"))]
mod mac;
#[cfg(any(feature = "full", feature = "derive"))]
pub use crate::mac::{Macro, MacroDelimiter};

#[cfg(any(feature = "full", feature = "derive"))]
mod derive;
#[cfg(feature = "derive")]
pub use crate::derive::{Data, DataEnum, DataStruct, DataUnion, DeriveInput};

#[cfg(any(feature = "full", feature = "derive"))]
mod op;
#[cfg(any(feature = "full", feature = "derive"))]
pub use crate::op::{BinOp, UnOp};

#[cfg(feature = "full")]
mod stmt;
#[cfg(feature = "full")]
pub use crate::stmt::{Block, Local, Stmt};

#[cfg(any(feature = "full", feature = "derive"))]
mod ty;
#[cfg(any(feature = "full", feature = "derive"))]
pub use crate::ty::{
    Abi, BareFnArg, ReturnType, Type, TypeArray, TypeBareFn, TypeGroup, TypeImplTrait, TypeInfer,
    TypeMacro, TypeNever, TypeParen, TypePath, TypePtr, TypeReference, TypeSlice, TypeTraitObject,
    TypeTuple, Variadic,
};

#[cfg(feature = "full")]
mod pat;
#[cfg(feature = "full")]
pub use crate::pat::{
    FieldPat, Pat, PatBox, PatIdent, PatLit, PatMacro, PatOr, PatPath, PatRange, PatReference,
    PatRest, PatSlice, PatStruct, PatTuple, PatTupleStruct, PatType, PatWild,
};

#[cfg(any(feature = "full", feature = "derive"))]
mod path;
#[cfg(any(feature = "full", feature = "derive"))]
pub use crate::path::{
    AngleBracketedGenericArguments, Binding, Constraint, GenericArgument,
    ParenthesizedGenericArguments, Path, PathArguments, PathSegment, QSelf,
};

#[cfg(feature = "parsing")]
#[cfg_attr(doc_cfg, doc(cfg(feature = "parsing")))]
pub mod buffer;
mod drops;
#[cfg(feature = "parsing")]
#[cfg_attr(doc_cfg, doc(cfg(feature = "parsing")))]
pub mod ext;
pub mod punctuated;
#[cfg(all(any(feature = "full", feature = "derive"), feature = "extra-traits"))]
mod tt;

// Not public API except the `parse_quote!` macro.
#[cfg(feature = "parsing")]
#[doc(hidden)]
pub mod parse_quote;

// Not public API except the `parse_macro_input!` macro.
#[cfg(all(
    not(all(target_arch = "wasm32", any(target_os = "unknown", target_os = "wasi"))),
    feature = "parsing",
    feature = "proc-macro"
))]
#[doc(hidden)]
pub mod parse_macro_input;

#[cfg(all(feature = "parsing", feature = "printing"))]
#[cfg_attr(doc_cfg, doc(cfg(all(feature = "parsing", feature = "printing"))))]
pub mod spanned;

#[cfg(all(feature = "parsing", feature = "full"))]
mod whitespace;

mod gen {
    /// Syntax tree traversal to walk a shared borrow of a syntax tree.
    ///
    /// Each method of the [`Visit`] trait is a hook that can be overridden to
    /// customize the behavior when visiting the corresponding type of node. By
    /// default, every method recursively visits the substructure of the input
    /// by invoking the right visitor method of each of its fields.
    ///
    /// [`Visit`]: visit::Visit
    ///
    /// ```
    /// # use syn::{Attribute, BinOp, Expr, ExprBinary};
    /// #
    /// pub trait Visit<'ast> {
    ///     /* ... */
    ///
    ///     fn visit_expr_binary(&mut self, node: &'ast ExprBinary) {
    ///         visit_expr_binary(self, node);
    ///     }
    ///
    ///     /* ... */
    ///     # fn visit_attribute(&mut self, node: &'ast Attribute);
    ///     # fn visit_expr(&mut self, node: &'ast Expr);
    ///     # fn visit_bin_op(&mut self, node: &'ast BinOp);
    /// }
    ///
    /// pub fn visit_expr_binary<'ast, V>(v: &mut V, node: &'ast ExprBinary)
    /// where
    ///     V: Visit<'ast> + ?Sized,
    /// {
    ///     for attr in &node.attrs {
    ///         v.visit_attribute(attr);
    ///     }
    ///     v.visit_expr(&*node.left);
    ///     v.visit_bin_op(&node.op);
    ///     v.visit_expr(&*node.right);
    /// }
    ///
    /// /* ... */
    /// ```
    ///
    /// *This module is available only if Syn is built with the `"visit"` feature.*
    ///
    /// <br>
    ///
    /// # Example
    ///
    /// This visitor will print the name of every freestanding function in the
    /// syntax tree, including nested functions.
    ///
    /// ```
    /// // [dependencies]
    /// // quote = "1.0"
    /// // syn = { version = "1.0", features = ["full", "visit"] }
    ///
    /// use quote::quote;
    /// use syn::visit::{self, Visit};
    /// use syn::{File, ItemFn};
    ///
    /// struct FnVisitor;
    ///
    /// impl<'ast> Visit<'ast> for FnVisitor {
    ///     fn visit_item_fn(&mut self, node: &'ast ItemFn) {
    ///         println!("Function with name={}", node.sig.ident);
    ///
    ///         // Delegate to the default impl to visit any nested functions.
    ///         visit::visit_item_fn(self, node);
    ///     }
    /// }
    ///
    /// fn main() {
    ///     let code = quote! {
    ///         pub fn f() {
    ///             fn g() {}
    ///         }
    ///     };
    ///
    ///     let syntax_tree: File = syn::parse2(code).unwrap();
    ///     FnVisitor.visit_file(&syntax_tree);
    /// }
    /// ```
    ///
    /// The `'ast` lifetime on the input references means that the syntax tree
    /// outlives the complete recursive visit call, so the visitor is allowed to
    /// hold on to references into the syntax tree.
    ///
    /// ```
    /// use quote::quote;
    /// use syn::visit::{self, Visit};
    /// use syn::{File, ItemFn};
    ///
    /// struct FnVisitor<'ast> {
    ///     functions: Vec<&'ast ItemFn>,
    /// }
    ///
    /// impl<'ast> Visit<'ast> for FnVisitor<'ast> {
    ///     fn visit_item_fn(&mut self, node: &'ast ItemFn) {
    ///         self.functions.push(node);
    ///         visit::visit_item_fn(self, node);
    ///     }
    /// }
    ///
    /// fn main() {
    ///     let code = quote! {
    ///         pub fn f() {
    ///             fn g() {}
    ///         }
    ///     };
    ///
    ///     let syntax_tree: File = syn::parse2(code).unwrap();
    ///     let mut visitor = FnVisitor { functions: Vec::new() };
    ///     visitor.visit_file(&syntax_tree);
    ///     for f in visitor.functions {
    ///         println!("Function with name={}", f.sig.ident);
    ///     }
    /// }
    /// ```
    #[cfg(feature = "visit")]
    #[cfg_attr(doc_cfg, doc(cfg(feature = "visit")))]
    #[rustfmt::skip]
    pub mod visit;

    /// Syntax tree traversal to mutate an exclusive borrow of a syntax tree in
    /// place.
    ///
    /// Each method of the [`VisitMut`] trait is a hook that can be overridden
    /// to customize the behavior when mutating the corresponding type of node.
    /// By default, every method recursively visits the substructure of the
    /// input by invoking the right visitor method of each of its fields.
    ///
    /// [`VisitMut`]: visit_mut::VisitMut
    ///
    /// ```
    /// # use syn::{Attribute, BinOp, Expr, ExprBinary};
    /// #
    /// pub trait VisitMut {
    ///     /* ... */
    ///
    ///     fn visit_expr_binary_mut(&mut self, node: &mut ExprBinary) {
    ///         visit_expr_binary_mut(self, node);
    ///     }
    ///
    ///     /* ... */
    ///     # fn visit_attribute_mut(&mut self, node: &mut Attribute);
    ///     # fn visit_expr_mut(&mut self, node: &mut Expr);
    ///     # fn visit_bin_op_mut(&mut self, node: &mut BinOp);
    /// }
    ///
    /// pub fn visit_expr_binary_mut<V>(v: &mut V, node: &mut ExprBinary)
    /// where
    ///     V: VisitMut + ?Sized,
    /// {
    ///     for attr in &mut node.attrs {
    ///         v.visit_attribute_mut(attr);
    ///     }
    ///     v.visit_expr_mut(&mut *node.left);
    ///     v.visit_bin_op_mut(&mut node.op);
    ///     v.visit_expr_mut(&mut *node.right);
    /// }
    ///
    /// /* ... */
    /// ```
    ///
    /// *This module is available only if Syn is built with the `"visit-mut"`
    /// feature.*
    ///
    /// <br>
    ///
    /// # Example
    ///
    /// This mut visitor replace occurrences of u256 suffixed integer literals
    /// like `999u256` with a macro invocation `bigint::u256!(999)`.
    ///
    /// ```
    /// // [dependencies]
    /// // quote = "1.0"
    /// // syn = { version = "1.0", features = ["full", "visit-mut"] }
    ///
    /// use quote::quote;
    /// use syn::visit_mut::{self, VisitMut};
    /// use syn::{parse_quote, Expr, File, Lit, LitInt};
    ///
    /// struct BigintReplace;
    ///
    /// impl VisitMut for BigintReplace {
    ///     fn visit_expr_mut(&mut self, node: &mut Expr) {
    ///         if let Expr::Lit(expr) = &node {
    ///             if let Lit::Int(int) = &expr.lit {
    ///                 if int.suffix() == "u256" {
    ///                     let digits = int.base10_digits();
    ///                     let unsuffixed: LitInt = syn::parse_str(digits).unwrap();
    ///                     *node = parse_quote!(bigint::u256!(#unsuffixed));
    ///                     return;
    ///                 }
    ///             }
    ///         }
    ///
    ///         // Delegate to the default impl to visit nested expressions.
    ///         visit_mut::visit_expr_mut(self, node);
    ///     }
    /// }
    ///
    /// fn main() {
    ///     let code = quote! {
    ///         fn main() {
    ///             let _ = 999u256;
    ///         }
    ///     };
    ///
    ///     let mut syntax_tree: File = syn::parse2(code).unwrap();
    ///     BigintReplace.visit_file_mut(&mut syntax_tree);
    ///     println!("{}", quote!(#syntax_tree));
    /// }
    /// ```
    #[cfg(feature = "visit-mut")]
    #[cfg_attr(doc_cfg, doc(cfg(feature = "visit-mut")))]
    #[rustfmt::skip]
    pub mod visit_mut;

    /// Syntax tree traversal to transform the nodes of an owned syntax tree.
    ///
    /// Each method of the [`Fold`] trait is a hook that can be overridden to
    /// customize the behavior when transforming the corresponding type of node.
    /// By default, every method recursively visits the substructure of the
    /// input by invoking the right visitor method of each of its fields.
    ///
    /// [`Fold`]: fold::Fold
    ///
    /// ```
    /// # use syn::{Attribute, BinOp, Expr, ExprBinary};
    /// #
    /// pub trait Fold {
    ///     /* ... */
    ///
    ///     fn fold_expr_binary(&mut self, node: ExprBinary) -> ExprBinary {
    ///         fold_expr_binary(self, node)
    ///     }
    ///
    ///     /* ... */
    ///     # fn fold_attribute(&mut self, node: Attribute) -> Attribute;
    ///     # fn fold_expr(&mut self, node: Expr) -> Expr;
    ///     # fn fold_bin_op(&mut self, node: BinOp) -> BinOp;
    /// }
    ///
    /// pub fn fold_expr_binary<V>(v: &mut V, node: ExprBinary) -> ExprBinary
    /// where
    ///     V: Fold + ?Sized,
    /// {
    ///     ExprBinary {
    ///         attrs: node
    ///             .attrs
    ///             .into_iter()
    ///             .map(|attr| v.fold_attribute(attr))
    ///             .collect(),
    ///         left: Box::new(v.fold_expr(*node.left)),
    ///         op: v.fold_bin_op(node.op),
    ///         right: Box::new(v.fold_expr(*node.right)),
    ///     }
    /// }
    ///
    /// /* ... */
    /// ```
    ///
    /// *This module is available only if Syn is built with the `"fold"` feature.*
    ///
    /// <br>
    ///
    /// # Example
    ///
    /// This fold inserts parentheses to fully parenthesizes any expression.
    ///
    /// ```
    /// // [dependencies]
    /// // quote = "1.0"
    /// // syn = { version = "1.0", features = ["fold", "full"] }
    ///
    /// use quote::quote;
    /// use syn::fold::{fold_expr, Fold};
    /// use syn::{token, Expr, ExprParen};
    ///
    /// struct ParenthesizeEveryExpr;
    ///
    /// impl Fold for ParenthesizeEveryExpr {
    ///     fn fold_expr(&mut self, expr: Expr) -> Expr {
    ///         Expr::Paren(ExprParen {
    ///             attrs: Vec::new(),
    ///             expr: Box::new(fold_expr(self, expr)),
    ///             paren_token: token::Paren::default(),
    ///         })
    ///     }
    /// }
    ///
    /// fn main() {
    ///     let code = quote! { a() + b(1) * c.d };
    ///     let expr: Expr = syn::parse2(code).unwrap();
    ///     let parenthesized = ParenthesizeEveryExpr.fold_expr(expr);
    ///     println!("{}", quote!(#parenthesized));
    ///
    ///     // Output: (((a)()) + (((b)((1))) * ((c).d)))
    /// }
    /// ```
    #[cfg(feature = "fold")]
    #[cfg_attr(doc_cfg, doc(cfg(feature = "fold")))]
    #[rustfmt::skip]
    pub mod fold;

    #[cfg(feature = "clone-impls")]
    #[rustfmt::skip]
    mod clone;

    #[cfg(feature = "extra-traits")]
    #[rustfmt::skip]
    mod eq;

    #[cfg(feature = "extra-traits")]
    #[rustfmt::skip]
    mod hash;

    #[cfg(feature = "extra-traits")]
    #[rustfmt::skip]
    mod debug;

    #[cfg(any(feature = "full", feature = "derive"))]
    #[path = "../gen_helper.rs"]
    mod helper;
}
pub use crate::gen::*;

// Not public API.
#[doc(hidden)]
#[path = "export.rs"]
pub mod __private;

mod custom_keyword;
mod custom_punctuation;
mod sealed;
mod span;
mod thread;

#[cfg(feature = "parsing")]
mod lookahead;

#[cfg(feature = "parsing")]
#[cfg_attr(doc_cfg, doc(cfg(feature = "parsing")))]
pub mod parse;

#[cfg(feature = "full")]
mod reserved;

#[cfg(all(any(feature = "full", feature = "derive"), feature = "parsing"))]
mod verbatim;

#[cfg(all(any(feature = "full", feature = "derive"), feature = "printing"))]
mod print;

////////////////////////////////////////////////////////////////////////////////

mod error;
pub use crate::error::{Error, Result};

/// Parse tokens of source code into the chosen syntax tree node.
///
/// This is preferred over parsing a string because tokens are able to preserve
/// information about where in the user's code they were originally written (the
/// "span" of the token), possibly allowing the compiler to produce better error
/// messages.
///
/// This function parses a `proc_macro::TokenStream` which is the type used for
/// interop with the compiler in a procedural macro. To parse a
/// `proc_macro2::TokenStream`, use [`syn::parse2`] instead.
///
/// [`syn::parse2`]: parse2
///
/// *This function is available only if Syn is built with both the `"parsing"` and
/// `"proc-macro"` features.*
///
/// # Examples
///
/// ```
/// # extern crate proc_macro;
/// #
/// use proc_macro::TokenStream;
/// use quote::quote;
/// use syn::DeriveInput;
///
/// # const IGNORE_TOKENS: &str = stringify! {
/// #[proc_macro_derive(MyMacro)]
/// # };
/// pub fn my_macro(input: TokenStream) -> TokenStream {
///     // Parse the tokens into a syntax tree
///     let ast: DeriveInput = syn::parse(input).unwrap();
///
///     // Build the output, possibly using quasi-quotation
///     let expanded = quote! {
///         /* ... */
///     };
///
///     // Convert into a token stream and return it
///     expanded.into()
/// }
/// ```
#[cfg(all(
    not(all(target_arch = "wasm32", any(target_os = "unknown", target_os = "wasi"))),
    feature = "parsing",
    feature = "proc-macro"
))]
#[cfg_attr(doc_cfg, doc(cfg(all(feature = "parsing", feature = "proc-macro"))))]
pub fn parse<T: parse::Parse>(tokens: proc_macro::TokenStream) -> Result<T> {
    parse::Parser::parse(T::parse, tokens)
}

/// Parse a proc-macro2 token stream into the chosen syntax tree node.
///
/// This function will check that the input is fully parsed. If there are
/// any unparsed tokens at the end of the stream, an error is returned.
///
/// This function parses a `proc_macro2::TokenStream` which is commonly useful
/// when the input comes from a node of the Syn syntax tree, for example the
/// body tokens of a [`Macro`] node. When in a procedural macro parsing the
/// `proc_macro::TokenStream` provided by the compiler, use [`syn::parse`]
/// instead.
///
/// [`syn::parse`]: parse()
///
/// *This function is available only if Syn is built with the `"parsing"` feature.*
#[cfg(feature = "parsing")]
#[cfg_attr(doc_cfg, doc(cfg(feature = "parsing")))]
pub fn parse2<T: parse::Parse>(tokens: proc_macro2::TokenStream) -> Result<T> {
    parse::Parser::parse2(T::parse, tokens)
}

/// Parse a string of Rust code into the chosen syntax tree node.
///
/// *This function is available only if Syn is built with the `"parsing"` feature.*
///
/// # Hygiene
///
/// Every span in the resulting syntax tree will be set to resolve at the macro
/// call site.
///
/// # Examples
///
/// ```
/// use syn::{Expr, Result};
///
/// fn run() -> Result<()> {
///     let code = "assert_eq!(u8::max_value(), 255)";
///     let expr = syn::parse_str::<Expr>(code)?;
///     println!("{:#?}", expr);
///     Ok(())
/// }
/// #
/// # run().unwrap();
/// ```
#[cfg(feature = "parsing")]
#[cfg_attr(doc_cfg, doc(cfg(feature = "parsing")))]
pub fn parse_str<T: parse::Parse>(s: &str) -> Result<T> {
    parse::Parser::parse_str(T::parse, s)
}

// FIXME the name parse_file makes it sound like you might pass in a path to a
// file, rather than the content.
/// Parse the content of a file of Rust code.
///
/// This is different from `syn::parse_str::<File>(content)` in two ways:
///
/// - It discards a leading byte order mark `\u{FEFF}` if the file has one.
/// - It preserves the shebang line of the file, such as `#!/usr/bin/env rustx`.
///
/// If present, either of these would be an error using `from_str`.
///
/// *This function is available only if Syn is built with the `"parsing"` and
/// `"full"` features.*
///
/// # Examples
///
/// ```no_run
/// use std::error::Error;
/// use std::fs::File;
/// use std::io::Read;
///
/// fn run() -> Result<(), Box<Error>> {
///     let mut file = File::open("path/to/code.rs")?;
///     let mut content = String::new();
///     file.read_to_string(&mut content)?;
///
///     let ast = syn::parse_file(&content)?;
///     if let Some(shebang) = ast.shebang {
///         println!("{}", shebang);
///     }
///     println!("{} items", ast.items.len());
///
///     Ok(())
/// }
/// #
/// # run().unwrap();
/// ```
#[cfg(all(feature = "parsing", feature = "full"))]
#[cfg_attr(doc_cfg, doc(cfg(all(feature = "parsing", feature = "full"))))]
pub fn parse_file(mut content: &str) -> Result<File> {
    // Strip the BOM if it is present
    const BOM: &str = "\u{feff}";
    if content.starts_with(BOM) {
        content = &content[BOM.len()..];
    }

    let mut shebang = None;
    if content.starts_with("#!") {
        let rest = whitespace::skip(&content[2..]);
        if !rest.starts_with('[') {
            if let Some(idx) = content.find('\n') {
                shebang = Some(content[..idx].to_string());
                content = &content[idx..];
            } else {
                shebang = Some(content.to_string());
                content = "";
            }
        }
    }

    let mut file: File = parse_str(content)?;
    file.shebang = shebang;
    Ok(file)
}

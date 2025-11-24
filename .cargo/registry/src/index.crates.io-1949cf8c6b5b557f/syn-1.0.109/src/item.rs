use super::*;
use crate::derive::{Data, DataEnum, DataStruct, DataUnion, DeriveInput};
use crate::punctuated::Punctuated;
use proc_macro2::TokenStream;

#[cfg(feature = "parsing")]
use std::mem;

ast_enum_of_structs! {
    /// Things that can appear directly inside of a module or scope.
    ///
    /// *This type is available only if Syn is built with the `"full"` feature.*
    ///
    /// # Syntax tree enum
    ///
    /// This type is a [syntax tree enum].
    ///
    /// [syntax tree enum]: Expr#syntax-tree-enums
    #[cfg_attr(doc_cfg, doc(cfg(feature = "full")))]
    #[cfg_attr(not(syn_no_non_exhaustive), non_exhaustive)]
    pub enum Item {
        /// A constant item: `const MAX: u16 = 65535`.
        Const(ItemConst),

        /// An enum definition: `enum Foo<A, B> { A(A), B(B) }`.
        Enum(ItemEnum),

        /// An `extern crate` item: `extern crate serde`.
        ExternCrate(ItemExternCrate),

        /// A free-standing function: `fn process(n: usize) -> Result<()> { ...
        /// }`.
        Fn(ItemFn),

        /// A block of foreign items: `extern "C" { ... }`.
        ForeignMod(ItemForeignMod),

        /// An impl block providing trait or associated items: `impl<A> Trait
        /// for Data<A> { ... }`.
        Impl(ItemImpl),

        /// A macro invocation, which includes `macro_rules!` definitions.
        Macro(ItemMacro),

        /// A 2.0-style declarative macro introduced by the `macro` keyword.
        Macro2(ItemMacro2),

        /// A module or module declaration: `mod m` or `mod m { ... }`.
        Mod(ItemMod),

        /// A static item: `static BIKE: Shed = Shed(42)`.
        Static(ItemStatic),

        /// A struct definition: `struct Foo<A> { x: A }`.
        Struct(ItemStruct),

        /// A trait definition: `pub trait Iterator { ... }`.
        Trait(ItemTrait),

        /// A trait alias: `pub trait SharableIterator = Iterator + Sync`.
        TraitAlias(ItemTraitAlias),

        /// A type alias: `type Result<T> = std::result::Result<T, MyError>`.
        Type(ItemType),

        /// A union definition: `union Foo<A, B> { x: A, y: B }`.
        Union(ItemUnion),

        /// A use declaration: `use std::collections::HashMap`.
        Use(ItemUse),

        /// Tokens forming an item not interpreted by Syn.
        Verbatim(TokenStream),

        // Not public API.
        //
        // For testing exhaustiveness in downstream code, use the following idiom:
        //
        //     match item {
        //         Item::Const(item) => {...}
        //         Item::Enum(item) => {...}
        //         ...
        //         Item::Verbatim(item) => {...}
        //
        //         #[cfg_attr(test, deny(non_exhaustive_omitted_patterns))]
        //         _ => { /* some sane fallback */ }
        //     }
        //
        // This way we fail your tests but don't break your library when adding
        // a variant. You will be notified by a test failure when a variant is
        // added, so that you can add code to handle it, but your library will
        // continue to compile and work for downstream users in the interim.
        #[cfg(syn_no_non_exhaustive)]
        #[doc(hidden)]
        __NonExhaustive,
    }
}

ast_struct! {
    /// A constant item: `const MAX: u16 = 65535`.
    ///
    /// *This type is available only if Syn is built with the `"full"` feature.*
    #[cfg_attr(doc_cfg, doc(cfg(feature = "full")))]
    pub struct ItemConst {
        pub attrs: Vec<Attribute>,
        pub vis: Visibility,
        pub const_token: Token![const],
        pub ident: Ident,
        pub colon_token: Token![:],
        pub ty: Box<Type>,
        pub eq_token: Token![=],
        pub expr: Box<Expr>,
        pub semi_token: Token![;],
    }
}

ast_struct! {
    /// An enum definition: `enum Foo<A, B> { A(A), B(B) }`.
    ///
    /// *This type is available only if Syn is built with the `"full"` feature.*
    #[cfg_attr(doc_cfg, doc(cfg(feature = "full")))]
    pub struct ItemEnum {
        pub attrs: Vec<Attribute>,
        pub vis: Visibility,
        pub enum_token: Token![enum],
        pub ident: Ident,
        pub generics: Generics,
        pub brace_token: token::Brace,
        pub variants: Punctuated<Variant, Token![,]>,
    }
}

ast_struct! {
    /// An `extern crate` item: `extern crate serde`.
    ///
    /// *This type is available only if Syn is built with the `"full"` feature.*
    #[cfg_attr(doc_cfg, doc(cfg(feature = "full")))]
    pub struct ItemExternCrate {
        pub attrs: Vec<Attribute>,
        pub vis: Visibility,
        pub extern_token: Token![extern],
        pub crate_token: Token![crate],
        pub ident: Ident,
        pub rename: Option<(Token![as], Ident)>,
        pub semi_token: Token![;],
    }
}

ast_struct! {
    /// A free-standing function: `fn process(n: usize) -> Result<()> { ...
    /// }`.
    ///
    /// *This type is available only if Syn is built with the `"full"` feature.*
    #[cfg_attr(doc_cfg, doc(cfg(feature = "full")))]
    pub struct ItemFn {
        pub attrs: Vec<Attribute>,
        pub vis: Visibility,
        pub sig: Signature,
        pub block: Box<Block>,
    }
}

ast_struct! {
    /// A block of foreign items: `extern "C" { ... }`.
    ///
    /// *This type is available only if Syn is built with the `"full"` feature.*
    #[cfg_attr(doc_cfg, doc(cfg(feature = "full")))]
    pub struct ItemForeignMod {
        pub attrs: Vec<Attribute>,
        pub abi: Abi,
        pub brace_token: token::Brace,
        pub items: Vec<ForeignItem>,
    }
}

ast_struct! {
    /// An impl block providing trait or associated items: `impl<A> Trait
    /// for Data<A> { ... }`.
    ///
    /// *This type is available only if Syn is built with the `"full"` feature.*
    #[cfg_attr(doc_cfg, doc(cfg(feature = "full")))]
    pub struct ItemImpl {
        pub attrs: Vec<Attribute>,
        pub defaultness: Option<Token![default]>,
        pub unsafety: Option<Token![unsafe]>,
        pub impl_token: Token![impl],
        pub generics: Generics,
        /// Trait this impl implements.
        pub trait_: Option<(Option<Token![!]>, Path, Token![for])>,
        /// The Self type of the impl.
        pub self_ty: Box<Type>,
        pub brace_token: token::Brace,
        pub items: Vec<ImplItem>,
    }
}

ast_struct! {
    /// A macro invocation, which includes `macro_rules!` definitions.
    ///
    /// *This type is available only if Syn is built with the `"full"` feature.*
    #[cfg_attr(doc_cfg, doc(cfg(feature = "full")))]
    pub struct ItemMacro {
        pub attrs: Vec<Attribute>,
        /// The `example` in `macro_rules! example { ... }`.
        pub ident: Option<Ident>,
        pub mac: Macro,
        pub semi_token: Option<Token![;]>,
    }
}

ast_struct! {
    /// A 2.0-style declarative macro introduced by the `macro` keyword.
    ///
    /// *This type is available only if Syn is built with the `"full"` feature.*
    #[cfg_attr(doc_cfg, doc(cfg(feature = "full")))]
    pub struct ItemMacro2 {
        pub attrs: Vec<Attribute>,
        pub vis: Visibility,
        pub macro_token: Token![macro],
        pub ident: Ident,
        pub rules: TokenStream,
    }
}

ast_struct! {
    /// A module or module declaration: `mod m` or `mod m { ... }`.
    ///
    /// *This type is available only if Syn is built with the `"full"` feature.*
    #[cfg_attr(doc_cfg, doc(cfg(feature = "full")))]
    pub struct ItemMod {
        pub attrs: Vec<Attribute>,
        pub vis: Visibility,
        pub mod_token: Token![mod],
        pub ident: Ident,
        pub content: Option<(token::Brace, Vec<Item>)>,
        pub semi: Option<Token![;]>,
    }
}

ast_struct! {
    /// A static item: `static BIKE: Shed = Shed(42)`.
    ///
    /// *This type is available only if Syn is built with the `"full"` feature.*
    #[cfg_attr(doc_cfg, doc(cfg(feature = "full")))]
    pub struct ItemStatic {
        pub attrs: Vec<Attribute>,
        pub vis: Visibility,
        pub static_token: Token![static],
        pub mutability: Option<Token![mut]>,
        pub ident: Ident,
        pub colon_token: Token![:],
        pub ty: Box<Type>,
        pub eq_token: Token![=],
        pub expr: Box<Expr>,
        pub semi_token: Token![;],
    }
}

ast_struct! {
    /// A struct definition: `struct Foo<A> { x: A }`.
    ///
    /// *This type is available only if Syn is built with the `"full"` feature.*
    #[cfg_attr(doc_cfg, doc(cfg(feature = "full")))]
    pub struct ItemStruct {
        pub attrs: Vec<Attribute>,
        pub vis: Visibility,
        pub struct_token: Token![struct],
        pub ident: Ident,
        pub generics: Generics,
        pub fields: Fields,
        pub semi_token: Option<Token![;]>,
    }
}

ast_struct! {
    /// A trait definition: `pub trait Iterator { ... }`.
    ///
    /// *This type is available only if Syn is built with the `"full"` feature.*
    #[cfg_attr(doc_cfg, doc(cfg(feature = "full")))]
    pub struct ItemTrait {
        pub attrs: Vec<Attribute>,
        pub vis: Visibility,
        pub unsafety: Option<Token![unsafe]>,
        pub auto_token: Option<Token![auto]>,
        pub trait_token: Token![trait],
        pub ident: Ident,
        pub generics: Generics,
        pub colon_token: Option<Token![:]>,
        pub supertraits: Punctuated<TypeParamBound, Token![+]>,
        pub brace_token: token::Brace,
        pub items: Vec<TraitItem>,
    }
}

ast_struct! {
    /// A trait alias: `pub trait SharableIterator = Iterator + Sync`.
    ///
    /// *This type is available only if Syn is built with the `"full"` feature.*
    #[cfg_attr(doc_cfg, doc(cfg(feature = "full")))]
    pub struct ItemTraitAlias {
        pub attrs: Vec<Attribute>,
        pub vis: Visibility,
        pub trait_token: Token![trait],
        pub ident: Ident,
        pub generics: Generics,
        pub eq_token: Token![=],
        pub bounds: Punctuated<TypeParamBound, Token![+]>,
        pub semi_token: Token![;],
    }
}

ast_struct! {
    /// A type alias: `type Result<T> = std::result::Result<T, MyError>`.
    ///
    /// *This type is available only if Syn is built with the `"full"` feature.*
    #[cfg_attr(doc_cfg, doc(cfg(feature = "full")))]
    pub struct ItemType {
        pub attrs: Vec<Attribute>,
        pub vis: Visibility,
        pub type_token: Token![type],
        pub ident: Ident,
        pub generics: Generics,
        pub eq_token: Token![=],
        pub ty: Box<Type>,
        pub semi_token: Token![;],
    }
}

ast_struct! {
    /// A union definition: `union Foo<A, B> { x: A, y: B }`.
    ///
    /// *This type is available only if Syn is built with the `"full"` feature.*
    #[cfg_attr(doc_cfg, doc(cfg(feature = "full")))]
    pub struct ItemUnion {
        pub attrs: Vec<Attribute>,
        pub vis: Visibility,
        pub union_token: Token![union],
        pub ident: Ident,
        pub generics: Generics,
        pub fields: FieldsNamed,
    }
}

ast_struct! {
    /// A use declaration: `use std::collections::HashMap`.
    ///
    /// *This type is available only if Syn is built with the `"full"` feature.*
    #[cfg_attr(doc_cfg, doc(cfg(feature = "full")))]
    pub struct ItemUse {
        pub attrs: Vec<Attribute>,
        pub vis: Visibility,
        pub use_token: Token![use],
        pub leading_colon: Option<Token![::]>,
        pub tree: UseTree,
        pub semi_token: Token![;],
    }
}

impl Item {
    #[cfg(feature = "parsing")]
    pub(crate) fn replace_attrs(&mut self, new: Vec<Attribute>) -> Vec<Attribute> {
        match self {
            Item::ExternCrate(ItemExternCrate { attrs, .. })
            | Item::Use(ItemUse { attrs, .. })
            | Item::Static(ItemStatic { attrs, .. })
            | Item::Const(ItemConst { attrs, .. })
            | Item::Fn(ItemFn { attrs, .. })
            | Item::Mod(ItemMod { attrs, .. })
            | Item::ForeignMod(ItemForeignMod { attrs, .. })
            | Item::Type(ItemType { attrs, .. })
            | Item::Struct(ItemStruct { attrs, .. })
            | Item::Enum(ItemEnum { attrs, .. })
            | Item::Union(ItemUnion { attrs, .. })
            | Item::Trait(ItemTrait { attrs, .. })
            | Item::TraitAlias(ItemTraitAlias { attrs, .. })
            | Item::Impl(ItemImpl { attrs, .. })
            | Item::Macro(ItemMacro { attrs, .. })
            | Item::Macro2(ItemMacro2 { attrs, .. }) => mem::replace(attrs, new),
            Item::Verbatim(_) => Vec::new(),

            #[cfg(syn_no_non_exhaustive)]
            _ => unreachable!(),
        }
    }
}

impl From<DeriveInput> for Item {
    fn from(input: DeriveInput) -> Item {
        match input.data {
            Data::Struct(data) => Item::Struct(ItemStruct {
                attrs: input.attrs,
                vis: input.vis,
                struct_token: data.struct_token,
                ident: input.ident,
                generics: input.generics,
                fields: data.fields,
                semi_token: data.semi_token,
            }),
            Data::Enum(data) => Item::Enum(ItemEnum {
                attrs: input.attrs,
                vis: input.vis,
                enum_token: data.enum_token,
                ident: input.ident,
                generics: input.generics,
                brace_token: data.brace_token,
                variants: data.variants,
            }),
            Data::Union(data) => Item::Union(ItemUnion {
                attrs: input.attrs,
                vis: input.vis,
                union_token: data.union_token,
                ident: input.ident,
                generics: input.generics,
                fields: data.fields,
            }),
        }
    }
}

impl From<ItemStruct> for DeriveInput {
    fn from(input: ItemStruct) -> DeriveInput {
        DeriveInput {
            attrs: input.attrs,
            vis: input.vis,
            ident: input.ident,
            generics: input.generics,
            data: Data::Struct(DataStruct {
                struct_token: input.struct_token,
                fields: input.fields,
                semi_token: input.semi_token,
            }),
        }
    }
}

impl From<ItemEnum> for DeriveInput {
    fn from(input: ItemEnum) -> DeriveInput {
        DeriveInput {
            attrs: input.attrs,
            vis: input.vis,
            ident: input.ident,
            generics: input.generics,
            data: Data::Enum(DataEnum {
                enum_token: input.enum_token,
                brace_token: input.brace_token,
                variants: input.variants,
            }),
        }
    }
}

impl From<ItemUnion> for DeriveInput {
    fn from(input: ItemUnion) -> DeriveInput {
        DeriveInput {
            attrs: input.attrs,
            vis: input.vis,
            ident: input.ident,
            generics: input.generics,
            data: Data::Union(DataUnion {
                union_token: input.union_token,
                fields: input.fields,
            }),
        }
    }
}

ast_enum_of_structs! {
    /// A suffix of an import tree in a `use` item: `Type as Renamed` or `*`.
    ///
    /// *This type is available only if Syn is built with the `"full"` feature.*
    ///
    /// # Syntax tree enum
    ///
    /// This type is a [syntax tree enum].
    ///
    /// [syntax tree enum]: Expr#syntax-tree-enums
    #[cfg_attr(doc_cfg, doc(cfg(feature = "full")))]
    pub enum UseTree {
        /// A path prefix of imports in a `use` item: `std::...`.
        Path(UsePath),

        /// An identifier imported by a `use` item: `HashMap`.
        Name(UseName),

        /// An renamed identifier imported by a `use` item: `HashMap as Map`.
        Rename(UseRename),

        /// A glob import in a `use` item: `*`.
        Glob(UseGlob),

        /// A braced group of imports in a `use` item: `{A, B, C}`.
        Group(UseGroup),
    }
}

ast_struct! {
    /// A path prefix of imports in a `use` item: `std::...`.
    ///
    /// *This type is available only if Syn is built with the `"full"` feature.*
    #[cfg_attr(doc_cfg, doc(cfg(feature = "full")))]
    pub struct UsePath {
        pub ident: Ident,
        pub colon2_token: Token![::],
        pub tree: Box<UseTree>,
    }
}

ast_struct! {
    /// An identifier imported by a `use` item: `HashMap`.
    ///
    /// *This type is available only if Syn is built with the `"full"` feature.*
    #[cfg_attr(doc_cfg, doc(cfg(feature = "full")))]
    pub struct UseName {
        pub ident: Ident,
    }
}

ast_struct! {
    /// An renamed identifier imported by a `use` item: `HashMap as Map`.
    ///
    /// *This type is available only if Syn is built with the `"full"` feature.*
    #[cfg_attr(doc_cfg, doc(cfg(feature = "full")))]
    pub struct UseRename {
        pub ident: Ident,
        pub as_token: Token![as],
        pub rename: Ident,
    }
}

ast_struct! {
    /// A glob import in a `use` item: `*`.
    ///
    /// *This type is available only if Syn is built with the `"full"` feature.*
    #[cfg_attr(doc_cfg, doc(cfg(feature = "full")))]
    pub struct UseGlob {
        pub star_token: Token![*],
    }
}

ast_struct! {
    /// A braced group of imports in a `use` item: `{A, B, C}`.
    ///
    /// *This type is available only if Syn is built with the `"full"` feature.*
    #[cfg_attr(doc_cfg, doc(cfg(feature = "full")))]
    pub struct UseGroup {
        pub brace_token: token::Brace,
        pub items: Punctuated<UseTree, Token![,]>,
    }
}

ast_enum_of_structs! {
    /// An item within an `extern` block.
    ///
    /// *This type is available only if Syn is built with the `"full"` feature.*
    ///
    /// # Syntax tree enum
    ///
    /// This type is a [syntax tree enum].
    ///
    /// [syntax tree enum]: Expr#syntax-tree-enums
    #[cfg_attr(doc_cfg, doc(cfg(feature = "full")))]
    #[cfg_attr(not(syn_no_non_exhaustive), non_exhaustive)]
    pub enum ForeignItem {
        /// A foreign function in an `extern` block.
        Fn(ForeignItemFn),

        /// A foreign static item in an `extern` block: `static ext: u8`.
        Static(ForeignItemStatic),

        /// A foreign type in an `extern` block: `type void`.
        Type(ForeignItemType),

        /// A macro invocation within an extern block.
        Macro(ForeignItemMacro),

        /// Tokens in an `extern` block not interpreted by Syn.
        Verbatim(TokenStream),

        // Not public API.
        //
        // For testing exhaustiveness in downstream code, use the following idiom:
        //
        //     match item {
        //         ForeignItem::Fn(item) => {...}
        //         ForeignItem::Static(item) => {...}
        //         ...
        //         ForeignItem::Verbatim(item) => {...}
        //
        //         #[cfg_attr(test, deny(non_exhaustive_omitted_patterns))]
        //         _ => { /* some sane fallback */ }
        //     }
        //
        // This way we fail your tests but don't break your library when adding
        // a variant. You will be notified by a test failure when a variant is
        // added, so that you can add code to handle it, but your library will
        // continue to compile and work for downstream users in the interim.
        #[cfg(syn_no_non_exhaustive)]
        #[doc(hidden)]
        __NonExhaustive,
    }
}

ast_struct! {
    /// A foreign function in an `extern` block.
    ///
    /// *This type is available only if Syn is built with the `"full"` feature.*
    #[cfg_attr(doc_cfg, doc(cfg(feature = "full")))]
    pub struct ForeignItemFn {
        pub attrs: Vec<Attribute>,
        pub vis: Visibility,
        pub sig: Signature,
        pub semi_token: Token![;],
    }
}

ast_struct! {
    /// A foreign static item in an `extern` block: `static ext: u8`.
    ///
    /// *This type is available only if Syn is built with the `"full"` feature.*
    #[cfg_attr(doc_cfg, doc(cfg(feature = "full")))]
    pub struct ForeignItemStatic {
        pub attrs: Vec<Attribute>,
        pub vis: Visibility,
        pub static_token: Token![static],
        pub mutability: Option<Token![mut]>,
        pub ident: Ident,
        pub colon_token: Token![:],
        pub ty: Box<Type>,
        pub semi_token: Token![;],
    }
}

ast_struct! {
    /// A foreign type in an `extern` block: `type void`.
    ///
    /// *This type is available only if Syn is built with the `"full"` feature.*
    #[cfg_attr(doc_cfg, doc(cfg(feature = "full")))]
    pub struct ForeignItemType {
        pub attrs: Vec<Attribute>,
        pub vis: Visibility,
        pub type_token: Token![type],
        pub ident: Ident,
        pub semi_token: Token![;],
    }
}

ast_struct! {
    /// A macro invocation within an extern block.
    ///
    /// *This type is available only if Syn is built with the `"full"` feature.*
    #[cfg_attr(doc_cfg, doc(cfg(feature = "full")))]
    pub struct ForeignItemMacro {
        pub attrs: Vec<Attribute>,
        pub mac: Macro,
        pub semi_token: Option<Token![;]>,
    }
}

ast_enum_of_structs! {
    /// An item declaration within the definition of a trait.
    ///
    /// *This type is available only if Syn is built with the `"full"` feature.*
    ///
    /// # Syntax tree enum
    ///
    /// This type is a [syntax tree enum].
    ///
    /// [syntax tree enum]: Expr#syntax-tree-enums
    #[cfg_attr(doc_cfg, doc(cfg(feature = "full")))]
    #[cfg_attr(not(syn_no_non_exhaustive), non_exhaustive)]
    pub enum TraitItem {
        /// An associated constant within the definition of a trait.
        Const(TraitItemConst),

        /// A trait method within the definition of a trait.
        Method(TraitItemMethod),

        /// An associated type within the definition of a trait.
        Type(TraitItemType),

        /// A macro invocation within the definition of a trait.
        Macro(TraitItemMacro),

        /// Tokens within the definition of a trait not interpreted by Syn.
        Verbatim(TokenStream),

        // Not public API.
        //
        // For testing exhaustiveness in downstream code, use the following idiom:
        //
        //     match item {
        //         TraitItem::Const(item) => {...}
        //         TraitItem::Method(item) => {...}
        //         ...
        //         TraitItem::Verbatim(item) => {...}
        //
        //         #[cfg_attr(test, deny(non_exhaustive_omitted_patterns))]
        //         _ => { /* some sane fallback */ }
        //     }
        //
        // This way we fail your tests but don't break your library when adding
        // a variant. You will be notified by a test failure when a variant is
        // added, so that you can add code to handle it, but your library will
        // continue to compile and work for downstream users in the interim.
        #[cfg(syn_no_non_exhaustive)]
        #[doc(hidden)]
        __NonExhaustive,
    }
}

ast_struct! {
    /// An associated constant within the definition of a trait.
    ///
    /// *This type is available only if Syn is built with the `"full"` feature.*
    #[cfg_attr(doc_cfg, doc(cfg(feature = "full")))]
    pub struct TraitItemConst {
        pub attrs: Vec<Attribute>,
        pub const_token: Token![const],
        pub ident: Ident,
        pub colon_token: Token![:],
        pub ty: Type,
        pub default: Option<(Token![=], Expr)>,
        pub semi_token: Token![;],
    }
}

ast_struct! {
    /// A trait method within the definition of a trait.
    ///
    /// *This type is available only if Syn is built with the `"full"` feature.*
    #[cfg_attr(doc_cfg, doc(cfg(feature = "full")))]
    pub struct TraitItemMethod {
        pub attrs: Vec<Attribute>,
        pub sig: Signature,
        pub default: Option<Block>,
        pub semi_token: Option<Token![;]>,
    }
}

ast_struct! {
    /// An associated type within the definition of a trait.
    ///
    /// *This type is available only if Syn is built with the `"full"` feature.*
    #[cfg_attr(doc_cfg, doc(cfg(feature = "full")))]
    pub struct TraitItemType {
        pub attrs: Vec<Attribute>,
        pub type_token: Token![type],
        pub ident: Ident,
        pub generics: Generics,
        pub colon_token: Option<Token![:]>,
        pub bounds: Punctuated<TypeParamBound, Token![+]>,
        pub default: Option<(Token![=], Type)>,
        pub semi_token: Token![;],
    }
}

ast_struct! {
    /// A macro invocation within the definition of a trait.
    ///
    /// *This type is available only if Syn is built with the `"full"` feature.*
    #[cfg_attr(doc_cfg, doc(cfg(feature = "full")))]
    pub struct TraitItemMacro {
        pub attrs: Vec<Attribute>,
        pub mac: Macro,
        pub semi_token: Option<Token![;]>,
    }
}

ast_enum_of_structs! {
    /// An item within an impl block.
    ///
    /// *This type is available only if Syn is built with the `"full"` feature.*
    ///
    /// # Syntax tree enum
    ///
    /// This type is a [syntax tree enum].
    ///
    /// [syntax tree enum]: Expr#syntax-tree-enums
    #[cfg_attr(doc_cfg, doc(cfg(feature = "full")))]
    #[cfg_attr(not(syn_no_non_exhaustive), non_exhaustive)]
    pub enum ImplItem {
        /// An associated constant within an impl block.
        Const(ImplItemConst),

        /// A method within an impl block.
        Method(ImplItemMethod),

        /// An associated type within an impl block.
        Type(ImplItemType),

        /// A macro invocation within an impl block.
        Macro(ImplItemMacro),

        /// Tokens within an impl block not interpreted by Syn.
        Verbatim(TokenStream),

        // Not public API.
        //
        // For testing exhaustiveness in downstream code, use the following idiom:
        //
        //     match item {
        //         ImplItem::Const(item) => {...}
        //         ImplItem::Method(item) => {...}
        //         ...
        //         ImplItem::Verbatim(item) => {...}
        //
        //         #[cfg_attr(test, deny(non_exhaustive_omitted_patterns))]
        //         _ => { /* some sane fallback */ }
        //     }
        //
        // This way we fail your tests but don't break your library when adding
        // a variant. You will be notified by a test failure when a variant is
        // added, so that you can add code to handle it, but your library will
        // continue to compile and work for downstream users in the interim.
        #[cfg(syn_no_non_exhaustive)]
        #[doc(hidden)]
        __NonExhaustive,
    }
}

ast_struct! {
    /// An associated constant within an impl block.
    ///
    /// *This type is available only if Syn is built with the `"full"` feature.*
    #[cfg_attr(doc_cfg, doc(cfg(feature = "full")))]
    pub struct ImplItemConst {
        pub attrs: Vec<Attribute>,
        pub vis: Visibility,
        pub defaultness: Option<Token![default]>,
        pub const_token: Token![const],
        pub ident: Ident,
        pub colon_token: Token![:],
        pub ty: Type,
        pub eq_token: Token![=],
        pub expr: Expr,
        pub semi_token: Token![;],
    }
}

ast_struct! {
    /// A method within an impl block.
    ///
    /// *This type is available only if Syn is built with the `"full"` feature.*
    #[cfg_attr(doc_cfg, doc(cfg(feature = "full")))]
    pub struct ImplItemMethod {
        pub attrs: Vec<Attribute>,
        pub vis: Visibility,
        pub defaultness: Option<Token![default]>,
        pub sig: Signature,
        pub block: Block,
    }
}

ast_struct! {
    /// An associated type within an impl block.
    ///
    /// *This type is available only if Syn is built with the `"full"` feature.*
    #[cfg_attr(doc_cfg, doc(cfg(feature = "full")))]
    pub struct ImplItemType {
        pub attrs: Vec<Attribute>,
        pub vis: Visibility,
        pub defaultness: Option<Token![default]>,
        pub type_token: Token![type],
        pub ident: Ident,
        pub generics: Generics,
        pub eq_token: Token![=],
        pub ty: Type,
        pub semi_token: Token![;],
    }
}

ast_struct! {
    /// A macro invocation within an impl block.
    ///
    /// *This type is available only if Syn is built with the `"full"` feature.*
    #[cfg_attr(doc_cfg, doc(cfg(feature = "full")))]
    pub struct ImplItemMacro {
        pub attrs: Vec<Attribute>,
        pub mac: Macro,
        pub semi_token: Option<Token![;]>,
    }
}

ast_struct! {
    /// A function signature in a trait or implementation: `unsafe fn
    /// initialize(&self)`.
    ///
    /// *This type is available only if Syn is built with the `"full"` feature.*
    #[cfg_attr(doc_cfg, doc(cfg(feature = "full")))]
    pub struct Signature {
        pub constness: Option<Token![const]>,
        pub asyncness: Option<Token![async]>,
        pub unsafety: Option<Token![unsafe]>,
        pub abi: Option<Abi>,
        pub fn_token: Token![fn],
        pub ident: Ident,
        pub generics: Generics,
        pub paren_token: token::Paren,
        pub inputs: Punctuated<FnArg, Token![,]>,
        pub variadic: Option<Variadic>,
        pub output: ReturnType,
    }
}

impl Signature {
    /// A method's `self` receiver, such as `&self` or `self: Box<Self>`.
    pub fn receiver(&self) -> Option<&FnArg> {
        let arg = self.inputs.first()?;
        match arg {
            FnArg::Receiver(_) => Some(arg),
            FnArg::Typed(PatType { pat, .. }) => {
                if let Pat::Ident(PatIdent { ident, .. }) = &**pat {
                    if ident == "self" {
                        return Some(arg);
                    }
                }
                None
            }
        }
    }
}

ast_enum_of_structs! {
    /// An argument in a function signature: the `n: usize` in `fn f(n: usize)`.
    ///
    /// *This type is available only if Syn is built with the `"full"` feature.*
    #[cfg_attr(doc_cfg, doc(cfg(feature = "full")))]
    pub enum FnArg {
        /// The `self` argument of an associated method, whether taken by value
        /// or by reference.
        ///
        /// Note that `self` receivers with a specified type, such as `self:
        /// Box<Self>`, are parsed as a `FnArg::Typed`.
        Receiver(Receiver),

        /// A function argument accepted by pattern and type.
        Typed(PatType),
    }
}

ast_struct! {
    /// The `self` argument of an associated method, whether taken by value
    /// or by reference.
    ///
    /// Note that `self` receivers with a specified type, such as `self:
    /// Box<Self>`, are parsed as a `FnArg::Typed`.
    ///
    /// *This type is available only if Syn is built with the `"full"` feature.*
    #[cfg_attr(doc_cfg, doc(cfg(feature = "full")))]
    pub struct Receiver {
        pub attrs: Vec<Attribute>,
        pub reference: Option<(Token![&], Option<Lifetime>)>,
        pub mutability: Option<Token![mut]>,
        pub self_token: Token![self],
    }
}

impl Receiver {
    pub fn lifetime(&self) -> Option<&Lifetime> {
        self.reference.as_ref()?.1.as_ref()
    }
}

#[cfg(feature = "parsing")]
pub mod parsing {
    use super::*;
    use crate::ext::IdentExt;
    use crate::parse::discouraged::Speculative;
    use crate::parse::{Parse, ParseBuffer, ParseStream, Result};
    use crate::token::Brace;
    use proc_macro2::{Delimiter, Group, Punct, Spacing, TokenTree};
    use std::iter::{self, FromIterator};

    crate::custom_keyword!(macro_rules);

    #[cfg_attr(doc_cfg, doc(cfg(feature = "parsing")))]
    impl Parse for Item {
        fn parse(input: ParseStream) -> Result<Self> {
            let begin = input.fork();
            let mut attrs = input.call(Attribute::parse_outer)?;
            let ahead = input.fork();
            let vis: Visibility = ahead.parse()?;

            let lookahead = ahead.lookahead1();
            let mut item = if lookahead.peek(Token![fn]) || peek_signature(&ahead) {
                let vis: Visibility = input.parse()?;
                let sig: Signature = input.parse()?;
                if input.peek(Token![;]) {
                    input.parse::<Token![;]>()?;
                    Ok(Item::Verbatim(verbatim::between(begin, input)))
                } else {
                    parse_rest_of_fn(input, Vec::new(), vis, sig).map(Item::Fn)
                }
            } else if lookahead.peek(Token![extern]) {
                ahead.parse::<Token![extern]>()?;
                let lookahead = ahead.lookahead1();
                if lookahead.peek(Token![crate]) {
                    input.parse().map(Item::ExternCrate)
                } else if lookahead.peek(token::Brace) {
                    input.parse().map(Item::ForeignMod)
                } else if lookahead.peek(LitStr) {
                    ahead.parse::<LitStr>()?;
                    let lookahead = ahead.lookahead1();
                    if lookahead.peek(token::Brace) {
                        input.parse().map(Item::ForeignMod)
                    } else {
                        Err(lookahead.error())
                    }
                } else {
                    Err(lookahead.error())
                }
            } else if lookahead.peek(Token![use]) {
                input.parse().map(Item::Use)
            } else if lookahead.peek(Token![static]) {
                let vis = input.parse()?;
                let static_token = input.parse()?;
                let mutability = input.parse()?;
                let ident = input.parse()?;
                if input.peek(Token![=]) {
                    input.parse::<Token![=]>()?;
                    input.parse::<Expr>()?;
                    input.parse::<Token![;]>()?;
                    Ok(Item::Verbatim(verbatim::between(begin, input)))
                } else {
                    let colon_token = input.parse()?;
                    let ty = input.parse()?;
                    if input.peek(Token![;]) {
                        input.parse::<Token![;]>()?;
                        Ok(Item::Verbatim(verbatim::between(begin, input)))
                    } else {
                        Ok(Item::Static(ItemStatic {
                            attrs: Vec::new(),
                            vis,
                            static_token,
                            mutability,
                            ident,
                            colon_token,
                            ty,
                            eq_token: input.parse()?,
                            expr: input.parse()?,
                            semi_token: input.parse()?,
                        }))
                    }
                }
            } else if lookahead.peek(Token![const]) {
                ahead.parse::<Token![const]>()?;
                let lookahead = ahead.lookahead1();
                if lookahead.peek(Ident) || lookahead.peek(Token![_]) {
                    let vis = input.parse()?;
                    let const_token = input.parse()?;
                    let ident = {
                        let lookahead = input.lookahead1();
                        if lookahead.peek(Ident) || lookahead.peek(Token![_]) {
                            input.call(Ident::parse_any)?
                        } else {
                            return Err(lookahead.error());
                        }
                    };
                    let colon_token = input.parse()?;
                    let ty = input.parse()?;
                    if input.peek(Token![;]) {
                        input.parse::<Token![;]>()?;
                        Ok(Item::Verbatim(verbatim::between(begin, input)))
                    } else {
                        Ok(Item::Const(ItemConst {
                            attrs: Vec::new(),
                            vis,
                            const_token,
                            ident,
                            colon_token,
                            ty,
                            eq_token: input.parse()?,
                            expr: input.parse()?,
                            semi_token: input.parse()?,
                        }))
                    }
                } else {
                    Err(lookahead.error())
                }
            } else if lookahead.peek(Token![unsafe]) {
                ahead.parse::<Token![unsafe]>()?;
                let lookahead = ahead.lookahead1();
                if lookahead.peek(Token![trait])
                    || lookahead.peek(Token![auto]) && ahead.peek2(Token![trait])
                {
                    input.parse().map(Item::Trait)
                } else if lookahead.peek(Token![impl]) {
                    let allow_verbatim_impl = true;
                    if let Some(item) = parse_impl(input, allow_verbatim_impl)? {
                        Ok(Item::Impl(item))
                    } else {
                        Ok(Item::Verbatim(verbatim::between(begin, input)))
                    }
                } else if lookahead.peek(Token![extern]) {
                    input.parse::<Visibility>()?;
                    input.parse::<Token![unsafe]>()?;
                    input.parse::<ItemForeignMod>()?;
                    Ok(Item::Verbatim(verbatim::between(begin, input)))
                } else if lookahead.peek(Token![mod]) {
                    input.parse::<Visibility>()?;
                    input.parse::<Token![unsafe]>()?;
                    input.parse::<ItemMod>()?;
                    Ok(Item::Verbatim(verbatim::between(begin, input)))
                } else {
                    Err(lookahead.error())
                }
            } else if lookahead.peek(Token![mod]) {
                input.parse().map(Item::Mod)
            } else if lookahead.peek(Token![type]) {
                parse_item_type(begin, input)
            } else if lookahead.peek(Token![struct]) {
                input.parse().map(Item::Struct)
            } else if lookahead.peek(Token![enum]) {
                input.parse().map(Item::Enum)
            } else if lookahead.peek(Token![union]) && ahead.peek2(Ident) {
                input.parse().map(Item::Union)
            } else if lookahead.peek(Token![trait]) {
                input.call(parse_trait_or_trait_alias)
            } else if lookahead.peek(Token![auto]) && ahead.peek2(Token![trait]) {
                input.parse().map(Item::Trait)
            } else if lookahead.peek(Token![impl])
                || lookahead.peek(Token![default]) && !ahead.peek2(Token![!])
            {
                let allow_verbatim_impl = true;
                if let Some(item) = parse_impl(input, allow_verbatim_impl)? {
                    Ok(Item::Impl(item))
                } else {
                    Ok(Item::Verbatim(verbatim::between(begin, input)))
                }
            } else if lookahead.peek(Token![macro]) {
                input.parse().map(Item::Macro2)
            } else if vis.is_inherited()
                && (lookahead.peek(Ident)
                    || lookahead.peek(Token![self])
                    || lookahead.peek(Token![super])
                    || lookahead.peek(Token![crate])
                    || lookahead.peek(Token![::]))
            {
                input.parse().map(Item::Macro)
            } else if ahead.peek(macro_rules) {
                input.advance_to(&ahead);
                input.parse::<ItemMacro>()?;
                Ok(Item::Verbatim(verbatim::between(begin, input)))
            } else {
                Err(lookahead.error())
            }?;

            attrs.extend(item.replace_attrs(Vec::new()));
            item.replace_attrs(attrs);
            Ok(item)
        }
    }

    struct FlexibleItemType {
        vis: Visibility,
        defaultness: Option<Token![default]>,
        type_token: Token![type],
        ident: Ident,
        generics: Generics,
        colon_token: Option<Token![:]>,
        bounds: Punctuated<TypeParamBound, Token![+]>,
        ty: Option<(Token![=], Type)>,
        semi_token: Token![;],
    }

    enum WhereClauseLocation {
        // type Ty<T> where T: 'static = T;
        BeforeEq,
        // type Ty<T> = T where T: 'static;
        #[allow(dead_code)]
        AfterEq,
        // TODO: goes away once the migration period on rust-lang/rust#89122 is over
        Both,
    }

    impl FlexibleItemType {
        fn parse(input: ParseStream, where_clause_location: WhereClauseLocation) -> Result<Self> {
            let vis: Visibility = input.parse()?;
            let defaultness: Option<Token![default]> = input.parse()?;
            let type_token: Token![type] = input.parse()?;
            let ident: Ident = input.parse()?;
            let mut generics: Generics = input.parse()?;
            let colon_token: Option<Token![:]> = input.parse()?;

            let mut bounds = Punctuated::new();
            if colon_token.is_some() {
                loop {
                    if input.peek(Token![where]) || input.peek(Token![=]) || input.peek(Token![;]) {
                        break;
                    }
                    bounds.push_value(input.parse::<TypeParamBound>()?);
                    if input.peek(Token![where]) || input.peek(Token![=]) || input.peek(Token![;]) {
                        break;
                    }
                    bounds.push_punct(input.parse::<Token![+]>()?);
                }
            }

            match where_clause_location {
                WhereClauseLocation::BeforeEq | WhereClauseLocation::Both => {
                    generics.where_clause = input.parse()?;
                }
                _ => {}
            }

            let ty = if let Some(eq_token) = input.parse()? {
                Some((eq_token, input.parse::<Type>()?))
            } else {
                None
            };

            match where_clause_location {
                WhereClauseLocation::AfterEq | WhereClauseLocation::Both
                    if generics.where_clause.is_none() =>
                {
                    generics.where_clause = input.parse()?;
                }
                _ => {}
            }

            let semi_token: Token![;] = input.parse()?;

            Ok(FlexibleItemType {
                vis,
                defaultness,
                type_token,
                ident,
                generics,
                colon_token,
                bounds,
                ty,
                semi_token,
            })
        }
    }

    #[cfg_attr(doc_cfg, doc(cfg(feature = "parsing")))]
    impl Parse for ItemMacro {
        fn parse(input: ParseStream) -> Result<Self> {
            let attrs = input.call(Attribute::parse_outer)?;
            let path = input.call(Path::parse_mod_style)?;
            let bang_token: Token![!] = input.parse()?;
            let ident: Option<Ident> = input.parse()?;
            let (delimiter, tokens) = input.call(mac::parse_delimiter)?;
            let semi_token: Option<Token![;]> = if !delimiter.is_brace() {
                Some(input.parse()?)
            } else {
                None
            };
            Ok(ItemMacro {
                attrs,
                ident,
                mac: Macro {
                    path,
                    bang_token,
                    delimiter,
                    tokens,
                },
                semi_token,
            })
        }
    }

    #[cfg_attr(doc_cfg, doc(cfg(feature = "parsing")))]
    impl Parse for ItemMacro2 {
        fn parse(input: ParseStream) -> Result<Self> {
            let attrs = input.call(Attribute::parse_outer)?;
            let vis: Visibility = input.parse()?;
            let macro_token: Token![macro] = input.parse()?;
            let ident: Ident = input.parse()?;
            let mut rules = TokenStream::new();

            let mut lookahead = input.lookahead1();
            if lookahead.peek(token::Paren) {
                let paren_content;
                let paren_token = parenthesized!(paren_content in input);
                let args: TokenStream = paren_content.parse()?;
                let mut args = Group::new(Delimiter::Parenthesis, args);
                args.set_span(paren_token.span);
                rules.extend(iter::once(TokenTree::Group(args)));
                lookahead = input.lookahead1();
            }

            if lookahead.peek(token::Brace) {
                let brace_content;
                let brace_token = braced!(brace_content in input);
                let body: TokenStream = brace_content.parse()?;
                let mut body = Group::new(Delimiter::Brace, body);
                body.set_span(brace_token.span);
                rules.extend(iter::once(TokenTree::Group(body)));
            } else {
                return Err(lookahead.error());
            }

            Ok(ItemMacro2 {
                attrs,
                vis,
                macro_token,
                ident,
                rules,
            })
        }
    }

    #[cfg_attr(doc_cfg, doc(cfg(feature = "parsing")))]
    impl Parse for ItemExternCrate {
        fn parse(input: ParseStream) -> Result<Self> {
            Ok(ItemExternCrate {
                attrs: input.call(Attribute::parse_outer)?,
                vis: input.parse()?,
                extern_token: input.parse()?,
                crate_token: input.parse()?,
                ident: {
                    if input.peek(Token![self]) {
                        input.call(Ident::parse_any)?
                    } else {
                        input.parse()?
                    }
                },
                rename: {
                    if input.peek(Token![as]) {
                        let as_token: Token![as] = input.parse()?;
                        let rename: Ident = if input.peek(Token![_]) {
                            Ident::from(input.parse::<Token![_]>()?)
                        } else {
                            input.parse()?
                        };
                        Some((as_token, rename))
                    } else {
                        None
                    }
                },
                semi_token: input.parse()?,
            })
        }
    }

    #[cfg_attr(doc_cfg, doc(cfg(feature = "parsing")))]
    impl Parse for ItemUse {
        fn parse(input: ParseStream) -> Result<Self> {
            Ok(ItemUse {
                attrs: input.call(Attribute::parse_outer)?,
                vis: input.parse()?,
                use_token: input.parse()?,
                leading_colon: input.parse()?,
                tree: input.parse()?,
                semi_token: input.parse()?,
            })
        }
    }

    #[cfg_attr(doc_cfg, doc(cfg(feature = "parsing")))]
    impl Parse for UseTree {
        fn parse(input: ParseStream) -> Result<UseTree> {
            let lookahead = input.lookahead1();
            if lookahead.peek(Ident)
                || lookahead.peek(Token![self])
                || lookahead.peek(Token![super])
                || lookahead.peek(Token![crate])
            {
                let ident = input.call(Ident::parse_any)?;
                if input.peek(Token![::]) {
                    Ok(UseTree::Path(UsePath {
                        ident,
                        colon2_token: input.parse()?,
                        tree: Box::new(input.parse()?),
                    }))
                } else if input.peek(Token![as]) {
                    Ok(UseTree::Rename(UseRename {
                        ident,
                        as_token: input.parse()?,
                        rename: {
                            if input.peek(Ident) {
                                input.parse()?
                            } else if input.peek(Token![_]) {
                                Ident::from(input.parse::<Token![_]>()?)
                            } else {
                                return Err(input.error("expected identifier or underscore"));
                            }
                        },
                    }))
                } else {
                    Ok(UseTree::Name(UseName { ident }))
                }
            } else if lookahead.peek(Token![*]) {
                Ok(UseTree::Glob(UseGlob {
                    star_token: input.parse()?,
                }))
            } else if lookahead.peek(token::Brace) {
                let content;
                Ok(UseTree::Group(UseGroup {
                    brace_token: braced!(content in input),
                    items: content.parse_terminated(UseTree::parse)?,
                }))
            } else {
                Err(lookahead.error())
            }
        }
    }

    #[cfg_attr(doc_cfg, doc(cfg(feature = "parsing")))]
    impl Parse for ItemStatic {
        fn parse(input: ParseStream) -> Result<Self> {
            Ok(ItemStatic {
                attrs: input.call(Attribute::parse_outer)?,
                vis: input.parse()?,
                static_token: input.parse()?,
                mutability: input.parse()?,
                ident: input.parse()?,
                colon_token: input.parse()?,
                ty: input.parse()?,
                eq_token: input.parse()?,
                expr: input.parse()?,
                semi_token: input.parse()?,
            })
        }
    }

    #[cfg_attr(doc_cfg, doc(cfg(feature = "parsing")))]
    impl Parse for ItemConst {
        fn parse(input: ParseStream) -> Result<Self> {
            Ok(ItemConst {
                attrs: input.call(Attribute::parse_outer)?,
                vis: input.parse()?,
                const_token: input.parse()?,
                ident: {
                    let lookahead = input.lookahead1();
                    if lookahead.peek(Ident) || lookahead.peek(Token![_]) {
                        input.call(Ident::parse_any)?
                    } else {
                        return Err(lookahead.error());
                    }
                },
                colon_token: input.parse()?,
                ty: input.parse()?,
                eq_token: input.parse()?,
                expr: input.parse()?,
                semi_token: input.parse()?,
            })
        }
    }

    fn pop_variadic(args: &mut Punctuated<FnArg, Token![,]>) -> Option<Variadic> {
        let trailing_punct = args.trailing_punct();

        let last = match args.last_mut()? {
            FnArg::Typed(last) => last,
            _ => return None,
        };

        let ty = match last.ty.as_ref() {
            Type::Verbatim(ty) => ty,
            _ => return None,
        };

        let mut variadic = Variadic {
            attrs: Vec::new(),
            dots: parse2(ty.clone()).ok()?,
        };

        if let Pat::Verbatim(pat) = last.pat.as_ref() {
            if pat.to_string() == "..." && !trailing_punct {
                variadic.attrs = mem::replace(&mut last.attrs, Vec::new());
                args.pop();
            }
        }

        Some(variadic)
    }

    fn variadic_to_tokens(dots: &Token![...]) -> TokenStream {
        TokenStream::from_iter(vec![
            TokenTree::Punct({
                let mut dot = Punct::new('.', Spacing::Joint);
                dot.set_span(dots.spans[0]);
                dot
            }),
            TokenTree::Punct({
                let mut dot = Punct::new('.', Spacing::Joint);
                dot.set_span(dots.spans[1]);
                dot
            }),
            TokenTree::Punct({
                let mut dot = Punct::new('.', Spacing::Alone);
                dot.set_span(dots.spans[2]);
                dot
            }),
        ])
    }

    fn peek_signature(input: ParseStream) -> bool {
        let fork = input.fork();
        fork.parse::<Option<Token![const]>>().is_ok()
            && fork.parse::<Option<Token![async]>>().is_ok()
            && fork.parse::<Option<Token![unsafe]>>().is_ok()
            && fork.parse::<Option<Abi>>().is_ok()
            && fork.peek(Token![fn])
    }

    #[cfg_attr(doc_cfg, doc(cfg(feature = "parsing")))]
    impl Parse for Signature {
        fn parse(input: ParseStream) -> Result<Self> {
            let constness: Option<Token![const]> = input.parse()?;
            let asyncness: Option<Token![async]> = input.parse()?;
            let unsafety: Option<Token![unsafe]> = input.parse()?;
            let abi: Option<Abi> = input.parse()?;
            let fn_token: Token![fn] = input.parse()?;
            let ident: Ident = input.parse()?;
            let mut generics: Generics = input.parse()?;

            let content;
            let paren_token = parenthesized!(content in input);
            let mut inputs = parse_fn_args(&content)?;
            let variadic = pop_variadic(&mut inputs);

            let output: ReturnType = input.parse()?;
            generics.where_clause = input.parse()?;

            Ok(Signature {
                constness,
                asyncness,
                unsafety,
                abi,
                fn_token,
                ident,
                generics,
                paren_token,
                inputs,
                variadic,
                output,
            })
        }
    }

    #[cfg_attr(doc_cfg, doc(cfg(feature = "parsing")))]
    impl Parse for ItemFn {
        fn parse(input: ParseStream) -> Result<Self> {
            let outer_attrs = input.call(Attribute::parse_outer)?;
            let vis: Visibility = input.parse()?;
            let sig: Signature = input.parse()?;
            parse_rest_of_fn(input, outer_attrs, vis, sig)
        }
    }

    fn parse_rest_of_fn(
        input: ParseStream,
        mut attrs: Vec<Attribute>,
        vis: Visibility,
        sig: Signature,
    ) -> Result<ItemFn> {
        let content;
        let brace_token = braced!(content in input);
        attr::parsing::parse_inner(&content, &mut attrs)?;
        let stmts = content.call(Block::parse_within)?;

        Ok(ItemFn {
            attrs,
            vis,
            sig,
            block: Box::new(Block { brace_token, stmts }),
        })
    }

    #[cfg_attr(doc_cfg, doc(cfg(feature = "parsing")))]
    impl Parse for FnArg {
        fn parse(input: ParseStream) -> Result<Self> {
            let attrs = input.call(Attribute::parse_outer)?;

            let ahead = input.fork();
            if let Ok(mut receiver) = ahead.parse::<Receiver>() {
                if !ahead.peek(Token![:]) {
                    input.advance_to(&ahead);
                    receiver.attrs = attrs;
                    return Ok(FnArg::Receiver(receiver));
                }
            }

            let mut typed = input.call(fn_arg_typed)?;
            typed.attrs = attrs;
            Ok(FnArg::Typed(typed))
        }
    }

    #[cfg_attr(doc_cfg, doc(cfg(feature = "parsing")))]
    impl Parse for Receiver {
        fn parse(input: ParseStream) -> Result<Self> {
            Ok(Receiver {
                attrs: Vec::new(),
                reference: {
                    if input.peek(Token![&]) {
                        Some((input.parse()?, input.parse()?))
                    } else {
                        None
                    }
                },
                mutability: input.parse()?,
                self_token: input.parse()?,
            })
        }
    }

    fn parse_fn_args(input: ParseStream) -> Result<Punctuated<FnArg, Token![,]>> {
        let mut args = Punctuated::new();
        let mut has_receiver = false;

        while !input.is_empty() {
            let attrs = input.call(Attribute::parse_outer)?;

            let arg = if let Some(dots) = input.parse::<Option<Token![...]>>()? {
                FnArg::Typed(PatType {
                    attrs,
                    pat: Box::new(Pat::Verbatim(variadic_to_tokens(&dots))),
                    colon_token: Token![:](dots.spans[0]),
                    ty: Box::new(Type::Verbatim(variadic_to_tokens(&dots))),
                })
            } else {
                let mut arg: FnArg = input.parse()?;
                match &mut arg {
                    FnArg::Receiver(receiver) if has_receiver => {
                        return Err(Error::new(
                            receiver.self_token.span,
                            "unexpected second method receiver",
                        ));
                    }
                    FnArg::Receiver(receiver) if !args.is_empty() => {
                        return Err(Error::new(
                            receiver.self_token.span,
                            "unexpected method receiver",
                        ));
                    }
                    FnArg::Receiver(receiver) => {
                        has_receiver = true;
                        receiver.attrs = attrs;
                    }
                    FnArg::Typed(arg) => arg.attrs = attrs,
                }
                arg
            };
            args.push_value(arg);

            if input.is_empty() {
                break;
            }

            let comma: Token![,] = input.parse()?;
            args.push_punct(comma);
        }

        Ok(args)
    }

    fn fn_arg_typed(input: ParseStream) -> Result<PatType> {
        // Hack to parse pre-2018 syntax in
        // test/ui/rfc-2565-param-attrs/param-attrs-pretty.rs
        // because the rest of the test case is valuable.
        if input.peek(Ident) && input.peek2(Token![<]) {
            let span = input.fork().parse::<Ident>()?.span();
            return Ok(PatType {
                attrs: Vec::new(),
                pat: Box::new(Pat::Wild(PatWild {
                    attrs: Vec::new(),
                    underscore_token: Token![_](span),
                })),
                colon_token: Token![:](span),
                ty: input.parse()?,
            });
        }

        Ok(PatType {
            attrs: Vec::new(),
            pat: Box::new(pat::parsing::multi_pat(input)?),
            colon_token: input.parse()?,
            ty: Box::new(match input.parse::<Option<Token![...]>>()? {
                Some(dot3) => Type::Verbatim(variadic_to_tokens(&dot3)),
                None => input.parse()?,
            }),
        })
    }

    #[cfg_attr(doc_cfg, doc(cfg(feature = "parsing")))]
    impl Parse for ItemMod {
        fn parse(input: ParseStream) -> Result<Self> {
            let mut attrs = input.call(Attribute::parse_outer)?;
            let vis: Visibility = input.parse()?;
            let mod_token: Token![mod] = input.parse()?;
            let ident: Ident = input.parse()?;

            let lookahead = input.lookahead1();
            if lookahead.peek(Token![;]) {
                Ok(ItemMod {
                    attrs,
                    vis,
                    mod_token,
                    ident,
                    content: None,
                    semi: Some(input.parse()?),
                })
            } else if lookahead.peek(token::Brace) {
                let content;
                let brace_token = braced!(content in input);
                attr::parsing::parse_inner(&content, &mut attrs)?;

                let mut items = Vec::new();
                while !content.is_empty() {
                    items.push(content.parse()?);
                }

                Ok(ItemMod {
                    attrs,
                    vis,
                    mod_token,
                    ident,
                    content: Some((brace_token, items)),
                    semi: None,
                })
            } else {
                Err(lookahead.error())
            }
        }
    }

    #[cfg_attr(doc_cfg, doc(cfg(feature = "parsing")))]
    impl Parse for ItemForeignMod {
        fn parse(input: ParseStream) -> Result<Self> {
            let mut attrs = input.call(Attribute::parse_outer)?;
            let abi: Abi = input.parse()?;

            let content;
            let brace_token = braced!(content in input);
            attr::parsing::parse_inner(&content, &mut attrs)?;
            let mut items = Vec::new();
            while !content.is_empty() {
                items.push(content.parse()?);
            }

            Ok(ItemForeignMod {
                attrs,
                abi,
                brace_token,
                items,
            })
        }
    }

    #[cfg_attr(doc_cfg, doc(cfg(feature = "parsing")))]
    impl Parse for ForeignItem {
        fn parse(input: ParseStream) -> Result<Self> {
            let begin = input.fork();
            let mut attrs = input.call(Attribute::parse_outer)?;
            let ahead = input.fork();
            let vis: Visibility = ahead.parse()?;

            let lookahead = ahead.lookahead1();
            let mut item = if lookahead.peek(Token![fn]) || peek_signature(&ahead) {
                let vis: Visibility = input.parse()?;
                let sig: Signature = input.parse()?;
                if input.peek(token::Brace) {
                    let content;
                    braced!(content in input);
                    content.call(Attribute::parse_inner)?;
                    content.call(Block::parse_within)?;

                    Ok(ForeignItem::Verbatim(verbatim::between(begin, input)))
                } else {
                    Ok(ForeignItem::Fn(ForeignItemFn {
                        attrs: Vec::new(),
                        vis,
                        sig,
                        semi_token: input.parse()?,
                    }))
                }
            } else if lookahead.peek(Token![static]) {
                let vis = input.parse()?;
                let static_token = input.parse()?;
                let mutability = input.parse()?;
                let ident = input.parse()?;
                let colon_token = input.parse()?;
                let ty = input.parse()?;
                if input.peek(Token![=]) {
                    input.parse::<Token![=]>()?;
                    input.parse::<Expr>()?;
                    input.parse::<Token![;]>()?;
                    Ok(ForeignItem::Verbatim(verbatim::between(begin, input)))
                } else {
                    Ok(ForeignItem::Static(ForeignItemStatic {
                        attrs: Vec::new(),
                        vis,
                        static_token,
                        mutability,
                        ident,
                        colon_token,
                        ty,
                        semi_token: input.parse()?,
                    }))
                }
            } else if lookahead.peek(Token![type]) {
                parse_foreign_item_type(begin, input)
            } else if vis.is_inherited()
                && (lookahead.peek(Ident)
                    || lookahead.peek(Token![self])
                    || lookahead.peek(Token![super])
                    || lookahead.peek(Token![crate])
                    || lookahead.peek(Token![::]))
            {
                input.parse().map(ForeignItem::Macro)
            } else {
                Err(lookahead.error())
            }?;

            let item_attrs = match &mut item {
                ForeignItem::Fn(item) => &mut item.attrs,
                ForeignItem::Static(item) => &mut item.attrs,
                ForeignItem::Type(item) => &mut item.attrs,
                ForeignItem::Macro(item) => &mut item.attrs,
                ForeignItem::Verbatim(_) => return Ok(item),

                #[cfg(syn_no_non_exhaustive)]
                _ => unreachable!(),
            };
            attrs.append(item_attrs);
            *item_attrs = attrs;

            Ok(item)
        }
    }

    #[cfg_attr(doc_cfg, doc(cfg(feature = "parsing")))]
    impl Parse for ForeignItemFn {
        fn parse(input: ParseStream) -> Result<Self> {
            let attrs = input.call(Attribute::parse_outer)?;
            let vis: Visibility = input.parse()?;
            let sig: Signature = input.parse()?;
            let semi_token: Token![;] = input.parse()?;
            Ok(ForeignItemFn {
                attrs,
                vis,
                sig,
                semi_token,
            })
        }
    }

    #[cfg_attr(doc_cfg, doc(cfg(feature = "parsing")))]
    impl Parse for ForeignItemStatic {
        fn parse(input: ParseStream) -> Result<Self> {
            Ok(ForeignItemStatic {
                attrs: input.call(Attribute::parse_outer)?,
                vis: input.parse()?,
                static_token: input.parse()?,
                mutability: input.parse()?,
                ident: input.parse()?,
                colon_token: input.parse()?,
                ty: input.parse()?,
                semi_token: input.parse()?,
            })
        }
    }

    #[cfg_attr(doc_cfg, doc(cfg(feature = "parsing")))]
    impl Parse for ForeignItemType {
        fn parse(input: ParseStream) -> Result<Self> {
            Ok(ForeignItemType {
                attrs: input.call(Attribute::parse_outer)?,
                vis: input.parse()?,
                type_token: input.parse()?,
                ident: input.parse()?,
                semi_token: input.parse()?,
            })
        }
    }

    fn parse_foreign_item_type(begin: ParseBuffer, input: ParseStream) -> Result<ForeignItem> {
        let FlexibleItemType {
            vis,
            defaultness,
            type_token,
            ident,
            generics,
            colon_token,
            bounds: _,
            ty,
            semi_token,
        } = FlexibleItemType::parse(input, WhereClauseLocation::BeforeEq)?;

        if defaultness.is_some()
            || generics.lt_token.is_some()
            || generics.where_clause.is_some()
            || colon_token.is_some()
            || ty.is_some()
        {
            Ok(ForeignItem::Verbatim(verbatim::between(begin, input)))
        } else {
            Ok(ForeignItem::Type(ForeignItemType {
                attrs: Vec::new(),
                vis,
                type_token,
                ident,
                semi_token,
            }))
        }
    }

    #[cfg_attr(doc_cfg, doc(cfg(feature = "parsing")))]
    impl Parse for ForeignItemMacro {
        fn parse(input: ParseStream) -> Result<Self> {
            let attrs = input.call(Attribute::parse_outer)?;
            let mac: Macro = input.parse()?;
            let semi_token: Option<Token![;]> = if mac.delimiter.is_brace() {
                None
            } else {
                Some(input.parse()?)
            };
            Ok(ForeignItemMacro {
                attrs,
                mac,
                semi_token,
            })
        }
    }

    #[cfg_attr(doc_cfg, doc(cfg(feature = "parsing")))]
    impl Parse for ItemType {
        fn parse(input: ParseStream) -> Result<Self> {
            Ok(ItemType {
                attrs: input.call(Attribute::parse_outer)?,
                vis: input.parse()?,
                type_token: input.parse()?,
                ident: input.parse()?,
                generics: {
                    let mut generics: Generics = input.parse()?;
                    generics.where_clause = input.parse()?;
                    generics
                },
                eq_token: input.parse()?,
                ty: input.parse()?,
                semi_token: input.parse()?,
            })
        }
    }

    fn parse_item_type(begin: ParseBuffer, input: ParseStream) -> Result<Item> {
        let FlexibleItemType {
            vis,
            defaultness,
            type_token,
            ident,
            generics,
            colon_token,
            bounds: _,
            ty,
            semi_token,
        } = FlexibleItemType::parse(input, WhereClauseLocation::BeforeEq)?;

        if defaultness.is_some() || colon_token.is_some() || ty.is_none() {
            Ok(Item::Verbatim(verbatim::between(begin, input)))
        } else {
            let (eq_token, ty) = ty.unwrap();
            Ok(Item::Type(ItemType {
                attrs: Vec::new(),
                vis,
                type_token,
                ident,
                generics,
                eq_token,
                ty: Box::new(ty),
                semi_token,
            }))
        }
    }

    #[cfg_attr(doc_cfg, doc(cfg(feature = "parsing")))]
    impl Parse for ItemStruct {
        fn parse(input: ParseStream) -> Result<Self> {
            let attrs = input.call(Attribute::parse_outer)?;
            let vis = input.parse::<Visibility>()?;
            let struct_token = input.parse::<Token![struct]>()?;
            let ident = input.parse::<Ident>()?;
            let generics = input.parse::<Generics>()?;
            let (where_clause, fields, semi_token) = derive::parsing::data_struct(input)?;
            Ok(ItemStruct {
                attrs,
                vis,
                struct_token,
                ident,
                generics: Generics {
                    where_clause,
                    ..generics
                },
                fields,
                semi_token,
            })
        }
    }

    #[cfg_attr(doc_cfg, doc(cfg(feature = "parsing")))]
    impl Parse for ItemEnum {
        fn parse(input: ParseStream) -> Result<Self> {
            let attrs = input.call(Attribute::parse_outer)?;
            let vis = input.parse::<Visibility>()?;
            let enum_token = input.parse::<Token![enum]>()?;
            let ident = input.parse::<Ident>()?;
            let generics = input.parse::<Generics>()?;
            let (where_clause, brace_token, variants) = derive::parsing::data_enum(input)?;
            Ok(ItemEnum {
                attrs,
                vis,
                enum_token,
                ident,
                generics: Generics {
                    where_clause,
                    ..generics
                },
                brace_token,
                variants,
            })
        }
    }

    #[cfg_attr(doc_cfg, doc(cfg(feature = "parsing")))]
    impl Parse for ItemUnion {
        fn parse(input: ParseStream) -> Result<Self> {
            let attrs = input.call(Attribute::parse_outer)?;
            let vis = input.parse::<Visibility>()?;
            let union_token = input.parse::<Token![union]>()?;
            let ident = input.parse::<Ident>()?;
            let generics = input.parse::<Generics>()?;
            let (where_clause, fields) = derive::parsing::data_union(input)?;
            Ok(ItemUnion {
                attrs,
                vis,
                union_token,
                ident,
                generics: Generics {
                    where_clause,
                    ..generics
                },
                fields,
            })
        }
    }

    fn parse_trait_or_trait_alias(input: ParseStream) -> Result<Item> {
        let (attrs, vis, trait_token, ident, generics) = parse_start_of_trait_alias(input)?;
        let lookahead = input.lookahead1();
        if lookahead.peek(token::Brace)
            || lookahead.peek(Token![:])
            || lookahead.peek(Token![where])
        {
            let unsafety = None;
            let auto_token = None;
            parse_rest_of_trait(
                input,
                attrs,
                vis,
                unsafety,
                auto_token,
                trait_token,
                ident,
                generics,
            )
            .map(Item::Trait)
        } else if lookahead.peek(Token![=]) {
            parse_rest_of_trait_alias(input, attrs, vis, trait_token, ident, generics)
                .map(Item::TraitAlias)
        } else {
            Err(lookahead.error())
        }
    }

    #[cfg_attr(doc_cfg, doc(cfg(feature = "parsing")))]
    impl Parse for ItemTrait {
        fn parse(input: ParseStream) -> Result<Self> {
            let outer_attrs = input.call(Attribute::parse_outer)?;
            let vis: Visibility = input.parse()?;
            let unsafety: Option<Token![unsafe]> = input.parse()?;
            let auto_token: Option<Token![auto]> = input.parse()?;
            let trait_token: Token![trait] = input.parse()?;
            let ident: Ident = input.parse()?;
            let generics: Generics = input.parse()?;
            parse_rest_of_trait(
                input,
                outer_attrs,
                vis,
                unsafety,
                auto_token,
                trait_token,
                ident,
                generics,
            )
        }
    }

    fn parse_rest_of_trait(
        input: ParseStream,
        mut attrs: Vec<Attribute>,
        vis: Visibility,
        unsafety: Option<Token![unsafe]>,
        auto_token: Option<Token![auto]>,
        trait_token: Token![trait],
        ident: Ident,
        mut generics: Generics,
    ) -> Result<ItemTrait> {
        let colon_token: Option<Token![:]> = input.parse()?;

        let mut supertraits = Punctuated::new();
        if colon_token.is_some() {
            loop {
                if input.peek(Token![where]) || input.peek(token::Brace) {
                    break;
                }
                supertraits.push_value(input.parse()?);
                if input.peek(Token![where]) || input.peek(token::Brace) {
                    break;
                }
                supertraits.push_punct(input.parse()?);
            }
        }

        generics.where_clause = input.parse()?;

        let content;
        let brace_token = braced!(content in input);
        attr::parsing::parse_inner(&content, &mut attrs)?;
        let mut items = Vec::new();
        while !content.is_empty() {
            items.push(content.parse()?);
        }

        Ok(ItemTrait {
            attrs,
            vis,
            unsafety,
            auto_token,
            trait_token,
            ident,
            generics,
            colon_token,
            supertraits,
            brace_token,
            items,
        })
    }

    #[cfg_attr(doc_cfg, doc(cfg(feature = "parsing")))]
    impl Parse for ItemTraitAlias {
        fn parse(input: ParseStream) -> Result<Self> {
            let (attrs, vis, trait_token, ident, generics) = parse_start_of_trait_alias(input)?;
            parse_rest_of_trait_alias(input, attrs, vis, trait_token, ident, generics)
        }
    }

    fn parse_start_of_trait_alias(
        input: ParseStream,
    ) -> Result<(Vec<Attribute>, Visibility, Token![trait], Ident, Generics)> {
        let attrs = input.call(Attribute::parse_outer)?;
        let vis: Visibility = input.parse()?;
        let trait_token: Token![trait] = input.parse()?;
        let ident: Ident = input.parse()?;
        let generics: Generics = input.parse()?;
        Ok((attrs, vis, trait_token, ident, generics))
    }

    fn parse_rest_of_trait_alias(
        input: ParseStream,
        attrs: Vec<Attribute>,
        vis: Visibility,
        trait_token: Token![trait],
        ident: Ident,
        mut generics: Generics,
    ) -> Result<ItemTraitAlias> {
        let eq_token: Token![=] = input.parse()?;

        let mut bounds = Punctuated::new();
        loop {
            if input.peek(Token![where]) || input.peek(Token![;]) {
                break;
            }
            bounds.push_value(input.parse()?);
            if input.peek(Token![where]) || input.peek(Token![;]) {
                break;
            }
            bounds.push_punct(input.parse()?);
        }

        generics.where_clause = input.parse()?;
        let semi_token: Token![;] = input.parse()?;

        Ok(ItemTraitAlias {
            attrs,
            vis,
            trait_token,
            ident,
            generics,
            eq_token,
            bounds,
            semi_token,
        })
    }

    #[cfg_attr(doc_cfg, doc(cfg(feature = "parsing")))]
    impl Parse for TraitItem {
        fn parse(input: ParseStream) -> Result<Self> {
            let begin = input.fork();
            let mut attrs = input.call(Attribute::parse_outer)?;
            let vis: Visibility = input.parse()?;
            let defaultness: Option<Token![default]> = input.parse()?;
            let ahead = input.fork();

            let lookahead = ahead.lookahead1();
            let mut item = if lookahead.peek(Token![fn]) || peek_signature(&ahead) {
                input.parse().map(TraitItem::Method)
            } else if lookahead.peek(Token![const]) {
                ahead.parse::<Token![const]>()?;
                let lookahead = ahead.lookahead1();
                if lookahead.peek(Ident) || lookahead.peek(Token![_]) {
                    input.parse().map(TraitItem::Const)
                } else if lookahead.peek(Token![async])
                    || lookahead.peek(Token![unsafe])
                    || lookahead.peek(Token![extern])
                    || lookahead.peek(Token![fn])
                {
                    input.parse().map(TraitItem::Method)
                } else {
                    Err(lookahead.error())
                }
            } else if lookahead.peek(Token![type]) {
                parse_trait_item_type(begin.fork(), input)
            } else if lookahead.peek(Ident)
                || lookahead.peek(Token![self])
                || lookahead.peek(Token![super])
                || lookahead.peek(Token![crate])
                || lookahead.peek(Token![::])
            {
                input.parse().map(TraitItem::Macro)
            } else {
                Err(lookahead.error())
            }?;

            match (vis, defaultness) {
                (Visibility::Inherited, None) => {}
                _ => return Ok(TraitItem::Verbatim(verbatim::between(begin, input))),
            }

            let item_attrs = match &mut item {
                TraitItem::Const(item) => &mut item.attrs,
                TraitItem::Method(item) => &mut item.attrs,
                TraitItem::Type(item) => &mut item.attrs,
                TraitItem::Macro(item) => &mut item.attrs,
                TraitItem::Verbatim(_) => unreachable!(),

                #[cfg(syn_no_non_exhaustive)]
                _ => unreachable!(),
            };
            attrs.append(item_attrs);
            *item_attrs = attrs;
            Ok(item)
        }
    }

    #[cfg_attr(doc_cfg, doc(cfg(feature = "parsing")))]
    impl Parse for TraitItemConst {
        fn parse(input: ParseStream) -> Result<Self> {
            Ok(TraitItemConst {
                attrs: input.call(Attribute::parse_outer)?,
                const_token: input.parse()?,
                ident: {
                    let lookahead = input.lookahead1();
                    if lookahead.peek(Ident) || lookahead.peek(Token![_]) {
                        input.call(Ident::parse_any)?
                    } else {
                        return Err(lookahead.error());
                    }
                },
                colon_token: input.parse()?,
                ty: input.parse()?,
                default: {
                    if input.peek(Token![=]) {
                        let eq_token: Token![=] = input.parse()?;
                        let default: Expr = input.parse()?;
                        Some((eq_token, default))
                    } else {
                        None
                    }
                },
                semi_token: input.parse()?,
            })
        }
    }

    #[cfg_attr(doc_cfg, doc(cfg(feature = "parsing")))]
    impl Parse for TraitItemMethod {
        fn parse(input: ParseStream) -> Result<Self> {
            let mut attrs = input.call(Attribute::parse_outer)?;
            let sig: Signature = input.parse()?;

            let lookahead = input.lookahead1();
            let (brace_token, stmts, semi_token) = if lookahead.peek(token::Brace) {
                let content;
                let brace_token = braced!(content in input);
                attr::parsing::parse_inner(&content, &mut attrs)?;
                let stmts = content.call(Block::parse_within)?;
                (Some(brace_token), stmts, None)
            } else if lookahead.peek(Token![;]) {
                let semi_token: Token![;] = input.parse()?;
                (None, Vec::new(), Some(semi_token))
            } else {
                return Err(lookahead.error());
            };

            Ok(TraitItemMethod {
                attrs,
                sig,
                default: brace_token.map(|brace_token| Block { brace_token, stmts }),
                semi_token,
            })
        }
    }

    #[cfg_attr(doc_cfg, doc(cfg(feature = "parsing")))]
    impl Parse for TraitItemType {
        fn parse(input: ParseStream) -> Result<Self> {
            let attrs = input.call(Attribute::parse_outer)?;
            let type_token: Token![type] = input.parse()?;
            let ident: Ident = input.parse()?;
            let mut generics: Generics = input.parse()?;
            let colon_token: Option<Token![:]> = input.parse()?;

            let mut bounds = Punctuated::new();
            if colon_token.is_some() {
                while !input.peek(Token![where]) && !input.peek(Token![=]) && !input.peek(Token![;])
                {
                    if !bounds.is_empty() {
                        bounds.push_punct(input.parse()?);
                    }
                    bounds.push_value(input.parse()?);
                }
            }

            let default = if input.peek(Token![=]) {
                let eq_token: Token![=] = input.parse()?;
                let default: Type = input.parse()?;
                Some((eq_token, default))
            } else {
                None
            };

            generics.where_clause = input.parse()?;
            let semi_token: Token![;] = input.parse()?;

            Ok(TraitItemType {
                attrs,
                type_token,
                ident,
                generics,
                colon_token,
                bounds,
                default,
                semi_token,
            })
        }
    }

    fn parse_trait_item_type(begin: ParseBuffer, input: ParseStream) -> Result<TraitItem> {
        let FlexibleItemType {
            vis,
            defaultness,
            type_token,
            ident,
            generics,
            colon_token,
            bounds,
            ty,
            semi_token,
        } = FlexibleItemType::parse(input, WhereClauseLocation::Both)?;

        if defaultness.is_some() || vis.is_some() {
            Ok(TraitItem::Verbatim(verbatim::between(begin, input)))
        } else {
            Ok(TraitItem::Type(TraitItemType {
                attrs: Vec::new(),
                type_token,
                ident,
                generics,
                colon_token,
                bounds,
                default: ty,
                semi_token,
            }))
        }
    }

    #[cfg_attr(doc_cfg, doc(cfg(feature = "parsing")))]
    impl Parse for TraitItemMacro {
        fn parse(input: ParseStream) -> Result<Self> {
            let attrs = input.call(Attribute::parse_outer)?;
            let mac: Macro = input.parse()?;
            let semi_token: Option<Token![;]> = if mac.delimiter.is_brace() {
                None
            } else {
                Some(input.parse()?)
            };
            Ok(TraitItemMacro {
                attrs,
                mac,
                semi_token,
            })
        }
    }

    #[cfg_attr(doc_cfg, doc(cfg(feature = "parsing")))]
    impl Parse for ItemImpl {
        fn parse(input: ParseStream) -> Result<Self> {
            let allow_verbatim_impl = false;
            parse_impl(input, allow_verbatim_impl).map(Option::unwrap)
        }
    }

    fn parse_impl(input: ParseStream, allow_verbatim_impl: bool) -> Result<Option<ItemImpl>> {
        let mut attrs = input.call(Attribute::parse_outer)?;
        let has_visibility = allow_verbatim_impl && input.parse::<Visibility>()?.is_some();
        let defaultness: Option<Token![default]> = input.parse()?;
        let unsafety: Option<Token![unsafe]> = input.parse()?;
        let impl_token: Token![impl] = input.parse()?;

        let has_generics = input.peek(Token![<])
            && (input.peek2(Token![>])
                || input.peek2(Token![#])
                || (input.peek2(Ident) || input.peek2(Lifetime))
                    && (input.peek3(Token![:])
                        || input.peek3(Token![,])
                        || input.peek3(Token![>])
                        || input.peek3(Token![=]))
                || input.peek2(Token![const]));
        let mut generics: Generics = if has_generics {
            input.parse()?
        } else {
            Generics::default()
        };

        let is_const_impl = allow_verbatim_impl
            && (input.peek(Token![const]) || input.peek(Token![?]) && input.peek2(Token![const]));
        if is_const_impl {
            input.parse::<Option<Token![?]>>()?;
            input.parse::<Token![const]>()?;
        }

        let begin = input.fork();
        let polarity = if input.peek(Token![!]) && !input.peek2(token::Brace) {
            Some(input.parse::<Token![!]>()?)
        } else {
            None
        };

        #[cfg(not(feature = "printing"))]
        let first_ty_span = input.span();
        let mut first_ty: Type = input.parse()?;
        let self_ty: Type;
        let trait_;

        let is_impl_for = input.peek(Token![for]);
        if is_impl_for {
            let for_token: Token![for] = input.parse()?;
            let mut first_ty_ref = &first_ty;
            while let Type::Group(ty) = first_ty_ref {
                first_ty_ref = &ty.elem;
            }
            if let Type::Path(TypePath { qself: None, .. }) = first_ty_ref {
                while let Type::Group(ty) = first_ty {
                    first_ty = *ty.elem;
                }
                if let Type::Path(TypePath { qself: None, path }) = first_ty {
                    trait_ = Some((polarity, path, for_token));
                } else {
                    unreachable!();
                }
            } else if !allow_verbatim_impl {
                #[cfg(feature = "printing")]
                return Err(Error::new_spanned(first_ty_ref, "expected trait path"));
                #[cfg(not(feature = "printing"))]
                return Err(Error::new(first_ty_span, "expected trait path"));
            } else {
                trait_ = None;
            }
            self_ty = input.parse()?;
        } else {
            trait_ = None;
            self_ty = if polarity.is_none() {
                first_ty
            } else {
                Type::Verbatim(verbatim::between(begin, input))
            };
        }

        generics.where_clause = input.parse()?;

        let content;
        let brace_token = braced!(content in input);
        attr::parsing::parse_inner(&content, &mut attrs)?;

        let mut items = Vec::new();
        while !content.is_empty() {
            items.push(content.parse()?);
        }

        if has_visibility || is_const_impl || is_impl_for && trait_.is_none() {
            Ok(None)
        } else {
            Ok(Some(ItemImpl {
                attrs,
                defaultness,
                unsafety,
                impl_token,
                generics,
                trait_,
                self_ty: Box::new(self_ty),
                brace_token,
                items,
            }))
        }
    }

    #[cfg_attr(doc_cfg, doc(cfg(feature = "parsing")))]
    impl Parse for ImplItem {
        fn parse(input: ParseStream) -> Result<Self> {
            let begin = input.fork();
            let mut attrs = input.call(Attribute::parse_outer)?;
            let ahead = input.fork();
            let vis: Visibility = ahead.parse()?;

            let mut lookahead = ahead.lookahead1();
            let defaultness = if lookahead.peek(Token![default]) && !ahead.peek2(Token![!]) {
                let defaultness: Token![default] = ahead.parse()?;
                lookahead = ahead.lookahead1();
                Some(defaultness)
            } else {
                None
            };

            let mut item = if lookahead.peek(Token![fn]) || peek_signature(&ahead) {
                input.parse().map(ImplItem::Method)
            } else if lookahead.peek(Token![const]) {
                let const_token: Token![const] = ahead.parse()?;
                let lookahead = ahead.lookahead1();
                if lookahead.peek(Ident) || lookahead.peek(Token![_]) {
                    input.advance_to(&ahead);
                    let ident: Ident = input.call(Ident::parse_any)?;
                    let colon_token: Token![:] = input.parse()?;
                    let ty: Type = input.parse()?;
                    if let Some(eq_token) = input.parse()? {
                        return Ok(ImplItem::Const(ImplItemConst {
                            attrs,
                            vis,
                            defaultness,
                            const_token,
                            ident,
                            colon_token,
                            ty,
                            eq_token,
                            expr: input.parse()?,
                            semi_token: input.parse()?,
                        }));
                    } else {
                        input.parse::<Token![;]>()?;
                        return Ok(ImplItem::Verbatim(verbatim::between(begin, input)));
                    }
                } else {
                    Err(lookahead.error())
                }
            } else if lookahead.peek(Token![type]) {
                parse_impl_item_type(begin, input)
            } else if vis.is_inherited()
                && defaultness.is_none()
                && (lookahead.peek(Ident)
                    || lookahead.peek(Token![self])
                    || lookahead.peek(Token![super])
                    || lookahead.peek(Token![crate])
                    || lookahead.peek(Token![::]))
            {
                input.parse().map(ImplItem::Macro)
            } else {
                Err(lookahead.error())
            }?;

            {
                let item_attrs = match &mut item {
                    ImplItem::Const(item) => &mut item.attrs,
                    ImplItem::Method(item) => &mut item.attrs,
                    ImplItem::Type(item) => &mut item.attrs,
                    ImplItem::Macro(item) => &mut item.attrs,
                    ImplItem::Verbatim(_) => return Ok(item),

                    #[cfg(syn_no_non_exhaustive)]
                    _ => unreachable!(),
                };
                attrs.append(item_attrs);
                *item_attrs = attrs;
            }

            Ok(item)
        }
    }

    #[cfg_attr(doc_cfg, doc(cfg(feature = "parsing")))]
    impl Parse for ImplItemConst {
        fn parse(input: ParseStream) -> Result<Self> {
            Ok(ImplItemConst {
                attrs: input.call(Attribute::parse_outer)?,
                vis: input.parse()?,
                defaultness: input.parse()?,
                const_token: input.parse()?,
                ident: {
                    let lookahead = input.lookahead1();
                    if lookahead.peek(Ident) || lookahead.peek(Token![_]) {
                        input.call(Ident::parse_any)?
                    } else {
                        return Err(lookahead.error());
                    }
                },
                colon_token: input.parse()?,
                ty: input.parse()?,
                eq_token: input.parse()?,
                expr: input.parse()?,
                semi_token: input.parse()?,
            })
        }
    }

    #[cfg_attr(doc_cfg, doc(cfg(feature = "parsing")))]
    impl Parse for ImplItemMethod {
        fn parse(input: ParseStream) -> Result<Self> {
            let mut attrs = input.call(Attribute::parse_outer)?;
            let vis: Visibility = input.parse()?;
            let defaultness: Option<Token![default]> = input.parse()?;
            let sig: Signature = input.parse()?;

            let block = if let Some(semi) = input.parse::<Option<Token![;]>>()? {
                // Accept methods without a body in an impl block because
                // rustc's *parser* does not reject them (the compilation error
                // is emitted later than parsing) and it can be useful for macro
                // DSLs.
                let mut punct = Punct::new(';', Spacing::Alone);
                punct.set_span(semi.span);
                let tokens = TokenStream::from_iter(vec![TokenTree::Punct(punct)]);
                Block {
                    brace_token: Brace { span: semi.span },
                    stmts: vec![Stmt::Item(Item::Verbatim(tokens))],
                }
            } else {
                let content;
                let brace_token = braced!(content in input);
                attrs.extend(content.call(Attribute::parse_inner)?);
                Block {
                    brace_token,
                    stmts: content.call(Block::parse_within)?,
                }
            };

            Ok(ImplItemMethod {
                attrs,
                vis,
                defaultness,
                sig,
                block,
            })
        }
    }

    #[cfg_attr(doc_cfg, doc(cfg(feature = "parsing")))]
    impl Parse for ImplItemType {
        fn parse(input: ParseStream) -> Result<Self> {
            let attrs = input.call(Attribute::parse_outer)?;
            let vis: Visibility = input.parse()?;
            let defaultness: Option<Token![default]> = input.parse()?;
            let type_token: Token![type] = input.parse()?;
            let ident: Ident = input.parse()?;
            let mut generics: Generics = input.parse()?;
            let eq_token: Token![=] = input.parse()?;
            let ty: Type = input.parse()?;
            generics.where_clause = input.parse()?;
            let semi_token: Token![;] = input.parse()?;
            Ok(ImplItemType {
                attrs,
                vis,
                defaultness,
                type_token,
                ident,
                generics,
                eq_token,
                ty,
                semi_token,
            })
        }
    }

    fn parse_impl_item_type(begin: ParseBuffer, input: ParseStream) -> Result<ImplItem> {
        let FlexibleItemType {
            vis,
            defaultness,
            type_token,
            ident,
            generics,
            colon_token,
            bounds: _,
            ty,
            semi_token,
        } = FlexibleItemType::parse(input, WhereClauseLocation::Both)?;

        if colon_token.is_some() || ty.is_none() {
            Ok(ImplItem::Verbatim(verbatim::between(begin, input)))
        } else {
            let (eq_token, ty) = ty.unwrap();
            Ok(ImplItem::Type(ImplItemType {
                attrs: Vec::new(),
                vis,
                defaultness,
                type_token,
                ident,
                generics,
                eq_token,
                ty,
                semi_token,
            }))
        }
    }

    #[cfg_attr(doc_cfg, doc(cfg(feature = "parsing")))]
    impl Parse for ImplItemMacro {
        fn parse(input: ParseStream) -> Result<Self> {
            let attrs = input.call(Attribute::parse_outer)?;
            let mac: Macro = input.parse()?;
            let semi_token: Option<Token![;]> = if mac.delimiter.is_brace() {
                None
            } else {
                Some(input.parse()?)
            };
            Ok(ImplItemMacro {
                attrs,
                mac,
                semi_token,
            })
        }
    }

    impl Visibility {
        fn is_inherited(&self) -> bool {
            match *self {
                Visibility::Inherited => true,
                _ => false,
            }
        }
    }

    impl MacroDelimiter {
        fn is_brace(&self) -> bool {
            match *self {
                MacroDelimiter::Brace(_) => true,
                MacroDelimiter::Paren(_) | MacroDelimiter::Bracket(_) => false,
            }
        }
    }
}

#[cfg(feature = "printing")]
mod printing {
    use super::*;
    use crate::attr::FilterAttrs;
    use crate::print::TokensOrDefault;
    use proc_macro2::TokenStream;
    use quote::{ToTokens, TokenStreamExt};

    #[cfg_attr(doc_cfg, doc(cfg(feature = "printing")))]
    impl ToTokens for ItemExternCrate {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            tokens.append_all(self.attrs.outer());
            self.vis.to_tokens(tokens);
            self.extern_token.to_tokens(tokens);
            self.crate_token.to_tokens(tokens);
            self.ident.to_tokens(tokens);
            if let Some((as_token, rename)) = &self.rename {
                as_token.to_tokens(tokens);
                rename.to_tokens(tokens);
            }
            self.semi_token.to_tokens(tokens);
        }
    }

    #[cfg_attr(doc_cfg, doc(cfg(feature = "printing")))]
    impl ToTokens for ItemUse {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            tokens.append_all(self.attrs.outer());
            self.vis.to_tokens(tokens);
            self.use_token.to_tokens(tokens);
            self.leading_colon.to_tokens(tokens);
            self.tree.to_tokens(tokens);
            self.semi_token.to_tokens(tokens);
        }
    }

    #[cfg_attr(doc_cfg, doc(cfg(feature = "printing")))]
    impl ToTokens for ItemStatic {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            tokens.append_all(self.attrs.outer());
            self.vis.to_tokens(tokens);
            self.static_token.to_tokens(tokens);
            self.mutability.to_tokens(tokens);
            self.ident.to_tokens(tokens);
            self.colon_token.to_tokens(tokens);
            self.ty.to_tokens(tokens);
            self.eq_token.to_tokens(tokens);
            self.expr.to_tokens(tokens);
            self.semi_token.to_tokens(tokens);
        }
    }

    #[cfg_attr(doc_cfg, doc(cfg(feature = "printing")))]
    impl ToTokens for ItemConst {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            tokens.append_all(self.attrs.outer());
            self.vis.to_tokens(tokens);
            self.const_token.to_tokens(tokens);
            self.ident.to_tokens(tokens);
            self.colon_token.to_tokens(tokens);
            self.ty.to_tokens(tokens);
            self.eq_token.to_tokens(tokens);
            self.expr.to_tokens(tokens);
            self.semi_token.to_tokens(tokens);
        }
    }

    #[cfg_attr(doc_cfg, doc(cfg(feature = "printing")))]
    impl ToTokens for ItemFn {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            tokens.append_all(self.attrs.outer());
            self.vis.to_tokens(tokens);
            self.sig.to_tokens(tokens);
            self.block.brace_token.surround(tokens, |tokens| {
                tokens.append_all(self.attrs.inner());
                tokens.append_all(&self.block.stmts);
            });
        }
    }

    #[cfg_attr(doc_cfg, doc(cfg(feature = "printing")))]
    impl ToTokens for ItemMod {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            tokens.append_all(self.attrs.outer());
            self.vis.to_tokens(tokens);
            self.mod_token.to_tokens(tokens);
            self.ident.to_tokens(tokens);
            if let Some((brace, items)) = &self.content {
                brace.surround(tokens, |tokens| {
                    tokens.append_all(self.attrs.inner());
                    tokens.append_all(items);
                });
            } else {
                TokensOrDefault(&self.semi).to_tokens(tokens);
            }
        }
    }

    #[cfg_attr(doc_cfg, doc(cfg(feature = "printing")))]
    impl ToTokens for ItemForeignMod {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            tokens.append_all(self.attrs.outer());
            self.abi.to_tokens(tokens);
            self.brace_token.surround(tokens, |tokens| {
                tokens.append_all(self.attrs.inner());
                tokens.append_all(&self.items);
            });
        }
    }

    #[cfg_attr(doc_cfg, doc(cfg(feature = "printing")))]
    impl ToTokens for ItemType {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            tokens.append_all(self.attrs.outer());
            self.vis.to_tokens(tokens);
            self.type_token.to_tokens(tokens);
            self.ident.to_tokens(tokens);
            self.generics.to_tokens(tokens);
            self.generics.where_clause.to_tokens(tokens);
            self.eq_token.to_tokens(tokens);
            self.ty.to_tokens(tokens);
            self.semi_token.to_tokens(tokens);
        }
    }

    #[cfg_attr(doc_cfg, doc(cfg(feature = "printing")))]
    impl ToTokens for ItemEnum {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            tokens.append_all(self.attrs.outer());
            self.vis.to_tokens(tokens);
            self.enum_token.to_tokens(tokens);
            self.ident.to_tokens(tokens);
            self.generics.to_tokens(tokens);
            self.generics.where_clause.to_tokens(tokens);
            self.brace_token.surround(tokens, |tokens| {
                self.variants.to_tokens(tokens);
            });
        }
    }

    #[cfg_attr(doc_cfg, doc(cfg(feature = "printing")))]
    impl ToTokens for ItemStruct {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            tokens.append_all(self.attrs.outer());
            self.vis.to_tokens(tokens);
            self.struct_token.to_tokens(tokens);
            self.ident.to_tokens(tokens);
            self.generics.to_tokens(tokens);
            match &self.fields {
                Fields::Named(fields) => {
                    self.generics.where_clause.to_tokens(tokens);
                    fields.to_tokens(tokens);
                }
                Fields::Unnamed(fields) => {
                    fields.to_tokens(tokens);
                    self.generics.where_clause.to_tokens(tokens);
                    TokensOrDefault(&self.semi_token).to_tokens(tokens);
                }
                Fields::Unit => {
                    self.generics.where_clause.to_tokens(tokens);
                    TokensOrDefault(&self.semi_token).to_tokens(tokens);
                }
            }
        }
    }

    #[cfg_attr(doc_cfg, doc(cfg(feature = "printing")))]
    impl ToTokens for ItemUnion {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            tokens.append_all(self.attrs.outer());
            self.vis.to_tokens(tokens);
            self.union_token.to_tokens(tokens);
            self.ident.to_tokens(tokens);
            self.generics.to_tokens(tokens);
            self.generics.where_clause.to_tokens(tokens);
            self.fields.to_tokens(tokens);
        }
    }

    #[cfg_attr(doc_cfg, doc(cfg(feature = "printing")))]
    impl ToTokens for ItemTrait {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            tokens.append_all(self.attrs.outer());
            self.vis.to_tokens(tokens);
            self.unsafety.to_tokens(tokens);
            self.auto_token.to_tokens(tokens);
            self.trait_token.to_tokens(tokens);
            self.ident.to_tokens(tokens);
            self.generics.to_tokens(tokens);
            if !self.supertraits.is_empty() {
                TokensOrDefault(&self.colon_token).to_tokens(tokens);
                self.supertraits.to_tokens(tokens);
            }
            self.generics.where_clause.to_tokens(tokens);
            self.brace_token.surround(tokens, |tokens| {
                tokens.append_all(self.attrs.inner());
                tokens.append_all(&self.items);
            });
        }
    }

    #[cfg_attr(doc_cfg, doc(cfg(feature = "printing")))]
    impl ToTokens for ItemTraitAlias {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            tokens.append_all(self.attrs.outer());
            self.vis.to_tokens(tokens);
            self.trait_token.to_tokens(tokens);
            self.ident.to_tokens(tokens);
            self.generics.to_tokens(tokens);
            self.eq_token.to_tokens(tokens);
            self.bounds.to_tokens(tokens);
            self.generics.where_clause.to_tokens(tokens);
            self.semi_token.to_tokens(tokens);
        }
    }

    #[cfg_attr(doc_cfg, doc(cfg(feature = "printing")))]
    impl ToTokens for ItemImpl {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            tokens.append_all(self.attrs.outer());
            self.defaultness.to_tokens(tokens);
            self.unsafety.to_tokens(tokens);
            self.impl_token.to_tokens(tokens);
            self.generics.to_tokens(tokens);
            if let Some((polarity, path, for_token)) = &self.trait_ {
                polarity.to_tokens(tokens);
                path.to_tokens(tokens);
                for_token.to_tokens(tokens);
            }
            self.self_ty.to_tokens(tokens);
            self.generics.where_clause.to_tokens(tokens);
            self.brace_token.surround(tokens, |tokens| {
                tokens.append_all(self.attrs.inner());
                tokens.append_all(&self.items);
            });
        }
    }

    #[cfg_attr(doc_cfg, doc(cfg(feature = "printing")))]
    impl ToTokens for ItemMacro {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            tokens.append_all(self.attrs.outer());
            self.mac.path.to_tokens(tokens);
            self.mac.bang_token.to_tokens(tokens);
            self.ident.to_tokens(tokens);
            match &self.mac.delimiter {
                MacroDelimiter::Paren(paren) => {
                    paren.surround(tokens, |tokens| self.mac.tokens.to_tokens(tokens));
                }
                MacroDelimiter::Brace(brace) => {
                    brace.surround(tokens, |tokens| self.mac.tokens.to_tokens(tokens));
                }
                MacroDelimiter::Bracket(bracket) => {
                    bracket.surround(tokens, |tokens| self.mac.tokens.to_tokens(tokens));
                }
            }
            self.semi_token.to_tokens(tokens);
        }
    }

    #[cfg_attr(doc_cfg, doc(cfg(feature = "printing")))]
    impl ToTokens for ItemMacro2 {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            tokens.append_all(self.attrs.outer());
            self.vis.to_tokens(tokens);
            self.macro_token.to_tokens(tokens);
            self.ident.to_tokens(tokens);
            self.rules.to_tokens(tokens);
        }
    }

    #[cfg_attr(doc_cfg, doc(cfg(feature = "printing")))]
    impl ToTokens for UsePath {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            self.ident.to_tokens(tokens);
            self.colon2_token.to_tokens(tokens);
            self.tree.to_tokens(tokens);
        }
    }

    #[cfg_attr(doc_cfg, doc(cfg(feature = "printing")))]
    impl ToTokens for UseName {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            self.ident.to_tokens(tokens);
        }
    }

    #[cfg_attr(doc_cfg, doc(cfg(feature = "printing")))]
    impl ToTokens for UseRename {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            self.ident.to_tokens(tokens);
            self.as_token.to_tokens(tokens);
            self.rename.to_tokens(tokens);
        }
    }

    #[cfg_attr(doc_cfg, doc(cfg(feature = "printing")))]
    impl ToTokens for UseGlob {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            self.star_token.to_tokens(tokens);
        }
    }

    #[cfg_attr(doc_cfg, doc(cfg(feature = "printing")))]
    impl ToTokens for UseGroup {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            self.brace_token.surround(tokens, |tokens| {
                self.items.to_tokens(tokens);
            });
        }
    }

    #[cfg_attr(doc_cfg, doc(cfg(feature = "printing")))]
    impl ToTokens for TraitItemConst {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            tokens.append_all(self.attrs.outer());
            self.const_token.to_tokens(tokens);
            self.ident.to_tokens(tokens);
            self.colon_token.to_tokens(tokens);
            self.ty.to_tokens(tokens);
            if let Some((eq_token, default)) = &self.default {
                eq_token.to_tokens(tokens);
                default.to_tokens(tokens);
            }
            self.semi_token.to_tokens(tokens);
        }
    }

    #[cfg_attr(doc_cfg, doc(cfg(feature = "printing")))]
    impl ToTokens for TraitItemMethod {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            tokens.append_all(self.attrs.outer());
            self.sig.to_tokens(tokens);
            match &self.default {
                Some(block) => {
                    block.brace_token.surround(tokens, |tokens| {
                        tokens.append_all(self.attrs.inner());
                        tokens.append_all(&block.stmts);
                    });
                }
                None => {
                    TokensOrDefault(&self.semi_token).to_tokens(tokens);
                }
            }
        }
    }

    #[cfg_attr(doc_cfg, doc(cfg(feature = "printing")))]
    impl ToTokens for TraitItemType {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            tokens.append_all(self.attrs.outer());
            self.type_token.to_tokens(tokens);
            self.ident.to_tokens(tokens);
            self.generics.to_tokens(tokens);
            if !self.bounds.is_empty() {
                TokensOrDefault(&self.colon_token).to_tokens(tokens);
                self.bounds.to_tokens(tokens);
            }
            if let Some((eq_token, default)) = &self.default {
                eq_token.to_tokens(tokens);
                default.to_tokens(tokens);
            }
            self.generics.where_clause.to_tokens(tokens);
            self.semi_token.to_tokens(tokens);
        }
    }

    #[cfg_attr(doc_cfg, doc(cfg(feature = "printing")))]
    impl ToTokens for TraitItemMacro {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            tokens.append_all(self.attrs.outer());
            self.mac.to_tokens(tokens);
            self.semi_token.to_tokens(tokens);
        }
    }

    #[cfg_attr(doc_cfg, doc(cfg(feature = "printing")))]
    impl ToTokens for ImplItemConst {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            tokens.append_all(self.attrs.outer());
            self.vis.to_tokens(tokens);
            self.defaultness.to_tokens(tokens);
            self.const_token.to_tokens(tokens);
            self.ident.to_tokens(tokens);
            self.colon_token.to_tokens(tokens);
            self.ty.to_tokens(tokens);
            self.eq_token.to_tokens(tokens);
            self.expr.to_tokens(tokens);
            self.semi_token.to_tokens(tokens);
        }
    }

    #[cfg_attr(doc_cfg, doc(cfg(feature = "printing")))]
    impl ToTokens for ImplItemMethod {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            tokens.append_all(self.attrs.outer());
            self.vis.to_tokens(tokens);
            self.defaultness.to_tokens(tokens);
            self.sig.to_tokens(tokens);
            if self.block.stmts.len() == 1 {
                if let Stmt::Item(Item::Verbatim(verbatim)) = &self.block.stmts[0] {
                    if verbatim.to_string() == ";" {
                        verbatim.to_tokens(tokens);
                        return;
                    }
                }
            }
            self.block.brace_token.surround(tokens, |tokens| {
                tokens.append_all(self.attrs.inner());
                tokens.append_all(&self.block.stmts);
            });
        }
    }

    #[cfg_attr(doc_cfg, doc(cfg(feature = "printing")))]
    impl ToTokens for ImplItemType {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            tokens.append_all(self.attrs.outer());
            self.vis.to_tokens(tokens);
            self.defaultness.to_tokens(tokens);
            self.type_token.to_tokens(tokens);
            self.ident.to_tokens(tokens);
            self.generics.to_tokens(tokens);
            self.eq_token.to_tokens(tokens);
            self.ty.to_tokens(tokens);
            self.generics.where_clause.to_tokens(tokens);
            self.semi_token.to_tokens(tokens);
        }
    }

    #[cfg_attr(doc_cfg, doc(cfg(feature = "printing")))]
    impl ToTokens for ImplItemMacro {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            tokens.append_all(self.attrs.outer());
            self.mac.to_tokens(tokens);
            self.semi_token.to_tokens(tokens);
        }
    }

    #[cfg_attr(doc_cfg, doc(cfg(feature = "printing")))]
    impl ToTokens for ForeignItemFn {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            tokens.append_all(self.attrs.outer());
            self.vis.to_tokens(tokens);
            self.sig.to_tokens(tokens);
            self.semi_token.to_tokens(tokens);
        }
    }

    #[cfg_attr(doc_cfg, doc(cfg(feature = "printing")))]
    impl ToTokens for ForeignItemStatic {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            tokens.append_all(self.attrs.outer());
            self.vis.to_tokens(tokens);
            self.static_token.to_tokens(tokens);
            self.mutability.to_tokens(tokens);
            self.ident.to_tokens(tokens);
            self.colon_token.to_tokens(tokens);
            self.ty.to_tokens(tokens);
            self.semi_token.to_tokens(tokens);
        }
    }

    #[cfg_attr(doc_cfg, doc(cfg(feature = "printing")))]
    impl ToTokens for ForeignItemType {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            tokens.append_all(self.attrs.outer());
            self.vis.to_tokens(tokens);
            self.type_token.to_tokens(tokens);
            self.ident.to_tokens(tokens);
            self.semi_token.to_tokens(tokens);
        }
    }

    #[cfg_attr(doc_cfg, doc(cfg(feature = "printing")))]
    impl ToTokens for ForeignItemMacro {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            tokens.append_all(self.attrs.outer());
            self.mac.to_tokens(tokens);
            self.semi_token.to_tokens(tokens);
        }
    }

    fn maybe_variadic_to_tokens(arg: &FnArg, tokens: &mut TokenStream) -> bool {
        let arg = match arg {
            FnArg::Typed(arg) => arg,
            FnArg::Receiver(receiver) => {
                receiver.to_tokens(tokens);
                return false;
            }
        };

        match arg.ty.as_ref() {
            Type::Verbatim(ty) if ty.to_string() == "..." => {
                match arg.pat.as_ref() {
                    Pat::Verbatim(pat) if pat.to_string() == "..." => {
                        tokens.append_all(arg.attrs.outer());
                        pat.to_tokens(tokens);
                    }
                    _ => arg.to_tokens(tokens),
                }
                true
            }
            _ => {
                arg.to_tokens(tokens);
                false
            }
        }
    }

    #[cfg_attr(doc_cfg, doc(cfg(feature = "printing")))]
    impl ToTokens for Signature {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            self.constness.to_tokens(tokens);
            self.asyncness.to_tokens(tokens);
            self.unsafety.to_tokens(tokens);
            self.abi.to_tokens(tokens);
            self.fn_token.to_tokens(tokens);
            self.ident.to_tokens(tokens);
            self.generics.to_tokens(tokens);
            self.paren_token.surround(tokens, |tokens| {
                let mut last_is_variadic = false;
                for pair in self.inputs.pairs() {
                    last_is_variadic = maybe_variadic_to_tokens(pair.value(), tokens);
                    pair.punct().to_tokens(tokens);
                }
                if self.variadic.is_some() && !last_is_variadic {
                    if !self.inputs.empty_or_trailing() {
                        <Token![,]>::default().to_tokens(tokens);
                    }
                    self.variadic.to_tokens(tokens);
                }
            });
            self.output.to_tokens(tokens);
            self.generics.where_clause.to_tokens(tokens);
        }
    }

    #[cfg_attr(doc_cfg, doc(cfg(feature = "printing")))]
    impl ToTokens for Receiver {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            tokens.append_all(self.attrs.outer());
            if let Some((ampersand, lifetime)) = &self.reference {
                ampersand.to_tokens(tokens);
                lifetime.to_tokens(tokens);
            }
            self.mutability.to_tokens(tokens);
            self.self_token.to_tokens(tokens);
        }
    }
}

use super::*;
use crate::punctuated::Punctuated;
use proc_macro2::TokenStream;
use std::iter;
use std::slice;

#[cfg(feature = "parsing")]
use crate::parse::{Parse, ParseBuffer, ParseStream, Parser, Result};
#[cfg(feature = "parsing")]
use crate::punctuated::Pair;

ast_struct! {
    /// An attribute like `#[repr(transparent)]`.
    ///
    /// *This type is available only if Syn is built with the `"derive"` or `"full"`
    /// feature.*
    ///
    /// <br>
    ///
    /// # Syntax
    ///
    /// Rust has six types of attributes.
    ///
    /// - Outer attributes like `#[repr(transparent)]`. These appear outside or
    ///   in front of the item they describe.
    /// - Inner attributes like `#![feature(proc_macro)]`. These appear inside
    ///   of the item they describe, usually a module.
    /// - Outer doc comments like `/// # Example`.
    /// - Inner doc comments like `//! Please file an issue`.
    /// - Outer block comments `/** # Example */`.
    /// - Inner block comments `/*! Please file an issue */`.
    ///
    /// The `style` field of type `AttrStyle` distinguishes whether an attribute
    /// is outer or inner. Doc comments and block comments are promoted to
    /// attributes, as this is how they are processed by the compiler and by
    /// `macro_rules!` macros.
    ///
    /// The `path` field gives the possibly colon-delimited path against which
    /// the attribute is resolved. It is equal to `"doc"` for desugared doc
    /// comments. The `tokens` field contains the rest of the attribute body as
    /// tokens.
    ///
    /// ```text
    /// #[derive(Copy)]      #[crate::precondition x < 5]
    ///   ^^^^^^~~~~~~         ^^^^^^^^^^^^^^^^^^^ ~~~~~
    ///   path  tokens                 path        tokens
    /// ```
    ///
    /// <br>
    ///
    /// # Parsing from tokens to Attribute
    ///
    /// This type does not implement the [`Parse`] trait and thus cannot be
    /// parsed directly by [`ParseStream::parse`]. Instead use
    /// [`ParseStream::call`] with one of the two parser functions
    /// [`Attribute::parse_outer`] or [`Attribute::parse_inner`] depending on
    /// which you intend to parse.
    ///
    /// [`Parse`]: parse::Parse
    /// [`ParseStream::parse`]: parse::ParseBuffer::parse
    /// [`ParseStream::call`]: parse::ParseBuffer::call
    ///
    /// ```
    /// use syn::{Attribute, Ident, Result, Token};
    /// use syn::parse::{Parse, ParseStream};
    ///
    /// // Parses a unit struct with attributes.
    /// //
    /// //     #[path = "s.tmpl"]
    /// //     struct S;
    /// struct UnitStruct {
    ///     attrs: Vec<Attribute>,
    ///     struct_token: Token![struct],
    ///     name: Ident,
    ///     semi_token: Token![;],
    /// }
    ///
    /// impl Parse for UnitStruct {
    ///     fn parse(input: ParseStream) -> Result<Self> {
    ///         Ok(UnitStruct {
    ///             attrs: input.call(Attribute::parse_outer)?,
    ///             struct_token: input.parse()?,
    ///             name: input.parse()?,
    ///             semi_token: input.parse()?,
    ///         })
    ///     }
    /// }
    /// ```
    ///
    /// <p><br></p>
    ///
    /// # Parsing from Attribute to structured arguments
    ///
    /// The grammar of attributes in Rust is very flexible, which makes the
    /// syntax tree not that useful on its own. In particular, arguments of the
    /// attribute are held in an arbitrary `tokens: TokenStream`. Macros are
    /// expected to check the `path` of the attribute, decide whether they
    /// recognize it, and then parse the remaining tokens according to whatever
    /// grammar they wish to require for that kind of attribute.
    ///
    /// If the attribute you are parsing is expected to conform to the
    /// conventional structured form of attribute, use [`parse_meta()`] to
    /// obtain that structured representation. If the attribute follows some
    /// other grammar of its own, use [`parse_args()`] to parse that into the
    /// expected data structure.
    ///
    /// [`parse_meta()`]: Attribute::parse_meta
    /// [`parse_args()`]: Attribute::parse_args
    ///
    /// <p><br></p>
    ///
    /// # Doc comments
    ///
    /// The compiler transforms doc comments, such as `/// comment` and `/*!
    /// comment */`, into attributes before macros are expanded. Each comment is
    /// expanded into an attribute of the form `#[doc = r"comment"]`.
    ///
    /// As an example, the following `mod` items are expanded identically:
    ///
    /// ```
    /// # use syn::{ItemMod, parse_quote};
    /// let doc: ItemMod = parse_quote! {
    ///     /// Single line doc comments
    ///     /// We write so many!
    ///     /**
    ///      * Multi-line comments...
    ///      * May span many lines
    ///      */
    ///     mod example {
    ///         //! Of course, they can be inner too
    ///         /*! And fit in a single line */
    ///     }
    /// };
    /// let attr: ItemMod = parse_quote! {
    ///     #[doc = r" Single line doc comments"]
    ///     #[doc = r" We write so many!"]
    ///     #[doc = r"
    ///      * Multi-line comments...
    ///      * May span many lines
    ///      "]
    ///     mod example {
    ///         #![doc = r" Of course, they can be inner too"]
    ///         #![doc = r" And fit in a single line "]
    ///     }
    /// };
    /// assert_eq!(doc, attr);
    /// ```
    #[cfg_attr(doc_cfg, doc(cfg(any(feature = "full", feature = "derive"))))]
    pub struct Attribute {
        pub pound_token: Token![#],
        pub style: AttrStyle,
        pub bracket_token: token::Bracket,
        pub path: Path,
        pub tokens: TokenStream,
    }
}

impl Attribute {
    /// Parses the content of the attribute, consisting of the path and tokens,
    /// as a [`Meta`] if possible.
    ///
    /// *This function is available only if Syn is built with the `"parsing"`
    /// feature.*
    #[cfg(feature = "parsing")]
    #[cfg_attr(doc_cfg, doc(cfg(feature = "parsing")))]
    pub fn parse_meta(&self) -> Result<Meta> {
        fn clone_ident_segment(segment: &PathSegment) -> PathSegment {
            PathSegment {
                ident: segment.ident.clone(),
                arguments: PathArguments::None,
            }
        }

        let path = Path {
            leading_colon: self
                .path
                .leading_colon
                .as_ref()
                .map(|colon| Token![::](colon.spans)),
            segments: self
                .path
                .segments
                .pairs()
                .map(|pair| match pair {
                    Pair::Punctuated(seg, punct) => {
                        Pair::Punctuated(clone_ident_segment(seg), Token![::](punct.spans))
                    }
                    Pair::End(seg) => Pair::End(clone_ident_segment(seg)),
                })
                .collect(),
        };

        let parser = |input: ParseStream| parsing::parse_meta_after_path(path, input);
        parse::Parser::parse2(parser, self.tokens.clone())
    }

    /// Parse the arguments to the attribute as a syntax tree.
    ///
    /// This is similar to `syn::parse2::<T>(attr.tokens)` except that:
    ///
    /// - the surrounding delimiters are *not* included in the input to the
    ///   parser; and
    /// - the error message has a more useful span when `tokens` is empty.
    ///
    /// ```text
    /// #[my_attr(value < 5)]
    ///           ^^^^^^^^^ what gets parsed
    /// ```
    ///
    /// *This function is available only if Syn is built with the `"parsing"`
    /// feature.*
    #[cfg(feature = "parsing")]
    #[cfg_attr(doc_cfg, doc(cfg(feature = "parsing")))]
    pub fn parse_args<T: Parse>(&self) -> Result<T> {
        self.parse_args_with(T::parse)
    }

    /// Parse the arguments to the attribute using the given parser.
    ///
    /// *This function is available only if Syn is built with the `"parsing"`
    /// feature.*
    #[cfg(feature = "parsing")]
    #[cfg_attr(doc_cfg, doc(cfg(feature = "parsing")))]
    pub fn parse_args_with<F: Parser>(&self, parser: F) -> Result<F::Output> {
        let parser = |input: ParseStream| {
            let args = enter_args(self, input)?;
            parse::parse_stream(parser, &args)
        };
        parser.parse2(self.tokens.clone())
    }

    /// Parses zero or more outer attributes from the stream.
    ///
    /// *This function is available only if Syn is built with the `"parsing"`
    /// feature.*
    #[cfg(feature = "parsing")]
    #[cfg_attr(doc_cfg, doc(cfg(feature = "parsing")))]
    pub fn parse_outer(input: ParseStream) -> Result<Vec<Self>> {
        let mut attrs = Vec::new();
        while input.peek(Token![#]) {
            attrs.push(input.call(parsing::single_parse_outer)?);
        }
        Ok(attrs)
    }

    /// Parses zero or more inner attributes from the stream.
    ///
    /// *This function is available only if Syn is built with the `"parsing"`
    /// feature.*
    #[cfg(feature = "parsing")]
    #[cfg_attr(doc_cfg, doc(cfg(feature = "parsing")))]
    pub fn parse_inner(input: ParseStream) -> Result<Vec<Self>> {
        let mut attrs = Vec::new();
        parsing::parse_inner(input, &mut attrs)?;
        Ok(attrs)
    }
}

#[cfg(feature = "parsing")]
fn expected_parentheses(attr: &Attribute) -> String {
    let style = match attr.style {
        AttrStyle::Outer => "#",
        AttrStyle::Inner(_) => "#!",
    };

    let mut path = String::new();
    for segment in &attr.path.segments {
        if !path.is_empty() || attr.path.leading_colon.is_some() {
            path += "::";
        }
        path += &segment.ident.to_string();
    }

    format!("{}[{}(...)]", style, path)
}

#[cfg(feature = "parsing")]
fn enter_args<'a>(attr: &Attribute, input: ParseStream<'a>) -> Result<ParseBuffer<'a>> {
    if input.is_empty() {
        let expected = expected_parentheses(attr);
        let msg = format!("expected attribute arguments in parentheses: {}", expected);
        return Err(crate::error::new2(
            attr.pound_token.span,
            attr.bracket_token.span,
            msg,
        ));
    } else if input.peek(Token![=]) {
        let expected = expected_parentheses(attr);
        let msg = format!("expected parentheses: {}", expected);
        return Err(input.error(msg));
    };

    let content;
    if input.peek(token::Paren) {
        parenthesized!(content in input);
    } else if input.peek(token::Bracket) {
        bracketed!(content in input);
    } else if input.peek(token::Brace) {
        braced!(content in input);
    } else {
        return Err(input.error("unexpected token in attribute arguments"));
    }

    if input.is_empty() {
        Ok(content)
    } else {
        Err(input.error("unexpected token in attribute arguments"))
    }
}

ast_enum! {
    /// Distinguishes between attributes that decorate an item and attributes
    /// that are contained within an item.
    ///
    /// *This type is available only if Syn is built with the `"derive"` or `"full"`
    /// feature.*
    ///
    /// # Outer attributes
    ///
    /// - `#[repr(transparent)]`
    /// - `/// # Example`
    /// - `/** Please file an issue */`
    ///
    /// # Inner attributes
    ///
    /// - `#![feature(proc_macro)]`
    /// - `//! # Example`
    /// - `/*! Please file an issue */`
    #[cfg_attr(doc_cfg, doc(cfg(any(feature = "full", feature = "derive"))))]
    pub enum AttrStyle {
        Outer,
        Inner(Token![!]),
    }
}

ast_enum_of_structs! {
    /// Content of a compile-time structured attribute.
    ///
    /// *This type is available only if Syn is built with the `"derive"` or `"full"`
    /// feature.*
    ///
    /// ## Path
    ///
    /// A meta path is like the `test` in `#[test]`.
    ///
    /// ## List
    ///
    /// A meta list is like the `derive(Copy)` in `#[derive(Copy)]`.
    ///
    /// ## NameValue
    ///
    /// A name-value meta is like the `path = "..."` in `#[path =
    /// "sys/windows.rs"]`.
    ///
    /// # Syntax tree enum
    ///
    /// This type is a [syntax tree enum].
    ///
    /// [syntax tree enum]: Expr#syntax-tree-enums
    #[cfg_attr(doc_cfg, doc(cfg(any(feature = "full", feature = "derive"))))]
    pub enum Meta {
        Path(Path),

        /// A structured list within an attribute, like `derive(Copy, Clone)`.
        List(MetaList),

        /// A name-value pair within an attribute, like `feature = "nightly"`.
        NameValue(MetaNameValue),
    }
}

ast_struct! {
    /// A structured list within an attribute, like `derive(Copy, Clone)`.
    ///
    /// *This type is available only if Syn is built with the `"derive"` or
    /// `"full"` feature.*
    #[cfg_attr(doc_cfg, doc(cfg(any(feature = "full", feature = "derive"))))]
    pub struct MetaList {
        pub path: Path,
        pub paren_token: token::Paren,
        pub nested: Punctuated<NestedMeta, Token![,]>,
    }
}

ast_struct! {
    /// A name-value pair within an attribute, like `feature = "nightly"`.
    ///
    /// *This type is available only if Syn is built with the `"derive"` or
    /// `"full"` feature.*
    #[cfg_attr(doc_cfg, doc(cfg(any(feature = "full", feature = "derive"))))]
    pub struct MetaNameValue {
        pub path: Path,
        pub eq_token: Token![=],
        pub lit: Lit,
    }
}

impl Meta {
    /// Returns the identifier that begins this structured meta item.
    ///
    /// For example this would return the `test` in `#[test]`, the `derive` in
    /// `#[derive(Copy)]`, and the `path` in `#[path = "sys/windows.rs"]`.
    pub fn path(&self) -> &Path {
        match self {
            Meta::Path(path) => path,
            Meta::List(meta) => &meta.path,
            Meta::NameValue(meta) => &meta.path,
        }
    }
}

ast_enum_of_structs! {
    /// Element of a compile-time attribute list.
    ///
    /// *This type is available only if Syn is built with the `"derive"` or `"full"`
    /// feature.*
    #[cfg_attr(doc_cfg, doc(cfg(any(feature = "full", feature = "derive"))))]
    pub enum NestedMeta {
        /// A structured meta item, like the `Copy` in `#[derive(Copy)]` which
        /// would be a nested `Meta::Path`.
        Meta(Meta),

        /// A Rust literal, like the `"new_name"` in `#[rename("new_name")]`.
        Lit(Lit),
    }
}

/// Conventional argument type associated with an invocation of an attribute
/// macro.
///
/// For example if we are developing an attribute macro that is intended to be
/// invoked on function items as follows:
///
/// ```
/// # const IGNORE: &str = stringify! {
/// #[my_attribute(path = "/v1/refresh")]
/// # };
/// pub fn refresh() {
///     /* ... */
/// }
/// ```
///
/// The implementation of this macro would want to parse its attribute arguments
/// as type `AttributeArgs`.
///
/// ```
/// # extern crate proc_macro;
/// #
/// use proc_macro::TokenStream;
/// use syn::{parse_macro_input, AttributeArgs, ItemFn};
///
/// # const IGNORE: &str = stringify! {
/// #[proc_macro_attribute]
/// # };
/// pub fn my_attribute(args: TokenStream, input: TokenStream) -> TokenStream {
///     let args = parse_macro_input!(args as AttributeArgs);
///     let input = parse_macro_input!(input as ItemFn);
///
///     /* ... */
/// #   "".parse().unwrap()
/// }
/// ```
#[cfg_attr(doc_cfg, doc(cfg(any(feature = "full", feature = "derive"))))]
pub type AttributeArgs = Vec<NestedMeta>;

pub trait FilterAttrs<'a> {
    type Ret: Iterator<Item = &'a Attribute>;

    fn outer(self) -> Self::Ret;
    fn inner(self) -> Self::Ret;
}

impl<'a> FilterAttrs<'a> for &'a [Attribute] {
    type Ret = iter::Filter<slice::Iter<'a, Attribute>, fn(&&Attribute) -> bool>;

    fn outer(self) -> Self::Ret {
        fn is_outer(attr: &&Attribute) -> bool {
            match attr.style {
                AttrStyle::Outer => true,
                AttrStyle::Inner(_) => false,
            }
        }
        self.iter().filter(is_outer)
    }

    fn inner(self) -> Self::Ret {
        fn is_inner(attr: &&Attribute) -> bool {
            match attr.style {
                AttrStyle::Inner(_) => true,
                AttrStyle::Outer => false,
            }
        }
        self.iter().filter(is_inner)
    }
}

#[cfg(feature = "parsing")]
pub mod parsing {
    use super::*;
    use crate::ext::IdentExt;
    use crate::parse::{Parse, ParseStream, Result};

    pub fn parse_inner(input: ParseStream, attrs: &mut Vec<Attribute>) -> Result<()> {
        while input.peek(Token![#]) && input.peek2(Token![!]) {
            attrs.push(input.call(parsing::single_parse_inner)?);
        }
        Ok(())
    }

    pub fn single_parse_inner(input: ParseStream) -> Result<Attribute> {
        let content;
        Ok(Attribute {
            pound_token: input.parse()?,
            style: AttrStyle::Inner(input.parse()?),
            bracket_token: bracketed!(content in input),
            path: content.call(Path::parse_mod_style)?,
            tokens: content.parse()?,
        })
    }

    pub fn single_parse_outer(input: ParseStream) -> Result<Attribute> {
        let content;
        Ok(Attribute {
            pound_token: input.parse()?,
            style: AttrStyle::Outer,
            bracket_token: bracketed!(content in input),
            path: content.call(Path::parse_mod_style)?,
            tokens: content.parse()?,
        })
    }

    // Like Path::parse_mod_style but accepts keywords in the path.
    fn parse_meta_path(input: ParseStream) -> Result<Path> {
        Ok(Path {
            leading_colon: input.parse()?,
            segments: {
                let mut segments = Punctuated::new();
                while input.peek(Ident::peek_any) {
                    let ident = Ident::parse_any(input)?;
                    segments.push_value(PathSegment::from(ident));
                    if !input.peek(Token![::]) {
                        break;
                    }
                    let punct = input.parse()?;
                    segments.push_punct(punct);
                }
                if segments.is_empty() {
                    return Err(input.error("expected path"));
                } else if segments.trailing_punct() {
                    return Err(input.error("expected path segment"));
                }
                segments
            },
        })
    }

    #[cfg_attr(doc_cfg, doc(cfg(feature = "parsing")))]
    impl Parse for Meta {
        fn parse(input: ParseStream) -> Result<Self> {
            let path = input.call(parse_meta_path)?;
            parse_meta_after_path(path, input)
        }
    }

    #[cfg_attr(doc_cfg, doc(cfg(feature = "parsing")))]
    impl Parse for MetaList {
        fn parse(input: ParseStream) -> Result<Self> {
            let path = input.call(parse_meta_path)?;
            parse_meta_list_after_path(path, input)
        }
    }

    #[cfg_attr(doc_cfg, doc(cfg(feature = "parsing")))]
    impl Parse for MetaNameValue {
        fn parse(input: ParseStream) -> Result<Self> {
            let path = input.call(parse_meta_path)?;
            parse_meta_name_value_after_path(path, input)
        }
    }

    #[cfg_attr(doc_cfg, doc(cfg(feature = "parsing")))]
    impl Parse for NestedMeta {
        fn parse(input: ParseStream) -> Result<Self> {
            if input.peek(Lit) && !(input.peek(LitBool) && input.peek2(Token![=])) {
                input.parse().map(NestedMeta::Lit)
            } else if input.peek(Ident::peek_any)
                || input.peek(Token![::]) && input.peek3(Ident::peek_any)
            {
                input.parse().map(NestedMeta::Meta)
            } else {
                Err(input.error("expected identifier or literal"))
            }
        }
    }

    pub fn parse_meta_after_path(path: Path, input: ParseStream) -> Result<Meta> {
        if input.peek(token::Paren) {
            parse_meta_list_after_path(path, input).map(Meta::List)
        } else if input.peek(Token![=]) {
            parse_meta_name_value_after_path(path, input).map(Meta::NameValue)
        } else {
            Ok(Meta::Path(path))
        }
    }

    fn parse_meta_list_after_path(path: Path, input: ParseStream) -> Result<MetaList> {
        let content;
        Ok(MetaList {
            path,
            paren_token: parenthesized!(content in input),
            nested: content.parse_terminated(NestedMeta::parse)?,
        })
    }

    fn parse_meta_name_value_after_path(path: Path, input: ParseStream) -> Result<MetaNameValue> {
        Ok(MetaNameValue {
            path,
            eq_token: input.parse()?,
            lit: input.parse()?,
        })
    }
}

#[cfg(feature = "printing")]
mod printing {
    use super::*;
    use proc_macro2::TokenStream;
    use quote::ToTokens;

    #[cfg_attr(doc_cfg, doc(cfg(feature = "printing")))]
    impl ToTokens for Attribute {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            self.pound_token.to_tokens(tokens);
            if let AttrStyle::Inner(b) = &self.style {
                b.to_tokens(tokens);
            }
            self.bracket_token.surround(tokens, |tokens| {
                self.path.to_tokens(tokens);
                self.tokens.to_tokens(tokens);
            });
        }
    }

    #[cfg_attr(doc_cfg, doc(cfg(feature = "printing")))]
    impl ToTokens for MetaList {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            self.path.to_tokens(tokens);
            self.paren_token.surround(tokens, |tokens| {
                self.nested.to_tokens(tokens);
            });
        }
    }

    #[cfg_attr(doc_cfg, doc(cfg(feature = "printing")))]
    impl ToTokens for MetaNameValue {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            self.path.to_tokens(tokens);
            self.eq_token.to_tokens(tokens);
            self.lit.to_tokens(tokens);
        }
    }
}

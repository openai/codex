use super::*;
use crate::punctuated::Punctuated;

ast_struct! {
    /// An enum variant.
    ///
    /// *This type is available only if Syn is built with the `"derive"` or `"full"`
    /// feature.*
    #[cfg_attr(doc_cfg, doc(cfg(any(feature = "full", feature = "derive"))))]
    pub struct Variant {
        /// Attributes tagged on the variant.
        pub attrs: Vec<Attribute>,

        /// Name of the variant.
        pub ident: Ident,

        /// Content stored in the variant.
        pub fields: Fields,

        /// Explicit discriminant: `Variant = 1`
        pub discriminant: Option<(Token![=], Expr)>,
    }
}

ast_enum_of_structs! {
    /// Data stored within an enum variant or struct.
    ///
    /// *This type is available only if Syn is built with the `"derive"` or `"full"`
    /// feature.*
    ///
    /// # Syntax tree enum
    ///
    /// This type is a [syntax tree enum].
    ///
    /// [syntax tree enum]: Expr#syntax-tree-enums
    #[cfg_attr(doc_cfg, doc(cfg(any(feature = "full", feature = "derive"))))]
    pub enum Fields {
        /// Named fields of a struct or struct variant such as `Point { x: f64,
        /// y: f64 }`.
        Named(FieldsNamed),

        /// Unnamed fields of a tuple struct or tuple variant such as `Some(T)`.
        Unnamed(FieldsUnnamed),

        /// Unit struct or unit variant such as `None`.
        Unit,
    }
}

ast_struct! {
    /// Named fields of a struct or struct variant such as `Point { x: f64,
    /// y: f64 }`.
    ///
    /// *This type is available only if Syn is built with the `"derive"` or
    /// `"full"` feature.*
    #[cfg_attr(doc_cfg, doc(cfg(any(feature = "full", feature = "derive"))))]
    pub struct FieldsNamed {
        pub brace_token: token::Brace,
        pub named: Punctuated<Field, Token![,]>,
    }
}

ast_struct! {
    /// Unnamed fields of a tuple struct or tuple variant such as `Some(T)`.
    ///
    /// *This type is available only if Syn is built with the `"derive"` or
    /// `"full"` feature.*
    #[cfg_attr(doc_cfg, doc(cfg(any(feature = "full", feature = "derive"))))]
    pub struct FieldsUnnamed {
        pub paren_token: token::Paren,
        pub unnamed: Punctuated<Field, Token![,]>,
    }
}

impl Fields {
    /// Get an iterator over the borrowed [`Field`] items in this object. This
    /// iterator can be used to iterate over a named or unnamed struct or
    /// variant's fields uniformly.
    pub fn iter(&self) -> punctuated::Iter<Field> {
        match self {
            Fields::Unit => crate::punctuated::empty_punctuated_iter(),
            Fields::Named(f) => f.named.iter(),
            Fields::Unnamed(f) => f.unnamed.iter(),
        }
    }

    /// Get an iterator over the mutably borrowed [`Field`] items in this
    /// object. This iterator can be used to iterate over a named or unnamed
    /// struct or variant's fields uniformly.
    pub fn iter_mut(&mut self) -> punctuated::IterMut<Field> {
        match self {
            Fields::Unit => crate::punctuated::empty_punctuated_iter_mut(),
            Fields::Named(f) => f.named.iter_mut(),
            Fields::Unnamed(f) => f.unnamed.iter_mut(),
        }
    }

    /// Returns the number of fields.
    pub fn len(&self) -> usize {
        match self {
            Fields::Unit => 0,
            Fields::Named(f) => f.named.len(),
            Fields::Unnamed(f) => f.unnamed.len(),
        }
    }

    /// Returns `true` if there are zero fields.
    pub fn is_empty(&self) -> bool {
        match self {
            Fields::Unit => true,
            Fields::Named(f) => f.named.is_empty(),
            Fields::Unnamed(f) => f.unnamed.is_empty(),
        }
    }
}

impl IntoIterator for Fields {
    type Item = Field;
    type IntoIter = punctuated::IntoIter<Field>;

    fn into_iter(self) -> Self::IntoIter {
        match self {
            Fields::Unit => Punctuated::<Field, ()>::new().into_iter(),
            Fields::Named(f) => f.named.into_iter(),
            Fields::Unnamed(f) => f.unnamed.into_iter(),
        }
    }
}

impl<'a> IntoIterator for &'a Fields {
    type Item = &'a Field;
    type IntoIter = punctuated::Iter<'a, Field>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a> IntoIterator for &'a mut Fields {
    type Item = &'a mut Field;
    type IntoIter = punctuated::IterMut<'a, Field>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter_mut()
    }
}

ast_struct! {
    /// A field of a struct or enum variant.
    ///
    /// *This type is available only if Syn is built with the `"derive"` or `"full"`
    /// feature.*
    #[cfg_attr(doc_cfg, doc(cfg(any(feature = "full", feature = "derive"))))]
    pub struct Field {
        /// Attributes tagged on the field.
        pub attrs: Vec<Attribute>,

        /// Visibility of the field.
        pub vis: Visibility,

        /// Name of the field, if any.
        ///
        /// Fields of tuple structs have no names.
        pub ident: Option<Ident>,

        pub colon_token: Option<Token![:]>,

        /// Type of the field.
        pub ty: Type,
    }
}

ast_enum_of_structs! {
    /// The visibility level of an item: inherited or `pub` or
    /// `pub(restricted)`.
    ///
    /// *This type is available only if Syn is built with the `"derive"` or `"full"`
    /// feature.*
    ///
    /// # Syntax tree enum
    ///
    /// This type is a [syntax tree enum].
    ///
    /// [syntax tree enum]: Expr#syntax-tree-enums
    #[cfg_attr(doc_cfg, doc(cfg(any(feature = "full", feature = "derive"))))]
    pub enum Visibility {
        /// A public visibility level: `pub`.
        Public(VisPublic),

        /// A crate-level visibility: `crate`.
        Crate(VisCrate),

        /// A visibility level restricted to some path: `pub(self)` or
        /// `pub(super)` or `pub(crate)` or `pub(in some::module)`.
        Restricted(VisRestricted),

        /// An inherited visibility, which usually means private.
        Inherited,
    }
}

ast_struct! {
    /// A public visibility level: `pub`.
    ///
    /// *This type is available only if Syn is built with the `"derive"` or
    /// `"full"` feature.*
    #[cfg_attr(doc_cfg, doc(cfg(any(feature = "full", feature = "derive"))))]
    pub struct VisPublic {
        pub pub_token: Token![pub],
    }
}

ast_struct! {
    /// A crate-level visibility: `crate`.
    ///
    /// *This type is available only if Syn is built with the `"derive"` or
    /// `"full"` feature.*
    #[cfg_attr(doc_cfg, doc(cfg(any(feature = "full", feature = "derive"))))]
    pub struct VisCrate {
        pub crate_token: Token![crate],
    }
}

ast_struct! {
    /// A visibility level restricted to some path: `pub(self)` or
    /// `pub(super)` or `pub(crate)` or `pub(in some::module)`.
    ///
    /// *This type is available only if Syn is built with the `"derive"` or
    /// `"full"` feature.*
    #[cfg_attr(doc_cfg, doc(cfg(any(feature = "full", feature = "derive"))))]
    pub struct VisRestricted {
        pub pub_token: Token![pub],
        pub paren_token: token::Paren,
        pub in_token: Option<Token![in]>,
        pub path: Box<Path>,
    }
}

#[cfg(feature = "parsing")]
pub mod parsing {
    use super::*;
    use crate::ext::IdentExt;
    use crate::parse::discouraged::Speculative;
    use crate::parse::{Parse, ParseStream, Result};

    #[cfg_attr(doc_cfg, doc(cfg(feature = "parsing")))]
    impl Parse for Variant {
        fn parse(input: ParseStream) -> Result<Self> {
            let attrs = input.call(Attribute::parse_outer)?;
            let _visibility: Visibility = input.parse()?;
            let ident: Ident = input.parse()?;
            let fields = if input.peek(token::Brace) {
                Fields::Named(input.parse()?)
            } else if input.peek(token::Paren) {
                Fields::Unnamed(input.parse()?)
            } else {
                Fields::Unit
            };
            let discriminant = if input.peek(Token![=]) {
                let eq_token: Token![=] = input.parse()?;
                let discriminant: Expr = input.parse()?;
                Some((eq_token, discriminant))
            } else {
                None
            };
            Ok(Variant {
                attrs,
                ident,
                fields,
                discriminant,
            })
        }
    }

    #[cfg_attr(doc_cfg, doc(cfg(feature = "parsing")))]
    impl Parse for FieldsNamed {
        fn parse(input: ParseStream) -> Result<Self> {
            let content;
            Ok(FieldsNamed {
                brace_token: braced!(content in input),
                named: content.parse_terminated(Field::parse_named)?,
            })
        }
    }

    #[cfg_attr(doc_cfg, doc(cfg(feature = "parsing")))]
    impl Parse for FieldsUnnamed {
        fn parse(input: ParseStream) -> Result<Self> {
            let content;
            Ok(FieldsUnnamed {
                paren_token: parenthesized!(content in input),
                unnamed: content.parse_terminated(Field::parse_unnamed)?,
            })
        }
    }

    impl Field {
        /// Parses a named (braced struct) field.
        #[cfg_attr(doc_cfg, doc(cfg(feature = "parsing")))]
        pub fn parse_named(input: ParseStream) -> Result<Self> {
            Ok(Field {
                attrs: input.call(Attribute::parse_outer)?,
                vis: input.parse()?,
                ident: Some(if input.peek(Token![_]) {
                    input.call(Ident::parse_any)
                } else {
                    input.parse()
                }?),
                colon_token: Some(input.parse()?),
                ty: input.parse()?,
            })
        }

        /// Parses an unnamed (tuple struct) field.
        #[cfg_attr(doc_cfg, doc(cfg(feature = "parsing")))]
        pub fn parse_unnamed(input: ParseStream) -> Result<Self> {
            Ok(Field {
                attrs: input.call(Attribute::parse_outer)?,
                vis: input.parse()?,
                ident: None,
                colon_token: None,
                ty: input.parse()?,
            })
        }
    }

    #[cfg_attr(doc_cfg, doc(cfg(feature = "parsing")))]
    impl Parse for Visibility {
        fn parse(input: ParseStream) -> Result<Self> {
            // Recognize an empty None-delimited group, as produced by a $:vis
            // matcher that matched no tokens.
            if input.peek(token::Group) {
                let ahead = input.fork();
                let group = crate::group::parse_group(&ahead)?;
                if group.content.is_empty() {
                    input.advance_to(&ahead);
                    return Ok(Visibility::Inherited);
                }
            }

            if input.peek(Token![pub]) {
                Self::parse_pub(input)
            } else if input.peek(Token![crate]) {
                Self::parse_crate(input)
            } else {
                Ok(Visibility::Inherited)
            }
        }
    }

    impl Visibility {
        fn parse_pub(input: ParseStream) -> Result<Self> {
            let pub_token = input.parse::<Token![pub]>()?;

            if input.peek(token::Paren) {
                let ahead = input.fork();

                let content;
                let paren_token = parenthesized!(content in ahead);
                if content.peek(Token![crate])
                    || content.peek(Token![self])
                    || content.peek(Token![super])
                {
                    let path = content.call(Ident::parse_any)?;

                    // Ensure there are no additional tokens within `content`.
                    // Without explicitly checking, we may misinterpret a tuple
                    // field as a restricted visibility, causing a parse error.
                    // e.g. `pub (crate::A, crate::B)` (Issue #720).
                    if content.is_empty() {
                        input.advance_to(&ahead);
                        return Ok(Visibility::Restricted(VisRestricted {
                            pub_token,
                            paren_token,
                            in_token: None,
                            path: Box::new(Path::from(path)),
                        }));
                    }
                } else if content.peek(Token![in]) {
                    let in_token: Token![in] = content.parse()?;
                    let path = content.call(Path::parse_mod_style)?;

                    input.advance_to(&ahead);
                    return Ok(Visibility::Restricted(VisRestricted {
                        pub_token,
                        paren_token,
                        in_token: Some(in_token),
                        path: Box::new(path),
                    }));
                }
            }

            Ok(Visibility::Public(VisPublic { pub_token }))
        }

        fn parse_crate(input: ParseStream) -> Result<Self> {
            if input.peek2(Token![::]) {
                Ok(Visibility::Inherited)
            } else {
                Ok(Visibility::Crate(VisCrate {
                    crate_token: input.parse()?,
                }))
            }
        }

        #[cfg(feature = "full")]
        pub(crate) fn is_some(&self) -> bool {
            match self {
                Visibility::Inherited => false,
                _ => true,
            }
        }
    }
}

#[cfg(feature = "printing")]
mod printing {
    use super::*;
    use crate::print::TokensOrDefault;
    use proc_macro2::TokenStream;
    use quote::{ToTokens, TokenStreamExt};

    #[cfg_attr(doc_cfg, doc(cfg(feature = "printing")))]
    impl ToTokens for Variant {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            tokens.append_all(&self.attrs);
            self.ident.to_tokens(tokens);
            self.fields.to_tokens(tokens);
            if let Some((eq_token, disc)) = &self.discriminant {
                eq_token.to_tokens(tokens);
                disc.to_tokens(tokens);
            }
        }
    }

    #[cfg_attr(doc_cfg, doc(cfg(feature = "printing")))]
    impl ToTokens for FieldsNamed {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            self.brace_token.surround(tokens, |tokens| {
                self.named.to_tokens(tokens);
            });
        }
    }

    #[cfg_attr(doc_cfg, doc(cfg(feature = "printing")))]
    impl ToTokens for FieldsUnnamed {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            self.paren_token.surround(tokens, |tokens| {
                self.unnamed.to_tokens(tokens);
            });
        }
    }

    #[cfg_attr(doc_cfg, doc(cfg(feature = "printing")))]
    impl ToTokens for Field {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            tokens.append_all(&self.attrs);
            self.vis.to_tokens(tokens);
            if let Some(ident) = &self.ident {
                ident.to_tokens(tokens);
                TokensOrDefault(&self.colon_token).to_tokens(tokens);
            }
            self.ty.to_tokens(tokens);
        }
    }

    #[cfg_attr(doc_cfg, doc(cfg(feature = "printing")))]
    impl ToTokens for VisPublic {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            self.pub_token.to_tokens(tokens);
        }
    }

    #[cfg_attr(doc_cfg, doc(cfg(feature = "printing")))]
    impl ToTokens for VisCrate {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            self.crate_token.to_tokens(tokens);
        }
    }

    #[cfg_attr(doc_cfg, doc(cfg(feature = "printing")))]
    impl ToTokens for VisRestricted {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            self.pub_token.to_tokens(tokens);
            self.paren_token.surround(tokens, |tokens| {
                // TODO: If we have a path which is not "self" or "super" or
                // "crate", automatically add the "in" token.
                self.in_token.to_tokens(tokens);
                self.path.to_tokens(tokens);
            });
        }
    }
}

use super::*;
use crate::punctuated::Punctuated;
use proc_macro2::TokenStream;

ast_enum_of_structs! {
    /// A pattern in a local binding, function signature, match expression, or
    /// various other places.
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
    pub enum Pat {
        /// A box pattern: `box v`.
        Box(PatBox),

        /// A pattern that binds a new variable: `ref mut binding @ SUBPATTERN`.
        Ident(PatIdent),

        /// A literal pattern: `0`.
        ///
        /// This holds an `Expr` rather than a `Lit` because negative numbers
        /// are represented as an `Expr::Unary`.
        Lit(PatLit),

        /// A macro in pattern position.
        Macro(PatMacro),

        /// A pattern that matches any one of a set of cases.
        Or(PatOr),

        /// A path pattern like `Color::Red`, optionally qualified with a
        /// self-type.
        ///
        /// Unqualified path patterns can legally refer to variants, structs,
        /// constants or associated constants. Qualified path patterns like
        /// `<A>::B::C` and `<A as Trait>::B::C` can only legally refer to
        /// associated constants.
        Path(PatPath),

        /// A range pattern: `1..=2`.
        Range(PatRange),

        /// A reference pattern: `&mut var`.
        Reference(PatReference),

        /// The dots in a tuple or slice pattern: `[0, 1, ..]`
        Rest(PatRest),

        /// A dynamically sized slice pattern: `[a, b, ref i @ .., y, z]`.
        Slice(PatSlice),

        /// A struct or struct variant pattern: `Variant { x, y, .. }`.
        Struct(PatStruct),

        /// A tuple pattern: `(a, b)`.
        Tuple(PatTuple),

        /// A tuple struct or tuple variant pattern: `Variant(x, y, .., z)`.
        TupleStruct(PatTupleStruct),

        /// A type ascription pattern: `foo: f64`.
        Type(PatType),

        /// Tokens in pattern position not interpreted by Syn.
        Verbatim(TokenStream),

        /// A pattern that matches any value: `_`.
        Wild(PatWild),

        // Not public API.
        //
        // For testing exhaustiveness in downstream code, use the following idiom:
        //
        //     match pat {
        //         Pat::Box(pat) => {...}
        //         Pat::Ident(pat) => {...}
        //         ...
        //         Pat::Wild(pat) => {...}
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
    /// A box pattern: `box v`.
    ///
    /// *This type is available only if Syn is built with the `"full"` feature.*
    #[cfg_attr(doc_cfg, doc(cfg(feature = "full")))]
    pub struct PatBox {
        pub attrs: Vec<Attribute>,
        pub box_token: Token![box],
        pub pat: Box<Pat>,
    }
}

ast_struct! {
    /// A pattern that binds a new variable: `ref mut binding @ SUBPATTERN`.
    ///
    /// It may also be a unit struct or struct variant (e.g. `None`), or a
    /// constant; these cannot be distinguished syntactically.
    ///
    /// *This type is available only if Syn is built with the `"full"` feature.*
    #[cfg_attr(doc_cfg, doc(cfg(feature = "full")))]
    pub struct PatIdent {
        pub attrs: Vec<Attribute>,
        pub by_ref: Option<Token![ref]>,
        pub mutability: Option<Token![mut]>,
        pub ident: Ident,
        pub subpat: Option<(Token![@], Box<Pat>)>,
    }
}

ast_struct! {
    /// A literal pattern: `0`.
    ///
    /// This holds an `Expr` rather than a `Lit` because negative numbers
    /// are represented as an `Expr::Unary`.
    ///
    /// *This type is available only if Syn is built with the `"full"` feature.*
    #[cfg_attr(doc_cfg, doc(cfg(feature = "full")))]
    pub struct PatLit {
        pub attrs: Vec<Attribute>,
        pub expr: Box<Expr>,
    }
}

ast_struct! {
    /// A macro in pattern position.
    ///
    /// *This type is available only if Syn is built with the `"full"` feature.*
    #[cfg_attr(doc_cfg, doc(cfg(feature = "full")))]
    pub struct PatMacro {
        pub attrs: Vec<Attribute>,
        pub mac: Macro,
    }
}

ast_struct! {
    /// A pattern that matches any one of a set of cases.
    ///
    /// *This type is available only if Syn is built with the `"full"` feature.*
    #[cfg_attr(doc_cfg, doc(cfg(feature = "full")))]
    pub struct PatOr {
        pub attrs: Vec<Attribute>,
        pub leading_vert: Option<Token![|]>,
        pub cases: Punctuated<Pat, Token![|]>,
    }
}

ast_struct! {
    /// A path pattern like `Color::Red`, optionally qualified with a
    /// self-type.
    ///
    /// Unqualified path patterns can legally refer to variants, structs,
    /// constants or associated constants. Qualified path patterns like
    /// `<A>::B::C` and `<A as Trait>::B::C` can only legally refer to
    /// associated constants.
    ///
    /// *This type is available only if Syn is built with the `"full"` feature.*
    #[cfg_attr(doc_cfg, doc(cfg(feature = "full")))]
    pub struct PatPath {
        pub attrs: Vec<Attribute>,
        pub qself: Option<QSelf>,
        pub path: Path,
    }
}

ast_struct! {
    /// A range pattern: `1..=2`.
    ///
    /// *This type is available only if Syn is built with the `"full"` feature.*
    #[cfg_attr(doc_cfg, doc(cfg(feature = "full")))]
    pub struct PatRange {
        pub attrs: Vec<Attribute>,
        pub lo: Box<Expr>,
        pub limits: RangeLimits,
        pub hi: Box<Expr>,
    }
}

ast_struct! {
    /// A reference pattern: `&mut var`.
    ///
    /// *This type is available only if Syn is built with the `"full"` feature.*
    #[cfg_attr(doc_cfg, doc(cfg(feature = "full")))]
    pub struct PatReference {
        pub attrs: Vec<Attribute>,
        pub and_token: Token![&],
        pub mutability: Option<Token![mut]>,
        pub pat: Box<Pat>,
    }
}

ast_struct! {
    /// The dots in a tuple or slice pattern: `[0, 1, ..]`
    ///
    /// *This type is available only if Syn is built with the `"full"` feature.*
    #[cfg_attr(doc_cfg, doc(cfg(feature = "full")))]
    pub struct PatRest {
        pub attrs: Vec<Attribute>,
        pub dot2_token: Token![..],
    }
}

ast_struct! {
    /// A dynamically sized slice pattern: `[a, b, ref i @ .., y, z]`.
    ///
    /// *This type is available only if Syn is built with the `"full"` feature.*
    #[cfg_attr(doc_cfg, doc(cfg(feature = "full")))]
    pub struct PatSlice {
        pub attrs: Vec<Attribute>,
        pub bracket_token: token::Bracket,
        pub elems: Punctuated<Pat, Token![,]>,
    }
}

ast_struct! {
    /// A struct or struct variant pattern: `Variant { x, y, .. }`.
    ///
    /// *This type is available only if Syn is built with the `"full"` feature.*
    #[cfg_attr(doc_cfg, doc(cfg(feature = "full")))]
    pub struct PatStruct {
        pub attrs: Vec<Attribute>,
        pub path: Path,
        pub brace_token: token::Brace,
        pub fields: Punctuated<FieldPat, Token![,]>,
        pub dot2_token: Option<Token![..]>,
    }
}

ast_struct! {
    /// A tuple pattern: `(a, b)`.
    ///
    /// *This type is available only if Syn is built with the `"full"` feature.*
    #[cfg_attr(doc_cfg, doc(cfg(feature = "full")))]
    pub struct PatTuple {
        pub attrs: Vec<Attribute>,
        pub paren_token: token::Paren,
        pub elems: Punctuated<Pat, Token![,]>,
    }
}

ast_struct! {
    /// A tuple struct or tuple variant pattern: `Variant(x, y, .., z)`.
    ///
    /// *This type is available only if Syn is built with the `"full"` feature.*
    #[cfg_attr(doc_cfg, doc(cfg(feature = "full")))]
    pub struct PatTupleStruct {
        pub attrs: Vec<Attribute>,
        pub path: Path,
        pub pat: PatTuple,
    }
}

ast_struct! {
    /// A type ascription pattern: `foo: f64`.
    ///
    /// *This type is available only if Syn is built with the `"full"` feature.*
    #[cfg_attr(doc_cfg, doc(cfg(feature = "full")))]
    pub struct PatType {
        pub attrs: Vec<Attribute>,
        pub pat: Box<Pat>,
        pub colon_token: Token![:],
        pub ty: Box<Type>,
    }
}

ast_struct! {
    /// A pattern that matches any value: `_`.
    ///
    /// *This type is available only if Syn is built with the `"full"` feature.*
    #[cfg_attr(doc_cfg, doc(cfg(feature = "full")))]
    pub struct PatWild {
        pub attrs: Vec<Attribute>,
        pub underscore_token: Token![_],
    }
}

ast_struct! {
    /// A single field in a struct pattern.
    ///
    /// Patterns like the fields of Foo `{ x, ref y, ref mut z }` are treated
    /// the same as `x: x, y: ref y, z: ref mut z` but there is no colon token.
    ///
    /// *This type is available only if Syn is built with the `"full"` feature.*
    #[cfg_attr(doc_cfg, doc(cfg(feature = "full")))]
    pub struct FieldPat {
        pub attrs: Vec<Attribute>,
        pub member: Member,
        pub colon_token: Option<Token![:]>,
        pub pat: Box<Pat>,
    }
}

#[cfg(feature = "parsing")]
pub mod parsing {
    use super::*;
    use crate::ext::IdentExt;
    use crate::parse::{Parse, ParseBuffer, ParseStream, Result};
    use crate::path;

    #[cfg_attr(doc_cfg, doc(cfg(feature = "parsing")))]
    impl Parse for Pat {
        fn parse(input: ParseStream) -> Result<Self> {
            let begin = input.fork();
            let lookahead = input.lookahead1();
            if {
                let ahead = input.fork();
                ahead.parse::<Option<Ident>>()?.is_some()
                    && (ahead.peek(Token![::])
                        || ahead.peek(Token![!])
                        || ahead.peek(token::Brace)
                        || ahead.peek(token::Paren)
                        || ahead.peek(Token![..])
                            && ahead.parse::<RangeLimits>().is_ok()
                            && !(ahead.is_empty() || ahead.peek(Token![,])))
            } || {
                let ahead = input.fork();
                ahead.parse::<Option<Token![self]>>()?.is_some() && ahead.peek(Token![::])
            } || lookahead.peek(Token![::])
                || lookahead.peek(Token![<])
                || input.peek(Token![Self])
                || input.peek(Token![super])
                || input.peek(Token![crate])
            {
                pat_path_or_macro_or_struct_or_range(input)
            } else if lookahead.peek(Token![_]) {
                input.call(pat_wild).map(Pat::Wild)
            } else if input.peek(Token![box]) {
                input.call(pat_box).map(Pat::Box)
            } else if input.peek(Token![-]) || lookahead.peek(Lit) || lookahead.peek(Token![const])
            {
                pat_lit_or_range(input)
            } else if lookahead.peek(Token![ref])
                || lookahead.peek(Token![mut])
                || input.peek(Token![self])
                || input.peek(Ident)
            {
                input.call(pat_ident).map(Pat::Ident)
            } else if lookahead.peek(Token![&]) {
                input.call(pat_reference).map(Pat::Reference)
            } else if lookahead.peek(token::Paren) {
                input.call(pat_tuple).map(Pat::Tuple)
            } else if lookahead.peek(token::Bracket) {
                input.call(pat_slice).map(Pat::Slice)
            } else if lookahead.peek(Token![..]) && !input.peek(Token![...]) {
                pat_range_half_open(input, begin)
            } else if lookahead.peek(Token![const]) {
                input.call(pat_const).map(Pat::Verbatim)
            } else {
                Err(lookahead.error())
            }
        }
    }

    fn pat_path_or_macro_or_struct_or_range(input: ParseStream) -> Result<Pat> {
        let begin = input.fork();
        let (qself, path) = path::parsing::qpath(input, true)?;

        if qself.is_none() && input.peek(Token![!]) && !input.peek(Token![!=]) {
            let mut contains_arguments = false;
            for segment in &path.segments {
                match segment.arguments {
                    PathArguments::None => {}
                    PathArguments::AngleBracketed(_) | PathArguments::Parenthesized(_) => {
                        contains_arguments = true;
                    }
                }
            }

            if !contains_arguments {
                let bang_token: Token![!] = input.parse()?;
                let (delimiter, tokens) = mac::parse_delimiter(input)?;
                return Ok(Pat::Macro(PatMacro {
                    attrs: Vec::new(),
                    mac: Macro {
                        path,
                        bang_token,
                        delimiter,
                        tokens,
                    },
                }));
            }
        }

        if input.peek(token::Brace) {
            let pat = pat_struct(begin.fork(), input, path)?;
            if qself.is_some() {
                Ok(Pat::Verbatim(verbatim::between(begin, input)))
            } else {
                Ok(pat)
            }
        } else if input.peek(token::Paren) {
            let pat = pat_tuple_struct(input, path)?;
            if qself.is_some() {
                Ok(Pat::Verbatim(verbatim::between(begin, input)))
            } else {
                Ok(Pat::TupleStruct(pat))
            }
        } else if input.peek(Token![..]) {
            pat_range(input, begin, qself, path)
        } else {
            Ok(Pat::Path(PatPath {
                attrs: Vec::new(),
                qself,
                path,
            }))
        }
    }

    fn pat_wild(input: ParseStream) -> Result<PatWild> {
        Ok(PatWild {
            attrs: Vec::new(),
            underscore_token: input.parse()?,
        })
    }

    fn pat_box(input: ParseStream) -> Result<PatBox> {
        Ok(PatBox {
            attrs: Vec::new(),
            box_token: input.parse()?,
            pat: input.parse()?,
        })
    }

    fn pat_ident(input: ParseStream) -> Result<PatIdent> {
        Ok(PatIdent {
            attrs: Vec::new(),
            by_ref: input.parse()?,
            mutability: input.parse()?,
            ident: input.call(Ident::parse_any)?,
            subpat: {
                if input.peek(Token![@]) {
                    let at_token: Token![@] = input.parse()?;
                    let subpat: Pat = input.parse()?;
                    Some((at_token, Box::new(subpat)))
                } else {
                    None
                }
            },
        })
    }

    fn pat_tuple_struct(input: ParseStream, path: Path) -> Result<PatTupleStruct> {
        Ok(PatTupleStruct {
            attrs: Vec::new(),
            path,
            pat: input.call(pat_tuple)?,
        })
    }

    fn pat_struct(begin: ParseBuffer, input: ParseStream, path: Path) -> Result<Pat> {
        let content;
        let brace_token = braced!(content in input);

        let mut fields = Punctuated::new();
        let mut dot2_token = None;
        while !content.is_empty() {
            let attrs = content.call(Attribute::parse_outer)?;
            if content.peek(Token![..]) {
                dot2_token = Some(content.parse()?);
                if !attrs.is_empty() {
                    return Ok(Pat::Verbatim(verbatim::between(begin, input)));
                }
                break;
            }
            let mut value = content.call(field_pat)?;
            value.attrs = attrs;
            fields.push_value(value);
            if content.is_empty() {
                break;
            }
            let punct: Token![,] = content.parse()?;
            fields.push_punct(punct);
        }

        Ok(Pat::Struct(PatStruct {
            attrs: Vec::new(),
            path,
            brace_token,
            fields,
            dot2_token,
        }))
    }

    impl Member {
        fn is_unnamed(&self) -> bool {
            match *self {
                Member::Named(_) => false,
                Member::Unnamed(_) => true,
            }
        }
    }

    fn field_pat(input: ParseStream) -> Result<FieldPat> {
        let boxed: Option<Token![box]> = input.parse()?;
        let by_ref: Option<Token![ref]> = input.parse()?;
        let mutability: Option<Token![mut]> = input.parse()?;
        let member: Member = input.parse()?;

        if boxed.is_none() && by_ref.is_none() && mutability.is_none() && input.peek(Token![:])
            || member.is_unnamed()
        {
            return Ok(FieldPat {
                attrs: Vec::new(),
                member,
                colon_token: input.parse()?,
                pat: Box::new(multi_pat_with_leading_vert(input)?),
            });
        }

        let ident = match member {
            Member::Named(ident) => ident,
            Member::Unnamed(_) => unreachable!(),
        };

        let mut pat = Pat::Ident(PatIdent {
            attrs: Vec::new(),
            by_ref,
            mutability,
            ident: ident.clone(),
            subpat: None,
        });

        if let Some(boxed) = boxed {
            pat = Pat::Box(PatBox {
                attrs: Vec::new(),
                box_token: boxed,
                pat: Box::new(pat),
            });
        }

        Ok(FieldPat {
            attrs: Vec::new(),
            member: Member::Named(ident),
            colon_token: None,
            pat: Box::new(pat),
        })
    }

    fn pat_range(
        input: ParseStream,
        begin: ParseBuffer,
        qself: Option<QSelf>,
        path: Path,
    ) -> Result<Pat> {
        let limits: RangeLimits = input.parse()?;
        let hi = input.call(pat_lit_expr)?;
        if let Some(hi) = hi {
            Ok(Pat::Range(PatRange {
                attrs: Vec::new(),
                lo: Box::new(Expr::Path(ExprPath {
                    attrs: Vec::new(),
                    qself,
                    path,
                })),
                limits,
                hi,
            }))
        } else {
            Ok(Pat::Verbatim(verbatim::between(begin, input)))
        }
    }

    fn pat_range_half_open(input: ParseStream, begin: ParseBuffer) -> Result<Pat> {
        let limits: RangeLimits = input.parse()?;
        let hi = input.call(pat_lit_expr)?;
        if hi.is_some() {
            Ok(Pat::Verbatim(verbatim::between(begin, input)))
        } else {
            match limits {
                RangeLimits::HalfOpen(dot2_token) => Ok(Pat::Rest(PatRest {
                    attrs: Vec::new(),
                    dot2_token,
                })),
                RangeLimits::Closed(_) => Err(input.error("expected range upper bound")),
            }
        }
    }

    fn pat_tuple(input: ParseStream) -> Result<PatTuple> {
        let content;
        let paren_token = parenthesized!(content in input);

        let mut elems = Punctuated::new();
        while !content.is_empty() {
            let value = multi_pat_with_leading_vert(&content)?;
            elems.push_value(value);
            if content.is_empty() {
                break;
            }
            let punct = content.parse()?;
            elems.push_punct(punct);
        }

        Ok(PatTuple {
            attrs: Vec::new(),
            paren_token,
            elems,
        })
    }

    fn pat_reference(input: ParseStream) -> Result<PatReference> {
        Ok(PatReference {
            attrs: Vec::new(),
            and_token: input.parse()?,
            mutability: input.parse()?,
            pat: input.parse()?,
        })
    }

    fn pat_lit_or_range(input: ParseStream) -> Result<Pat> {
        let begin = input.fork();
        let lo = input.call(pat_lit_expr)?.unwrap();
        if input.peek(Token![..]) {
            let limits: RangeLimits = input.parse()?;
            let hi = input.call(pat_lit_expr)?;
            if let Some(hi) = hi {
                Ok(Pat::Range(PatRange {
                    attrs: Vec::new(),
                    lo,
                    limits,
                    hi,
                }))
            } else {
                Ok(Pat::Verbatim(verbatim::between(begin, input)))
            }
        } else if let Expr::Verbatim(verbatim) = *lo {
            Ok(Pat::Verbatim(verbatim))
        } else {
            Ok(Pat::Lit(PatLit {
                attrs: Vec::new(),
                expr: lo,
            }))
        }
    }

    fn pat_lit_expr(input: ParseStream) -> Result<Option<Box<Expr>>> {
        if input.is_empty()
            || input.peek(Token![|])
            || input.peek(Token![=])
            || input.peek(Token![:]) && !input.peek(Token![::])
            || input.peek(Token![,])
            || input.peek(Token![;])
        {
            return Ok(None);
        }

        let neg: Option<Token![-]> = input.parse()?;

        let lookahead = input.lookahead1();
        let expr = if lookahead.peek(Lit) {
            Expr::Lit(input.parse()?)
        } else if lookahead.peek(Ident)
            || lookahead.peek(Token![::])
            || lookahead.peek(Token![<])
            || lookahead.peek(Token![self])
            || lookahead.peek(Token![Self])
            || lookahead.peek(Token![super])
            || lookahead.peek(Token![crate])
        {
            Expr::Path(input.parse()?)
        } else if lookahead.peek(Token![const]) {
            Expr::Verbatim(input.call(expr::parsing::expr_const)?)
        } else {
            return Err(lookahead.error());
        };

        Ok(Some(Box::new(if let Some(neg) = neg {
            Expr::Unary(ExprUnary {
                attrs: Vec::new(),
                op: UnOp::Neg(neg),
                expr: Box::new(expr),
            })
        } else {
            expr
        })))
    }

    fn pat_slice(input: ParseStream) -> Result<PatSlice> {
        let content;
        let bracket_token = bracketed!(content in input);

        let mut elems = Punctuated::new();
        while !content.is_empty() {
            let value = multi_pat_with_leading_vert(&content)?;
            elems.push_value(value);
            if content.is_empty() {
                break;
            }
            let punct = content.parse()?;
            elems.push_punct(punct);
        }

        Ok(PatSlice {
            attrs: Vec::new(),
            bracket_token,
            elems,
        })
    }

    fn pat_const(input: ParseStream) -> Result<TokenStream> {
        let begin = input.fork();
        input.parse::<Token![const]>()?;

        let content;
        braced!(content in input);
        content.call(Attribute::parse_inner)?;
        content.call(Block::parse_within)?;

        Ok(verbatim::between(begin, input))
    }

    pub fn multi_pat(input: ParseStream) -> Result<Pat> {
        multi_pat_impl(input, None)
    }

    pub fn multi_pat_with_leading_vert(input: ParseStream) -> Result<Pat> {
        let leading_vert: Option<Token![|]> = input.parse()?;
        multi_pat_impl(input, leading_vert)
    }

    fn multi_pat_impl(input: ParseStream, leading_vert: Option<Token![|]>) -> Result<Pat> {
        let mut pat: Pat = input.parse()?;
        if leading_vert.is_some()
            || input.peek(Token![|]) && !input.peek(Token![||]) && !input.peek(Token![|=])
        {
            let mut cases = Punctuated::new();
            cases.push_value(pat);
            while input.peek(Token![|]) && !input.peek(Token![||]) && !input.peek(Token![|=]) {
                let punct = input.parse()?;
                cases.push_punct(punct);
                let pat: Pat = input.parse()?;
                cases.push_value(pat);
            }
            pat = Pat::Or(PatOr {
                attrs: Vec::new(),
                leading_vert,
                cases,
            });
        }
        Ok(pat)
    }
}

#[cfg(feature = "printing")]
mod printing {
    use super::*;
    use crate::attr::FilterAttrs;
    use proc_macro2::TokenStream;
    use quote::{ToTokens, TokenStreamExt};

    #[cfg_attr(doc_cfg, doc(cfg(feature = "printing")))]
    impl ToTokens for PatWild {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            tokens.append_all(self.attrs.outer());
            self.underscore_token.to_tokens(tokens);
        }
    }

    #[cfg_attr(doc_cfg, doc(cfg(feature = "printing")))]
    impl ToTokens for PatIdent {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            tokens.append_all(self.attrs.outer());
            self.by_ref.to_tokens(tokens);
            self.mutability.to_tokens(tokens);
            self.ident.to_tokens(tokens);
            if let Some((at_token, subpat)) = &self.subpat {
                at_token.to_tokens(tokens);
                subpat.to_tokens(tokens);
            }
        }
    }

    #[cfg_attr(doc_cfg, doc(cfg(feature = "printing")))]
    impl ToTokens for PatStruct {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            tokens.append_all(self.attrs.outer());
            self.path.to_tokens(tokens);
            self.brace_token.surround(tokens, |tokens| {
                self.fields.to_tokens(tokens);
                // NOTE: We need a comma before the dot2 token if it is present.
                if !self.fields.empty_or_trailing() && self.dot2_token.is_some() {
                    <Token![,]>::default().to_tokens(tokens);
                }
                self.dot2_token.to_tokens(tokens);
            });
        }
    }

    #[cfg_attr(doc_cfg, doc(cfg(feature = "printing")))]
    impl ToTokens for PatTupleStruct {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            tokens.append_all(self.attrs.outer());
            self.path.to_tokens(tokens);
            self.pat.to_tokens(tokens);
        }
    }

    #[cfg_attr(doc_cfg, doc(cfg(feature = "printing")))]
    impl ToTokens for PatType {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            tokens.append_all(self.attrs.outer());
            self.pat.to_tokens(tokens);
            self.colon_token.to_tokens(tokens);
            self.ty.to_tokens(tokens);
        }
    }

    #[cfg_attr(doc_cfg, doc(cfg(feature = "printing")))]
    impl ToTokens for PatPath {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            tokens.append_all(self.attrs.outer());
            path::printing::print_path(tokens, &self.qself, &self.path);
        }
    }

    #[cfg_attr(doc_cfg, doc(cfg(feature = "printing")))]
    impl ToTokens for PatTuple {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            tokens.append_all(self.attrs.outer());
            self.paren_token.surround(tokens, |tokens| {
                self.elems.to_tokens(tokens);
            });
        }
    }

    #[cfg_attr(doc_cfg, doc(cfg(feature = "printing")))]
    impl ToTokens for PatBox {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            tokens.append_all(self.attrs.outer());
            self.box_token.to_tokens(tokens);
            self.pat.to_tokens(tokens);
        }
    }

    #[cfg_attr(doc_cfg, doc(cfg(feature = "printing")))]
    impl ToTokens for PatReference {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            tokens.append_all(self.attrs.outer());
            self.and_token.to_tokens(tokens);
            self.mutability.to_tokens(tokens);
            self.pat.to_tokens(tokens);
        }
    }

    #[cfg_attr(doc_cfg, doc(cfg(feature = "printing")))]
    impl ToTokens for PatRest {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            tokens.append_all(self.attrs.outer());
            self.dot2_token.to_tokens(tokens);
        }
    }

    #[cfg_attr(doc_cfg, doc(cfg(feature = "printing")))]
    impl ToTokens for PatLit {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            tokens.append_all(self.attrs.outer());
            self.expr.to_tokens(tokens);
        }
    }

    #[cfg_attr(doc_cfg, doc(cfg(feature = "printing")))]
    impl ToTokens for PatRange {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            tokens.append_all(self.attrs.outer());
            self.lo.to_tokens(tokens);
            self.limits.to_tokens(tokens);
            self.hi.to_tokens(tokens);
        }
    }

    #[cfg_attr(doc_cfg, doc(cfg(feature = "printing")))]
    impl ToTokens for PatSlice {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            tokens.append_all(self.attrs.outer());
            self.bracket_token.surround(tokens, |tokens| {
                self.elems.to_tokens(tokens);
            });
        }
    }

    #[cfg_attr(doc_cfg, doc(cfg(feature = "printing")))]
    impl ToTokens for PatMacro {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            tokens.append_all(self.attrs.outer());
            self.mac.to_tokens(tokens);
        }
    }

    #[cfg_attr(doc_cfg, doc(cfg(feature = "printing")))]
    impl ToTokens for PatOr {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            tokens.append_all(self.attrs.outer());
            self.leading_vert.to_tokens(tokens);
            self.cases.to_tokens(tokens);
        }
    }

    #[cfg_attr(doc_cfg, doc(cfg(feature = "printing")))]
    impl ToTokens for FieldPat {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            tokens.append_all(self.attrs.outer());
            if let Some(colon_token) = &self.colon_token {
                self.member.to_tokens(tokens);
                colon_token.to_tokens(tokens);
            }
            self.pat.to_tokens(tokens);
        }
    }
}

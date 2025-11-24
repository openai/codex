use super::*;
use crate::punctuated::Punctuated;

ast_struct! {
    /// A path at which a named item is exported (e.g. `std::collections::HashMap`).
    ///
    /// *This type is available only if Syn is built with the `"derive"` or `"full"`
    /// feature.*
    #[cfg_attr(doc_cfg, doc(cfg(any(feature = "full", feature = "derive"))))]
    pub struct Path {
        pub leading_colon: Option<Token![::]>,
        pub segments: Punctuated<PathSegment, Token![::]>,
    }
}

impl<T> From<T> for Path
where
    T: Into<PathSegment>,
{
    fn from(segment: T) -> Self {
        let mut path = Path {
            leading_colon: None,
            segments: Punctuated::new(),
        };
        path.segments.push_value(segment.into());
        path
    }
}

ast_struct! {
    /// A segment of a path together with any path arguments on that segment.
    ///
    /// *This type is available only if Syn is built with the `"derive"` or `"full"`
    /// feature.*
    #[cfg_attr(doc_cfg, doc(cfg(any(feature = "full", feature = "derive"))))]
    pub struct PathSegment {
        pub ident: Ident,
        pub arguments: PathArguments,
    }
}

impl<T> From<T> for PathSegment
where
    T: Into<Ident>,
{
    fn from(ident: T) -> Self {
        PathSegment {
            ident: ident.into(),
            arguments: PathArguments::None,
        }
    }
}

ast_enum! {
    /// Angle bracketed or parenthesized arguments of a path segment.
    ///
    /// *This type is available only if Syn is built with the `"derive"` or `"full"`
    /// feature.*
    ///
    /// ## Angle bracketed
    ///
    /// The `<'a, T>` in `std::slice::iter<'a, T>`.
    ///
    /// ## Parenthesized
    ///
    /// The `(A, B) -> C` in `Fn(A, B) -> C`.
    #[cfg_attr(doc_cfg, doc(cfg(any(feature = "full", feature = "derive"))))]
    pub enum PathArguments {
        None,
        /// The `<'a, T>` in `std::slice::iter<'a, T>`.
        AngleBracketed(AngleBracketedGenericArguments),
        /// The `(A, B) -> C` in `Fn(A, B) -> C`.
        Parenthesized(ParenthesizedGenericArguments),
    }
}

impl Default for PathArguments {
    fn default() -> Self {
        PathArguments::None
    }
}

impl PathArguments {
    pub fn is_empty(&self) -> bool {
        match self {
            PathArguments::None => true,
            PathArguments::AngleBracketed(bracketed) => bracketed.args.is_empty(),
            PathArguments::Parenthesized(_) => false,
        }
    }

    pub fn is_none(&self) -> bool {
        match self {
            PathArguments::None => true,
            PathArguments::AngleBracketed(_) | PathArguments::Parenthesized(_) => false,
        }
    }
}

ast_enum! {
    /// An individual generic argument, like `'a`, `T`, or `Item = T`.
    ///
    /// *This type is available only if Syn is built with the `"derive"` or `"full"`
    /// feature.*
    #[cfg_attr(doc_cfg, doc(cfg(any(feature = "full", feature = "derive"))))]
    pub enum GenericArgument {
        /// A lifetime argument.
        Lifetime(Lifetime),
        /// A type argument.
        Type(Type),
        /// A const expression. Must be inside of a block.
        ///
        /// NOTE: Identity expressions are represented as Type arguments, as
        /// they are indistinguishable syntactically.
        Const(Expr),
        /// A binding (equality constraint) on an associated type: the `Item =
        /// u8` in `Iterator<Item = u8>`.
        Binding(Binding),
        /// An associated type bound: `Iterator<Item: Display>`.
        Constraint(Constraint),
    }
}

ast_struct! {
    /// Angle bracketed arguments of a path segment: the `<K, V>` in `HashMap<K,
    /// V>`.
    ///
    /// *This type is available only if Syn is built with the `"derive"` or `"full"`
    /// feature.*
    #[cfg_attr(doc_cfg, doc(cfg(any(feature = "full", feature = "derive"))))]
    pub struct AngleBracketedGenericArguments {
        pub colon2_token: Option<Token![::]>,
        pub lt_token: Token![<],
        pub args: Punctuated<GenericArgument, Token![,]>,
        pub gt_token: Token![>],
    }
}

ast_struct! {
    /// A binding (equality constraint) on an associated type: `Item = u8`.
    ///
    /// *This type is available only if Syn is built with the `"derive"` or `"full"`
    /// feature.*
    #[cfg_attr(doc_cfg, doc(cfg(any(feature = "full", feature = "derive"))))]
    pub struct Binding {
        pub ident: Ident,
        pub eq_token: Token![=],
        pub ty: Type,
    }
}

ast_struct! {
    /// An associated type bound: `Iterator<Item: Display>`.
    ///
    /// *This type is available only if Syn is built with the `"derive"` or `"full"`
    /// feature.*
    #[cfg_attr(doc_cfg, doc(cfg(any(feature = "full", feature = "derive"))))]
    pub struct Constraint {
        pub ident: Ident,
        pub colon_token: Token![:],
        pub bounds: Punctuated<TypeParamBound, Token![+]>,
    }
}

ast_struct! {
    /// Arguments of a function path segment: the `(A, B) -> C` in `Fn(A,B) ->
    /// C`.
    ///
    /// *This type is available only if Syn is built with the `"derive"` or `"full"`
    /// feature.*
    #[cfg_attr(doc_cfg, doc(cfg(any(feature = "full", feature = "derive"))))]
    pub struct ParenthesizedGenericArguments {
        pub paren_token: token::Paren,
        /// `(A, B)`
        pub inputs: Punctuated<Type, Token![,]>,
        /// `C`
        pub output: ReturnType,
    }
}

ast_struct! {
    /// The explicit Self type in a qualified path: the `T` in `<T as
    /// Display>::fmt`.
    ///
    /// The actual path, including the trait and the associated item, is stored
    /// separately. The `position` field represents the index of the associated
    /// item qualified with this Self type.
    ///
    /// ```text
    /// <Vec<T> as a::b::Trait>::AssociatedItem
    ///  ^~~~~~    ~~~~~~~~~~~~~~^
    ///  ty        position = 3
    ///
    /// <Vec<T>>::AssociatedItem
    ///  ^~~~~~   ^
    ///  ty       position = 0
    /// ```
    ///
    /// *This type is available only if Syn is built with the `"derive"` or `"full"`
    /// feature.*
    #[cfg_attr(doc_cfg, doc(cfg(any(feature = "full", feature = "derive"))))]
    pub struct QSelf {
        pub lt_token: Token![<],
        pub ty: Box<Type>,
        pub position: usize,
        pub as_token: Option<Token![as]>,
        pub gt_token: Token![>],
    }
}

#[cfg(feature = "parsing")]
pub mod parsing {
    use super::*;

    use crate::ext::IdentExt;
    use crate::parse::{Parse, ParseStream, Result};

    #[cfg_attr(doc_cfg, doc(cfg(feature = "parsing")))]
    impl Parse for Path {
        fn parse(input: ParseStream) -> Result<Self> {
            Self::parse_helper(input, false)
        }
    }

    #[cfg_attr(doc_cfg, doc(cfg(feature = "parsing")))]
    impl Parse for GenericArgument {
        fn parse(input: ParseStream) -> Result<Self> {
            if input.peek(Lifetime) && !input.peek2(Token![+]) {
                return Ok(GenericArgument::Lifetime(input.parse()?));
            }

            if input.peek(Ident) && input.peek2(Token![=]) {
                let ident: Ident = input.parse()?;
                let eq_token: Token![=] = input.parse()?;

                let ty = if input.peek(Lit) {
                    let begin = input.fork();
                    input.parse::<Lit>()?;
                    Type::Verbatim(verbatim::between(begin, input))
                } else if input.peek(token::Brace) {
                    let begin = input.fork();

                    #[cfg(feature = "full")]
                    {
                        input.parse::<ExprBlock>()?;
                    }

                    #[cfg(not(feature = "full"))]
                    {
                        let content;
                        braced!(content in input);
                        content.parse::<Expr>()?;
                    }

                    Type::Verbatim(verbatim::between(begin, input))
                } else {
                    input.parse()?
                };

                return Ok(GenericArgument::Binding(Binding {
                    ident,
                    eq_token,
                    ty,
                }));
            }

            #[cfg(feature = "full")]
            {
                if input.peek(Ident) && input.peek2(Token![:]) && !input.peek2(Token![::]) {
                    return Ok(GenericArgument::Constraint(input.parse()?));
                }
            }

            if input.peek(Lit) || input.peek(token::Brace) {
                return const_argument(input).map(GenericArgument::Const);
            }

            #[cfg(feature = "full")]
            let begin = input.fork();

            let argument: Type = input.parse()?;

            #[cfg(feature = "full")]
            {
                if match &argument {
                    Type::Path(argument)
                        if argument.qself.is_none()
                            && argument.path.leading_colon.is_none()
                            && argument.path.segments.len() == 1 =>
                    {
                        match argument.path.segments[0].arguments {
                            PathArguments::AngleBracketed(_) => true,
                            _ => false,
                        }
                    }
                    _ => false,
                } && if input.peek(Token![=]) {
                    input.parse::<Token![=]>()?;
                    input.parse::<Type>()?;
                    true
                } else if input.peek(Token![:]) {
                    input.parse::<Token![:]>()?;
                    input.call(constraint_bounds)?;
                    true
                } else {
                    false
                } {
                    let verbatim = verbatim::between(begin, input);
                    return Ok(GenericArgument::Type(Type::Verbatim(verbatim)));
                }
            }

            Ok(GenericArgument::Type(argument))
        }
    }

    pub fn const_argument(input: ParseStream) -> Result<Expr> {
        let lookahead = input.lookahead1();

        if input.peek(Lit) {
            let lit = input.parse()?;
            return Ok(Expr::Lit(lit));
        }

        #[cfg(feature = "full")]
        {
            if input.peek(Ident) {
                let ident: Ident = input.parse()?;
                return Ok(Expr::Path(ExprPath {
                    attrs: Vec::new(),
                    qself: None,
                    path: Path::from(ident),
                }));
            }
        }

        if input.peek(token::Brace) {
            #[cfg(feature = "full")]
            {
                let block: ExprBlock = input.parse()?;
                return Ok(Expr::Block(block));
            }

            #[cfg(not(feature = "full"))]
            {
                let begin = input.fork();
                let content;
                braced!(content in input);
                content.parse::<Expr>()?;
                let verbatim = verbatim::between(begin, input);
                return Ok(Expr::Verbatim(verbatim));
            }
        }

        Err(lookahead.error())
    }

    #[cfg_attr(doc_cfg, doc(cfg(feature = "parsing")))]
    impl Parse for AngleBracketedGenericArguments {
        fn parse(input: ParseStream) -> Result<Self> {
            Ok(AngleBracketedGenericArguments {
                colon2_token: input.parse()?,
                lt_token: input.parse()?,
                args: {
                    let mut args = Punctuated::new();
                    loop {
                        if input.peek(Token![>]) {
                            break;
                        }
                        let value = input.parse()?;
                        args.push_value(value);
                        if input.peek(Token![>]) {
                            break;
                        }
                        let punct = input.parse()?;
                        args.push_punct(punct);
                    }
                    args
                },
                gt_token: input.parse()?,
            })
        }
    }

    #[cfg_attr(doc_cfg, doc(cfg(feature = "parsing")))]
    impl Parse for ParenthesizedGenericArguments {
        fn parse(input: ParseStream) -> Result<Self> {
            let content;
            Ok(ParenthesizedGenericArguments {
                paren_token: parenthesized!(content in input),
                inputs: content.parse_terminated(Type::parse)?,
                output: input.call(ReturnType::without_plus)?,
            })
        }
    }

    #[cfg_attr(doc_cfg, doc(cfg(feature = "parsing")))]
    impl Parse for PathSegment {
        fn parse(input: ParseStream) -> Result<Self> {
            Self::parse_helper(input, false)
        }
    }

    impl PathSegment {
        fn parse_helper(input: ParseStream, expr_style: bool) -> Result<Self> {
            if input.peek(Token![super]) || input.peek(Token![self]) || input.peek(Token![crate]) {
                let ident = input.call(Ident::parse_any)?;
                return Ok(PathSegment::from(ident));
            }

            let ident = if input.peek(Token![Self]) {
                input.call(Ident::parse_any)?
            } else {
                input.parse()?
            };

            if !expr_style && input.peek(Token![<]) && !input.peek(Token![<=])
                || input.peek(Token![::]) && input.peek3(Token![<])
            {
                Ok(PathSegment {
                    ident,
                    arguments: PathArguments::AngleBracketed(input.parse()?),
                })
            } else {
                Ok(PathSegment::from(ident))
            }
        }
    }

    #[cfg_attr(doc_cfg, doc(cfg(feature = "parsing")))]
    impl Parse for Binding {
        fn parse(input: ParseStream) -> Result<Self> {
            Ok(Binding {
                ident: input.parse()?,
                eq_token: input.parse()?,
                ty: input.parse()?,
            })
        }
    }

    #[cfg(feature = "full")]
    #[cfg_attr(doc_cfg, doc(cfg(feature = "parsing")))]
    impl Parse for Constraint {
        fn parse(input: ParseStream) -> Result<Self> {
            Ok(Constraint {
                ident: input.parse()?,
                colon_token: input.parse()?,
                bounds: constraint_bounds(input)?,
            })
        }
    }

    #[cfg(feature = "full")]
    fn constraint_bounds(input: ParseStream) -> Result<Punctuated<TypeParamBound, Token![+]>> {
        let mut bounds = Punctuated::new();
        loop {
            if input.peek(Token![,]) || input.peek(Token![>]) {
                break;
            }
            let value = input.parse()?;
            bounds.push_value(value);
            if !input.peek(Token![+]) {
                break;
            }
            let punct = input.parse()?;
            bounds.push_punct(punct);
        }
        Ok(bounds)
    }

    impl Path {
        /// Parse a `Path` containing no path arguments on any of its segments.
        ///
        /// *This function is available only if Syn is built with the `"parsing"`
        /// feature.*
        ///
        /// # Example
        ///
        /// ```
        /// use syn::{Path, Result, Token};
        /// use syn::parse::{Parse, ParseStream};
        ///
        /// // A simplified single `use` statement like:
        /// //
        /// //     use std::collections::HashMap;
        /// //
        /// // Note that generic parameters are not allowed in a `use` statement
        /// // so the following must not be accepted.
        /// //
        /// //     use a::<b>::c;
        /// struct SingleUse {
        ///     use_token: Token![use],
        ///     path: Path,
        /// }
        ///
        /// impl Parse for SingleUse {
        ///     fn parse(input: ParseStream) -> Result<Self> {
        ///         Ok(SingleUse {
        ///             use_token: input.parse()?,
        ///             path: input.call(Path::parse_mod_style)?,
        ///         })
        ///     }
        /// }
        /// ```
        #[cfg_attr(doc_cfg, doc(cfg(feature = "parsing")))]
        pub fn parse_mod_style(input: ParseStream) -> Result<Self> {
            Ok(Path {
                leading_colon: input.parse()?,
                segments: {
                    let mut segments = Punctuated::new();
                    loop {
                        if !input.peek(Ident)
                            && !input.peek(Token![super])
                            && !input.peek(Token![self])
                            && !input.peek(Token![Self])
                            && !input.peek(Token![crate])
                        {
                            break;
                        }
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

        /// Determines whether this is a path of length 1 equal to the given
        /// ident.
        ///
        /// For them to compare equal, it must be the case that:
        ///
        /// - the path has no leading colon,
        /// - the number of path segments is 1,
        /// - the first path segment has no angle bracketed or parenthesized
        ///   path arguments, and
        /// - the ident of the first path segment is equal to the given one.
        ///
        /// *This function is available only if Syn is built with the `"parsing"`
        /// feature.*
        ///
        /// # Example
        ///
        /// ```
        /// use syn::{Attribute, Error, Meta, NestedMeta, Result};
        /// # use std::iter::FromIterator;
        ///
        /// fn get_serde_meta_items(attr: &Attribute) -> Result<Vec<NestedMeta>> {
        ///     if attr.path.is_ident("serde") {
        ///         match attr.parse_meta()? {
        ///             Meta::List(meta) => Ok(Vec::from_iter(meta.nested)),
        ///             bad => Err(Error::new_spanned(bad, "unrecognized attribute")),
        ///         }
        ///     } else {
        ///         Ok(Vec::new())
        ///     }
        /// }
        /// ```
        #[cfg_attr(doc_cfg, doc(cfg(feature = "parsing")))]
        pub fn is_ident<I: ?Sized>(&self, ident: &I) -> bool
        where
            Ident: PartialEq<I>,
        {
            match self.get_ident() {
                Some(id) => id == ident,
                None => false,
            }
        }

        /// If this path consists of a single ident, returns the ident.
        ///
        /// A path is considered an ident if:
        ///
        /// - the path has no leading colon,
        /// - the number of path segments is 1, and
        /// - the first path segment has no angle bracketed or parenthesized
        ///   path arguments.
        ///
        /// *This function is available only if Syn is built with the `"parsing"`
        /// feature.*
        #[cfg_attr(doc_cfg, doc(cfg(feature = "parsing")))]
        pub fn get_ident(&self) -> Option<&Ident> {
            if self.leading_colon.is_none()
                && self.segments.len() == 1
                && self.segments[0].arguments.is_none()
            {
                Some(&self.segments[0].ident)
            } else {
                None
            }
        }

        pub(crate) fn parse_helper(input: ParseStream, expr_style: bool) -> Result<Self> {
            let mut path = Path {
                leading_colon: input.parse()?,
                segments: {
                    let mut segments = Punctuated::new();
                    let value = PathSegment::parse_helper(input, expr_style)?;
                    segments.push_value(value);
                    segments
                },
            };
            Path::parse_rest(input, &mut path, expr_style)?;
            Ok(path)
        }

        pub(crate) fn parse_rest(
            input: ParseStream,
            path: &mut Self,
            expr_style: bool,
        ) -> Result<()> {
            while input.peek(Token![::]) && !input.peek3(token::Paren) {
                let punct: Token![::] = input.parse()?;
                path.segments.push_punct(punct);
                let value = PathSegment::parse_helper(input, expr_style)?;
                path.segments.push_value(value);
            }
            Ok(())
        }
    }

    pub fn qpath(input: ParseStream, expr_style: bool) -> Result<(Option<QSelf>, Path)> {
        if input.peek(Token![<]) {
            let lt_token: Token![<] = input.parse()?;
            let this: Type = input.parse()?;
            let path = if input.peek(Token![as]) {
                let as_token: Token![as] = input.parse()?;
                let path: Path = input.parse()?;
                Some((as_token, path))
            } else {
                None
            };
            let gt_token: Token![>] = input.parse()?;
            let colon2_token: Token![::] = input.parse()?;
            let mut rest = Punctuated::new();
            loop {
                let path = PathSegment::parse_helper(input, expr_style)?;
                rest.push_value(path);
                if !input.peek(Token![::]) {
                    break;
                }
                let punct: Token![::] = input.parse()?;
                rest.push_punct(punct);
            }
            let (position, as_token, path) = match path {
                Some((as_token, mut path)) => {
                    let pos = path.segments.len();
                    path.segments.push_punct(colon2_token);
                    path.segments.extend(rest.into_pairs());
                    (pos, Some(as_token), path)
                }
                None => {
                    let path = Path {
                        leading_colon: Some(colon2_token),
                        segments: rest,
                    };
                    (0, None, path)
                }
            };
            let qself = QSelf {
                lt_token,
                ty: Box::new(this),
                position,
                as_token,
                gt_token,
            };
            Ok((Some(qself), path))
        } else {
            let path = Path::parse_helper(input, expr_style)?;
            Ok((None, path))
        }
    }
}

#[cfg(feature = "printing")]
pub(crate) mod printing {
    use super::*;
    use crate::print::TokensOrDefault;
    use proc_macro2::TokenStream;
    use quote::ToTokens;
    use std::cmp;

    #[cfg_attr(doc_cfg, doc(cfg(feature = "printing")))]
    impl ToTokens for Path {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            self.leading_colon.to_tokens(tokens);
            self.segments.to_tokens(tokens);
        }
    }

    #[cfg_attr(doc_cfg, doc(cfg(feature = "printing")))]
    impl ToTokens for PathSegment {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            self.ident.to_tokens(tokens);
            self.arguments.to_tokens(tokens);
        }
    }

    #[cfg_attr(doc_cfg, doc(cfg(feature = "printing")))]
    impl ToTokens for PathArguments {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            match self {
                PathArguments::None => {}
                PathArguments::AngleBracketed(arguments) => {
                    arguments.to_tokens(tokens);
                }
                PathArguments::Parenthesized(arguments) => {
                    arguments.to_tokens(tokens);
                }
            }
        }
    }

    #[cfg_attr(doc_cfg, doc(cfg(feature = "printing")))]
    impl ToTokens for GenericArgument {
        #[allow(clippy::match_same_arms)]
        fn to_tokens(&self, tokens: &mut TokenStream) {
            match self {
                GenericArgument::Lifetime(lt) => lt.to_tokens(tokens),
                GenericArgument::Type(ty) => ty.to_tokens(tokens),
                GenericArgument::Const(e) => match *e {
                    Expr::Lit(_) => e.to_tokens(tokens),

                    // NOTE: We should probably support parsing blocks with only
                    // expressions in them without the full feature for const
                    // generics.
                    #[cfg(feature = "full")]
                    Expr::Block(_) => e.to_tokens(tokens),

                    // ERROR CORRECTION: Add braces to make sure that the
                    // generated code is valid.
                    _ => token::Brace::default().surround(tokens, |tokens| {
                        e.to_tokens(tokens);
                    }),
                },
                GenericArgument::Binding(tb) => tb.to_tokens(tokens),
                GenericArgument::Constraint(tc) => tc.to_tokens(tokens),
            }
        }
    }

    #[cfg_attr(doc_cfg, doc(cfg(feature = "printing")))]
    impl ToTokens for AngleBracketedGenericArguments {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            self.colon2_token.to_tokens(tokens);
            self.lt_token.to_tokens(tokens);

            // Print lifetimes before types/consts/bindings, regardless of their
            // order in self.args.
            let mut trailing_or_empty = true;
            for param in self.args.pairs() {
                match **param.value() {
                    GenericArgument::Lifetime(_) => {
                        param.to_tokens(tokens);
                        trailing_or_empty = param.punct().is_some();
                    }
                    GenericArgument::Type(_)
                    | GenericArgument::Const(_)
                    | GenericArgument::Binding(_)
                    | GenericArgument::Constraint(_) => {}
                }
            }
            for param in self.args.pairs() {
                match **param.value() {
                    GenericArgument::Type(_)
                    | GenericArgument::Const(_)
                    | GenericArgument::Binding(_)
                    | GenericArgument::Constraint(_) => {
                        if !trailing_or_empty {
                            <Token![,]>::default().to_tokens(tokens);
                        }
                        param.to_tokens(tokens);
                        trailing_or_empty = param.punct().is_some();
                    }
                    GenericArgument::Lifetime(_) => {}
                }
            }

            self.gt_token.to_tokens(tokens);
        }
    }

    #[cfg_attr(doc_cfg, doc(cfg(feature = "printing")))]
    impl ToTokens for Binding {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            self.ident.to_tokens(tokens);
            self.eq_token.to_tokens(tokens);
            self.ty.to_tokens(tokens);
        }
    }

    #[cfg_attr(doc_cfg, doc(cfg(feature = "printing")))]
    impl ToTokens for Constraint {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            self.ident.to_tokens(tokens);
            self.colon_token.to_tokens(tokens);
            self.bounds.to_tokens(tokens);
        }
    }

    #[cfg_attr(doc_cfg, doc(cfg(feature = "printing")))]
    impl ToTokens for ParenthesizedGenericArguments {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            self.paren_token.surround(tokens, |tokens| {
                self.inputs.to_tokens(tokens);
            });
            self.output.to_tokens(tokens);
        }
    }

    pub(crate) fn print_path(tokens: &mut TokenStream, qself: &Option<QSelf>, path: &Path) {
        let qself = match qself {
            Some(qself) => qself,
            None => {
                path.to_tokens(tokens);
                return;
            }
        };
        qself.lt_token.to_tokens(tokens);
        qself.ty.to_tokens(tokens);

        let pos = cmp::min(qself.position, path.segments.len());
        let mut segments = path.segments.pairs();
        if pos > 0 {
            TokensOrDefault(&qself.as_token).to_tokens(tokens);
            path.leading_colon.to_tokens(tokens);
            for (i, segment) in segments.by_ref().take(pos).enumerate() {
                if i + 1 == pos {
                    segment.value().to_tokens(tokens);
                    qself.gt_token.to_tokens(tokens);
                    segment.punct().to_tokens(tokens);
                } else {
                    segment.to_tokens(tokens);
                }
            }
        } else {
            qself.gt_token.to_tokens(tokens);
            path.leading_colon.to_tokens(tokens);
        }
        for segment in segments {
            segment.to_tokens(tokens);
        }
    }
}

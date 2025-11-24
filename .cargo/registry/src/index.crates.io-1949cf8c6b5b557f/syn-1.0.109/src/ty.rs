use super::*;
use crate::punctuated::Punctuated;
use proc_macro2::TokenStream;

ast_enum_of_structs! {
    /// The possible types that a Rust value could have.
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
    #[cfg_attr(not(syn_no_non_exhaustive), non_exhaustive)]
    pub enum Type {
        /// A fixed size array type: `[T; n]`.
        Array(TypeArray),

        /// A bare function type: `fn(usize) -> bool`.
        BareFn(TypeBareFn),

        /// A type contained within invisible delimiters.
        Group(TypeGroup),

        /// An `impl Bound1 + Bound2 + Bound3` type where `Bound` is a trait or
        /// a lifetime.
        ImplTrait(TypeImplTrait),

        /// Indication that a type should be inferred by the compiler: `_`.
        Infer(TypeInfer),

        /// A macro in the type position.
        Macro(TypeMacro),

        /// The never type: `!`.
        Never(TypeNever),

        /// A parenthesized type equivalent to the inner type.
        Paren(TypeParen),

        /// A path like `std::slice::Iter`, optionally qualified with a
        /// self-type as in `<Vec<T> as SomeTrait>::Associated`.
        Path(TypePath),

        /// A raw pointer type: `*const T` or `*mut T`.
        Ptr(TypePtr),

        /// A reference type: `&'a T` or `&'a mut T`.
        Reference(TypeReference),

        /// A dynamically sized slice type: `[T]`.
        Slice(TypeSlice),

        /// A trait object type `dyn Bound1 + Bound2 + Bound3` where `Bound` is a
        /// trait or a lifetime.
        TraitObject(TypeTraitObject),

        /// A tuple type: `(A, B, C, String)`.
        Tuple(TypeTuple),

        /// Tokens in type position not interpreted by Syn.
        Verbatim(TokenStream),

        // Not public API.
        //
        // For testing exhaustiveness in downstream code, use the following idiom:
        //
        //     match ty {
        //         Type::Array(ty) => {...}
        //         Type::BareFn(ty) => {...}
        //         ...
        //         Type::Verbatim(ty) => {...}
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
    /// A fixed size array type: `[T; n]`.
    ///
    /// *This type is available only if Syn is built with the `"derive"` or
    /// `"full"` feature.*
    #[cfg_attr(doc_cfg, doc(cfg(any(feature = "full", feature = "derive"))))]
    pub struct TypeArray {
        pub bracket_token: token::Bracket,
        pub elem: Box<Type>,
        pub semi_token: Token![;],
        pub len: Expr,
    }
}

ast_struct! {
    /// A bare function type: `fn(usize) -> bool`.
    ///
    /// *This type is available only if Syn is built with the `"derive"` or
    /// `"full"` feature.*
    #[cfg_attr(doc_cfg, doc(cfg(any(feature = "full", feature = "derive"))))]
    pub struct TypeBareFn {
        pub lifetimes: Option<BoundLifetimes>,
        pub unsafety: Option<Token![unsafe]>,
        pub abi: Option<Abi>,
        pub fn_token: Token![fn],
        pub paren_token: token::Paren,
        pub inputs: Punctuated<BareFnArg, Token![,]>,
        pub variadic: Option<Variadic>,
        pub output: ReturnType,
    }
}

ast_struct! {
    /// A type contained within invisible delimiters.
    ///
    /// *This type is available only if Syn is built with the `"derive"` or
    /// `"full"` feature.*
    #[cfg_attr(doc_cfg, doc(cfg(any(feature = "full", feature = "derive"))))]
    pub struct TypeGroup {
        pub group_token: token::Group,
        pub elem: Box<Type>,
    }
}

ast_struct! {
    /// An `impl Bound1 + Bound2 + Bound3` type where `Bound` is a trait or
    /// a lifetime.
    ///
    /// *This type is available only if Syn is built with the `"derive"` or
    /// `"full"` feature.*
    #[cfg_attr(doc_cfg, doc(cfg(any(feature = "full", feature = "derive"))))]
    pub struct TypeImplTrait {
        pub impl_token: Token![impl],
        pub bounds: Punctuated<TypeParamBound, Token![+]>,
    }
}

ast_struct! {
    /// Indication that a type should be inferred by the compiler: `_`.
    ///
    /// *This type is available only if Syn is built with the `"derive"` or
    /// `"full"` feature.*
    #[cfg_attr(doc_cfg, doc(cfg(any(feature = "full", feature = "derive"))))]
    pub struct TypeInfer {
        pub underscore_token: Token![_],
    }
}

ast_struct! {
    /// A macro in the type position.
    ///
    /// *This type is available only if Syn is built with the `"derive"` or
    /// `"full"` feature.*
    #[cfg_attr(doc_cfg, doc(cfg(any(feature = "full", feature = "derive"))))]
    pub struct TypeMacro {
        pub mac: Macro,
    }
}

ast_struct! {
    /// The never type: `!`.
    ///
    /// *This type is available only if Syn is built with the `"derive"` or
    /// `"full"` feature.*
    #[cfg_attr(doc_cfg, doc(cfg(any(feature = "full", feature = "derive"))))]
    pub struct TypeNever {
        pub bang_token: Token![!],
    }
}

ast_struct! {
    /// A parenthesized type equivalent to the inner type.
    ///
    /// *This type is available only if Syn is built with the `"derive"` or
    /// `"full"` feature.*
    #[cfg_attr(doc_cfg, doc(cfg(any(feature = "full", feature = "derive"))))]
    pub struct TypeParen {
        pub paren_token: token::Paren,
        pub elem: Box<Type>,
    }
}

ast_struct! {
    /// A path like `std::slice::Iter`, optionally qualified with a
    /// self-type as in `<Vec<T> as SomeTrait>::Associated`.
    ///
    /// *This type is available only if Syn is built with the `"derive"` or
    /// `"full"` feature.*
    #[cfg_attr(doc_cfg, doc(cfg(any(feature = "full", feature = "derive"))))]
    pub struct TypePath {
        pub qself: Option<QSelf>,
        pub path: Path,
    }
}

ast_struct! {
    /// A raw pointer type: `*const T` or `*mut T`.
    ///
    /// *This type is available only if Syn is built with the `"derive"` or
    /// `"full"` feature.*
    #[cfg_attr(doc_cfg, doc(cfg(any(feature = "full", feature = "derive"))))]
    pub struct TypePtr {
        pub star_token: Token![*],
        pub const_token: Option<Token![const]>,
        pub mutability: Option<Token![mut]>,
        pub elem: Box<Type>,
    }
}

ast_struct! {
    /// A reference type: `&'a T` or `&'a mut T`.
    ///
    /// *This type is available only if Syn is built with the `"derive"` or
    /// `"full"` feature.*
    #[cfg_attr(doc_cfg, doc(cfg(any(feature = "full", feature = "derive"))))]
    pub struct TypeReference {
        pub and_token: Token![&],
        pub lifetime: Option<Lifetime>,
        pub mutability: Option<Token![mut]>,
        pub elem: Box<Type>,
    }
}

ast_struct! {
    /// A dynamically sized slice type: `[T]`.
    ///
    /// *This type is available only if Syn is built with the `"derive"` or
    /// `"full"` feature.*
    #[cfg_attr(doc_cfg, doc(cfg(any(feature = "full", feature = "derive"))))]
    pub struct TypeSlice {
        pub bracket_token: token::Bracket,
        pub elem: Box<Type>,
    }
}

ast_struct! {
    /// A trait object type `dyn Bound1 + Bound2 + Bound3` where `Bound` is a
    /// trait or a lifetime.
    ///
    /// *This type is available only if Syn is built with the `"derive"` or
    /// `"full"` feature.*
    #[cfg_attr(doc_cfg, doc(cfg(any(feature = "full", feature = "derive"))))]
    pub struct TypeTraitObject {
        pub dyn_token: Option<Token![dyn]>,
        pub bounds: Punctuated<TypeParamBound, Token![+]>,
    }
}

ast_struct! {
    /// A tuple type: `(A, B, C, String)`.
    ///
    /// *This type is available only if Syn is built with the `"derive"` or
    /// `"full"` feature.*
    #[cfg_attr(doc_cfg, doc(cfg(any(feature = "full", feature = "derive"))))]
    pub struct TypeTuple {
        pub paren_token: token::Paren,
        pub elems: Punctuated<Type, Token![,]>,
    }
}

ast_struct! {
    /// The binary interface of a function: `extern "C"`.
    ///
    /// *This type is available only if Syn is built with the `"derive"` or `"full"`
    /// feature.*
    #[cfg_attr(doc_cfg, doc(cfg(any(feature = "full", feature = "derive"))))]
    pub struct Abi {
        pub extern_token: Token![extern],
        pub name: Option<LitStr>,
    }
}

ast_struct! {
    /// An argument in a function type: the `usize` in `fn(usize) -> bool`.
    ///
    /// *This type is available only if Syn is built with the `"derive"` or `"full"`
    /// feature.*
    #[cfg_attr(doc_cfg, doc(cfg(any(feature = "full", feature = "derive"))))]
    pub struct BareFnArg {
        pub attrs: Vec<Attribute>,
        pub name: Option<(Ident, Token![:])>,
        pub ty: Type,
    }
}

ast_struct! {
    /// The variadic argument of a foreign function.
    ///
    /// ```rust
    /// # struct c_char;
    /// # struct c_int;
    /// #
    /// extern "C" {
    ///     fn printf(format: *const c_char, ...) -> c_int;
    ///     //                               ^^^
    /// }
    /// ```
    ///
    /// *This type is available only if Syn is built with the `"derive"` or `"full"`
    /// feature.*
    #[cfg_attr(doc_cfg, doc(cfg(any(feature = "full", feature = "derive"))))]
    pub struct Variadic {
        pub attrs: Vec<Attribute>,
        pub dots: Token![...],
    }
}

ast_enum! {
    /// Return type of a function signature.
    ///
    /// *This type is available only if Syn is built with the `"derive"` or `"full"`
    /// feature.*
    #[cfg_attr(doc_cfg, doc(cfg(any(feature = "full", feature = "derive"))))]
    pub enum ReturnType {
        /// Return type is not specified.
        ///
        /// Functions default to `()` and closures default to type inference.
        Default,
        /// A particular type is returned.
        Type(Token![->], Box<Type>),
    }
}

#[cfg(feature = "parsing")]
pub mod parsing {
    use super::*;
    use crate::ext::IdentExt;
    use crate::parse::{Parse, ParseStream, Result};
    use crate::path;
    use proc_macro2::{Punct, Spacing, Span, TokenTree};

    #[cfg_attr(doc_cfg, doc(cfg(feature = "parsing")))]
    impl Parse for Type {
        fn parse(input: ParseStream) -> Result<Self> {
            let allow_plus = true;
            let allow_group_generic = true;
            ambig_ty(input, allow_plus, allow_group_generic)
        }
    }

    impl Type {
        /// In some positions, types may not contain the `+` character, to
        /// disambiguate them. For example in the expression `1 as T`, T may not
        /// contain a `+` character.
        ///
        /// This parser does not allow a `+`, while the default parser does.
        #[cfg_attr(doc_cfg, doc(cfg(feature = "parsing")))]
        pub fn without_plus(input: ParseStream) -> Result<Self> {
            let allow_plus = false;
            let allow_group_generic = true;
            ambig_ty(input, allow_plus, allow_group_generic)
        }
    }

    pub(crate) fn ambig_ty(
        input: ParseStream,
        allow_plus: bool,
        allow_group_generic: bool,
    ) -> Result<Type> {
        let begin = input.fork();

        if input.peek(token::Group) {
            let mut group: TypeGroup = input.parse()?;
            if input.peek(Token![::]) && input.peek3(Ident::peek_any) {
                if let Type::Path(mut ty) = *group.elem {
                    Path::parse_rest(input, &mut ty.path, false)?;
                    return Ok(Type::Path(ty));
                } else {
                    return Ok(Type::Path(TypePath {
                        qself: Some(QSelf {
                            lt_token: Token![<](group.group_token.span),
                            position: 0,
                            as_token: None,
                            gt_token: Token![>](group.group_token.span),
                            ty: group.elem,
                        }),
                        path: Path::parse_helper(input, false)?,
                    }));
                }
            } else if input.peek(Token![<]) && allow_group_generic
                || input.peek(Token![::]) && input.peek3(Token![<])
            {
                if let Type::Path(mut ty) = *group.elem {
                    let arguments = &mut ty.path.segments.last_mut().unwrap().arguments;
                    if let PathArguments::None = arguments {
                        *arguments = PathArguments::AngleBracketed(input.parse()?);
                        Path::parse_rest(input, &mut ty.path, false)?;
                        return Ok(Type::Path(ty));
                    } else {
                        group.elem = Box::new(Type::Path(ty));
                    }
                }
            }
            return Ok(Type::Group(group));
        }

        let mut lifetimes = None::<BoundLifetimes>;
        let mut lookahead = input.lookahead1();
        if lookahead.peek(Token![for]) {
            lifetimes = input.parse()?;
            lookahead = input.lookahead1();
            if !lookahead.peek(Ident)
                && !lookahead.peek(Token![fn])
                && !lookahead.peek(Token![unsafe])
                && !lookahead.peek(Token![extern])
                && !lookahead.peek(Token![super])
                && !lookahead.peek(Token![self])
                && !lookahead.peek(Token![Self])
                && !lookahead.peek(Token![crate])
                || input.peek(Token![dyn])
            {
                return Err(lookahead.error());
            }
        }

        if lookahead.peek(token::Paren) {
            let content;
            let paren_token = parenthesized!(content in input);
            if content.is_empty() {
                return Ok(Type::Tuple(TypeTuple {
                    paren_token,
                    elems: Punctuated::new(),
                }));
            }
            if content.peek(Lifetime) {
                return Ok(Type::Paren(TypeParen {
                    paren_token,
                    elem: Box::new(Type::TraitObject(content.parse()?)),
                }));
            }
            if content.peek(Token![?]) {
                return Ok(Type::TraitObject(TypeTraitObject {
                    dyn_token: None,
                    bounds: {
                        let mut bounds = Punctuated::new();
                        bounds.push_value(TypeParamBound::Trait(TraitBound {
                            paren_token: Some(paren_token),
                            ..content.parse()?
                        }));
                        while let Some(plus) = input.parse()? {
                            bounds.push_punct(plus);
                            bounds.push_value(input.parse()?);
                        }
                        bounds
                    },
                }));
            }
            let mut first: Type = content.parse()?;
            if content.peek(Token![,]) {
                return Ok(Type::Tuple(TypeTuple {
                    paren_token,
                    elems: {
                        let mut elems = Punctuated::new();
                        elems.push_value(first);
                        elems.push_punct(content.parse()?);
                        while !content.is_empty() {
                            elems.push_value(content.parse()?);
                            if content.is_empty() {
                                break;
                            }
                            elems.push_punct(content.parse()?);
                        }
                        elems
                    },
                }));
            }
            if allow_plus && input.peek(Token![+]) {
                loop {
                    let first = match first {
                        Type::Path(TypePath { qself: None, path }) => {
                            TypeParamBound::Trait(TraitBound {
                                paren_token: Some(paren_token),
                                modifier: TraitBoundModifier::None,
                                lifetimes: None,
                                path,
                            })
                        }
                        Type::TraitObject(TypeTraitObject {
                            dyn_token: None,
                            bounds,
                        }) => {
                            if bounds.len() > 1 || bounds.trailing_punct() {
                                first = Type::TraitObject(TypeTraitObject {
                                    dyn_token: None,
                                    bounds,
                                });
                                break;
                            }
                            match bounds.into_iter().next().unwrap() {
                                TypeParamBound::Trait(trait_bound) => {
                                    TypeParamBound::Trait(TraitBound {
                                        paren_token: Some(paren_token),
                                        ..trait_bound
                                    })
                                }
                                other @ TypeParamBound::Lifetime(_) => other,
                            }
                        }
                        _ => break,
                    };
                    return Ok(Type::TraitObject(TypeTraitObject {
                        dyn_token: None,
                        bounds: {
                            let mut bounds = Punctuated::new();
                            bounds.push_value(first);
                            while let Some(plus) = input.parse()? {
                                bounds.push_punct(plus);
                                bounds.push_value(input.parse()?);
                            }
                            bounds
                        },
                    }));
                }
            }
            Ok(Type::Paren(TypeParen {
                paren_token,
                elem: Box::new(first),
            }))
        } else if lookahead.peek(Token![fn])
            || lookahead.peek(Token![unsafe])
            || lookahead.peek(Token![extern])
        {
            let allow_mut_self = true;
            if let Some(mut bare_fn) = parse_bare_fn(input, allow_mut_self)? {
                bare_fn.lifetimes = lifetimes;
                Ok(Type::BareFn(bare_fn))
            } else {
                Ok(Type::Verbatim(verbatim::between(begin, input)))
            }
        } else if lookahead.peek(Ident)
            || input.peek(Token![super])
            || input.peek(Token![self])
            || input.peek(Token![Self])
            || input.peek(Token![crate])
            || lookahead.peek(Token![::])
            || lookahead.peek(Token![<])
        {
            let dyn_token: Option<Token![dyn]> = input.parse()?;
            if let Some(dyn_token) = dyn_token {
                let dyn_span = dyn_token.span;
                let star_token: Option<Token![*]> = input.parse()?;
                let bounds = TypeTraitObject::parse_bounds(dyn_span, input, allow_plus)?;
                return Ok(if star_token.is_some() {
                    Type::Verbatim(verbatim::between(begin, input))
                } else {
                    Type::TraitObject(TypeTraitObject {
                        dyn_token: Some(dyn_token),
                        bounds,
                    })
                });
            }

            let ty: TypePath = input.parse()?;
            if ty.qself.is_some() {
                return Ok(Type::Path(ty));
            }

            if input.peek(Token![!]) && !input.peek(Token![!=]) {
                let mut contains_arguments = false;
                for segment in &ty.path.segments {
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
                    return Ok(Type::Macro(TypeMacro {
                        mac: Macro {
                            path: ty.path,
                            bang_token,
                            delimiter,
                            tokens,
                        },
                    }));
                }
            }

            if lifetimes.is_some() || allow_plus && input.peek(Token![+]) {
                let mut bounds = Punctuated::new();
                bounds.push_value(TypeParamBound::Trait(TraitBound {
                    paren_token: None,
                    modifier: TraitBoundModifier::None,
                    lifetimes,
                    path: ty.path,
                }));
                if allow_plus {
                    while input.peek(Token![+]) {
                        bounds.push_punct(input.parse()?);
                        if !(input.peek(Ident::peek_any)
                            || input.peek(Token![::])
                            || input.peek(Token![?])
                            || input.peek(Lifetime)
                            || input.peek(token::Paren))
                        {
                            break;
                        }
                        bounds.push_value(input.parse()?);
                    }
                }
                return Ok(Type::TraitObject(TypeTraitObject {
                    dyn_token: None,
                    bounds,
                }));
            }

            Ok(Type::Path(ty))
        } else if lookahead.peek(token::Bracket) {
            let content;
            let bracket_token = bracketed!(content in input);
            let elem: Type = content.parse()?;
            if content.peek(Token![;]) {
                Ok(Type::Array(TypeArray {
                    bracket_token,
                    elem: Box::new(elem),
                    semi_token: content.parse()?,
                    len: content.parse()?,
                }))
            } else {
                Ok(Type::Slice(TypeSlice {
                    bracket_token,
                    elem: Box::new(elem),
                }))
            }
        } else if lookahead.peek(Token![*]) {
            input.parse().map(Type::Ptr)
        } else if lookahead.peek(Token![&]) {
            input.parse().map(Type::Reference)
        } else if lookahead.peek(Token![!]) && !input.peek(Token![=]) {
            input.parse().map(Type::Never)
        } else if lookahead.peek(Token![impl]) {
            TypeImplTrait::parse(input, allow_plus).map(Type::ImplTrait)
        } else if lookahead.peek(Token![_]) {
            input.parse().map(Type::Infer)
        } else if lookahead.peek(Lifetime) {
            input.parse().map(Type::TraitObject)
        } else {
            Err(lookahead.error())
        }
    }

    #[cfg_attr(doc_cfg, doc(cfg(feature = "parsing")))]
    impl Parse for TypeSlice {
        fn parse(input: ParseStream) -> Result<Self> {
            let content;
            Ok(TypeSlice {
                bracket_token: bracketed!(content in input),
                elem: content.parse()?,
            })
        }
    }

    #[cfg_attr(doc_cfg, doc(cfg(feature = "parsing")))]
    impl Parse for TypeArray {
        fn parse(input: ParseStream) -> Result<Self> {
            let content;
            Ok(TypeArray {
                bracket_token: bracketed!(content in input),
                elem: content.parse()?,
                semi_token: content.parse()?,
                len: content.parse()?,
            })
        }
    }

    #[cfg_attr(doc_cfg, doc(cfg(feature = "parsing")))]
    impl Parse for TypePtr {
        fn parse(input: ParseStream) -> Result<Self> {
            let star_token: Token![*] = input.parse()?;

            let lookahead = input.lookahead1();
            let (const_token, mutability) = if lookahead.peek(Token![const]) {
                (Some(input.parse()?), None)
            } else if lookahead.peek(Token![mut]) {
                (None, Some(input.parse()?))
            } else {
                return Err(lookahead.error());
            };

            Ok(TypePtr {
                star_token,
                const_token,
                mutability,
                elem: Box::new(input.call(Type::without_plus)?),
            })
        }
    }

    #[cfg_attr(doc_cfg, doc(cfg(feature = "parsing")))]
    impl Parse for TypeReference {
        fn parse(input: ParseStream) -> Result<Self> {
            Ok(TypeReference {
                and_token: input.parse()?,
                lifetime: input.parse()?,
                mutability: input.parse()?,
                // & binds tighter than +, so we don't allow + here.
                elem: Box::new(input.call(Type::without_plus)?),
            })
        }
    }

    #[cfg_attr(doc_cfg, doc(cfg(feature = "parsing")))]
    impl Parse for TypeBareFn {
        fn parse(input: ParseStream) -> Result<Self> {
            let allow_mut_self = false;
            parse_bare_fn(input, allow_mut_self).map(Option::unwrap)
        }
    }

    fn parse_bare_fn(input: ParseStream, allow_mut_self: bool) -> Result<Option<TypeBareFn>> {
        let args;
        let mut variadic = None;
        let mut has_mut_self = false;

        let bare_fn = TypeBareFn {
            lifetimes: input.parse()?,
            unsafety: input.parse()?,
            abi: input.parse()?,
            fn_token: input.parse()?,
            paren_token: parenthesized!(args in input),
            inputs: {
                let mut inputs = Punctuated::new();

                while !args.is_empty() {
                    let attrs = args.call(Attribute::parse_outer)?;

                    if inputs.empty_or_trailing() && args.peek(Token![...]) {
                        variadic = Some(Variadic {
                            attrs,
                            dots: args.parse()?,
                        });
                        break;
                    }

                    if let Some(arg) = parse_bare_fn_arg(&args, allow_mut_self)? {
                        inputs.push_value(BareFnArg { attrs, ..arg });
                    } else {
                        has_mut_self = true;
                    }
                    if args.is_empty() {
                        break;
                    }

                    let comma = args.parse()?;
                    if !has_mut_self {
                        inputs.push_punct(comma);
                    }
                }

                inputs
            },
            variadic,
            output: input.call(ReturnType::without_plus)?,
        };

        if has_mut_self {
            Ok(None)
        } else {
            Ok(Some(bare_fn))
        }
    }

    #[cfg_attr(doc_cfg, doc(cfg(feature = "parsing")))]
    impl Parse for TypeNever {
        fn parse(input: ParseStream) -> Result<Self> {
            Ok(TypeNever {
                bang_token: input.parse()?,
            })
        }
    }

    #[cfg_attr(doc_cfg, doc(cfg(feature = "parsing")))]
    impl Parse for TypeInfer {
        fn parse(input: ParseStream) -> Result<Self> {
            Ok(TypeInfer {
                underscore_token: input.parse()?,
            })
        }
    }

    #[cfg_attr(doc_cfg, doc(cfg(feature = "parsing")))]
    impl Parse for TypeTuple {
        fn parse(input: ParseStream) -> Result<Self> {
            let content;
            let paren_token = parenthesized!(content in input);

            if content.is_empty() {
                return Ok(TypeTuple {
                    paren_token,
                    elems: Punctuated::new(),
                });
            }

            let first: Type = content.parse()?;
            Ok(TypeTuple {
                paren_token,
                elems: {
                    let mut elems = Punctuated::new();
                    elems.push_value(first);
                    elems.push_punct(content.parse()?);
                    while !content.is_empty() {
                        elems.push_value(content.parse()?);
                        if content.is_empty() {
                            break;
                        }
                        elems.push_punct(content.parse()?);
                    }
                    elems
                },
            })
        }
    }

    #[cfg_attr(doc_cfg, doc(cfg(feature = "parsing")))]
    impl Parse for TypeMacro {
        fn parse(input: ParseStream) -> Result<Self> {
            Ok(TypeMacro {
                mac: input.parse()?,
            })
        }
    }

    #[cfg_attr(doc_cfg, doc(cfg(feature = "parsing")))]
    impl Parse for TypePath {
        fn parse(input: ParseStream) -> Result<Self> {
            let expr_style = false;
            let (qself, mut path) = path::parsing::qpath(input, expr_style)?;

            while path.segments.last().unwrap().arguments.is_empty()
                && (input.peek(token::Paren) || input.peek(Token![::]) && input.peek3(token::Paren))
            {
                input.parse::<Option<Token![::]>>()?;
                let args: ParenthesizedGenericArguments = input.parse()?;
                let allow_associated_type = cfg!(feature = "full")
                    && match &args.output {
                        ReturnType::Default => true,
                        ReturnType::Type(_, ty) => match **ty {
                            // TODO: probably some of the other kinds allow this too.
                            Type::Paren(_) => true,
                            _ => false,
                        },
                    };
                let parenthesized = PathArguments::Parenthesized(args);
                path.segments.last_mut().unwrap().arguments = parenthesized;
                if allow_associated_type {
                    Path::parse_rest(input, &mut path, expr_style)?;
                }
            }

            Ok(TypePath { qself, path })
        }
    }

    impl ReturnType {
        #[cfg_attr(doc_cfg, doc(cfg(feature = "parsing")))]
        pub fn without_plus(input: ParseStream) -> Result<Self> {
            let allow_plus = false;
            Self::parse(input, allow_plus)
        }

        pub(crate) fn parse(input: ParseStream, allow_plus: bool) -> Result<Self> {
            if input.peek(Token![->]) {
                let arrow = input.parse()?;
                let allow_group_generic = true;
                let ty = ambig_ty(input, allow_plus, allow_group_generic)?;
                Ok(ReturnType::Type(arrow, Box::new(ty)))
            } else {
                Ok(ReturnType::Default)
            }
        }
    }

    #[cfg_attr(doc_cfg, doc(cfg(feature = "parsing")))]
    impl Parse for ReturnType {
        fn parse(input: ParseStream) -> Result<Self> {
            let allow_plus = true;
            Self::parse(input, allow_plus)
        }
    }

    #[cfg_attr(doc_cfg, doc(cfg(feature = "parsing")))]
    impl Parse for TypeTraitObject {
        fn parse(input: ParseStream) -> Result<Self> {
            let allow_plus = true;
            Self::parse(input, allow_plus)
        }
    }

    impl TypeTraitObject {
        #[cfg_attr(doc_cfg, doc(cfg(feature = "parsing")))]
        pub fn without_plus(input: ParseStream) -> Result<Self> {
            let allow_plus = false;
            Self::parse(input, allow_plus)
        }

        // Only allow multiple trait references if allow_plus is true.
        pub(crate) fn parse(input: ParseStream, allow_plus: bool) -> Result<Self> {
            let dyn_token: Option<Token![dyn]> = input.parse()?;
            let dyn_span = match &dyn_token {
                Some(token) => token.span,
                None => input.span(),
            };
            let bounds = Self::parse_bounds(dyn_span, input, allow_plus)?;
            Ok(TypeTraitObject { dyn_token, bounds })
        }

        fn parse_bounds(
            dyn_span: Span,
            input: ParseStream,
            allow_plus: bool,
        ) -> Result<Punctuated<TypeParamBound, Token![+]>> {
            let bounds = TypeParamBound::parse_multiple(input, allow_plus)?;
            let mut last_lifetime_span = None;
            let mut at_least_one_trait = false;
            for bound in &bounds {
                match bound {
                    TypeParamBound::Trait(_) => {
                        at_least_one_trait = true;
                        break;
                    }
                    TypeParamBound::Lifetime(lifetime) => {
                        last_lifetime_span = Some(lifetime.ident.span());
                    }
                }
            }
            // Just lifetimes like `'a + 'b` is not a TraitObject.
            if !at_least_one_trait {
                let msg = "at least one trait is required for an object type";
                return Err(error::new2(dyn_span, last_lifetime_span.unwrap(), msg));
            }
            Ok(bounds)
        }
    }

    #[cfg_attr(doc_cfg, doc(cfg(feature = "parsing")))]
    impl Parse for TypeImplTrait {
        fn parse(input: ParseStream) -> Result<Self> {
            let allow_plus = true;
            Self::parse(input, allow_plus)
        }
    }

    impl TypeImplTrait {
        #[cfg_attr(doc_cfg, doc(cfg(feature = "parsing")))]
        pub fn without_plus(input: ParseStream) -> Result<Self> {
            let allow_plus = false;
            Self::parse(input, allow_plus)
        }

        pub(crate) fn parse(input: ParseStream, allow_plus: bool) -> Result<Self> {
            let impl_token: Token![impl] = input.parse()?;
            let bounds = TypeParamBound::parse_multiple(input, allow_plus)?;
            let mut last_lifetime_span = None;
            let mut at_least_one_trait = false;
            for bound in &bounds {
                match bound {
                    TypeParamBound::Trait(_) => {
                        at_least_one_trait = true;
                        break;
                    }
                    TypeParamBound::Lifetime(lifetime) => {
                        last_lifetime_span = Some(lifetime.ident.span());
                    }
                }
            }
            if !at_least_one_trait {
                let msg = "at least one trait must be specified";
                return Err(error::new2(
                    impl_token.span,
                    last_lifetime_span.unwrap(),
                    msg,
                ));
            }
            Ok(TypeImplTrait { impl_token, bounds })
        }
    }

    #[cfg_attr(doc_cfg, doc(cfg(feature = "parsing")))]
    impl Parse for TypeGroup {
        fn parse(input: ParseStream) -> Result<Self> {
            let group = crate::group::parse_group(input)?;
            Ok(TypeGroup {
                group_token: group.token,
                elem: group.content.parse()?,
            })
        }
    }

    #[cfg_attr(doc_cfg, doc(cfg(feature = "parsing")))]
    impl Parse for TypeParen {
        fn parse(input: ParseStream) -> Result<Self> {
            let allow_plus = false;
            Self::parse(input, allow_plus)
        }
    }

    impl TypeParen {
        fn parse(input: ParseStream, allow_plus: bool) -> Result<Self> {
            let content;
            Ok(TypeParen {
                paren_token: parenthesized!(content in input),
                elem: Box::new({
                    let allow_group_generic = true;
                    ambig_ty(&content, allow_plus, allow_group_generic)?
                }),
            })
        }
    }

    #[cfg_attr(doc_cfg, doc(cfg(feature = "parsing")))]
    impl Parse for BareFnArg {
        fn parse(input: ParseStream) -> Result<Self> {
            let allow_mut_self = false;
            parse_bare_fn_arg(input, allow_mut_self).map(Option::unwrap)
        }
    }

    fn parse_bare_fn_arg(
        input: ParseStream,
        mut allow_mut_self: bool,
    ) -> Result<Option<BareFnArg>> {
        let mut has_mut_self = false;
        let arg = BareFnArg {
            attrs: input.call(Attribute::parse_outer)?,
            name: {
                if (input.peek(Ident) || input.peek(Token![_]) || input.peek(Token![self]))
                    && input.peek2(Token![:])
                    && !input.peek2(Token![::])
                {
                    let name = input.call(Ident::parse_any)?;
                    let colon: Token![:] = input.parse()?;
                    Some((name, colon))
                } else if allow_mut_self
                    && input.peek(Token![mut])
                    && input.peek2(Token![self])
                    && input.peek3(Token![:])
                    && !input.peek3(Token![::])
                {
                    has_mut_self = true;
                    allow_mut_self = false;
                    input.parse::<Token![mut]>()?;
                    input.parse::<Token![self]>()?;
                    input.parse::<Token![:]>()?;
                    None
                } else {
                    None
                }
            },
            ty: if !has_mut_self && input.peek(Token![...]) {
                let dot3 = input.parse::<Token![...]>()?;
                let args = vec![
                    TokenTree::Punct(Punct::new('.', Spacing::Joint)),
                    TokenTree::Punct(Punct::new('.', Spacing::Joint)),
                    TokenTree::Punct(Punct::new('.', Spacing::Alone)),
                ];
                let tokens: TokenStream = args
                    .into_iter()
                    .zip(&dot3.spans)
                    .map(|(mut arg, span)| {
                        arg.set_span(*span);
                        arg
                    })
                    .collect();
                Type::Verbatim(tokens)
            } else if allow_mut_self && input.peek(Token![mut]) && input.peek2(Token![self]) {
                has_mut_self = true;
                input.parse::<Token![mut]>()?;
                Type::Path(TypePath {
                    qself: None,
                    path: input.parse::<Token![self]>()?.into(),
                })
            } else {
                input.parse()?
            },
        };

        if has_mut_self {
            Ok(None)
        } else {
            Ok(Some(arg))
        }
    }

    #[cfg_attr(doc_cfg, doc(cfg(feature = "parsing")))]
    impl Parse for Abi {
        fn parse(input: ParseStream) -> Result<Self> {
            Ok(Abi {
                extern_token: input.parse()?,
                name: input.parse()?,
            })
        }
    }

    #[cfg_attr(doc_cfg, doc(cfg(feature = "parsing")))]
    impl Parse for Option<Abi> {
        fn parse(input: ParseStream) -> Result<Self> {
            if input.peek(Token![extern]) {
                input.parse().map(Some)
            } else {
                Ok(None)
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
    impl ToTokens for TypeSlice {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            self.bracket_token.surround(tokens, |tokens| {
                self.elem.to_tokens(tokens);
            });
        }
    }

    #[cfg_attr(doc_cfg, doc(cfg(feature = "printing")))]
    impl ToTokens for TypeArray {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            self.bracket_token.surround(tokens, |tokens| {
                self.elem.to_tokens(tokens);
                self.semi_token.to_tokens(tokens);
                self.len.to_tokens(tokens);
            });
        }
    }

    #[cfg_attr(doc_cfg, doc(cfg(feature = "printing")))]
    impl ToTokens for TypePtr {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            self.star_token.to_tokens(tokens);
            match &self.mutability {
                Some(tok) => tok.to_tokens(tokens),
                None => {
                    TokensOrDefault(&self.const_token).to_tokens(tokens);
                }
            }
            self.elem.to_tokens(tokens);
        }
    }

    #[cfg_attr(doc_cfg, doc(cfg(feature = "printing")))]
    impl ToTokens for TypeReference {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            self.and_token.to_tokens(tokens);
            self.lifetime.to_tokens(tokens);
            self.mutability.to_tokens(tokens);
            self.elem.to_tokens(tokens);
        }
    }

    #[cfg_attr(doc_cfg, doc(cfg(feature = "printing")))]
    impl ToTokens for TypeBareFn {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            self.lifetimes.to_tokens(tokens);
            self.unsafety.to_tokens(tokens);
            self.abi.to_tokens(tokens);
            self.fn_token.to_tokens(tokens);
            self.paren_token.surround(tokens, |tokens| {
                self.inputs.to_tokens(tokens);
                if let Some(variadic) = &self.variadic {
                    if !self.inputs.empty_or_trailing() {
                        let span = variadic.dots.spans[0];
                        Token![,](span).to_tokens(tokens);
                    }
                    variadic.to_tokens(tokens);
                }
            });
            self.output.to_tokens(tokens);
        }
    }

    #[cfg_attr(doc_cfg, doc(cfg(feature = "printing")))]
    impl ToTokens for TypeNever {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            self.bang_token.to_tokens(tokens);
        }
    }

    #[cfg_attr(doc_cfg, doc(cfg(feature = "printing")))]
    impl ToTokens for TypeTuple {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            self.paren_token.surround(tokens, |tokens| {
                self.elems.to_tokens(tokens);
            });
        }
    }

    #[cfg_attr(doc_cfg, doc(cfg(feature = "printing")))]
    impl ToTokens for TypePath {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            path::printing::print_path(tokens, &self.qself, &self.path);
        }
    }

    #[cfg_attr(doc_cfg, doc(cfg(feature = "printing")))]
    impl ToTokens for TypeTraitObject {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            self.dyn_token.to_tokens(tokens);
            self.bounds.to_tokens(tokens);
        }
    }

    #[cfg_attr(doc_cfg, doc(cfg(feature = "printing")))]
    impl ToTokens for TypeImplTrait {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            self.impl_token.to_tokens(tokens);
            self.bounds.to_tokens(tokens);
        }
    }

    #[cfg_attr(doc_cfg, doc(cfg(feature = "printing")))]
    impl ToTokens for TypeGroup {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            self.group_token.surround(tokens, |tokens| {
                self.elem.to_tokens(tokens);
            });
        }
    }

    #[cfg_attr(doc_cfg, doc(cfg(feature = "printing")))]
    impl ToTokens for TypeParen {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            self.paren_token.surround(tokens, |tokens| {
                self.elem.to_tokens(tokens);
            });
        }
    }

    #[cfg_attr(doc_cfg, doc(cfg(feature = "printing")))]
    impl ToTokens for TypeInfer {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            self.underscore_token.to_tokens(tokens);
        }
    }

    #[cfg_attr(doc_cfg, doc(cfg(feature = "printing")))]
    impl ToTokens for TypeMacro {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            self.mac.to_tokens(tokens);
        }
    }

    #[cfg_attr(doc_cfg, doc(cfg(feature = "printing")))]
    impl ToTokens for ReturnType {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            match self {
                ReturnType::Default => {}
                ReturnType::Type(arrow, ty) => {
                    arrow.to_tokens(tokens);
                    ty.to_tokens(tokens);
                }
            }
        }
    }

    #[cfg_attr(doc_cfg, doc(cfg(feature = "printing")))]
    impl ToTokens for BareFnArg {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            tokens.append_all(self.attrs.outer());
            if let Some((name, colon)) = &self.name {
                name.to_tokens(tokens);
                colon.to_tokens(tokens);
            }
            self.ty.to_tokens(tokens);
        }
    }

    #[cfg_attr(doc_cfg, doc(cfg(feature = "printing")))]
    impl ToTokens for Variadic {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            tokens.append_all(self.attrs.outer());
            self.dots.to_tokens(tokens);
        }
    }

    #[cfg_attr(doc_cfg, doc(cfg(feature = "printing")))]
    impl ToTokens for Abi {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            self.extern_token.to_tokens(tokens);
            self.name.to_tokens(tokens);
        }
    }
}

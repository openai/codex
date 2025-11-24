//! Implementations of [`fmt`]-like derive macros.
//!
//! [`fmt`]: std::fmt

#[cfg(feature = "debug")]
pub(crate) mod debug;
#[cfg(feature = "display")]
pub(crate) mod display;
mod parsing;

use proc_macro2::TokenStream;
use quote::{format_ident, quote, ToTokens};
use syn::{
    parse::{Parse, ParseStream},
    punctuated::Punctuated,
    spanned::Spanned as _,
    token,
};

use crate::{
    parsing::Expr,
    utils::{attr, Either, Spanning},
};

/// Representation of a `bound` macro attribute, expressing additional trait bounds.
///
/// ```rust,ignore
/// #[<attribute>(bound(<where-predicates>))]
/// #[<attribute>(bounds(<where-predicates>))]
/// #[<attribute>(where(<where-predicates>))]
/// ```
#[derive(Debug, Default)]
struct BoundsAttribute(Punctuated<syn::WherePredicate, token::Comma>);

impl Parse for BoundsAttribute {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        Self::check_legacy_fmt(input)?;

        let _ = input.parse::<syn::Path>().and_then(|p| {
            if ["bound", "bounds", "where"]
                .into_iter()
                .any(|i| p.is_ident(i))
            {
                Ok(p)
            } else {
                Err(syn::Error::new(
                    p.span(),
                    "unknown attribute argument, expected `bound(...)`",
                ))
            }
        })?;

        let content;
        syn::parenthesized!(content in input);

        content
            .parse_terminated(syn::WherePredicate::parse, token::Comma)
            .map(Self)
    }
}

impl BoundsAttribute {
    /// Errors in case legacy syntax is encountered: `bound = "..."`.
    fn check_legacy_fmt(input: ParseStream<'_>) -> syn::Result<()> {
        let fork = input.fork();

        let path = fork
            .parse::<syn::Path>()
            .and_then(|path| fork.parse::<token::Eq>().map(|_| path));
        match path {
            Ok(path) if path.is_ident("bound") => fork
                .parse::<syn::Lit>()
                .ok()
                .and_then(|lit| match lit {
                    syn::Lit::Str(s) => Some(s.value()),
                    _ => None,
                })
                .map_or(Ok(()), |bound| {
                    Err(syn::Error::new(
                        input.span(),
                        format!("legacy syntax, use `bound({bound})` instead"),
                    ))
                }),
            Ok(_) | Err(_) => Ok(()),
        }
    }
}

/// Representation of a [`fmt`]-like attribute.
///
/// ```rust,ignore
/// #[<attribute>("<fmt-literal>", <fmt-args>)]
/// ```
///
/// [`fmt`]: std::fmt
#[derive(Debug)]
struct FmtAttribute {
    /// Interpolation [`syn::LitStr`].
    ///
    /// [`syn::LitStr`]: struct@syn::LitStr
    lit: syn::LitStr,

    /// Optional [`token::Comma`].
    ///
    /// [`token::Comma`]: struct@token::Comma
    comma: Option<token::Comma>,

    /// Interpolation arguments.
    args: Punctuated<FmtArgument, token::Comma>,
}

impl Parse for FmtAttribute {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        Self::check_legacy_fmt(input)?;

        let mut parsed = Self {
            lit: input.parse()?,
            comma: input
                .peek(token::Comma)
                .then(|| input.parse())
                .transpose()?,
            args: input.parse_terminated(FmtArgument::parse, token::Comma)?,
        };
        parsed.args.pop_punct();
        Ok(parsed)
    }
}

impl attr::ParseMultiple for FmtAttribute {}

impl ToTokens for FmtAttribute {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        self.lit.to_tokens(tokens);
        self.comma.to_tokens(tokens);
        self.args.to_tokens(tokens);
    }
}

impl FmtAttribute {
    /// Checks whether this [`FmtAttribute`] can be replaced with a transparent delegation (calling
    /// a formatting trait directly instead of interpolation syntax).
    ///
    /// If such transparent call is possible, the returns an [`Ident`] of the delegated trait and
    /// the [`Expr`] to pass into the call, otherwise [`None`].
    ///
    /// [`Ident`]: struct@syn::Ident
    fn transparent_call(&self) -> Option<(Expr, syn::Ident)> {
        // `FmtAttribute` is transparent when:

        // (1) There is exactly one formatting parameter.
        let lit = self.lit.value();
        let param =
            parsing::format(&lit).and_then(|(more, p)| more.is_empty().then_some(p))?;

        // (2) And the formatting parameter doesn't contain any modifiers.
        if param
            .spec
            .map(|s| {
                s.align.is_some()
                    || s.sign.is_some()
                    || s.alternate.is_some()
                    || s.zero_padding.is_some()
                    || s.width.is_some()
                    || s.precision.is_some()
                    || !s.ty.is_trivial()
            })
            .unwrap_or_default()
        {
            return None;
        }

        let expr = match param.arg {
            // (3) And either exactly one positional argument is specified.
            Some(parsing::Argument::Integer(_)) | None => (self.args.len() == 1)
                .then(|| self.args.first())
                .flatten()
                .map(|a| a.expr.clone()),

            // (4) Or the formatting parameter's name refers to some outer binding.
            Some(parsing::Argument::Identifier(name)) if self.args.is_empty() => {
                Some(format_ident!("{name}").into())
            }

            // (5) Or exactly one named argument is specified for the formatting parameter's name.
            Some(parsing::Argument::Identifier(name)) => (self.args.len() == 1)
                .then(|| self.args.first())
                .flatten()
                .filter(|a| a.alias.as_ref().map(|a| a.0 == name).unwrap_or_default())
                .map(|a| a.expr.clone()),
        }?;

        let trait_name = param
            .spec
            .map(|s| s.ty)
            .unwrap_or(parsing::Type::Display)
            .trait_name();

        Some((expr, format_ident!("{trait_name}")))
    }

    /// Returns an [`Iterator`] over bounded [`syn::Type`]s (and correspondent trait names) by this
    /// [`FmtAttribute`].
    fn bounded_types<'a>(
        &'a self,
        fields: &'a syn::Fields,
    ) -> impl Iterator<Item = (&'a syn::Type, &'static str)> {
        let placeholders = Placeholder::parse_fmt_string(&self.lit.value());

        // We ignore unknown fields, as compiler will produce better error messages.
        placeholders.into_iter().filter_map(move |placeholder| {
            let name = match placeholder.arg {
                Parameter::Named(name) => self
                    .args
                    .iter()
                    .find_map(|a| (a.alias()? == &name).then_some(&a.expr))
                    .map_or(Some(name), |expr| expr.ident().map(ToString::to_string))?,
                Parameter::Positional(i) => self
                    .args
                    .iter()
                    .nth(i)
                    .and_then(|a| a.expr.ident().filter(|_| a.alias.is_none()))?
                    .to_string(),
            };

            let unnamed = name.strip_prefix('_').and_then(|s| s.parse().ok());
            let ty = match (&fields, unnamed) {
                (syn::Fields::Unnamed(f), Some(i)) => {
                    f.unnamed.iter().nth(i).map(|f| &f.ty)
                }
                (syn::Fields::Named(f), None) => f.named.iter().find_map(|f| {
                    f.ident.as_ref().filter(|s| **s == name).map(|_| &f.ty)
                }),
                _ => None,
            }?;

            Some((ty, placeholder.trait_name))
        })
    }

    #[cfg(feature = "display")]
    /// Checks whether this [`FmtAttribute`] contains an argument with the provided `name` (either
    /// in its direct [`FmtArgument`]s or inside [`Placeholder`]s).
    fn contains_arg(&self, name: &str) -> bool {
        self.placeholders_by_arg(name).next().is_some()
    }

    #[cfg(feature = "display")]
    /// Returns an [`Iterator`] over [`Placeholder`]s using an argument with the provided `name`
    /// (either in its direct [`FmtArgument`]s of this [`FmtAttribute`] or inside the
    /// [`Placeholder`] itself).
    fn placeholders_by_arg<'a>(
        &'a self,
        name: &'a str,
    ) -> impl Iterator<Item = Placeholder> + 'a {
        let placeholders = Placeholder::parse_fmt_string(&self.lit.value());

        placeholders.into_iter().filter(move |placeholder| {
            match &placeholder.arg {
                Parameter::Named(name) => self
                    .args
                    .iter()
                    .find_map(|a| (a.alias()? == name).then_some(&a.expr))
                    .map_or(Some(name.clone()), |expr| {
                        expr.ident().map(ToString::to_string)
                    }),
                Parameter::Positional(i) => self
                    .args
                    .iter()
                    .nth(*i)
                    .and_then(|a| a.expr.ident().filter(|_| a.alias.is_none()))
                    .map(ToString::to_string),
            }
            .as_deref()
                == Some(name)
        })
    }

    /// Returns an [`Iterator`] over the additional formatting arguments doing the dereferencing
    /// replacement in this [`FmtAttribute`] for those [`Placeholder`] representing the provided
    /// [`syn::Fields`] and requiring it
    fn additional_deref_args<'fmt: 'ret, 'fields: 'ret, 'ret>(
        &'fmt self,
        fields: &'fields syn::Fields,
    ) -> impl Iterator<Item = TokenStream> + 'ret {
        let used_args = Placeholder::parse_fmt_string(&self.lit.value())
            .into_iter()
            .filter_map(|placeholder| match placeholder.arg {
                Parameter::Named(name) => Some(name),
                _ => None,
            })
            .collect::<Vec<_>>();

        fields.fmt_args_idents().filter_map(move |field_name| {
            (used_args.iter().any(|arg| field_name == arg)
                && !self.args.iter().any(|arg| {
                    arg.alias.as_ref().map_or(false, |(n, _)| n == &field_name)
                }))
            .then(|| quote! { #field_name = *#field_name })
        })
    }

    /// Errors in case legacy syntax is encountered: `fmt = "...", (arg),*`.
    fn check_legacy_fmt(input: ParseStream<'_>) -> syn::Result<()> {
        let fork = input.fork();

        let path = fork
            .parse::<syn::Path>()
            .and_then(|path| fork.parse::<token::Eq>().map(|_| path));
        match path {
            Ok(path) if path.is_ident("fmt") => (|| {
                let args = fork
                    .parse_terminated(
                        <Either<syn::Lit, syn::Ident>>::parse,
                        token::Comma,
                    )
                    .ok()?
                    .into_iter()
                    .enumerate()
                    .filter_map(|(i, arg)| match arg {
                        Either::Left(syn::Lit::Str(str)) => Some(if i == 0 {
                            format!("\"{}\"", str.value())
                        } else {
                            str.value()
                        }),
                        Either::Right(ident) => Some(ident.to_string()),
                        _ => None,
                    })
                    .collect::<Vec<_>>();
                (!args.is_empty()).then_some(args)
            })()
            .map_or(Ok(()), |fmt| {
                Err(syn::Error::new(
                    input.span(),
                    format!(
                        "legacy syntax, remove `fmt =` and use `{}` instead",
                        fmt.join(", "),
                    ),
                ))
            }),
            Ok(_) | Err(_) => Ok(()),
        }
    }
}

/// Representation of a [named parameter][1] (`identifier '=' expression`) in a [`FmtAttribute`].
///
/// [1]: https://doc.rust-lang.org/stable/std/fmt/index.html#named-parameters
#[derive(Debug)]
struct FmtArgument {
    /// `identifier =` [`Ident`].
    ///
    /// [`Ident`]: struct@syn::Ident
    alias: Option<(syn::Ident, token::Eq)>,

    /// `expression` [`Expr`].
    expr: Expr,
}

impl FmtArgument {
    /// Returns an `identifier` of the [named parameter][1].
    ///
    /// [1]: https://doc.rust-lang.org/stable/std/fmt/index.html#named-parameters
    fn alias(&self) -> Option<&syn::Ident> {
        self.alias.as_ref().map(|(ident, _)| ident)
    }
}

impl Parse for FmtArgument {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        Ok(Self {
            alias: (input.peek(syn::Ident) && input.peek2(token::Eq))
                .then(|| Ok::<_, syn::Error>((input.parse()?, input.parse()?)))
                .transpose()?,
            expr: input.parse()?,
        })
    }
}

impl ToTokens for FmtArgument {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        if let Some((ident, eq)) = &self.alias {
            ident.to_tokens(tokens);
            eq.to_tokens(tokens);
        }
        self.expr.to_tokens(tokens);
    }
}

/// Representation of a [parameter][1] used in a [`Placeholder`].
///
/// [1]: https://doc.rust-lang.org/stable/std/fmt/index.html#formatting-parameters
#[derive(Debug, Eq, PartialEq)]
enum Parameter {
    /// [Positional parameter][1].
    ///
    /// [1]: https://doc.rust-lang.org/stable/std/fmt/index.html#positional-parameters
    Positional(usize),

    /// [Named parameter][1].
    ///
    /// [1]: https://doc.rust-lang.org/stable/std/fmt/index.html#named-parameters
    Named(String),
}

impl<'a> From<parsing::Argument<'a>> for Parameter {
    fn from(arg: parsing::Argument<'a>) -> Self {
        match arg {
            parsing::Argument::Integer(i) => Self::Positional(i),
            parsing::Argument::Identifier(i) => Self::Named(i.to_owned()),
        }
    }
}

/// Representation of a formatting placeholder.
#[derive(Debug, Eq, PartialEq)]
struct Placeholder {
    /// Formatting argument (either named or positional) to be used by this [`Placeholder`].
    arg: Parameter,

    /// Indicator whether this [`Placeholder`] has any formatting modifiers.
    has_modifiers: bool,

    /// Name of [`std::fmt`] trait to be used for rendering this [`Placeholder`].
    trait_name: &'static str,
}

impl Placeholder {
    /// Parses [`Placeholder`]s from the provided formatting string.
    fn parse_fmt_string(s: &str) -> Vec<Self> {
        let mut n = 0;
        parsing::format_string(s)
            .into_iter()
            .flat_map(|f| f.formats)
            .map(|format| {
                let (maybe_arg, ty) = (
                    format.arg,
                    format.spec.map(|s| s.ty).unwrap_or(parsing::Type::Display),
                );
                let position = maybe_arg.map(Into::into).unwrap_or_else(|| {
                    // Assign "the next argument".
                    // https://doc.rust-lang.org/stable/std/fmt/index.html#positional-parameters
                    n += 1;
                    Parameter::Positional(n - 1)
                });

                Self {
                    arg: position,
                    has_modifiers: format
                        .spec
                        .map(|s| {
                            s.align.is_some()
                                || s.sign.is_some()
                                || s.alternate.is_some()
                                || s.zero_padding.is_some()
                                || s.width.is_some()
                                || s.precision.is_some()
                                || !s.ty.is_trivial()
                        })
                        .unwrap_or_default(),
                    trait_name: ty.trait_name(),
                }
            })
            .collect()
    }
}

/// Representation of a [`fmt::Display`]-like derive macro attributes placed on a container (struct
/// or enum variant).
///
/// ```rust,ignore
/// #[<attribute>("<fmt-literal>", <fmt-args>)]
/// #[<attribute>(bound(<where-predicates>))]
/// ```
///
/// `#[<attribute>(...)]` can be specified only once, while multiple `#[<attribute>(bound(...))]`
/// are allowed.
///
/// [`fmt::Display`]: std::fmt::Display
#[derive(Debug, Default)]
struct ContainerAttributes {
    /// Interpolation [`FmtAttribute`].
    fmt: Option<FmtAttribute>,

    /// Addition trait bounds.
    bounds: BoundsAttribute,
}

impl Parse for ContainerAttributes {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        // We do check `FmtAttribute::check_legacy_fmt` eagerly here, because `Either` will swallow
        // any error of the `Either::Left` if the `Either::Right` succeeds.
        FmtAttribute::check_legacy_fmt(input)?;
        <Either<FmtAttribute, BoundsAttribute>>::parse(input).map(|v| match v {
            Either::Left(fmt) => Self {
                bounds: BoundsAttribute::default(),
                fmt: Some(fmt),
            },
            Either::Right(bounds) => Self { bounds, fmt: None },
        })
    }
}

impl attr::ParseMultiple for ContainerAttributes {
    fn merge_attrs(
        prev: Spanning<Self>,
        new: Spanning<Self>,
        name: &syn::Ident,
    ) -> syn::Result<Spanning<Self>> {
        let Spanning {
            span: prev_span,
            item: mut prev,
        } = prev;
        let Spanning {
            span: new_span,
            item: new,
        } = new;

        if new.fmt.and_then(|n| prev.fmt.replace(n)).is_some() {
            return Err(syn::Error::new(
                new_span,
                format!("multiple `#[{name}(\"...\", ...)]` attributes aren't allowed"),
            ));
        }
        prev.bounds.0.extend(new.bounds.0);

        Ok(Spanning::new(
            prev,
            prev_span.join(new_span).unwrap_or(prev_span),
        ))
    }
}

/// Matches the provided `trait_name` to appropriate [`FmtAttribute`]'s argument name.
fn trait_name_to_attribute_name<T>(trait_name: T) -> &'static str
where
    T: for<'a> PartialEq<&'a str>,
{
    match () {
        _ if trait_name == "Binary" => "binary",
        _ if trait_name == "Debug" => "debug",
        _ if trait_name == "Display" => "display",
        _ if trait_name == "LowerExp" => "lower_exp",
        _ if trait_name == "LowerHex" => "lower_hex",
        _ if trait_name == "Octal" => "octal",
        _ if trait_name == "Pointer" => "pointer",
        _ if trait_name == "UpperExp" => "upper_exp",
        _ if trait_name == "UpperHex" => "upper_hex",
        _ => unimplemented!(),
    }
}

/// Extension of a [`syn::Type`] and a [`syn::Path`] allowing to travers its type parameters.
trait ContainsGenericsExt {
    /// Checks whether this definition contains any of the provided `type_params`.
    fn contains_generics(&self, type_params: &[&syn::Ident]) -> bool;
}

impl ContainsGenericsExt for syn::Type {
    fn contains_generics(&self, type_params: &[&syn::Ident]) -> bool {
        if type_params.is_empty() {
            return false;
        }
        match self {
            Self::Path(syn::TypePath { qself, path }) => {
                if let Some(qself) = qself {
                    if qself.ty.contains_generics(type_params) {
                        return true;
                    }
                }

                if let Some(ident) = path.get_ident() {
                    type_params.iter().any(|param| *param == ident)
                } else {
                    path.contains_generics(type_params)
                }
            }

            Self::Array(syn::TypeArray { elem, .. })
            | Self::Group(syn::TypeGroup { elem, .. })
            | Self::Paren(syn::TypeParen { elem, .. })
            | Self::Ptr(syn::TypePtr { elem, .. })
            | Self::Reference(syn::TypeReference { elem, .. })
            | Self::Slice(syn::TypeSlice { elem, .. }) => {
                elem.contains_generics(type_params)
            }

            Self::BareFn(syn::TypeBareFn { inputs, output, .. }) => {
                inputs
                    .iter()
                    .any(|arg| arg.ty.contains_generics(type_params))
                    || match output {
                        syn::ReturnType::Default => false,
                        syn::ReturnType::Type(_, ty) => {
                            ty.contains_generics(type_params)
                        }
                    }
            }

            Self::Tuple(syn::TypeTuple { elems, .. }) => {
                elems.iter().any(|ty| ty.contains_generics(type_params))
            }

            Self::TraitObject(syn::TypeTraitObject { bounds, .. }) => {
                bounds.iter().any(|bound| match bound {
                    syn::TypeParamBound::Trait(syn::TraitBound { path, .. }) => {
                        path.contains_generics(type_params)
                    }
                    syn::TypeParamBound::Lifetime(..)
                    | syn::TypeParamBound::Verbatim(..) => false,
                    _ => unimplemented!(
                        "syntax is not supported by `derive_more`, please report a bug",
                    ),
                })
            }

            Self::ImplTrait(..)
            | Self::Infer(..)
            | Self::Macro(..)
            | Self::Never(..)
            | Self::Verbatim(..) => false,
            _ => unimplemented!(
                "syntax is not supported by `derive_more`, please report a bug",
            ),
        }
    }
}

impl ContainsGenericsExt for syn::Path {
    fn contains_generics(&self, type_params: &[&syn::Ident]) -> bool {
        if type_params.is_empty() {
            return false;
        }
        self.segments
            .iter()
            .any(|segment| match &segment.arguments {
                syn::PathArguments::None => false,
                syn::PathArguments::AngleBracketed(
                    syn::AngleBracketedGenericArguments { args, .. },
                ) => args.iter().any(|generic| match generic {
                    syn::GenericArgument::Type(ty)
                    | syn::GenericArgument::AssocType(syn::AssocType { ty, .. }) => {
                        ty.contains_generics(type_params)
                    }

                    syn::GenericArgument::Lifetime(..)
                    | syn::GenericArgument::Const(..)
                    | syn::GenericArgument::AssocConst(..)
                    | syn::GenericArgument::Constraint(..) => false,
                    _ => unimplemented!(
                        "syntax is not supported by `derive_more`, please report a bug",
                    ),
                }),
                syn::PathArguments::Parenthesized(
                    syn::ParenthesizedGenericArguments { inputs, output, .. },
                ) => {
                    inputs.iter().any(|ty| ty.contains_generics(type_params))
                        || match output {
                            syn::ReturnType::Default => false,
                            syn::ReturnType::Type(_, ty) => {
                                ty.contains_generics(type_params)
                            }
                        }
                }
            })
    }
}

/// Extension of [`syn::Fields`] providing helpers for a [`FmtAttribute`].
trait FieldsExt {
    /// Returns an [`Iterator`] over [`syn::Ident`]s representing these [`syn::Fields`] in a
    /// [`FmtAttribute`] as [`FmtArgument`]s or named [`Placeholder`]s.
    ///
    /// [`syn::Ident`]: struct@syn::Ident
    fn fmt_args_idents(&self) -> impl Iterator<Item = syn::Ident> + '_;
}

impl FieldsExt for syn::Fields {
    fn fmt_args_idents(&self) -> impl Iterator<Item = syn::Ident> + '_ {
        self.iter()
            .enumerate()
            .map(|(i, f)| f.ident.clone().unwrap_or_else(|| format_ident!("_{i}")))
    }
}

#[cfg(test)]
mod fmt_attribute_spec {
    use itertools::Itertools as _;
    use quote::ToTokens;

    use super::FmtAttribute;

    fn assert<'a>(input: &'a str, parsed: impl AsRef<[&'a str]>) {
        let parsed = parsed.as_ref();
        let attr = syn::parse_str::<FmtAttribute>(&format!("\"\", {}", input)).unwrap();
        let fmt_args = attr
            .args
            .into_iter()
            .map(|arg| arg.into_token_stream().to_string())
            .collect::<Vec<String>>();
        fmt_args.iter().zip_eq(parsed).enumerate().for_each(
            |(i, (found, expected))| {
                assert_eq!(
                    *expected, found,
                    "Mismatch at index {i}\n\
                     Expected: {parsed:?}\n\
                     Found: {fmt_args:?}",
                );
            },
        );
    }

    #[test]
    fn cases() {
        let cases = [
            "ident",
            "alias = ident",
            "[a , b , c , d]",
            "counter += 1",
            "async { fut . await }",
            "a < b",
            "a > b",
            "{ let x = (a , b) ; }",
            "invoke (a , b)",
            "foo as f64",
            "| a , b | a + b",
            "obj . k",
            "for pat in expr { break pat ; }",
            "if expr { true } else { false }",
            "vector [2]",
            "1",
            "\"foo\"",
            "loop { break i ; }",
            "format ! (\"{}\" , q)",
            "match n { Some (n) => { } , None => { } }",
            "x . foo ::< T > (a , b)",
            "x . foo ::< T < [T < T >; if a < b { 1 } else { 2 }] >, { a < b } > (a , b)",
            "(a + b)",
            "i32 :: MAX",
            "1 .. 2",
            "& a",
            "[0u8 ; N]",
            "(a , b , c , d)",
            "< Ty as Trait > :: T",
            "< Ty < Ty < T >, { a < b } > as Trait < T > > :: T",
        ];

        assert("", []);
        for i in 1..4 {
            for permutations in cases.into_iter().permutations(i) {
                let mut input = permutations.clone().join(",");
                assert(&input, &permutations);
                input.push(',');
                assert(&input, &permutations);
            }
        }
    }
}

#[cfg(test)]
mod placeholder_parse_fmt_string_spec {
    use super::{Parameter, Placeholder};

    #[test]
    fn indicates_position_and_trait_name_for_each_fmt_placeholder() {
        let fmt_string = "{},{:?},{{}},{{{1:0$}}}-{2:.1$x}{par:#?}{:width$}";
        assert_eq!(
            Placeholder::parse_fmt_string(fmt_string),
            vec![
                Placeholder {
                    arg: Parameter::Positional(0),
                    has_modifiers: false,
                    trait_name: "Display",
                },
                Placeholder {
                    arg: Parameter::Positional(1),
                    has_modifiers: false,
                    trait_name: "Debug",
                },
                Placeholder {
                    arg: Parameter::Positional(1),
                    has_modifiers: true,
                    trait_name: "Display",
                },
                Placeholder {
                    arg: Parameter::Positional(2),
                    has_modifiers: true,
                    trait_name: "LowerHex",
                },
                Placeholder {
                    arg: Parameter::Named("par".to_owned()),
                    has_modifiers: true,
                    trait_name: "Debug",
                },
                Placeholder {
                    arg: Parameter::Positional(2),
                    has_modifiers: true,
                    trait_name: "Display",
                },
            ],
        );
    }
}

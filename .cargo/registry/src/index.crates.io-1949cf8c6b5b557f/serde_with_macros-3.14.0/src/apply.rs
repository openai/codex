use darling::{ast::NestedMeta, Error as DarlingError, FromMeta};
use proc_macro::TokenStream;
use quote::ToTokens as _;
use syn::{
    parse::{Parse, ParseStream},
    punctuated::Punctuated,
    Attribute, Error, Field, Path, Token, Type, TypeArray, TypeGroup, TypeParen, TypePath, TypePtr,
    TypeReference, TypeSlice, TypeTuple,
};

/// Parsed form of a single rule in the `#[apply(...)]` attribute.
///
/// This parses tokens in the shape of `Type => Attribute`.
/// For example, `Option<String> => #[serde(default)]`.
struct AddAttributesRule {
    /// A type pattern determining the fields to which the attributes are applied.
    ty: Type,
    /// The attributes to apply.
    ///
    /// All attributes are appended to the list of existing field attributes.
    attrs: Vec<Attribute>,
}

impl Parse for AddAttributesRule {
    fn parse(input: ParseStream<'_>) -> Result<Self, Error> {
        let ty: Type = input.parse()?;
        input.parse::<Token![=>]>()?;
        let attr = Attribute::parse_outer(input)?;
        Ok(AddAttributesRule { ty, attrs: attr })
    }
}

/// Parsed form of the `#[apply(...)]` attribute.
///
/// The `apply` attribute takes a comma separated list of rules in the shape of `Type => Attribute`.
/// Each rule is stored as a [`AddAttributesRule`].
struct ApplyInput {
    metas: Vec<NestedMeta>,
    rules: Punctuated<AddAttributesRule, Token![,]>,
}

impl Parse for ApplyInput {
    fn parse(input: ParseStream<'_>) -> Result<Self, Error> {
        let mut metas: Vec<NestedMeta> = Vec::new();

        while input.peek2(Token![=]) && !input.peek2(Token![=>]) {
            let value = NestedMeta::parse(input)?;
            metas.push(value);
            if !input.peek(Token![,]) {
                break;
            }
            input.parse::<Token![,]>()?;
        }

        let rules: Punctuated<AddAttributesRule, Token![,]> =
            input.parse_terminated(AddAttributesRule::parse, Token![,])?;
        Ok(Self { metas, rules })
    }
}

pub fn apply(args: TokenStream, input: TokenStream) -> TokenStream {
    let args = syn::parse_macro_input!(args as ApplyInput);

    #[derive(FromMeta)]
    struct SerdeContainerOptions {
        #[darling(rename = "crate")]
        alt_crate_path: Option<Path>,
    }

    let container_options = match SerdeContainerOptions::from_list(&args.metas) {
        Ok(v) => v,
        Err(e) => {
            return TokenStream::from(e.write_errors());
        }
    };
    let serde_with_crate_path = container_options
        .alt_crate_path
        .unwrap_or_else(|| syn::parse_quote!(::serde_with));

    let res = match super::apply_function_to_struct_and_enum_fields_darling(
        input,
        &serde_with_crate_path,
        &prepare_apply_attribute_to_field(args),
    ) {
        Ok(res) => res,
        Err(err) => err.write_errors(),
    };
    TokenStream::from(res)
}

/// Create a function compatible with [`super::apply_function_to_struct_and_enum_fields`] based on [`ApplyInput`].
///
/// A single [`ApplyInput`] can apply to multiple field types.
/// To account for this a new function must be created to stay compatible with the function signature or [`super::apply_function_to_struct_and_enum_fields`].
fn prepare_apply_attribute_to_field(
    input: ApplyInput,
) -> impl Fn(&mut Field) -> Result<(), DarlingError> {
    move |field: &mut Field| {
        let has_skip_attr = super::field_has_attribute(field, "serde_with", "skip_apply");
        if has_skip_attr {
            return Ok(());
        }

        for matcher in input.rules.iter() {
            if ty_pattern_matches_ty(&matcher.ty, &field.ty) {
                field.attrs.extend(matcher.attrs.clone());
            }
        }
        Ok(())
    }
}

fn ty_pattern_matches_ty(ty_pattern: &Type, ty: &Type) -> bool {
    match (ty_pattern, ty) {
        // Groups are invisible groupings which can for example come from macro_rules expansion.
        // This can lead to a mismatch where the `ty` is "Group { Option<String> }" and the `ty_pattern` is "Option<String>".
        // To account for this we unwrap the group and compare the inner types.
        (
            Type::Group(TypeGroup {
                elem: ty_pattern, ..
            }),
            ty,
        ) => ty_pattern_matches_ty(ty_pattern, ty),
        (ty_pattern, Type::Group(TypeGroup { elem: ty, .. })) => {
            ty_pattern_matches_ty(ty_pattern, ty)
        }

        // Processing of the other types
        (
            Type::Array(TypeArray {
                elem: ty_pattern,
                len: len_pattern,
                ..
            }),
            Type::Array(TypeArray { elem: ty, len, .. }),
        ) => {
            let ty_match = ty_pattern_matches_ty(ty_pattern, ty);
            let len_match = len_pattern == len || len_pattern.to_token_stream().to_string() == "_";
            ty_match && len_match
        }
        (Type::BareFn(ty_pattern), Type::BareFn(ty)) => ty_pattern == ty,
        (Type::ImplTrait(ty_pattern), Type::ImplTrait(ty)) => ty_pattern == ty,
        (Type::Infer(_), _) => true,
        (Type::Macro(ty_pattern), Type::Macro(ty)) => ty_pattern == ty,
        (Type::Never(_), Type::Never(_)) => true,
        (
            Type::Paren(TypeParen {
                elem: ty_pattern, ..
            }),
            Type::Paren(TypeParen { elem: ty, .. }),
        ) => ty_pattern_matches_ty(ty_pattern, ty),
        (
            Type::Path(TypePath {
                qself: qself_pattern,
                path: path_pattern,
            }),
            Type::Path(TypePath { qself, path }),
        ) => {
            /// Compare two paths for relaxed equality.
            ///
            /// Two paths match if they are equal except for the path arguments.
            /// Path arguments are generics on types or functions.
            /// If the pattern has no argument, it can match with everything.
            /// If the pattern does have an argument, the other side must be equal.
            fn path_pattern_matches_path(path_pattern: &Path, path: &Path) -> bool {
                if path_pattern.leading_colon != path.leading_colon
                    || path_pattern.segments.len() != path.segments.len()
                {
                    return false;
                }
                // Both parts are equal length
                std::iter::zip(&path_pattern.segments, &path.segments).all(
                    |(path_pattern_segment, path_segment)| {
                        let ident_equal = path_pattern_segment.ident == path_segment.ident;
                        let args_match =
                            match (&path_pattern_segment.arguments, &path_segment.arguments) {
                                (syn::PathArguments::None, _) => true,
                                (
                                    syn::PathArguments::AngleBracketed(
                                        syn::AngleBracketedGenericArguments {
                                            args: args_pattern,
                                            ..
                                        },
                                    ),
                                    syn::PathArguments::AngleBracketed(
                                        syn::AngleBracketedGenericArguments { args, .. },
                                    ),
                                ) => {
                                    args_pattern.len() == args.len()
                                        && std::iter::zip(args_pattern, args).all(|(a, b)| {
                                            match (a, b) {
                                                (
                                                    syn::GenericArgument::Type(ty_pattern),
                                                    syn::GenericArgument::Type(ty),
                                                ) => ty_pattern_matches_ty(ty_pattern, ty),
                                                (a, b) => a == b,
                                            }
                                        })
                                }
                                (args_pattern, args) => args_pattern == args,
                            };
                        ident_equal && args_match
                    },
                )
            }
            qself_pattern == qself && path_pattern_matches_path(path_pattern, path)
        }
        (
            Type::Ptr(TypePtr {
                const_token: const_token_pattern,
                mutability: mutability_pattern,
                elem: ty_pattern,
                ..
            }),
            Type::Ptr(TypePtr {
                const_token,
                mutability,
                elem: ty,
                ..
            }),
        ) => {
            const_token_pattern == const_token
                && mutability_pattern == mutability
                && ty_pattern_matches_ty(ty_pattern, ty)
        }
        (
            Type::Reference(TypeReference {
                lifetime: lifetime_pattern,
                elem: ty_pattern,
                ..
            }),
            Type::Reference(TypeReference {
                lifetime, elem: ty, ..
            }),
        ) => {
            (lifetime_pattern.is_none() || lifetime_pattern == lifetime)
                && ty_pattern_matches_ty(ty_pattern, ty)
        }
        (
            Type::Slice(TypeSlice {
                elem: ty_pattern, ..
            }),
            Type::Slice(TypeSlice { elem: ty, .. }),
        ) => ty_pattern_matches_ty(ty_pattern, ty),
        (Type::TraitObject(ty_pattern), Type::TraitObject(ty)) => ty_pattern == ty,
        (
            Type::Tuple(TypeTuple {
                elems: ty_pattern, ..
            }),
            Type::Tuple(TypeTuple { elems: ty, .. }),
        ) => {
            ty_pattern.len() == ty.len()
                && std::iter::zip(ty_pattern, ty)
                    .all(|(ty_pattern, ty)| ty_pattern_matches_ty(ty_pattern, ty))
        }
        (Type::Verbatim(_), Type::Verbatim(_)) => false,
        _ => false,
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[track_caller]
    fn matches(ty_pattern: &str, ty: &str) -> bool {
        let ty_pattern = syn::parse_str(ty_pattern).unwrap();
        let ty = syn::parse_str(ty).unwrap();
        ty_pattern_matches_ty(&ty_pattern, &ty)
    }

    #[test]
    fn test_ty_generic() {
        assert!(matches("Option<u8>", "Option<u8>"));
        assert!(matches("Option", "Option<u8>"));
        assert!(!matches("Option<u8>", "Option<String>"));

        assert!(matches("BTreeMap<u8, u8>", "BTreeMap<u8, u8>"));
        assert!(matches("BTreeMap", "BTreeMap<u8, u8>"));
        assert!(!matches("BTreeMap<String, String>", "BTreeMap<u8, u8>"));
        assert!(matches("BTreeMap<_, _>", "BTreeMap<u8, u8>"));
        assert!(matches("BTreeMap<_, u8>", "BTreeMap<u8, u8>"));
        assert!(!matches("BTreeMap<String, _>", "BTreeMap<u8, u8>"));
    }

    #[test]
    fn test_array() {
        assert!(matches("[u8; 1]", "[u8; 1]"));
        assert!(matches("[_; 1]", "[u8; 1]"));
        assert!(matches("[u8; _]", "[u8; 1]"));
        assert!(matches("[u8; _]", "[u8; N]"));

        assert!(!matches("[u8; 1]", "[u8; 2]"));
        assert!(!matches("[u8; 1]", "[u8; _]"));
        assert!(!matches("[u8; 1]", "[String; 1]"));
    }

    #[test]
    fn test_reference() {
        assert!(matches("&str", "&str"));
        assert!(matches("&mut str", "&str"));
        assert!(matches("&str", "&mut str"));
        assert!(matches("&str", "&'a str"));
        assert!(matches("&str", "&'static str"));
        assert!(matches("&str", "&'static mut str"));

        assert!(matches("&'a str", "&'a str"));
        assert!(matches("&'a mut str", "&'a str"));

        assert!(!matches("&'b str", "&'a str"));
    }
}

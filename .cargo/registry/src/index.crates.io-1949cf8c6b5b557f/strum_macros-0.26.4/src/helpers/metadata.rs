use proc_macro2::TokenStream;
use syn::{
    parenthesized,
    parse::{Parse, ParseStream},
    parse2, parse_str,
    punctuated::Punctuated,
    Attribute, DeriveInput, Expr, ExprLit, Field, Ident, Lit, LitBool, LitStr, Meta, MetaNameValue,
    Path, Token, Variant, Visibility,
};

use super::case_style::CaseStyle;

pub mod kw {
    use syn::custom_keyword;
    pub use syn::token::Crate;

    // enum metadata
    custom_keyword!(serialize_all);
    custom_keyword!(use_phf);
    custom_keyword!(prefix);

    // enum discriminant metadata
    custom_keyword!(derive);
    custom_keyword!(name);
    custom_keyword!(vis);

    // variant metadata
    custom_keyword!(message);
    custom_keyword!(detailed_message);
    custom_keyword!(serialize);
    custom_keyword!(to_string);
    custom_keyword!(disabled);
    custom_keyword!(default);
    custom_keyword!(default_with);
    custom_keyword!(props);
    custom_keyword!(ascii_case_insensitive);
}

pub enum EnumMeta {
    SerializeAll {
        kw: kw::serialize_all,
        case_style: CaseStyle,
    },
    AsciiCaseInsensitive(kw::ascii_case_insensitive),
    Crate {
        kw: kw::Crate,
        crate_module_path: Path,
    },
    UsePhf(kw::use_phf),
    Prefix {
        kw: kw::prefix,
        prefix: LitStr,
    },
}

impl Parse for EnumMeta {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let lookahead = input.lookahead1();
        if lookahead.peek(kw::serialize_all) {
            let kw = input.parse::<kw::serialize_all>()?;
            input.parse::<Token![=]>()?;
            let case_style = input.parse()?;
            Ok(EnumMeta::SerializeAll { kw, case_style })
        } else if lookahead.peek(kw::Crate) {
            let kw = input.parse::<kw::Crate>()?;
            input.parse::<Token![=]>()?;
            let path_str: LitStr = input.parse()?;
            let path_tokens = parse_str(&path_str.value())?;
            let crate_module_path = parse2(path_tokens)?;
            Ok(EnumMeta::Crate {
                kw,
                crate_module_path,
            })
        } else if lookahead.peek(kw::ascii_case_insensitive) {
            Ok(EnumMeta::AsciiCaseInsensitive(input.parse()?))
        } else if lookahead.peek(kw::use_phf) {
            Ok(EnumMeta::UsePhf(input.parse()?))
        } else if lookahead.peek(kw::prefix) {
            let kw = input.parse::<kw::prefix>()?;
            input.parse::<Token![=]>()?;
            let prefix = input.parse()?;
            Ok(EnumMeta::Prefix { kw, prefix })
        } else {
            Err(lookahead.error())
        }
    }
}

pub enum EnumDiscriminantsMeta {
    Derive { kw: kw::derive, paths: Vec<Path> },
    Name { kw: kw::name, name: Ident },
    Vis { kw: kw::vis, vis: Visibility },
    Other { path: Path, nested: TokenStream },
}

impl Parse for EnumDiscriminantsMeta {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        if input.peek(kw::derive) {
            let kw = input.parse()?;
            let content;
            parenthesized!(content in input);
            let paths = content.parse_terminated(Path::parse, Token![,])?;
            Ok(EnumDiscriminantsMeta::Derive {
                kw,
                paths: paths.into_iter().collect(),
            })
        } else if input.peek(kw::name) {
            let kw = input.parse()?;
            let content;
            parenthesized!(content in input);
            let name = content.parse()?;
            Ok(EnumDiscriminantsMeta::Name { kw, name })
        } else if input.peek(kw::vis) {
            let kw = input.parse()?;
            let content;
            parenthesized!(content in input);
            let vis = content.parse()?;
            Ok(EnumDiscriminantsMeta::Vis { kw, vis })
        } else {
            let path = input.parse()?;
            let content;
            parenthesized!(content in input);
            let nested = content.parse()?;
            Ok(EnumDiscriminantsMeta::Other { path, nested })
        }
    }
}

pub trait DeriveInputExt {
    /// Get all the strum metadata associated with an enum.
    fn get_metadata(&self) -> syn::Result<Vec<EnumMeta>>;

    /// Get all the `strum_discriminants` metadata associated with an enum.
    fn get_discriminants_metadata(&self) -> syn::Result<Vec<EnumDiscriminantsMeta>>;
}

impl DeriveInputExt for DeriveInput {
    fn get_metadata(&self) -> syn::Result<Vec<EnumMeta>> {
        get_metadata_inner("strum", &self.attrs)
    }

    fn get_discriminants_metadata(&self) -> syn::Result<Vec<EnumDiscriminantsMeta>> {
        get_metadata_inner("strum_discriminants", &self.attrs)
    }
}

pub enum VariantMeta {
    Message {
        kw: kw::message,
        value: LitStr,
    },
    DetailedMessage {
        kw: kw::detailed_message,
        value: LitStr,
    },
    Serialize {
        kw: kw::serialize,
        value: LitStr,
    },
    Documentation {
        value: LitStr,
    },
    ToString {
        kw: kw::to_string,
        value: LitStr,
    },
    Disabled(kw::disabled),
    Default(kw::default),
    DefaultWith {
        kw: kw::default_with,
        value: LitStr,
    },
    AsciiCaseInsensitive {
        kw: kw::ascii_case_insensitive,
        value: bool,
    },
    Props {
        kw: kw::props,
        props: Vec<(LitStr, LitStr)>,
    },
}

impl Parse for VariantMeta {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let lookahead = input.lookahead1();
        if lookahead.peek(kw::message) {
            let kw = input.parse()?;
            let _: Token![=] = input.parse()?;
            let value = input.parse()?;
            Ok(VariantMeta::Message { kw, value })
        } else if lookahead.peek(kw::detailed_message) {
            let kw = input.parse()?;
            let _: Token![=] = input.parse()?;
            let value = input.parse()?;
            Ok(VariantMeta::DetailedMessage { kw, value })
        } else if lookahead.peek(kw::serialize) {
            let kw = input.parse()?;
            let _: Token![=] = input.parse()?;
            let value = input.parse()?;
            Ok(VariantMeta::Serialize { kw, value })
        } else if lookahead.peek(kw::to_string) {
            let kw = input.parse()?;
            let _: Token![=] = input.parse()?;
            let value = input.parse()?;
            Ok(VariantMeta::ToString { kw, value })
        } else if lookahead.peek(kw::disabled) {
            Ok(VariantMeta::Disabled(input.parse()?))
        } else if lookahead.peek(kw::default) {
            Ok(VariantMeta::Default(input.parse()?))
        } else if lookahead.peek(kw::default_with) {
            let kw = input.parse()?;
            let _: Token![=] = input.parse()?;
            let value = input.parse()?;
            Ok(VariantMeta::DefaultWith { kw, value })
        } else if lookahead.peek(kw::ascii_case_insensitive) {
            let kw = input.parse()?;
            let value = if input.peek(Token![=]) {
                let _: Token![=] = input.parse()?;
                input.parse::<LitBool>()?.value
            } else {
                true
            };
            Ok(VariantMeta::AsciiCaseInsensitive { kw, value })
        } else if lookahead.peek(kw::props) {
            let kw = input.parse()?;
            let content;
            parenthesized!(content in input);
            let props = content.parse_terminated(Prop::parse, Token![,])?;
            Ok(VariantMeta::Props {
                kw,
                props: props
                    .into_iter()
                    .map(|Prop(k, v)| (LitStr::new(&k.to_string(), k.span()), v))
                    .collect(),
            })
        } else {
            Err(lookahead.error())
        }
    }
}

struct Prop(Ident, LitStr);

impl Parse for Prop {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        use syn::ext::IdentExt;

        let k = Ident::parse_any(input)?;
        let _: Token![=] = input.parse()?;
        let v = input.parse()?;

        Ok(Prop(k, v))
    }
}

pub trait VariantExt {
    /// Get all the metadata associated with an enum variant.
    fn get_metadata(&self) -> syn::Result<Vec<VariantMeta>>;
}

impl VariantExt for Variant {
    fn get_metadata(&self) -> syn::Result<Vec<VariantMeta>> {
        let result = get_metadata_inner("strum", &self.attrs)?;
        self.attrs
            .iter()
            .filter(|attr| attr.meta.path().is_ident("doc"))
            .try_fold(result, |mut vec, attr| {
                if let Meta::NameValue(MetaNameValue {
                    value:
                        Expr::Lit(ExprLit {
                            lit: Lit::Str(value),
                            ..
                        }),
                    ..
                }) = &attr.meta
                {
                    vec.push(VariantMeta::Documentation {
                        value: value.clone(),
                    })
                }
                Ok(vec)
            })
    }
}

fn get_metadata_inner<'a, T: Parse>(
    ident: &str,
    it: impl IntoIterator<Item = &'a Attribute>,
) -> syn::Result<Vec<T>> {
    it.into_iter()
        .filter(|attr| attr.path().is_ident(ident))
        .try_fold(Vec::new(), |mut vec, attr| {
            vec.extend(attr.parse_args_with(Punctuated::<T, Token![,]>::parse_terminated)?);
            Ok(vec)
        })
}

#[derive(Debug)]
pub enum InnerVariantMeta {
    DefaultWith { kw: kw::default_with, value: LitStr },
}

impl Parse for InnerVariantMeta {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let lookahead = input.lookahead1();
        if lookahead.peek(kw::default_with) {
            let kw = input.parse()?;
            let _: Token![=] = input.parse()?;
            let value = input.parse()?;
            Ok(InnerVariantMeta::DefaultWith { kw, value })
        } else {
            Err(lookahead.error())
        }
    }
}

pub trait InnerVariantExt {
    /// Get all the metadata associated with an enum variant inner.
    fn get_named_metadata(&self) -> syn::Result<Vec<InnerVariantMeta>>;
}

impl InnerVariantExt for Field {
    fn get_named_metadata(&self) -> syn::Result<Vec<InnerVariantMeta>> {
        let result = get_metadata_inner("strum", &self.attrs)?;
        self.attrs
            .iter()
            .filter(|attr| attr.meta.path().is_ident("default_with"))
            .try_fold(result, |vec, _attr| Ok(vec))
    }
}

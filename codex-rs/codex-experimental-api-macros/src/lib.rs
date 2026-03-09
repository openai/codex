use proc_macro::TokenStream;
use proc_macro2::Span;
use quote::quote;
use syn::Attribute;
use syn::Data;
use syn::DataEnum;
use syn::DataStruct;
use syn::DeriveInput;
use syn::Field;
use syn::Fields;
use syn::Ident;
use syn::LitStr;
use syn::Type;
use syn::parse_macro_input;

#[derive(Default)]
struct EnumSerdeConfig {
    rename_all: Option<String>,
    tag: Option<String>,
    untagged: bool,
}

#[proc_macro_derive(ExperimentalApi, attributes(experimental))]
pub fn derive_experimental_api(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    match &input.data {
        Data::Struct(data) => derive_for_struct(&input, data),
        Data::Enum(data) => derive_for_enum(&input, data),
        Data::Union(_) => {
            syn::Error::new_spanned(&input.ident, "ExperimentalApi does not support unions")
                .to_compile_error()
                .into()
        }
    }
}

fn derive_for_struct(input: &DeriveInput, data: &DataStruct) -> TokenStream {
    let name = &input.ident;
    let type_name_lit = LitStr::new(&name.to_string(), Span::call_site());

    let (checks, experimental_fields, registrations) = match &data.fields {
        Fields::Named(named) => {
            let mut checks = Vec::new();
            let mut experimental_fields = Vec::new();
            let mut registrations = Vec::new();
            for field in &named.named {
                let reason = experimental_reason(&field.attrs);
                if let Some(reason) = reason {
                    let expr = experimental_presence_expr(field, false);
                    checks.push(quote! {
                        if #expr {
                            return Some(#reason);
                        }
                    });

                    if let Some(field_name) = field_serialized_name(field) {
                        let field_name_lit = LitStr::new(&field_name, Span::call_site());
                        experimental_fields.push(quote! {
                            crate::experimental_api::ExperimentalField {
                                type_name: #type_name_lit,
                                field_name: #field_name_lit,
                                reason: #reason,
                            }
                        });
                        registrations.push(quote! {
                            ::inventory::submit! {
                                crate::experimental_api::ExperimentalField {
                                    type_name: #type_name_lit,
                                    field_name: #field_name_lit,
                                    reason: #reason,
                                }
                            }
                        });
                    }
                }
            }
            (checks, experimental_fields, registrations)
        }
        Fields::Unnamed(unnamed) => {
            let mut checks = Vec::new();
            let mut experimental_fields = Vec::new();
            let mut registrations = Vec::new();
            for (index, field) in unnamed.unnamed.iter().enumerate() {
                let reason = experimental_reason(&field.attrs);
                if let Some(reason) = reason {
                    let expr = index_presence_expr(index, &field.ty);
                    checks.push(quote! {
                        if #expr {
                            return Some(#reason);
                        }
                    });

                    let field_name_lit = LitStr::new(&index.to_string(), Span::call_site());
                    experimental_fields.push(quote! {
                        crate::experimental_api::ExperimentalField {
                            type_name: #type_name_lit,
                            field_name: #field_name_lit,
                            reason: #reason,
                        }
                    });
                    registrations.push(quote! {
                        ::inventory::submit! {
                            crate::experimental_api::ExperimentalField {
                                type_name: #type_name_lit,
                                field_name: #field_name_lit,
                                reason: #reason,
                            }
                        }
                    });
                }
            }
            (checks, experimental_fields, registrations)
        }
        Fields::Unit => (Vec::new(), Vec::new(), Vec::new()),
    };

    let checks = if checks.is_empty() {
        quote! { None }
    } else {
        quote! {
            #(#checks)*
            None
        }
    };

    let experimental_fields = if experimental_fields.is_empty() {
        quote! { &[] }
    } else {
        quote! { &[ #(#experimental_fields,)* ] }
    };

    let expanded = quote! {
        #(#registrations)*

        impl #name {
            pub(crate) const EXPERIMENTAL_FIELDS: &'static [crate::experimental_api::ExperimentalField] =
                #experimental_fields;
        }

        impl crate::experimental_api::ExperimentalApi for #name {
            fn experimental_reason(&self) -> Option<&'static str> {
                #checks
            }
        }
    };
    expanded.into()
}

fn derive_for_enum(input: &DeriveInput, data: &DataEnum) -> TokenStream {
    let name = &input.ident;
    let type_name_lit = LitStr::new(&name.to_string(), Span::call_site());
    let serde_config = enum_serde_config(&input.attrs);
    let mut match_arms = Vec::new();
    let mut registrations = Vec::new();

    for variant in &data.variants {
        let variant_name = &variant.ident;
        let pattern = match &variant.fields {
            Fields::Named(_) => quote!(Self::#variant_name { .. }),
            Fields::Unnamed(_) => quote!(Self::#variant_name ( .. )),
            Fields::Unit => quote!(Self::#variant_name),
        };
        let reason = experimental_reason(&variant.attrs);
        if let Some(reason) = reason {
            match_arms.push(quote! {
                #pattern => Some(#reason),
            });
            if let Some(registration) = experimental_enum_variant_registration(
                &type_name_lit,
                &serde_config,
                variant,
                &reason,
            ) {
                registrations.push(registration);
            }
        } else {
            match_arms.push(quote! {
                #pattern => None,
            });
        }
    }

    let expanded = quote! {
        #(#registrations)*

        impl crate::experimental_api::ExperimentalApi for #name {
            fn experimental_reason(&self) -> Option<&'static str> {
                match self {
                    #(#match_arms)*
                }
            }
        }
    };
    expanded.into()
}

fn experimental_reason(attrs: &[Attribute]) -> Option<LitStr> {
    let attr = attrs
        .iter()
        .find(|attr| attr.path().is_ident("experimental"))?;
    attr.parse_args::<LitStr>().ok()
}

fn enum_serde_config(attrs: &[Attribute]) -> EnumSerdeConfig {
    let mut config = EnumSerdeConfig::default();
    for attr in attrs.iter().filter(|attr| attr.path().is_ident("serde")) {
        let _ = attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("rename_all") {
                config.rename_all = Some(meta.value()?.parse::<LitStr>()?.value());
            } else if meta.path.is_ident("tag") {
                config.tag = Some(meta.value()?.parse::<LitStr>()?.value());
            } else if meta.path.is_ident("untagged") {
                config.untagged = true;
            }
            Ok(())
        });
    }
    config
}

fn variant_serialized_name(variant: &syn::Variant, rename_all: Option<&str>) -> String {
    if let Some(rename) = serde_rename(&variant.attrs) {
        return rename;
    }
    // The experimental enum-variant registry has to record the wire name, not
    // the Rust variant name, because schema/TS filtering later matches against
    // serialized enum arms. For now we only need a small subset of serde's
    // rename_all behavior, so keep this local mapping intentionally narrow.
    apply_rename_all(&variant.ident.to_string(), rename_all)
}

fn serde_rename(attrs: &[Attribute]) -> Option<String> {
    let mut rename = None;
    for attr in attrs.iter().filter(|attr| attr.path().is_ident("serde")) {
        let _ = attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("rename") {
                rename = Some(meta.value()?.parse::<LitStr>()?.value());
            }
            Ok(())
        });
    }
    rename
}

fn experimental_enum_variant_registration(
    type_name_lit: &LitStr,
    serde_config: &EnumSerdeConfig,
    variant: &syn::Variant,
    reason: &LitStr,
) -> Option<proc_macro2::TokenStream> {
    if serde_config.untagged {
        return None;
    }

    let serialized_name = variant_serialized_name(variant, serde_config.rename_all.as_deref());
    let serialized_name_lit = LitStr::new(&serialized_name, Span::call_site());
    let encoding = if let Some(tag_name) = serde_config.tag.as_deref() {
        let tag_name_lit = LitStr::new(tag_name, Span::call_site());
        quote! {
            crate::experimental_api::ExperimentalEnumVariantEncoding::TaggedObject {
                tag_name: #tag_name_lit,
            }
        }
    } else if matches!(variant.fields, Fields::Unit) {
        quote! {
            crate::experimental_api::ExperimentalEnumVariantEncoding::StringLiteral
        }
    } else {
        return None;
    };

    Some(quote! {
        ::inventory::submit! {
            crate::experimental_api::ExperimentalEnumVariant {
                type_name: #type_name_lit,
                serialized_name: #serialized_name_lit,
                reason: #reason,
                encoding: #encoding,
            }
        }
    })
}

fn field_serialized_name(field: &Field) -> Option<String> {
    let ident = field.ident.as_ref()?;
    let name = ident.to_string();
    Some(snake_to_camel(&name))
}

fn apply_rename_all(name: &str, rename_all: Option<&str>) -> String {
    let words = split_words(name);
    match rename_all {
        Some("camelCase") => {
            let mut out = String::new();
            for (index, word) in words.iter().enumerate() {
                if index == 0 {
                    out.push_str(word);
                } else {
                    out.push_str(&capitalize(word));
                }
            }
            out
        }
        Some("snake_case") => words.join("_"),
        Some("kebab-case") => words.join("-"),
        Some("PascalCase") => words
            .iter()
            .map(|word| capitalize(word))
            .collect::<Vec<_>>()
            .join(""),
        Some("SCREAMING_SNAKE_CASE") => words.join("_").to_ascii_uppercase(),
        Some("UPPERCASE") => words.concat().to_ascii_uppercase(),
        Some("lowercase") => words.concat(),
        Some(_) | None => name.to_string(),
    }
}

fn split_words(name: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut current = String::new();
    let chars: Vec<char> = name.chars().collect();
    for (index, ch) in chars.iter().copied().enumerate() {
        if ch == '_' || ch == '-' {
            if !current.is_empty() {
                words.push(std::mem::take(&mut current));
            }
            continue;
        }

        let previous = chars.get(index.wrapping_sub(1)).copied();
        let next = chars.get(index + 1).copied();
        let boundary_before = previous.is_some_and(|previous| {
            (previous.is_ascii_lowercase() && ch.is_ascii_uppercase())
                || (previous.is_ascii_uppercase()
                    && ch.is_ascii_uppercase()
                    && next.is_some_and(|next| next.is_ascii_lowercase()))
        });
        if boundary_before && !current.is_empty() {
            words.push(std::mem::take(&mut current));
        }
        current.push(ch.to_ascii_lowercase());
    }
    if !current.is_empty() {
        words.push(current);
    }
    words
}

fn capitalize(word: &str) -> String {
    let mut chars = word.chars();
    let Some(first) = chars.next() else {
        return String::new();
    };
    let mut out = String::new();
    out.push(first.to_ascii_uppercase());
    out.extend(chars);
    out
}

fn snake_to_camel(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut upper = false;
    for ch in s.chars() {
        if ch == '_' {
            upper = true;
            continue;
        }
        if upper {
            out.push(ch.to_ascii_uppercase());
            upper = false;
        } else {
            out.push(ch);
        }
    }
    out
}

fn experimental_presence_expr(
    field: &Field,
    tuple_struct: bool,
) -> Option<proc_macro2::TokenStream> {
    if tuple_struct {
        return None;
    }
    let ident = field.ident.as_ref()?;
    Some(presence_expr_for_access(quote!(self.#ident), &field.ty))
}

fn index_presence_expr(index: usize, ty: &Type) -> proc_macro2::TokenStream {
    let index = syn::Index::from(index);
    presence_expr_for_access(quote!(self.#index), ty)
}

fn presence_expr_for_access(
    access: proc_macro2::TokenStream,
    ty: &Type,
) -> proc_macro2::TokenStream {
    if let Some(inner) = option_inner(ty) {
        let inner_expr = presence_expr_for_ref(quote!(value), inner);
        return quote! {
            #access.as_ref().is_some_and(|value| #inner_expr)
        };
    }
    if is_vec_like(ty) || is_map_like(ty) {
        return quote! { !#access.is_empty() };
    }
    if is_bool(ty) {
        return quote! { #access };
    }
    quote! { true }
}

fn presence_expr_for_ref(access: proc_macro2::TokenStream, ty: &Type) -> proc_macro2::TokenStream {
    if let Some(inner) = option_inner(ty) {
        let inner_expr = presence_expr_for_ref(quote!(value), inner);
        return quote! {
            #access.as_ref().is_some_and(|value| #inner_expr)
        };
    }
    if is_vec_like(ty) || is_map_like(ty) {
        return quote! { !#access.is_empty() };
    }
    if is_bool(ty) {
        return quote! { *#access };
    }
    quote! { true }
}

fn option_inner(ty: &Type) -> Option<&Type> {
    let Type::Path(type_path) = ty else {
        return None;
    };
    let segment = type_path.path.segments.last()?;
    if segment.ident != "Option" {
        return None;
    }
    let syn::PathArguments::AngleBracketed(args) = &segment.arguments else {
        return None;
    };
    args.args.iter().find_map(|arg| match arg {
        syn::GenericArgument::Type(inner) => Some(inner),
        _ => None,
    })
}

fn is_vec_like(ty: &Type) -> bool {
    type_last_ident(ty).is_some_and(|ident| ident == "Vec")
}

fn is_map_like(ty: &Type) -> bool {
    type_last_ident(ty).is_some_and(|ident| ident == "HashMap" || ident == "BTreeMap")
}

fn is_bool(ty: &Type) -> bool {
    type_last_ident(ty).is_some_and(|ident| ident == "bool")
}

fn type_last_ident(ty: &Type) -> Option<Ident> {
    let Type::Path(type_path) = ty else {
        return None;
    };
    type_path.path.segments.last().map(|seg| seg.ident.clone())
}

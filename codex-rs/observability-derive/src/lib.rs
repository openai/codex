//! Derive macro implementation for `codex-observability`.
//!
//! The macro is intentionally small: it turns annotated struct fields into
//! `ObservationFieldVisitor` calls and refuses to compile if any exported field
//! is missing policy metadata. Filtering, serialization, redaction, and
//! destination-specific mapping stay in sinks/reducers.

use proc_macro::TokenStream;
use quote::format_ident;
use quote::quote;
use syn::Data;
use syn::DeriveInput;
use syn::Expr;
use syn::ExprLit;
use syn::Fields;
use syn::Lit;
use syn::LitStr;
use syn::Path;
use syn::parse_macro_input;

/// Derives `codex_observability::Observation` for a named struct.
///
/// Required attributes:
///
/// - `#[observation(name = "domain.event")]` on the struct.
/// - `#[obs(level = "basic|detailed|trace", class = "...")]` on every field.
/// - Optional struct-level or field-level uses markers for exact sink
///   selection. Field-level markers override the struct default.
///
/// Event definitions inside `codex-observability` itself may use
/// `#[observation(crate = "crate")]` so generated code refers to local types
/// instead of the externally visible crate path.
#[proc_macro_derive(Observation, attributes(observation, obs))]
pub fn derive_observation(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    expand_observation(input)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

fn expand_observation(input: DeriveInput) -> syn::Result<proc_macro2::TokenStream> {
    let name = input.ident;
    let observation_attr = observation_attr(&input.attrs)?;
    let event_name = observation_attr.name;
    let crate_path = observation_attr.crate_path;
    let default_uses = observation_attr.uses;
    let Data::Struct(data) = input.data else {
        return Err(syn::Error::new_spanned(
            name,
            "Observation can only be derived for structs",
        ));
    };
    let Fields::Named(fields) = data.fields else {
        return Err(syn::Error::new_spanned(
            name,
            "Observation requires a struct with named fields",
        ));
    };

    let mut visits = Vec::new();
    for field in fields.named {
        let Some(field_name) = field.ident else {
            continue;
        };
        let meta = obs_meta(&field.attrs, &field_name, &crate_path, &default_uses)?;
        let field_name_lit = LitStr::new(&field_name.to_string(), field_name.span());
        visits.push(quote! {
            visitor.field(#field_name_lit, #meta, &self.#field_name);
        });
    }

    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();
    Ok(quote! {
        impl #impl_generics #crate_path::Observation for #name #ty_generics #where_clause {
            const NAME: &'static str = #event_name;

            fn visit_fields<V: #crate_path::ObservationFieldVisitor>(&self, visitor: &mut V) {
                #(#visits)*
            }
        }
    })
}

struct ObservationAttr {
    name: LitStr,
    crate_path: Path,
    uses: Vec<LitStr>,
}

fn observation_attr(attrs: &[syn::Attribute]) -> syn::Result<ObservationAttr> {
    for attr in attrs {
        if !attr.path().is_ident("observation") {
            continue;
        }
        let mut name = None;
        let mut crate_path = None;
        let mut uses = Vec::new();
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("name") {
                name = Some(meta.value()?.parse::<LitStr>()?);
                Ok(())
            } else if meta.path.is_ident("crate") {
                let value = meta.value()?.parse::<LitStr>()?;
                // The override is needed only when deriving observations from
                // inside `codex-observability`, where the external crate name is
                // not available. Ordinary users get the stable public path.
                crate_path = Some(value.parse::<Path>()?);
                Ok(())
            } else if meta.path.is_ident("uses") {
                uses = use_literals(meta.value()?.parse::<Expr>()?)?;
                Ok(())
            } else {
                Err(meta.error("unsupported observation attribute"))
            }
        })?;
        if let Some(name) = name {
            return Ok(ObservationAttr {
                name,
                crate_path: crate_path.unwrap_or_else(|| syn::parse_quote!(::codex_observability)),
                uses,
            });
        }
    }
    Err(syn::Error::new(
        proc_macro2::Span::call_site(),
        "missing #[observation(name = \"...\")]",
    ))
}

fn obs_meta(
    attrs: &[syn::Attribute],
    field_name: &syn::Ident,
    crate_path: &Path,
    default_uses: &[LitStr],
) -> syn::Result<proc_macro2::TokenStream> {
    for attr in attrs {
        if !attr.path().is_ident("obs") {
            continue;
        }
        let mut level = None;
        let mut class = None;
        let mut uses = None;
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("level") {
                level = Some(meta.value()?.parse::<LitStr>()?);
                Ok(())
            } else if meta.path.is_ident("class") {
                class = Some(meta.value()?.parse::<LitStr>()?);
                Ok(())
            } else if meta.path.is_ident("uses") {
                uses = Some(use_literals(meta.value()?.parse::<Expr>()?)?);
                Ok(())
            } else {
                Err(meta.error("unsupported obs attribute"))
            }
        })?;
        let level = level.ok_or_else(|| {
            syn::Error::new_spanned(attr, "missing obs level, expected level = \"...\"")
        })?;
        let class = class.ok_or_else(|| {
            syn::Error::new_spanned(attr, "missing obs class, expected class = \"...\"")
        })?;
        let detail = detail_expr(&level, crate_path)?;
        let data_class = data_class_expr(&class, crate_path)?;
        let uses = uses
            .as_deref()
            .unwrap_or(default_uses)
            .iter()
            .map(|value| field_use_expr(value, crate_path))
            .collect::<syn::Result<Vec<_>>>()?;
        return Ok(quote! {
            #crate_path::FieldMeta::with_uses(#detail, #data_class, &[#(#uses),*])
        });
    }
    Err(syn::Error::new_spanned(
        field_name,
        "missing #[obs(level = \"...\", class = \"...\")]",
    ))
}

fn use_literals(expr: Expr) -> syn::Result<Vec<LitStr>> {
    let Expr::Array(array) = expr else {
        return Err(syn::Error::new_spanned(
            expr,
            "obs uses must be a string array, for example uses = [\"analytics\"]",
        ));
    };

    array
        .elems
        .into_iter()
        .map(|elem| match elem {
            Expr::Lit(ExprLit {
                lit: Lit::Str(value),
                ..
            }) => Ok(value),
            other => Err(syn::Error::new_spanned(
                other,
                "obs uses entries must be string literals",
            )),
        })
        .collect()
}

fn detail_expr(value: &LitStr, crate_path: &Path) -> syn::Result<proc_macro2::TokenStream> {
    enum_expr(
        value,
        "detail level",
        &[
            ("basic", "Basic"),
            ("detailed", "Detailed"),
            ("trace", "Trace"),
        ],
        quote!(#crate_path::DetailLevel),
    )
}

fn data_class_expr(value: &LitStr, crate_path: &Path) -> syn::Result<proc_macro2::TokenStream> {
    enum_expr(
        value,
        "data class",
        &[
            ("identifier", "Identifier"),
            ("operational", "Operational"),
            ("environment", "Environment"),
            ("content", "Content"),
            ("secret_risk", "SecretRisk"),
        ],
        quote!(#crate_path::DataClass),
    )
}

fn field_use_expr(value: &LitStr, crate_path: &Path) -> syn::Result<proc_macro2::TokenStream> {
    enum_expr(
        value,
        "field use",
        &[
            ("analytics", "Analytics"),
            ("otel", "Otel"),
            ("rollout_trace", "RolloutTrace"),
        ],
        quote!(#crate_path::FieldUse),
    )
}

fn enum_expr(
    value: &LitStr,
    label: &str,
    variants: &[(&str, &str)],
    path: proc_macro2::TokenStream,
) -> syn::Result<proc_macro2::TokenStream> {
    let raw = value.value();
    let Some((_, variant)) = variants.iter().find(|(name, _)| *name == raw) else {
        let expected = variants
            .iter()
            .map(|(name, _)| format!("\"{name}\""))
            .collect::<Vec<_>>()
            .join(", ");
        return Err(syn::Error::new_spanned(
            value,
            format!("invalid {label} {raw:?}, expected one of {expected}"),
        ));
    };
    let variant = format_ident!("{variant}");
    let expr: Expr = syn::parse_quote!(#path::#variant);
    Ok(quote!(#expr))
}

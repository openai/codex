// The use of fields in debug print commands does not count as "used",
// which causes the fields to trigger an unwanted dead code warning.
#![allow(dead_code)]

//! This example shows how to do struct and field parsing using darling.

use darling::{ast, FromDeriveInput, FromField, FromMeta};
use proc_macro2::TokenStream;
use quote::{quote, ToTokens};
use syn::parse_str;

/// A speaking volume. Deriving `FromMeta` will cause this to be usable
/// as a string value for a meta-item key.
#[derive(Debug, Clone, Copy, FromMeta)]
#[darling(default)]
enum Volume {
    Normal,
    Whisper,
    Shout,
}

impl Default for Volume {
    fn default() -> Self {
        Volume::Normal
    }
}

/// Support parsing from a full derive input. Unlike FromMeta, this isn't
/// composable; each darling-dependent crate should have its own struct to handle
/// when its trait is derived.
#[derive(Debug, FromDeriveInput)]
// This line says that we want to process all attributes declared with `my_trait`,
// and that darling should panic if this receiver is given an enum.
#[darling(attributes(my_trait), supports(struct_any))]
struct MyInputReceiver {
    /// The struct ident.
    ident: syn::Ident,

    /// The type's generics. You'll need these any time your trait is expected
    /// to work with types that declare generics.
    generics: syn::Generics,

    /// Receives the body of the struct or enum. We don't care about
    /// struct fields because we previously told darling we only accept structs.
    data: ast::Data<(), MyFieldReceiver>,

    /// The Input Receiver demands a volume, so use `Volume::Normal` if the
    /// caller doesn't provide one.
    #[darling(default)]
    volume: Volume,
}

impl ToTokens for MyInputReceiver {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let MyInputReceiver {
            ref ident,
            ref generics,
            ref data,
            volume,
        } = *self;

        let (imp, ty, wher) = generics.split_for_impl();
        let fields = data
            .as_ref()
            .take_struct()
            .expect("Should never be enum")
            .fields;

        // Generate the format string which shows each field and its name
        let fmt_string = fields
            .iter()
            .enumerate()
            .map(|(i, f)| {
                // We have to preformat the ident in this case so we can fall back
                // to the field index for unnamed fields. It's not easy to read,
                // unfortunately.
                format!(
                    "{} = {{}}",
                    f.ident
                        .as_ref()
                        .map(|v| format!("{}", v))
                        .unwrap_or_else(|| format!("{}", i))
                )
            })
            .collect::<Vec<_>>()
            .join(", ");

        // Generate the actual values to fill the format string.
        let field_list = fields
            .into_iter()
            .enumerate()
            .map(|(i, f)| {
                let field_volume = f.volume.unwrap_or(volume);

                // This works with named or indexed fields, so we'll fall back to the index so we can
                // write the output as a key-value pair.
                let field_ident = f.ident
                    .as_ref()
                    .map(|v| quote!(#v))
                    .unwrap_or_else(|| {
                        let i = syn::Index::from(i);
                        quote!(#i)
                    });

                match field_volume {
                    Volume::Normal => quote!(self.#field_ident),
                    Volume::Shout => {
                        quote!(::std::string::ToString::to_string(&self.#field_ident).to_uppercase())
                    }
                    Volume::Whisper => {
                        quote!(::std::string::ToString::to_string(&self.#field_ident).to_lowercase())
                    }
                }
            })
            .collect::<Vec<_>>();

        tokens.extend(quote! {
            impl #imp Speak for #ident #ty #wher {
                fn speak(&self, writer: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
                    write!(writer, #fmt_string, #(#field_list),*)
                }
            }
        });
    }
}

#[derive(Debug, FromField)]
#[darling(attributes(my_trait))]
struct MyFieldReceiver {
    /// Get the ident of the field. For fields in tuple or newtype structs or
    /// enum bodies, this can be `None`.
    ident: Option<syn::Ident>,

    /// This magic field name pulls the type from the input.
    ty: syn::Type,

    /// We declare this as an `Option` so that during tokenization we can write
    /// `field.volume.unwrap_or(derive_input.volume)` to facilitate field-level
    /// overrides of struct-level settings.
    ///
    /// Because this field is an `Option`, we don't need to include `#[darling(default)]`
    volume: Option<Volume>,
}

fn main() {
    let input = r#"#[derive(MyTrait)]
#[my_trait(volume = "shout")]
pub struct Foo {
    #[my_trait(volume = "whisper")]
    bar: bool,

    baz: i64,
}"#;

    let parsed = parse_str(input).unwrap();
    let receiver = MyInputReceiver::from_derive_input(&parsed).unwrap();
    let tokens = quote!(#receiver);

    println!(
        r#"
INPUT:

{}

PARSED AS:

{:?}

EMITS:

{}
    "#,
        input, receiver, tokens
    );
}

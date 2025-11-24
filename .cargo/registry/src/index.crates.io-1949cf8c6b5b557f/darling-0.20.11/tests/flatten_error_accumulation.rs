use darling::{util::Flag, Error, FromDeriveInput, FromMeta};
use proc_macro2::Ident;
use syn::parse_quote;

#[derive(FromMeta)]
#[darling(and_then = Self::validate)]
struct Vis {
    public: Flag,
    private: Flag,
}

impl Vis {
    fn validate(self) -> darling::Result<Self> {
        if self.public.is_present() && self.private.is_present() {
            return Err(Error::custom("Cannot be both public and private"));
        }

        Ok(self)
    }
}

#[derive(FromDeriveInput)]
#[darling(attributes(sample))]
#[allow(dead_code)]
struct Example {
    ident: Ident,
    label: String,
    volume: usize,
    #[darling(flatten)]
    visibility: Vis,
}

#[test]
fn many_errors() {
    let e = Example::from_derive_input(&parse_quote! {
        #[sample(volume = 10, public, private)]
        struct Demo {}
    })
    .map(|_| "Should have failed")
    .unwrap_err();

    // We are expecting an error from the Vis::validate method and an error for the
    // missing `label` field.
    assert_eq!(e.len(), 2);
}

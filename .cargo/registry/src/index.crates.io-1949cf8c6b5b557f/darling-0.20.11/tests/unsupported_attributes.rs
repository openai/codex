use darling::FromDeriveInput;
use syn::{parse_quote, Ident, LitStr, Path};

#[derive(Debug, FromDeriveInput)]
#[darling(supports(struct_unit), attributes(bar))]
pub struct Bar {
    pub ident: Ident,
    pub st: Path,
    pub file: LitStr,
}

/// Per [#96](https://github.com/TedDriggs/darling/issues/96), make sure that an
/// attribute which isn't a valid meta gets an error.
/// Properties can be split across multiple attributes; this test ensures that one
/// non-meta attribute does not interfere with the parsing of other, well-formed attributes.
#[test]
fn non_meta_attribute_does_not_block_others() {
    let di = parse_quote! {
        #[derive(Bar)]
        #[bar(st = RocketEngine: Debug)]
        #[bar(file = "motors/example_6.csv")]
        pub struct EstesC6;
    };

    let errors: darling::Error = Bar::from_derive_input(&di).unwrap_err().flatten();
    // The number of errors here is 2:
    // - The parsing error caused by a where-clause body where it doesn't belong
    // - The missing `st` value because the parsing failure blocked that attribute from
    //   being read.
    assert_eq!(2, errors.len());
}

use darling::{FromDeriveInput, FromMeta};

#[derive(FromDeriveInput)]
#[darling(attributes(demo))]
pub struct Receiver {
    example1: String,
    #[darling(
        // This should fail because `example1` is a local that's been captured
        // from the `FromDeriveInput` impl. That's disallowed because exposing
        // those internals would make any change to the derived method body a
        // potentially-breaking change.
        with = |m| Ok(
            String::from_meta(m)?.to_uppercase()
            + example1.1.as_ref().map(|s| s.as_str()).unwrap_or("")
        ),
        default
    )]
    example2: String,
}

fn main() {}

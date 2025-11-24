//! This example demonstrates techniques for performing custom error handling
//! in a derive-input receiver.
//!
//! 1. Using `darling::Result` as a carrier to preserve the error for later display
//! 1. Using `Result<T, syn::Meta>` to attempt a recovery in imperative code
//! 1. Using the `map` darling meta-item to post-process a field before returning
//! 1. Using the `and_then` darling meta-item to post-process the receiver before returning

use darling::{FromDeriveInput, FromMeta};
use syn::parse_str;

#[derive(Debug, FromDeriveInput)]
#[darling(attributes(my_trait), and_then = MyInputReceiver::autocorrect)]
pub struct MyInputReceiver {
    /// This field must be present and a string or else parsing will panic.
    #[darling(map = MyInputReceiver::make_string_shouty)]
    name: String,

    /// If this field fails to parse, the struct can still be built; the field
    /// will contain the error. The consuming struct can then decide if this
    /// blocks code generation. If so, panic or fail in `and_then`.
    frequency: darling::Result<i64>,

    /// If this field fails to parse, the struct can still be built; the field
    /// will contain an `Err` with the original `syn::Meta`. This can be used
    /// for alternate parsing attempts before panicking.
    amplitude: Result<u64, syn::Meta>,
}

impl MyInputReceiver {
    /// This function will be called by `darling` _after_ it's finished parsing the
    /// `name` field but before initializing `name` with the resulting value. It's
    /// a good place for transforms that are easiest to express on already-built
    /// types.
    fn make_string_shouty(s: String) -> String {
        s.to_uppercase()
    }

    /// This function will be called by `darling` _after_ it's finished parsing the
    /// input but before returning to the caller. This is a good place to initialize
    /// skipped fields or to perform corrections that don't lend themselves to being
    /// done elsewhere.
    fn autocorrect(self) -> darling::Result<Self> {
        let Self {
            name,
            frequency,
            amplitude,
        } = self;

        // Amplitude doesn't have a sign, so if we received a negative number then
        // we'll go ahead and make it positive.
        let amplitude = match amplitude {
            Ok(amp) => amp,
            Err(mi) => (i64::from_meta(&mi)?).unsigned_abs(),
        };

        Ok(Self {
            name,
            frequency,
            amplitude: Ok(amplitude),
        })
    }
}

fn main() {
    let input = r#"#[derive(MyTrait)]
#[my_trait(name="Jon", amplitude = "-1", frequency = 1)]
pub struct Foo;"#;

    let parsed = parse_str(input).unwrap();
    let receiver = MyInputReceiver::from_derive_input(&parsed).unwrap();

    println!(
        r#"
INPUT:

{}

PARSED AS:

{:?}
    "#,
        input, receiver
    );
}

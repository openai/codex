use darling::{FromDeriveInput, FromMeta};

#[derive(FromMeta, PartialEq, Eq, Debug)]
enum Volume {
    Whisper,
    Talk,
    Shout,
}

/// A more complex example showing the ability to skip at a field or struct
/// level while still tracking which type parameters need to be bounded.
/// This can be seen by expanding this example using `cargo expand`.
#[derive(FromMeta)]
#[allow(dead_code)]
enum Emphasis<T> {
    Constant(Volume),
    Variable(darling::util::PathList),
    #[darling(skip)]
    PerPhoneme(Option<T>),
    Strided {
        #[darling(skip)]
        step: Vec<T>,
        #[darling(multiple)]
        volume: Vec<Volume>,
    },
}

#[derive(FromDeriveInput)]
#[darling(attributes(speak))]
struct SpeakingOptions<T, U> {
    max_volume: U,
    #[darling(skip, default)]
    additional_data: Vec<T>,
}

#[derive(Default)]
struct Phoneme {
    #[allow(dead_code)]
    first: String,
}

// This is probably the holy grail for `darling`'s own internal use-case:
// Auto-apply `Default` bound to skipped *field* types in `where` clause.
impl<T, U> Default for SpeakingOptions<T, U>
where
    Vec<T>: Default,
    U: Default,
{
    fn default() -> Self {
        Self {
            max_volume: Default::default(),
            additional_data: Default::default(),
        }
    }
}

fn main() {
    let derive_input = syn::parse_str(
        r#"
        #[derive(Speak)]
        #[speak(max_volume = "shout")]
        enum HtmlElement {
            Div(String)
        }
    "#,
    )
    .unwrap();

    let parsed: SpeakingOptions<Phoneme, Volume> =
        FromDeriveInput::from_derive_input(&derive_input).unwrap();
    assert_eq!(parsed.max_volume, Volume::Shout);
    assert_eq!(parsed.additional_data.len(), 0);
}

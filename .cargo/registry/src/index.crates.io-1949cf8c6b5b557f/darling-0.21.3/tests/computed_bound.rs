use darling::{FromDeriveInput, FromMeta};

fn parse<T: FromDeriveInput>(src: &str) -> T {
    let ast = syn::parse_str(src).unwrap();
    FromDeriveInput::from_derive_input(&ast).unwrap()
}

#[derive(FromMeta, PartialEq, Eq, Debug)]
enum Volume {
    Whisper,
    Talk,
    Shout,
}

#[derive(FromDeriveInput)]
#[darling(attributes(speak))]
struct SpeakingOptions<T: Default, U> {
    max_volume: U,
    #[darling(skip)]
    #[allow(dead_code)]
    additional_data: T,
}

#[derive(Default)]
struct Phoneme {
    #[allow(dead_code)]
    first: String,
}

#[test]
fn skipped_field() {
    let parsed: SpeakingOptions<Phoneme, Volume> = parse(
        r#"
        #[derive(Speak)]
        #[speak(max_volume = "shout")]
        enum HtmlElement {
            Div(String)
        }
    "#,
    );
    assert_eq!(parsed.max_volume, Volume::Shout);
}

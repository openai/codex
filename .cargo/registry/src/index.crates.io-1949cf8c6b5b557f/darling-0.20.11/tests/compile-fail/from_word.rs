use darling::FromMeta;

// This usage of `from_word` is invalid because unit structs already generate a from_word
// method, and we don't allow using the from_word override when it conflicts with the macro's
// "normal" operation.
#[derive(FromMeta)]
#[darling(from_word = || Ok(Unit))]
struct Unit;

fn newtype_from_word() -> darling::Result<Newtype> {
    Ok(Newtype(true))
}

// This usage of `from_word` is invalid because newtype structs call the inner type's `from_meta`
// directly from their `from_meta`, so the custom `from_word` will never be called in normal usage.
#[derive(FromMeta)]
#[darling(from_word = newtype_from_word)]
struct Newtype(bool);

#[derive(FromMeta)]
#[darling(from_word = || Ok(Wordy::Options { thing: "Hello".to_string() }))]
enum Wordy {
    #[darling(word)]
    Normal,
    Options {
        thing: String,
    },
}

fn main() {}

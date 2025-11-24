use darling::FromMeta;

#[derive(FromMeta)]
enum Choice {
    #[darling(word, word)]
    A,
    B,
}

fn main() {}

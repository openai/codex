use darling::FromMeta;

#[derive(FromMeta)]
enum Choice {
    #[darling(word)]
    A,
    #[darling(word)]
    B,
    C,
}

fn main() {}

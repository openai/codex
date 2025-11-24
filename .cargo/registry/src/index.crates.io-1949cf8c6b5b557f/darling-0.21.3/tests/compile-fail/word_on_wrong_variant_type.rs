use darling::FromMeta;

#[derive(FromMeta)]
enum Meta {
    Unit,
    #[darling(word)]
    NotUnit(String)
}

fn main() {}

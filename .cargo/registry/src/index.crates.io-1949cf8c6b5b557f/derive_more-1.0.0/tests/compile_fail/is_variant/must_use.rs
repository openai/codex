#[derive(derive_more::IsVariant)]
enum MustUse {
    Yes,
}

#[forbid(unused_must_use)]
fn main() {
    let must_use = MustUse::Yes;
    must_use.is_yes();
}

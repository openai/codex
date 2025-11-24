use darling::FromMeta;

struct NotImplFm;

#[derive(FromMeta)]
struct OuterFm {
    inner: NotImplFm,
}

#[derive(darling::FromDeriveInput)]
#[darling(attributes(hello))]
struct OuterFdi {
    inner: NotImplFm,
}

fn main() {}

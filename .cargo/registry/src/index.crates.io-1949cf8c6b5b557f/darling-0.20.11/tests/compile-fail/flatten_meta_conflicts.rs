use darling::FromMeta;

#[derive(FromMeta)]
struct Inner {
    left: String,
    right: String,
}

#[derive(FromMeta)]
struct Outer {
    #[darling(flatten, multiple, with = demo, skip = true)]
    field: Inner,
}

#[derive(FromMeta)]
struct ThisIsFine {
    #[darling(flatten, multiple = false)]
    field: Inner,
}

fn main() {}

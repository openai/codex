use darling::FromMeta;

#[derive(FromMeta)]
struct NoDefault(String);

#[derive(FromMeta)]
struct Recevier {
    #[darling(skip)]
    skipped: NoDefault,

    #[darling(skip = true)]
    explicitly_skipped: NoDefault,

    #[darling(skip = false)]
    not_skipped_no_problem: NoDefault,
}

fn main() {}

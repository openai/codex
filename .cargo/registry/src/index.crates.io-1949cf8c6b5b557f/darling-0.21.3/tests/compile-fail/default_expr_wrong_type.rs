use darling::FromMeta;

#[derive(FromMeta)]
struct Receiver {
    #[darling(default = "usize::default")]
    not_u32: String,

    #[darling(multiple, default = "usize::default")]
    also_not_u32: Vec<String>,
}

fn main() {}

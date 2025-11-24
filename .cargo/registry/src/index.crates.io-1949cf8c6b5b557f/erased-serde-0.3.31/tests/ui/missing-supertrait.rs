use erased_serde::serialize_trait_object;

pub trait MyTrait {}

serialize_trait_object!(MyTrait);

fn main() {}

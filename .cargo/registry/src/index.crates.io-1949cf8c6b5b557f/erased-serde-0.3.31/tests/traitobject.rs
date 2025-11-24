use erased_serde::serialize_trait_object;

pub trait MyTrait: erased_serde::Serialize {}

serialize_trait_object!(MyTrait);

pub trait MyGenericTrait<'a, T>: erased_serde::Serialize {}

serialize_trait_object!(<'a, T> MyGenericTrait<'a, T>);

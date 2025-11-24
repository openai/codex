#[derive(derive_more::Into)]
#[into(owned(types("Cow<'_ str>")), ref, ref_mut, types(i32, "&str"))]
struct Foo(String);

fn main() {}

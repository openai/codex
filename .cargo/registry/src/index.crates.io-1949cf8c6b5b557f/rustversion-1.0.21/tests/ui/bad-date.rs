#[rustversion::nightly(stable)]
struct S;

#[rustversion::any(nightly(stable))]
struct S;

fn main() {}

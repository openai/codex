#[rustversion::stable(nightly)]
struct S;

#[rustversion::any(stable(nightly))]
struct S;

fn main() {}

#[rustversion::any(not)]
struct S;

#[rustversion::any(not, not)]
struct S;

fn main() {}

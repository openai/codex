struct NoDebug<'a> {
    a: &'a f64,
}

#[derive(derive_more::Debug)]
struct SomeType<'a> {
    no_debug: NoDebug<'a>,
}

fn main() {}

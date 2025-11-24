//! This program is a regression test for [#3306], where shadowing
//! caused compilation failure in certain cases due to the original
//! function body not getting its own scope.
//!
//! [#3306]: https://github.com/tokio-rs/tracing/issues/3306
type Foo = ();
enum Bar {
    Foo,
}

#[tracing::instrument]
fn this_is_fine() -> Foo {
    // glob import imports Bar::Foo, shadowing Foo
    use Bar::*;
}

fn main() {}

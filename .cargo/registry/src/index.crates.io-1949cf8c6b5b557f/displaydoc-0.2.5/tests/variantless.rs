use displaydoc::Display;

#[derive(Display)]
enum EmptyInside {}

static_assertions::assert_impl_all!(EmptyInside: core::fmt::Display);

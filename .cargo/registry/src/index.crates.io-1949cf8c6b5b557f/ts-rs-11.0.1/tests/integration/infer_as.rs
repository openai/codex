#![allow(dead_code)]

use ts_rs::TS;

trait Bar {
    type Baz;
}

impl Bar for String {
    type Baz = i32;
}

#[derive(TS)]
#[ts(export)]
struct Foo {
    #[ts(optional, as = "Option<_>")]
    my_optional_bool: bool,

    #[ts(as = "<_ as Bar>::Baz")]
    q_self: String,
}

#[test]
fn test() {
    assert_eq!(
        Foo::inline(),
        "{ my_optional_bool?: boolean, q_self: number, }"
    );
}

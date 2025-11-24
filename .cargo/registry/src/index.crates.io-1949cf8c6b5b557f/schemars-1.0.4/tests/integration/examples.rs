use crate::prelude::*;

#[derive(Default, JsonSchema, Serialize)]
#[schemars(example = Struct::default(), example = ())]
struct Struct {
    #[schemars(example = 4 + 4, example = ())]
    foo: i32,
    bar: bool,
    #[schemars(example = (), example = &"foo")]
    baz: Option<&'static str>,
}

#[test]
fn examples() {
    test!(Struct).assert_snapshot();
}

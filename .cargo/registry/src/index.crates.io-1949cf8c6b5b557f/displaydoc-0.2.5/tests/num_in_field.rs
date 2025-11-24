/// {foo1} {foo2}
#[derive(displaydoc::Display)]
pub struct Test {
    foo1: String,
    foo2: String,
}

fn assert_display<T: std::fmt::Display>(input: T, expected: &'static str) {
    let out = format!("{}", input);
    assert_eq!(expected, out);
}

#[test]
fn does_it_print() {
    assert_display(
        Test {
            foo1: "hi".into(),
            foo2: "hello".into(),
        },
        "hi hello",
    );
}

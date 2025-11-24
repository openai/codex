use super::*;
use core::fmt;
use serde_with::SerializeDisplay;

#[derive(Debug, SerializeDisplay)]
struct A {
    a: u32,
    b: bool,
}

impl fmt::Display for A {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_fmt(format_args!("->{} <> {}<-", self.a, self.b))
    }
}

#[test]
fn test_serialize_display() {
    check_serialization(A { a: 123, b: false }, expect![[r#""->123 <> false<-""#]]);
    check_serialization(A { a: 0, b: true }, expect![[r#""->0 <> true<-""#]]);
    check_serialization(A { a: 999, b: true }, expect![[r#""->999 <> true<-""#]]);
}

#[test]
fn test_serialize_display_in_vec() {
    check_serialization(
        vec![
            A { a: 123, b: false },
            A { a: 0, b: true },
            A { a: 999, b: true },
        ],
        expect![[r#"
        [
          "->123 <> false<-",
          "->0 <> true<-",
          "->999 <> true<-"
        ]"#]],
    );
}

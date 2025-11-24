#![allow(unused, dead_code, clippy::disallowed_names)]

use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Serialize, Deserialize, TS)]
struct Foo {
    a: i32,
}

#[derive(Serialize, Deserialize, TS)]
struct Bar {
    a: i32,
}

mod deser {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    use super::Foo;

    pub fn serialize<S: Serializer>(foo: &Foo, serializer: S) -> Result<S::Ok, S::Error> {
        foo.serialize(serializer)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Foo, D::Error> {
        Foo::deserialize(deserializer)
    }
}

// This test should pass when serde-compat is disabled,
// otherwise, it should fail to compile
#[test]
#[cfg(not(feature = "serde-compat"))]
fn no_serde_compat() {
    #[derive(Serialize, Deserialize, TS)]
    struct Baz {
        #[serde(with = "deser")]
        a: Foo,
    }

    assert_eq!(Baz::inline(), "{ a: Foo, }")
}

#[test]
fn serde_compat_as() {
    #[derive(Serialize, Deserialize, TS)]
    struct Baz {
        #[serde(with = "deser")]
        #[ts(as = "Bar")]
        a: Foo,
    }

    assert_eq!(Baz::inline(), "{ a: Bar, }")
}

#[test]
fn serde_compat_type() {
    #[derive(Serialize, Deserialize, TS)]
    struct Baz {
        #[serde(with = "deser")]
        #[ts(type = "{ a: number }")]
        a: Foo,
    }

    assert_eq!(Baz::inline(), "{ a: { a: number }, }")
}

use crate::prelude::*;

mod external {
    #[derive(Default)]
    pub struct Duration {
        pub secs: i64,
        pub nanos: i32,
    }

    #[allow(dead_code)]
    pub enum Or<A, B> {
        A(A),
        B(B),
    }

    pub struct Str<'a>(pub &'a str);
}

#[derive(JsonSchema, Deserialize, Serialize)]
#[serde(remote = "external::Duration")]
struct DurationDef {
    secs: i64,
    nanos: i32,
}

#[derive(JsonSchema, Deserialize, Serialize, Default)]
struct Process {
    #[serde(with = "DurationDef")]
    wall_time: external::Duration,
}

#[test]
fn simple() {
    test!(Process)
        .assert_snapshot()
        .assert_allows_ser_roundtrip_default()
        .assert_matches_de_roundtrip(arbitrary_values());
}

#[derive(JsonSchema, Deserialize, Serialize)]
#[serde(untagged, remote = "external::Or")]
#[schemars(rename = "{A}_or_{B}")]
enum OrDef<A, B> {
    A(A),
    B(B),
}

#[derive(JsonSchema, Deserialize, Serialize)]
#[serde(bound = "T: serde::de::DeserializeOwned + Serialize")]
struct TypeParam<T> {
    #[serde(with = "OrDef::<u8, bool>")]
    byte_or_bool: external::Or<u8, bool>,
    #[serde(with = "OrDef::<(), T>")]
    unit_or_t: external::Or<(), T>,
}

#[test]
fn type_param() {
    test!(TypeParam<String>)
        .assert_snapshot()
        .assert_allows_ser_roundtrip([
            TypeParam {
                byte_or_bool: external::Or::A(123),
                unit_or_t: external::Or::A(()),
            },
            TypeParam {
                byte_or_bool: external::Or::B(true),
                unit_or_t: external::Or::B("test".to_owned()),
            },
        ])
        .assert_matches_de_roundtrip(arbitrary_values());
}

#[allow(dead_code)]
#[derive(JsonSchema, Deserialize, Serialize)]
#[serde(remote = "external::Str")]
struct StrDef<'a>(&'a str);

#[derive(JsonSchema, Deserialize, Serialize)]
struct LifetimeParam<'a> {
    #[serde(borrow, with = "StrDef")]
    s: external::Str<'a>,
}

#[test]
fn lifetime_param() {
    let s = external::Str("test");

    test!(LifetimeParam)
        .assert_snapshot()
        .assert_allows_ser_only([LifetimeParam { s }]);
}

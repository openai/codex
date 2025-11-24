mod util;

use schemars::JsonSchema;
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use util::*;

#[allow(dead_code)]
enum Or<A, B> {
    A(A),
    B(B),
}

#[derive(JsonSchema, Serialize)]
#[serde(untagged, remote = "Or")]
enum OrDef<A, B> {
    A(A),
    B(B),
}

struct Str<'a>(&'a str);

#[allow(dead_code)]
#[derive(JsonSchema, Serialize)]
#[serde(remote = "Str")]
struct StrDef<'a>(&'a str);

#[derive(JsonSchema, Serialize)]
struct MyStruct<'a, T: Serialize> {
    // #[serde(with = "OrDef::<_, _>")]
    // byte_or_bool1: Or<u8, bool>,
    #[serde(with = "OrDef::<u8, bool>")]
    byte_or_bool2: Or<u8, bool>,
    // #[serde(with = "OrDef::<_, _>")]
    // unit_or_t1: Or<(), T>,
    #[serde(with = "OrDef::<(), T>")]
    unit_or_t2: Or<(), T>,
    #[serde(borrow, with = "StrDef")]
    s: Str<'a>,
    // #[schemars(with = "HashMap::<_, HashSet<_>>")]
    // map: BTreeMap<String, BTreeSet<String>>,
    #[schemars(with = "HashMap::<String, HashSet<String>>")]
    fake_map: (),
}

#[test]
fn remote_derive_json_schema() -> TestResult {
    test_default_generated_schema::<MyStruct<i32>>("remote_derive_generic")
}

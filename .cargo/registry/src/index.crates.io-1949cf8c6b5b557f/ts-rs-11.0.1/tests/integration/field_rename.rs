#![allow(dead_code)]

use ts_rs::TS;

#[derive(TS)]
#[cfg_attr(feature = "serde-compat", derive(serde::Serialize, serde::Deserialize))]
struct Rename {
    #[cfg_attr(
        feature = "serde-compat",
        serde(rename = "c", skip_serializing_if = "String::is_empty")
    )]
    a: String,
    #[ts(rename = "bb")]
    b: i32,
}

#[test]
fn test() {
    if (cfg!(feature = "serde-compat")) {
        assert_eq!(Rename::inline(), "{ c: string, bb: number, }")
    } else {
        assert_eq!(Rename::inline(), "{ a: string, bb: number, }")
    }
}

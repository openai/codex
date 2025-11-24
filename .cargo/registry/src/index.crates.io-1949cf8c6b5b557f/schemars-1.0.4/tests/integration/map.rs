use crate::prelude::*;
use std::collections::{BTreeMap, HashMap};

#[derive(JsonSchema, Deserialize, Serialize, Default, PartialEq, Eq, PartialOrd, Ord)]
#[schemars(extend("pattern" = "^[0-9a-f]*$"))]
struct HexNumber(String);

#[derive(JsonSchema, Deserialize, Serialize, Default)]
struct Maps {
    s_map: HashMap<String, bool>,
    i_map: BTreeMap<i8, bool>,
    u_map: HashMap<u64, bool>,
    pattern_map: BTreeMap<HexNumber, bool>,
}

#[test]
fn maps() {
    test!(Maps)
        .assert_snapshot()
        .assert_allows_ser_roundtrip([Maps {
            s_map: HashMap::from_iter([("test".to_owned(), true)]),
            i_map: BTreeMap::from_iter([(-123, true), (123, true)]),
            u_map: HashMap::from_iter([(123, true)]),
            pattern_map: BTreeMap::from_iter([(HexNumber("b4df00d".to_owned()), true)]),
        }])
        .assert_matches_de_roundtrip(arbitrary_values());
}

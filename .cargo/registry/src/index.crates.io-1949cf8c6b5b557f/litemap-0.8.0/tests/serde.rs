// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

use litemap::LiteMap;

#[test]
fn test_ser() {
    let mut map = LiteMap::new_vec();
    map.insert(1, "jat");
    map.insert(4, "sei");
    map.insert(3, "saam");
    map.insert(2, "ji");

    let json_string = serde_json::to_string(&map).unwrap();

    assert_eq!(json_string, r#"{"1":"jat","2":"ji","3":"saam","4":"sei"}"#);

    let new_map = serde_json::from_str(&json_string).unwrap();

    assert_eq!(map, new_map);
}

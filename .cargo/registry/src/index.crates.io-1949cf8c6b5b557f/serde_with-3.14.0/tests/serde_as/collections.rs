use super::*;
use fnv::{FnvHashMap, FnvHashSet};

/// Test that `HashSet`s are also supported with non-default hashers.
#[test]
fn test_fnv_hashset() {
    #[serde_as]
    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct S(#[serde_as(as = "FnvHashSet<DisplayFromStr>")] FnvHashSet<u32>);

    // Normal
    is_equal(
        S([1, 2, 3, 4, 5].iter().copied().collect()),
        expect![[r#"
            [
              "5",
              "4",
              "1",
              "3",
              "2"
            ]"#]],
    );
    is_equal(S(FnvHashSet::default()), expect![[r#"[]"#]]);
}

/// Test that `HashSet`s are also supported with non-default hashers.
#[test]
fn test_fnv_hashmap() {
    #[serde_as]
    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct S(#[serde_as(as = "FnvHashMap<DisplayFromStr, DisplayFromStr>")] FnvHashMap<u8, u32>);

    // Normal
    is_equal(
        S([(1, 1), (3, 3), (111, 111)].iter().copied().collect()),
        expect![[r#"
            {
              "1": "1",
              "3": "3",
              "111": "111"
            }"#]],
    );
    is_equal(S(FnvHashMap::default()), expect![[r#"{}"#]]);
}

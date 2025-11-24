#![allow(dead_code)]

use std::collections::{BTreeMap, HashMap, HashSet};

use ts_rs::TS;

#[derive(TS)]
#[ts(export, export_to = "hashmap/")]
struct Hashes {
    map: HashMap<String, String>,
    set: HashSet<String>,
}

#[test]
fn hashmap() {
    assert_eq!(
        Hashes::decl(),
        "type Hashes = { map: { [key in string]?: string }, set: Array<string>, };"
    )
}

struct CustomHasher {}

type CustomHashMap<K, V> = HashMap<K, V, CustomHasher>;
type CustomHashSet<K> = HashSet<K, CustomHasher>;

#[derive(TS)]
#[ts(export, export_to = "hashmap/")]
struct HashesHasher {
    map: CustomHashMap<String, String>,
    set: CustomHashSet<String>,
}

#[test]
fn hashmap_with_custom_hasher() {
    assert_eq!(
        HashesHasher::decl(),
        "type HashesHasher = { map: { [key in string]?: string }, set: Array<string>, };"
    )
}

#[derive(TS, Eq, PartialEq, Hash)]
#[ts(export, export_to = "hashmap/")]
struct CustomKey(String);

#[derive(TS)]
#[ts(export, export_to = "hashmap/")]
struct CustomValue;

#[derive(TS)]
#[ts(export, export_to = "hashmap/")]
struct HashMapWithCustomTypes {
    map: HashMap<CustomKey, CustomValue>,
}

#[derive(TS)]
#[ts(export, export_to = "hashmap/")]
struct BTreeMapWithCustomTypes {
    map: BTreeMap<CustomKey, CustomValue>,
}

#[test]
fn with_custom_types() {
    assert_eq!(
        HashMapWithCustomTypes::inline(),
        BTreeMapWithCustomTypes::inline()
    );
    assert_eq!(
        HashMapWithCustomTypes::decl(),
        "type HashMapWithCustomTypes = { map: { [key in CustomKey]?: CustomValue }, };"
    );
}

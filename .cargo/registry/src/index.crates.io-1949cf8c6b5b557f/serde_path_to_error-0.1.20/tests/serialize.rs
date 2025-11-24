use serde::Serialize;
use serde_derive::Serialize;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::fmt::Debug;

fn test<T>(value: &T, expected: &str)
where
    T: ?Sized + Serialize + Debug,
{
    let mut out = Vec::new();
    let ser = &mut serde_json::Serializer::new(&mut out);
    let result = serde_path_to_error::serialize(value, ser);
    let path = result.unwrap_err().path().to_string();
    assert_eq!(path, expected);
}

#[test]
fn test_refcell_already_borrowed() {
    #[derive(Serialize, Debug)]
    struct Outer<'a> {
        k: Inner<'a>,
    }

    #[derive(Serialize, Debug)]
    struct Inner<'a> {
        refcell: &'a RefCell<String>,
    }

    let refcell = RefCell::new(String::new());
    let outer = Outer {
        k: Inner { refcell: &refcell },
    };

    let _borrowed = refcell.borrow_mut();
    test(&outer, "k.refcell");
}

#[test]
fn test_map_nonstring_key() {
    fn singleton_map<K: Ord, V>(key: K, value: V) -> BTreeMap<K, V> {
        let mut map = BTreeMap::new();
        map.insert(key, value);
        map
    }

    let map = singleton_map(b"", 0);
    let map = singleton_map("k", map);
    let map = singleton_map(100, map);

    test(&map, "100.k");
}

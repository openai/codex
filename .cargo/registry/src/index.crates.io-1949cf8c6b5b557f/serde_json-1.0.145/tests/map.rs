use serde_json::{from_str, Map, Value};

#[test]
fn test_preserve_order() {
    // Sorted order
    #[cfg(not(feature = "preserve_order"))]
    const EXPECTED: &[&str] = &["a", "b", "c"];

    // Insertion order
    #[cfg(feature = "preserve_order")]
    const EXPECTED: &[&str] = &["b", "a", "c"];

    let v: Value = from_str(r#"{"b":null,"a":null,"c":null}"#).unwrap();
    let keys: Vec<_> = v.as_object().unwrap().keys().collect();
    assert_eq!(keys, EXPECTED);
}

#[test]
#[cfg(feature = "preserve_order")]
fn test_shift_insert() {
    let mut v: Value = from_str(r#"{"b":null,"a":null,"c":null}"#).unwrap();
    let val = v.as_object_mut().unwrap();
    val.shift_insert(0, "d".to_owned(), Value::Null);

    let keys: Vec<_> = val.keys().collect();
    assert_eq!(keys, &["d", "b", "a", "c"]);
}

#[test]
fn test_append() {
    // Sorted order
    #[cfg(not(feature = "preserve_order"))]
    const EXPECTED: &[&str] = &["a", "b", "c"];

    // Insertion order
    #[cfg(feature = "preserve_order")]
    const EXPECTED: &[&str] = &["b", "a", "c"];

    let mut v: Value = from_str(r#"{"b":null,"a":null,"c":null}"#).unwrap();
    let val = v.as_object_mut().unwrap();
    let mut m = Map::new();
    m.append(val);
    let keys: Vec<_> = m.keys().collect();

    assert_eq!(keys, EXPECTED);
    assert!(val.is_empty());
}

#[test]
fn test_retain() {
    let mut v: Value = from_str(r#"{"b":null,"a":null,"c":null}"#).unwrap();
    let val = v.as_object_mut().unwrap();
    val.retain(|k, _| k.as_str() != "b");

    let keys: Vec<_> = val.keys().collect();
    assert_eq!(keys, &["a", "c"]);
}

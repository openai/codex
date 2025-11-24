// Please also update README.md when making changes to this code.

use erased_serde::{Deserializer, Serialize, Serializer};
use std::collections::BTreeMap as Map;
use std::io;

#[test]
fn serialization() {
    // Construct some serializers.
    let json = &mut serde_json::Serializer::new(io::stdout());
    let cbor = &mut serde_cbor::Serializer::new(serde_cbor::ser::IoWrite::new(io::stdout()));

    // The values in this map are boxed trait objects. Ordinarily this would not
    // be possible with serde::Serializer because of object safety, but type
    // erasure makes it possible with erased_serde::Serializer.
    let mut formats: Map<&str, Box<dyn Serializer>> = Map::new();
    formats.insert("json", Box::new(<dyn Serializer>::erase(json)));
    formats.insert("cbor", Box::new(<dyn Serializer>::erase(cbor)));

    // These are boxed trait objects as well. Same thing here - type erasure
    // makes this possible.
    let mut values: Map<&str, Box<dyn Serialize>> = Map::new();
    values.insert("vec", Box::new(vec!["a", "b"]));
    values.insert("int", Box::new(65536));

    // Pick a Serializer out of the formats map.
    let format = formats.get_mut("json").unwrap();

    // Pick a Serialize out of the values map.
    let value = values.get("vec").unwrap();

    // This line prints `["a","b"]` to stdout.
    value.erased_serialize(format).unwrap();
}

#[test]
fn deserialization() {
    static JSON: &[u8] = br#"{"A": 65, "B": 66}"#;
    static CBOR: &[u8] = &[162, 97, 65, 24, 65, 97, 66, 24, 66];

    // Construct some deserializers.
    let json = &mut serde_json::Deserializer::from_slice(JSON);
    let cbor = &mut serde_cbor::Deserializer::from_slice(CBOR);

    // The values in this map are boxed trait objects, which is not possible
    // with the normal serde::Deserializer because of object safety.
    let mut formats: Map<&str, Box<dyn Deserializer>> = Map::new();
    formats.insert("json", Box::new(<dyn Deserializer>::erase(json)));
    formats.insert("cbor", Box::new(<dyn Deserializer>::erase(cbor)));

    // Pick a Deserializer out of the formats map.
    let format = formats.get_mut("json").unwrap();

    let data: Map<String, usize> = erased_serde::deserialize(format).unwrap();

    println!("{}", data["A"] + data["B"]);
}
